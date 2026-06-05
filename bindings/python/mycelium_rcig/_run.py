"""Process runner for the Mycelium SDK (RFC-0111, Python Phase 2).

Spawns the resolved ``mycelium`` binary with an argv list (never a shell
string — no injection surface), captures stdout/stderr, and maps the result to
a parsed value or a typed error. The spawn function is injectable so the runner
is unit-testable without a real binary.
"""
import json
import signal as _signal
import subprocess


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


def default_spawn(binary, args):
    """Run ``binary args``; return ``{status, signal, stdout, stderr}``.

    Never raises — process-level failures are surfaced as a non-zero status so
    the caller's error model is the single source of truth.
    """
    try:
        proc = subprocess.run([binary, *args], capture_output=True, text=True)
    except FileNotFoundError as err:  # binary not found on PATH
        return {"status": 127, "signal": None, "stdout": "", "stderr": str(err)}

    rc = proc.returncode
    if rc is not None and rc < 0:  # killed by signal -N (POSIX)
        try:
            name = _signal.Signals(-rc).name
        except ValueError:
            name = str(-rc)
        return {"status": None, "signal": name, "stdout": proc.stdout, "stderr": proc.stderr}

    return {"status": rc, "signal": None, "stdout": proc.stdout, "stderr": proc.stderr}


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
