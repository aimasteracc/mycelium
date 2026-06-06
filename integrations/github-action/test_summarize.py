"""Unit tests for the GitHub Action summary formatter."""
import unittest

from summarize import build_summary, _count, _stat


class SummarizeTests(unittest.TestCase):
    def test_full_summary(self):
        md = build_summary(
            status={"node_count": 1200, "edge_count": 5400},
            dead={"dead_symbols": ["a", "b", "c"]},
            isolated={"isolated_symbols": ["x"]},
            entry={"entry_points": ["main", "run"]},
            root="src/",
        )
        self.assertIn("Mycelium — code intelligence", md)
        self.assertIn("`1200`", md)
        self.assertIn("`5400`", md)
        self.assertIn("`3`", md)  # dead
        self.assertIn("`1`", md)  # isolated
        self.assertIn("`2`", md)  # entry points
        self.assertIn("src/", md)

    def test_degraded_inputs_render_dashes(self):
        # Missing/malformed JSON must not crash; render em-dash.
        md = build_summary(status=None, dead=None, isolated="oops", entry={}, root=".")
        self.assertIn("| Symbols (nodes) | — |", md)
        self.assertIn("| Dead symbols (no incoming calls/imports) | — |", md)

    def test_count_handles_list_count_and_keyed(self):
        self.assertEqual(_count(["a", "b"]), 2)
        self.assertEqual(_count({"count": 7}), 7)
        self.assertEqual(_count({"dead_symbols": ["a"]}, "dead_symbols"), 1)
        self.assertIsNone(_count({"unknown": 1}, "dead_symbols"))

    def test_stat(self):
        self.assertEqual(_stat({"node_count": 9}, "node_count"), 9)
        self.assertIsNone(_stat({}, "node_count"))
        self.assertIsNone(_stat("nope", "node_count"))


if __name__ == "__main__":
    unittest.main()
