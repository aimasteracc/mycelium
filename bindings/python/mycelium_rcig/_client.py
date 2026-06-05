"""The Mycelium SDK client (RFC-0111, Python Phase 2).

A thin, typed wrapper over the ``mycelium`` CLI: each method assembles an argv
list, spawns the binary, and returns parsed JSON (or text for the format-less
commands). It adds no capabilities of its own — every method maps 1:1 onto an
existing CLI+MCP pair (Charter §5.13). Commands without a typed method are
reachable via the low-level :meth:`Mycelium.run` escape hatch.
"""
from __future__ import annotations

from typing import Any, List, Mapping, Optional, Sequence

from ._resolve import resolve_binary
from ._run import run_json, run_text


class Mycelium:
    """A thin, typed client over the ``mycelium`` CLI."""

    def __init__(
        self,
        root: str = ".",
        bin: Optional[str] = None,
        budget: Optional[str] = None,
        env: Optional[Mapping[str, str]] = None,
        runner: Optional[Any] = None,
    ) -> None:
        """
        :param root: project root passed as ``--root`` (default ``"."``).
        :param bin: explicit binary path; skips resolution.
        :param budget: default RFC-0102 budget for budget-aware methods.
        :param env: environment for binary resolution (default ``os.environ``).
        :param runner: injected runner exposing ``json``/``text`` (tests).
        """
        self.root = root
        self.budget = budget
        self._bin = bin if bin is not None else resolve_binary(env=env)
        self._json = runner.json if runner is not None else run_json
        self._text = runner.text if runner is not None else run_text

    def _json_args(
        self,
        cmd: str,
        positionals: Sequence[str] = (),
        extra: Sequence[str] = (),
    ) -> List[str]:
        return [cmd, *positionals, "--root", self.root, "--format", "json", *extra]

    def run(self, args: Sequence[str]) -> Any:
        """Low-level escape hatch: spawn with exactly ``args``, JSON-parse stdout."""
        return self._json(self._bin, list(args))

    def version(self) -> str:
        """Engine version string, e.g. ``"mycelium 0.2.1"``."""
        return self._text(self._bin, ["version"])

    def index(self, path: Optional[str] = None) -> str:
        """Index a project directory; returns the CLI's plain-text status report."""
        return self._text(self._bin, ["index", path if path is not None else self.root])

    def query(self, expr: str) -> Any:
        """Execute a Hyphae selector; returns the parsed JSON result."""
        return self._json(self._bin, self._json_args("query", [expr]))

    def search_symbol(self, query: str, limit: Optional[int] = None) -> Any:
        """Case-insensitive substring search over symbol names."""
        extra = [] if limit is None else ["--limit", str(limit)]
        return self._json(self._bin, self._json_args("search-symbol", [query], extra))

    def get_symbol_info(self, path: str) -> Any:
        """All structural info about a symbol in one call."""
        return self._json(self._bin, self._json_args("get-symbol-info", [path]))

    def get_callers(
        self,
        path: str,
        edge_kind: Optional[str] = None,
        include_virtual: bool = False,
        budget: Optional[str] = None,
    ) -> Any:
        """Direct callers of a symbol (incoming edges)."""
        extra: List[str] = []
        if edge_kind:
            extra += ["--edge-kind", edge_kind]
        if include_virtual:
            extra += ["--include-virtual"]
        budget = budget if budget is not None else self.budget
        if budget:
            extra += ["--budget", budget]
        return self._json(self._bin, self._json_args("get-callers", [path], extra))

    def get_callees(
        self,
        path: str,
        edge_kind: Optional[str] = None,
        budget: Optional[str] = None,
    ) -> Any:
        """Direct callees of a symbol (outgoing edges)."""
        extra: List[str] = []
        if edge_kind:
            extra += ["--edge-kind", edge_kind]
        budget = budget if budget is not None else self.budget
        if budget:
            extra += ["--budget", budget]
        return self._json(self._bin, self._json_args("get-callees", [path], extra))

    def context(
        self,
        task: str,
        max_nodes: Optional[int] = None,
        max_code_blocks: Optional[int] = None,
        budget: Optional[str] = None,
    ) -> Any:
        """Task-focused context bundle (the ``mycelium_context`` twin)."""
        extra: List[str] = []
        if max_nodes is not None:
            extra += ["--max-nodes", str(max_nodes)]
        if max_code_blocks is not None:
            extra += ["--max-code-blocks", str(max_code_blocks)]
        budget = budget if budget is not None else self.budget
        if budget:
            extra += ["--budget", budget]
        return self._json(self._bin, self._json_args("context", ["--task", task], extra))

    def server_status(self) -> Any:
        """Whether an index is loaded, plus node/edge counts."""
        return self._json(self._bin, self._json_args("server-status"))
