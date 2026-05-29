# RFC-0049 — `Store::leaf_symbols` + `mycelium_get_leaf_symbols` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0049                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0022 (entry_points), RFC-0048 (most_connected), RFC-0046 (node_degree) |

## Summary

Add `Store::leaf_symbols(kind, limit)` — symbol nodes with out-degree 0 for a
given `EdgeKind` — and expose it as `mycelium_get_leaf_symbols`.

For `Calls` edges these are symbols that call nothing: leaf implementations,
utility functions, or true dead-ends in the call graph.  Symmetric complement
to RFC-0022 `entry_points` (in-degree 0).  Together they bracket the call
graph: entry_points are roots, leaf_symbols are leaves.

## Design

### Store method

```rust
pub fn leaf_symbols(&self, kind: EdgeKind, limit: usize) -> Vec<String>
```

Iterates all symbol nodes (paths containing `>`), retains those where
`outgoing(id, kind).is_empty()`, sorts results alphabetically, and returns
up to `limit` entries (capped at 100).

### MCP tool — `mycelium_get_leaf_symbols`

Request:
```json
{ "edge_kind": "calls", "limit": 10 }
```

`edge_kind` must be `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
`limit` defaults to 10, capped at 100.

Response:
```json
{
  "symbols": ["src/util.rs>helper", "src/math.rs>add"],
  "count": 2
}
```

Unknown `edge_kind` returns `{ "error": "unknown edge_kind: ..." }`.

## Acceptance Criteria

- [x] `Store::leaf_symbols(kind, limit)` returns symbol nodes with out-degree 0 for `kind`.
- [x] Symbol nodes only (paths containing `>`); file nodes excluded.
- [x] Results sorted ascending by path.
- [x] `limit` respected; capped at 100 internally.
- [x] `mycelium_get_leaf_symbols`: valid edge_kind returns `{ symbols, count }`.
- [x] `mycelium_get_leaf_symbols`: unknown edge_kind returns `{ error }`.
- [x] `limit` defaults to 10 when omitted.
- [x] All prior tests pass.
