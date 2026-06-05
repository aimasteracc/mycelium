# Unit tests for SDK binary resolution (RFC-0111, Python Phase 2).
# Run with: python3 -m unittest discover -s tests
import unittest

from mycelium_rcig._resolve import resolve_binary, binary_name


class ResolveBinaryTests(unittest.TestCase):
    def test_env_override_wins(self):
        def which(_name):
            raise AssertionError("which must not be consulted when env is set")

        got = resolve_binary(
            env={"MYCELIUM_BIN": "/custom/mycelium"}, which=which, platform="linux"
        )
        self.assertEqual(got, "/custom/mycelium")

    def test_resolves_via_which_on_path(self):
        seen = []

        def which(name):
            seen.append(name)
            return f"/usr/local/bin/{name}"

        got = resolve_binary(env={}, which=which, platform="linux")
        self.assertEqual(got, "/usr/local/bin/mycelium")
        self.assertEqual(seen, ["mycelium"])

    def test_windows_looks_for_exe(self):
        got = resolve_binary(env={}, which=lambda n: f"C:/bin/{n}", platform="win32")
        self.assertEqual(got, "C:/bin/mycelium.exe")

    def test_falls_back_to_command_name_when_not_found(self):
        got = resolve_binary(env={}, which=lambda _n: None, platform="linux")
        self.assertEqual(got, "mycelium")

    def test_falls_back_to_exe_on_windows(self):
        got = resolve_binary(env={}, which=lambda _n: None, platform="win32")
        self.assertEqual(got, "mycelium.exe")

    def test_binary_name(self):
        self.assertEqual(binary_name("win32"), "mycelium.exe")
        self.assertEqual(binary_name("linux"), "mycelium")
        self.assertEqual(binary_name("darwin"), "mycelium")


if __name__ == "__main__":
    unittest.main()
