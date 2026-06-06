#!/usr/bin/env python3
"""Build a Markdown code-intelligence summary from Mycelium CLI JSON output.

Reads the JSON emitted by `mycelium server-status / get-dead-symbols /
get-isolated-symbols / get-entry-points` (passed as file paths) and prints a
GitHub-flavored Markdown summary to stdout. Pure formatting + defensive parsing
so the action degrades gracefully on partial output. Run directly by the
composite action; unit-tested with sample JSON.
"""
from __future__ import annotations

import json
import sys
from typing import Any


def _load(path: str) -> Any:
    try:
        with open(path, encoding="utf-8") as fh:
            return json.load(fh)
    except (OSError, ValueError):
        return None


def _count(value: Any, *keys: str) -> int | None:
    """Best-effort count: a list, or `{count: n}`, or `{<key>: [...]}`."""
    if isinstance(value, list):
        return len(value)
    if isinstance(value, dict):
        if isinstance(value.get("count"), int):
            return value["count"]
        for key in keys:
            inner = value.get(key)
            if isinstance(inner, list):
                return len(inner)
    return None


def _stat(status: Any, key: str) -> int | None:
    return status[key] if isinstance(status, dict) and isinstance(status.get(key), int) else None


def build_summary(
    status: Any,
    dead: Any,
    isolated: Any,
    entry: Any,
    root: str,
) -> str:
    """Return the Markdown summary block."""
    nodes = _stat(status, "node_count")
    edges = _stat(status, "edge_count")
    dead_n = _count(dead, "dead_symbols", "symbols")
    iso_n = _count(isolated, "isolated_symbols", "symbols")
    entry_n = _count(entry, "entry_points", "symbols")

    def cell(value: int | None) -> str:
        return "—" if value is None else f"`{value}`"

    lines = [
        "<!-- mycelium-code-intel -->",
        "## \U0001f344 Mycelium — code intelligence",
        "",
        f"Indexed **`{root}`** with the [Mycelium](https://github.com/aimasteracc/mycelium) graph engine.",
        "",
        "| Metric | Value |",
        "|---|---|",
        f"| Symbols (nodes) | {cell(nodes)} |",
        f"| Relationships (edges) | {cell(edges)} |",
        f"| Dead symbols (no incoming calls/imports) | {cell(dead_n)} |",
        f"| Isolated symbols (no edges) | {cell(iso_n)} |",
        f"| Entry points (no incoming calls) | {cell(entry_n)} |",
        "",
        "<sub>Structural intelligence from the RCIG graph — no live LSP. "
        "Powered by `@aimasteracc/mycelium`.</sub>",
    ]
    return "\n".join(lines)


def main(argv: list[str]) -> int:
    if len(argv) != 6:
        print("usage: summarize.py <status.json> <dead.json> <isolated.json> <entry.json> <root>", file=sys.stderr)
        return 2
    status, dead, isolated, entry, root = (
        _load(argv[1]),
        _load(argv[2]),
        _load(argv[3]),
        _load(argv[4]),
        argv[5],
    )
    print(build_summary(status, dead, isolated, entry, root))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
