# RFC-0053 — `Store::fan_out_rank` + `mycelium_get_fan_out_rank` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0053                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0019 (rank_symbols — in-degree), RFC-0048 (most_connected — total degree) |

## Summary

Add `Store::fan_out_rank(kind, limit)` — top-N symbol nodes ranked by
out-degree (number of outgoing edges) for a given `EdgeKind` — and expose
it as `mycelium_get_fan_out_rank`.

Complements `rank_symbols` (in-degree for Calls only) and `most_connected`
(total degree for any kind).  High fan-out symbols are "orchestrators" —
functions that call many others, or modules that import many others.

## Design

### Store method

```rust
pub fn fan_out_rank(&self, kind: EdgeKind, limit: usize) -> Vec<(String, usize)>
```

Iterates all symbol nodes (paths containing `>`), computes
`outgoing(id, kind).len()`, excludes nodes with out-degree 0, sorts
descending by degree (ties broken alphabetically), and returns up to `limit`
entries (capped at 100).

### MCP tool — `mycelium_get_fan_out_rank`

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
    { "path": "src/orchestrator.rs>run", "out_degree": 15 },
    { "path": "src/core.rs>init", "out_degree": 8 }
  ],
  "count": 2
}
```

Unknown `edge_kind` returns `{ "error": "unknown edge_kind: ..." }`.

## Acceptance Criteria

- [x] `Store::fan_out_rank(kind, limit)` ranks by out-degree for `kind`.
- [x] Symbol nodes only (paths containing `>`); file nodes excluded.
- [x] Nodes with out-degree 0 excluded.
- [x] Results sorted descending by degree, ties broken by path ascending.
- [x] `limit` respected; capped at 100 internally.
- [x] `mycelium_get_fan_out_rank`: valid edge_kind returns `{ symbols, count }`.
- [x] `mycelium_get_fan_out_rank`: unknown edge_kind returns `{ error }`.
- [x] `limit` defaults to 10 when omitted.
- [x] All prior tests pass.
