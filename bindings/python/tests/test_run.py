# Unit tests for the SDK runner (spawn + parse + error model, RFC-0111 Phase 2).
import sys
import tempfile
import unittest
from unittest import mock

from mycelium_rcig import _run
from mycelium_rcig._run import run_json, run_text, default_spawn, MyceliumError


def fake_spawn(result, calls):
    def spawn(binary, args):
        calls.append((binary, list(args)))
        return result

    return spawn


class RunTests(unittest.TestCase):
    def test_run_json_parses_stdout_on_clean_exit(self):
        calls = []
        spawn = fake_spawn(
            {"status": 0, "signal": None, "stdout": '["a","b"]', "stderr": ""}, calls
        )
        out = run_json("mycelium", ["query", "#x", "--format", "json"], spawn=spawn)
        self.assertEqual(out, ["a", "b"])
        self.assertEqual(calls, [("mycelium", ["query", "#x", "--format", "json"])])

    def test_run_json_raises_with_code_and_stderr_on_nonzero(self):
        spawn = fake_spawn({"status": 2, "signal": None, "stdout": "", "stderr": "boom"}, [])
        with self.assertRaises(MyceliumError) as ctx:
            run_json("mycelium", ["query", "#x"], spawn=spawn)
        err = ctx.exception
        self.assertEqual(err.code, 2)
        self.assertEqual(err.stderr, "boom")
        self.assertEqual(err.args_, ["query", "#x"])

    def test_run_json_raises_on_invalid_json(self):
        spawn = fake_spawn({"status": 0, "signal": None, "stdout": "not json", "stderr": ""}, [])
        with self.assertRaises(MyceliumError) as ctx:
            run_json("mycelium", ["query"], spawn=spawn)
        self.assertIn("invalid JSON", str(ctx.exception))

    def test_run_json_raises_on_signal(self):
        spawn = fake_spawn({"status": None, "signal": "SIGKILL", "stdout": "", "stderr": ""}, [])
        with self.assertRaises(MyceliumError) as ctx:
            run_json("mycelium", ["query"], spawn=spawn)
        self.assertEqual(ctx.exception.signal, "SIGKILL")

    def test_run_text_returns_trimmed_stdout(self):
        spawn = fake_spawn(
            {"status": 0, "signal": None, "stdout": "mycelium 0.2.1\n", "stderr": ""}, []
        )
        self.assertEqual(run_text("mycelium", ["version"], spawn=spawn), "mycelium 0.2.1")

    def test_run_text_raises_on_nonzero(self):
        spawn = fake_spawn({"status": 1, "signal": None, "stdout": "", "stderr": "nope"}, [])
        with self.assertRaises(MyceliumError):
            run_text("mycelium", ["version"], spawn=spawn)

    def test_error_is_exception(self):
        self.assertTrue(issubclass(MyceliumError, Exception))


class DefaultSpawnOsErrorTests(unittest.TestCase):
    """default_spawn must normalize *all* spawn-time OS errors to status 127."""

    def test_nonexistent_binary_returns_127(self):
        result = default_spawn("/no/such/mycelium-binary-xyz", [])
        self.assertEqual(result["status"], 127)
        self.assertIsNone(result["signal"])

    def test_non_executable_path_returns_127(self):
        # A directory is not executable: spawning it raises PermissionError /
        # IsADirectoryError (OSError subclasses), not FileNotFoundError.
        with tempfile.TemporaryDirectory() as d:
            result = default_spawn(d, [])
            self.assertEqual(result["status"], 127)
            self.assertIsNone(result["signal"])


class DefaultSpawnOutputCapTests(unittest.TestCase):
    """default_spawn streams output with a hard cap and kills overflowing children."""

    def test_output_within_cap_passes_through(self):
        # Emit 100 bytes under a 4 KiB cap — clean status, full payload.
        prog = "import sys; sys.stdout.write('a' * 100)"
        with mock.patch.object(_run, "MAX_OUTPUT_BYTES", 4096):
            result = default_spawn(sys.executable, ["-c", prog])
        self.assertEqual(result["status"], 0)
        self.assertEqual(result["stdout"], "a" * 100)

    def test_overflowing_output_is_capped_and_terminated(self):
        # Emit 50 KiB against a 1 KiB cap — surfaced as status 137, payload
        # truncated to the cap, stderr explains the termination.
        prog = "import sys; sys.stdout.write('a' * (50 * 1024))"
        with mock.patch.object(_run, "MAX_OUTPUT_BYTES", 1024):
            result = default_spawn(sys.executable, ["-c", prog])
        self.assertEqual(result["status"], 137)
        self.assertLessEqual(len(result["stdout"]), 1024)
        self.assertIn("cap", result["stderr"])


if __name__ == "__main__":
    unittest.main()
