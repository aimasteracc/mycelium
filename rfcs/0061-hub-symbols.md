# RFC-0061 — `Store::hub_symbols` + `mycelium_get_hub_symbols` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0061                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0053 (fan_out_rank), RFC-0054 (fan_in_rank), RFC-0048 (most_connected) |

## Summary

Add `Store::hub_symbols(kind, min_in, min_out, limit)` — symbols with both
in-degree ≥ `min_in` AND out-degree ≥ `min_out` for a given `EdgeKind` —
and expose it as `mycelium_get_hub_symbols`.

Hubs are the architectural "connectors": they are called by many (high in-degree)
and also call many (high out-degree). Unlike `fan_in_rank` (only in-degree) or
`fan_out_rank` (only out-degree), `hub_symbols` identifies the intersection — the
symbols that sit at the centre of the network and are both widely-used and
highly-dependent.

## Design

### Store method

```rust
pub fn hub_symbols(
    &self,
    kind: EdgeKind,
    min_in: usize,
    min_out: usize,
    limit: usize,
) -> Vec<(String, usize, usize)>
```

- Filters symbol nodes where `in_degree >= min_in AND out_degree >= min_out`.
- `limit` capped at 100.
- Results sorted by `in_degree + out_degree` descending; ties broken by path ascending.
- Returns `(path, in_degree, out_degree)` tuples.
- File nodes excluded (paths not containing `>`).

### MCP tool — `mycelium_get_hub_symbols`

Request:
```json
{ "edge_kind": "calls", "min_in": 2, "min_out": 2, "limit": 10 }
```

`min_in` and `min_out` default to 1 if omitted. `limit` defaults to 10.
`edge_kind` must be `"calls"`, `"imports"`, `"extends"`, or `"implements"`.

Response:
```json
{
  "hubs": [
    { "path": "src/svc.rs>Service", "in_degree": 5, "out_degree": 8 }
  ],
  "count": 1
}
```

Unknown `edge_kind` returns `{ "error": "unknown edge_kind: ..." }`.

## Acceptance Criteria

- [ ] `Store::hub_symbols(kind, min_in, min_out, limit)` filters by in AND out degree.
- [ ] `limit` capped at 100.
- [ ] Sorted by `in_degree + out_degree` descending; ties by path ascending.
- [ ] File nodes excluded.
- [ ] Returns `(path, in_degree, out_degree)` tuples.
- [ ] `mycelium_get_hub_symbols`: valid request returns `{ hubs, count }`.
- [ ] `mycelium_get_hub_symbols`: unknown edge_kind returns `{ error }`.
- [ ] `min_in` and `min_out` default to 1 if omitted in the MCP request.
- [ ] All prior tests pass.
