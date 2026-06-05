# End-to-end integration test against a real `mycelium` binary (RFC-0111 Phase 2).
#
# Skipped unless MYCELIUM_BIN is set, e.g.:
#   MYCELIUM_BIN=../../target/debug/mycelium python3 -m unittest discover -s tests
import os
import tempfile
import unittest

from mycelium_rcig import Mycelium, MyceliumError

BIN = os.environ.get("MYCELIUM_BIN")


@unittest.skipUnless(BIN, "set MYCELIUM_BIN to run the live integration test")
class IntegrationTests(unittest.TestCase):
    def test_index_and_query_roundtrip(self):
        with tempfile.TemporaryDirectory(prefix="mycelium-py-") as d:
            with open(os.path.join(d, "main.py"), "w") as f:
                f.write("def helper():\n    return 1\n\ndef main():\n    return helper()\n")
            m = Mycelium(root=d, bin=BIN)

            self.assertRegex(m.version(), r"^mycelium \d+\.\d+\.\d+")
            m.index()

            status = m.server_status()
            self.assertGreater(status["node_count"], 0)

            functions = m.query(".function")
            self.assertIsInstance(functions, list)
            self.assertGreaterEqual(len(functions), 2)

    def test_cli_failure_raises(self):
        m = Mycelium(root=".", bin=BIN)
        with self.assertRaises(MyceliumError):
            m.query("(((")


if __name__ == "__main__":
    unittest.main()
