# RFC-0048 — `Store::most_connected` + `mycelium_get_most_connected` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0048                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0019 (rank_symbols), RFC-0046 (node_degree) |

## Summary

Add `Store::most_connected(limit, kind)` — top-N symbol nodes ranked by
total degree (in-degree + out-degree) for a given `EdgeKind` — and expose
it as `mycelium_get_most_connected`.

`rank_symbols` ranks by incoming Calls only.  `most_connected` ranks by
total connectivity for any edge kind, surfacing hub nodes: symbols that
are both widely called and widely calling, or widely imported, etc.

## Design

### Store method

```rust
pub fn most_connected(&self, limit: usize, kind: EdgeKind) -> Vec<(String, usize)>
```

Iterates all symbol nodes (paths containing `>`), computes
`incoming(id, kind).len() + outgoing(id, kind).len()`, and returns the
top `limit` entries (capped at 100) sorted descending by total degree,
ties broken alphabetically.  Nodes with degree 0 are excluded.

### MCP tool — `mycelium_get_most_connected`

Request:
```json
{ "edge_kind": "calls", "limit": 10 }
```

`edge_kind` must be `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
`limit` defaults to 10, capped at 100.

Response:
```json
{
  "symbols": [
    { "path": "src/core.rs>Core", "degree": 12 },
    { "path": "src/utils.rs>helper", "degree": 9 }
  ],
  "count": 2
}
```

Unknown `edge_kind` returns `{ "error": "unknown edge_kind: ..." }`.

## Acceptance Criteria

- [x] `Store::most_connected(limit, kind)` ranks by `in + out` degree for `kind`.
- [x] Symbol nodes only (paths containing `>`); file nodes excluded.
- [x] Nodes with degree 0 excluded.
- [x] Results sorted descending by degree, ties broken by path ascending.
- [x] `limit` respected; capped at 100 internally.
- [x] `mycelium_get_most_connected`: valid edge_kind returns `{ symbols, count }`.
- [x] `mycelium_get_most_connected`: unknown edge_kind returns `{ error }`.
- [x] `limit` defaults to 10 when omitted.
- [x] All prior tests pass.
