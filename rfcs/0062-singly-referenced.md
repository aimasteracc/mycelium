# RFC-0062 — `Store::singly_referenced` + `mycelium_get_singly_referenced` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0062                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0022 (entry_points, in-degree=0), RFC-0049 (leaf_symbols, out-degree=0), RFC-0054 (fan_in_rank) |

## Summary

Add `Store::singly_referenced(kind, limit)` — symbols that have **exactly one**
incoming edge for a given `EdgeKind` — and expose it as
`mycelium_get_singly_referenced`.

Singly-referenced symbols are "privately owned": only one other symbol depends
on them.  They are strong candidates for inlining, privatisation, or moving
closer to their sole consumer.  This fills the gap in the in-degree family:

| in-degree | existing tool        |
|-----------|----------------------|
| 0         | `entry_points` (Calls), `leaf_symbols` (any kind, out-degree 0) |
| 1         | **`singly_referenced`** (this RFC) |
| top-N     | `fan_in_rank`        |

## Design

### Store method

```rust
pub fn singly_referenced(
    &self,
    kind: EdgeKind,
    limit: usize,
) -> Vec<(String, String)>
```

- Iterates all symbol nodes (paths containing `>`).
- Keeps those where the incoming-edge count for `kind` equals exactly 1.
- For each hit, resolves the sole referencing node to its path string.
- If the referencing node path cannot be resolved, the pair is skipped.
- `limit` capped at 100.
- Results sorted by symbol path ascending.
- File nodes excluded from the **result** set; file nodes *are* allowed as the
  sole referencing node (a file that imports exactly one other thing).
- Returns `Vec<(symbol_path, referencing_path)>`.

### MCP tool — `mycelium_get_singly_referenced`

Request:
```json
{ "edge_kind": "calls", "limit": 20 }
```

`limit` defaults to 10. `edge_kind` must be `"calls"`, `"imports"`,
`"extends"`, or `"implements"`.

Response:
```json
{
  "symbols": [
    { "path": "src/util.rs>helper", "referenced_by": "src/main.rs>main" }
  ],
  "count": 1
}
```

Unknown `edge_kind` returns `{ "error": "unknown edge_kind: ..." }`.

## Acceptance Criteria

- [x] `Store::singly_referenced(kind, limit)` returns symbols with in-degree == 1.
- [x] `limit` capped at 100.
- [x] Results sorted by symbol path ascending.
- [x] File nodes excluded from result set.
- [x] Returns `(symbol_path, referencing_path)` tuples.
- [x] Pairs where the referencing path cannot be resolved are skipped.
- [x] `mycelium_get_singly_referenced`: valid request returns `{ symbols, count }`.
- [x] `mycelium_get_singly_referenced`: unknown edge_kind returns `{ error }`.
- [x] `limit` defaults to 10 if omitted in the MCP request.
- [x] All prior tests pass.
