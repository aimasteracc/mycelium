"""Process runner for the Mycelium SDK (RFC-0111, Python Phase 2).

Spawns the resolved ``mycelium`` binary with an argv list (never a shell
string — no injection surface), captures stdout/stderr, and maps the result to
a parsed value or a typed error. The spawn function is injectable so the runner
is unit-testable without a real binary.
"""
import json
import signal as _signal
import subprocess
import threading

# Mirror the Node SDK's ``execFile`` ``maxBuffer`` (64 MiB). ``subprocess.run``
# buffers stdout unbounded, so a runaway or hostile binary could exhaust host
# memory; we stream with a hard cap and kill the child the moment it overflows.
MAX_OUTPUT_BYTES = 64 * 1024 * 1024


class MyceliumError(Exception):
    """Raised when the CLI fails, is signalled, or emits unparseable JSON.

    The CLI argv is stored as ``args_`` (trailing underscore) because
    ``Exception.args`` is reserved for the exception's own constructor args.
    """

    def __init__(self, message, code=None, signal=None, stderr="", stdout="", args=None):
        super().__init__(message)
        self.code = code
        self.signal = signal
        self.stderr = stderr
        self.stdout = stdout
        self.args_ = list(args) if args is not None else []


def _drain_capped(stream, limit, out, on_overflow):
    """Read ``stream`` into ``out['data']`` (a bytearray), capped at ``limit``.

    On the first byte past ``limit`` this sets ``out['overflow']`` and calls
    ``on_overflow`` — which kills the child so *both* pipes reach EOF promptly —
    then keeps looping only to drain whatever the kernel already buffered
    (a runaway child that never closes the pipe would otherwise hang the read
    forever; killing it is what guarantees EOF).
    """
    data = out["data"]
    overflow = False
    while True:
        # read1: one underlying syscall, returns whatever is already available
        # rather than blocking until the full 64 KiB (or EOF) arrives. With
        # plain .read(n) the loop would block past the cap until the child
        # closed the pipe — defeating the kill-on-overflow guard entirely.
        chunk = stream.read1(65536)
        if not chunk:
            break
        if not overflow and len(data) + len(chunk) > limit:
            overflow = True
            out["overflow"] = True
            keep = limit - len(data)
            if keep > 0:
                data.extend(chunk[:keep])
            on_overflow()  # kill the child NOW so this and the sibling pipe EOF
        elif not overflow:
            data.extend(chunk)
        # past the cap we keep looping to drain the already-buffered bytes; the
        # kill above ensures the stream closes rather than feeding us forever.


def default_spawn(binary, args):
    """Run ``binary args``; return ``{status, signal, stdout, stderr}``.

    Never raises — process-level failures are surfaced as a non-zero status so
    the caller's error model is the single source of truth. Output is streamed
    with a hard :data:`MAX_OUTPUT_BYTES` cap; if the child exceeds it, the child
    is killed and the result is surfaced as a non-zero (137) status, mirroring
    the Node SDK's ``maxBuffer`` overflow behaviour.
    """
    try:
        proc = subprocess.Popen(
            [binary, *args],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as err:
        # All spawn-time OS failures — not found (FileNotFoundError), not
        # executable / a directory / wrong format (PermissionError,
        # IsADirectoryError, OSError) — normalize to a 127 status so the
        # caller's MyceliumError model stays the single source of truth.
        return {"status": 127, "signal": None, "stdout": "", "stderr": str(err)}

    # Drain stderr on a thread so a child that fills its stderr pipe while we
    # read stdout can't deadlock. stderr shares the same cap. The moment either
    # stream overflows, `kill_once` terminates the child so BOTH pipes reach EOF
    # and neither drain loop can hang on a child that never closes the pipe.
    kill_lock = threading.Lock()
    killed = {"done": False}

    def kill_once():
        with kill_lock:
            if not killed["done"]:
                killed["done"] = True
                proc.kill()

    out = {"data": bytearray(), "overflow": False}
    err_out = {"data": bytearray(), "overflow": False}
    err_thread = threading.Thread(
        target=_drain_capped, args=(proc.stderr, MAX_OUTPUT_BYTES, err_out, kill_once)
    )
    err_thread.start()
    _drain_capped(proc.stdout, MAX_OUTPUT_BYTES, out, kill_once)
    err_thread.join()
    proc.stdout.close()
    proc.stderr.close()

    overflowed = out["overflow"] or err_out["overflow"]
    rc = proc.wait()

    stdout = out["data"].decode("utf-8", "replace")
    stderr = err_out["data"].decode("utf-8", "replace")

    if overflowed:
        return {
            "status": 137,
            "signal": None,
            "stdout": stdout,
            "stderr": "mycelium output exceeded the {}-byte cap and was terminated".format(
                MAX_OUTPUT_BYTES
            ),
        }

    if rc is not None and rc < 0:  # killed by signal -N (POSIX)
        try:
            name = _signal.Signals(-rc).name
        except ValueError:
            name = str(-rc)
        return {"status": None, "signal": name, "stdout": stdout, "stderr": stderr}

    return {"status": rc, "signal": None, "stdout": stdout, "stderr": stderr}


def _run_raw(binary, args, spawn=None):
    spawn = spawn if spawn is not None else default_spawn
    result = spawn(binary, args)

    sig = result.get("signal")
    if sig:
        raise MyceliumError(
            "mycelium was killed by signal {}".format(sig),
            signal=sig, stderr=result.get("stderr", ""), args=args,
        )

    status = result.get("status")
    if status != 0:
        stderr = result.get("stderr", "")
        suffix = ": {}".format(stderr.strip()) if stderr else ""
        raise MyceliumError(
            "mycelium exited with code {}{}".format(status, suffix),
            code=status, stderr=stderr, stdout=result.get("stdout", ""), args=args,
        )

    return result.get("stdout", "")


def run_json(binary, args, spawn=None):
    """Run the CLI and JSON-parse its stdout. Raises MyceliumError on failure."""
    stdout = _run_raw(binary, args, spawn)
    try:
        return json.loads(stdout)
    except ValueError as err:
        raise MyceliumError(
            "mycelium produced invalid JSON: {}".format(err),
            code=0, stdout=stdout, args=args,
        )


def run_text(binary, args, spawn=None):
    """Run the CLI and return its trimmed stdout text. Raises on failure."""
    return _run_raw(binary, args, spawn).strip()
