# Unit tests for the Mycelium client argv assembly (RFC-0111, Python Phase 2).
# A spy runner records the argv each method emits — fully hermetic, no binary.
import unittest

from mycelium_rcig import Mycelium, MyceliumError


class SpyRunner:
    def __init__(self):
        self.calls = []

    def json(self, binary, args):
        self.calls.append(("json", binary, list(args)))
        return {"ok": True}

    def text(self, binary, args):
        self.calls.append(("text", binary, list(args)))
        return "mycelium 0.2.1"


def spy_client(**kwargs):
    runner = SpyRunner()
    client = Mycelium(bin="mycelium", runner=runner, **kwargs)
    return client, runner.calls


class ClientTests(unittest.TestCase):
    def test_public_surface(self):
        self.assertTrue(callable(Mycelium))
        self.assertTrue(issubclass(MyceliumError, Exception))

    def test_version_runs_text(self):
        client, calls = spy_client()
        self.assertEqual(client.version(), "mycelium 0.2.1")
        self.assertEqual(calls, [("text", "mycelium", ["version"])])

    def test_index_runs_text_no_format(self):
        client, calls = spy_client()
        client.index("./src")
        self.assertEqual(calls[0], ("text", "mycelium", ["index", "./src"]))

    def test_index_defaults_to_root(self):
        client, calls = spy_client(root="/proj")
        client.index()
        self.assertEqual(calls[0][2], ["index", "/proj"])

    def test_query_appends_root_and_format(self):
        client, calls = spy_client()
        client.query("#login")
        self.assertEqual(
            calls[0], ("json", "mycelium", ["query", "#login", "--root", ".", "--format", "json"])
        )

    def test_query_custom_root(self):
        client, calls = spy_client(root="/repo")
        client.query(".function")
        self.assertEqual(calls[0][2], ["query", ".function", "--root", "/repo", "--format", "json"])

    def test_search_symbol(self):
        client, calls = spy_client()
        client.search_symbol("login", limit=10)
        self.assertEqual(
            calls[0][2],
            ["search-symbol", "login", "--root", ".", "--format", "json", "--limit", "10"],
        )

    def test_get_symbol_info(self):
        client, calls = spy_client()
        client.get_symbol_info("src/lib.rs>App>render")
        self.assertEqual(
            calls[0][2],
            ["get-symbol-info", "src/lib.rs>App>render", "--root", ".", "--format", "json"],
        )

    def test_get_callers_full(self):
        client, calls = spy_client()
        client.get_callers("a>b", edge_kind="calls", include_virtual=True, budget="small")
        self.assertEqual(
            calls[0][2],
            [
                "get-callers", "a>b", "--root", ".", "--format", "json",
                "--edge-kind", "calls", "--include-virtual", "--budget", "small",
            ],
        )

    def test_get_callers_minimal(self):
        client, calls = spy_client()
        client.get_callers("a>b")
        self.assertEqual(calls[0][2], ["get-callers", "a>b", "--root", ".", "--format", "json"])

    def test_get_callees(self):
        client, calls = spy_client()
        client.get_callees("a>b", edge_kind="imports", budget="large")
        self.assertEqual(
            calls[0][2],
            ["get-callees", "a>b", "--root", ".", "--format", "json",
             "--edge-kind", "imports", "--budget", "large"],
        )

    def test_context_with_limits(self):
        client, calls = spy_client()
        client.context("trace X to Y", max_nodes=30, max_code_blocks=6)
        self.assertEqual(
            calls[0][2],
            ["context", "--task", "trace X to Y", "--root", ".", "--format", "json",
             "--max-nodes", "30", "--max-code-blocks", "6"],
        )

    def test_context_forwards_budget(self):
        client, calls = spy_client()
        client.context("trace X to Y", budget="disabled")
        self.assertEqual(
            calls[0][2],
            ["context", "--task", "trace X to Y", "--root", ".", "--format", "json",
             "--budget", "disabled"],
        )

    def test_context_falls_back_to_constructor_budget(self):
        client, calls = spy_client(budget="small")
        client.context("trace X to Y")
        self.assertEqual(
            calls[0][2],
            ["context", "--task", "trace X to Y", "--root", ".", "--format", "json",
             "--budget", "small"],
        )

    def test_server_status(self):
        client, calls = spy_client()
        client.server_status()
        self.assertEqual(calls[0][2], ["server-status", "--root", ".", "--format", "json"])

    def test_run_is_raw_passthrough(self):
        client, calls = spy_client()
        client.run(["get-dead-symbols", "--prefix", "src/", "--format", "json"])
        self.assertEqual(
            calls[0], ("json", "mycelium", ["get-dead-symbols", "--prefix", "src/", "--format", "json"])
        )

    def test_constructor_budget_applied_to_callees(self):
        client, calls = spy_client(budget="small")
        client.get_callees("a>b")
        self.assertEqual(
            calls[0][2],
            ["get-callees", "a>b", "--root", ".", "--format", "json", "--budget", "small"],
        )

    def test_argv_smuggling_guard_rejects_leading_dash(self):
        # Every bare positional that flows into argv must reject a leading "-"
        # so it can't be re-parsed by the CLI as a flag — and reject *before*
        # the binary is ever spawned.
        client, calls = spy_client()
        for call in (
            lambda: client.query("--root"),
            lambda: client.search_symbol("-x"),
            lambda: client.get_symbol_info("--format"),
            lambda: client.get_callers("-evil"),
            lambda: client.get_callees("-evil"),
            lambda: client.index("--root=/etc"),
        ):
            with self.assertRaises(MyceliumError):
                call()
        self.assertEqual(calls, [])

    def test_argv_smuggling_guard_allows_ordinary_values(self):
        client, calls = spy_client()
        client.query("#login")  # '#', not '-' — fine
        client.index("./src")
        self.assertEqual(len(calls), 2)


if __name__ == "__main__":
    unittest.main()
