# RFC-0054 — `Store::fan_in_rank` + `mycelium_get_fan_in_rank` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0054                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0019 (rank_symbols — in-degree Calls), RFC-0053 (fan_out_rank — out-degree) |

## Summary

Add `Store::fan_in_rank(kind, limit)` — top-N symbol nodes ranked by
in-degree (number of incoming edges) for a given `EdgeKind` — and expose
it as `mycelium_get_fan_in_rank`.

Complements `fan_out_rank` (RFC-0053, out-degree) to complete the
symmetric degree-ranking pair. High fan-in symbols are "hotspots" —
functions called by many others, or modules imported by many others.

`rank_symbols` (RFC-0019) already provides in-degree for `Calls` only.
`fan_in_rank` generalises this to any `EdgeKind`.

## Design

### Store method

```rust
pub fn fan_in_rank(&self, kind: EdgeKind, limit: usize) -> Vec<(String, usize)>
```

Iterates all symbol nodes (paths containing `>`), computes
`incoming(id, kind).len()`, excludes nodes with in-degree 0, sorts
descending by degree (ties broken alphabetically), and returns up to `limit`
entries (capped at 100).

### MCP tool — `mycelium_get_fan_in_rank`

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
    { "path": "src/utils.rs>format", "in_degree": 42 },
    { "path": "src/core.rs>init",    "in_degree": 17 }
  ],
  "count": 2
}
```

Unknown `edge_kind` returns `{ "error": "unknown edge_kind: ..." }`.

## Acceptance Criteria

- [x] `Store::fan_in_rank(kind, limit)` ranks by in-degree for `kind`.
- [x] Symbol nodes only (paths containing `>`); file nodes excluded.
- [x] Nodes with in-degree 0 excluded.
- [x] Results sorted descending by degree, ties broken by path ascending.
- [x] `limit` respected; capped at 100 internally.
- [x] `mycelium_get_fan_in_rank`: valid edge_kind returns `{ symbols, count }`.
- [x] `mycelium_get_fan_in_rank`: unknown edge_kind returns `{ error }`.
- [x] `limit` defaults to 10 when omitted.
- [x] All prior tests pass.
