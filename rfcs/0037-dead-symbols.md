# RFC-0037 — `Store::dead_symbols` + `mycelium_get_dead_symbols` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0037                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0011 (Calls edges), RFC-0023 (Imports edges), RFC-0028 (NodeKind) |

## Summary

Add `Store::dead_symbols` — a method that returns all symbol paths with
zero incoming `Calls` edges **and** zero incoming `Imports` edges — and
expose it as the `mycelium_get_dead_symbols` MCP tool.  Dead symbols are
candidates for deletion or documentation review; this is high-value for
AI agents doing code quality and dead-code analysis.

## Motivation

After indexing a workspace, an agent has a complete call/import graph.
The missing primitive is "which symbols are never called and never
imported?"  These are dead-code candidates.  No existing tool answers
this in one call.

File-level nodes (paths with no `>`) are excluded — they are indexed
containers, not callable symbols, and they may be imported via the
`Imports` edge already covered by the check.

## Design

### Store method

```rust
pub fn dead_symbols(&self, prefix: Option<&str>) -> Vec<String>
```

Returns all node paths where:
1. The path contains `>` (is a symbol, not a file node)
2. `self.synapse.incoming(id, EdgeKind::Calls)` is empty
3. `self.synapse.incoming(id, EdgeKind::Imports)` is empty

Optional `prefix` filters results to a subtree.  Results sorted
lexicographically.

### MCP tool — `mycelium_get_dead_symbols`

Request: `{ "path_prefix": "src/utils" }` (optional)

Response:
```json
{
  "dead_symbols": [
    "src/utils.py>_helper",
    "src/utils.py>old_method"
  ],
  "count": 2
}
```

Empty result: `{ "dead_symbols": [], "count": 0 }`.

## Acceptance Criteria

- [x] `Store::dead_symbols(prefix)` returns only symbol paths (containing `>`).
- [x] File-level nodes are excluded.
- [x] Symbols with any incoming Calls edge are excluded.
- [x] Symbols with any incoming Imports edge are excluded.
- [x] Optional `path_prefix` filters results.
- [x] Results are sorted lexicographically.
- [x] `mycelium_get_dead_symbols`: returns `{ dead_symbols, count }`.
- [x] All prior tests pass.
