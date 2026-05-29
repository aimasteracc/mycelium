# RFC-0066 — `Store::batch_node_degree` + `mycelium_batch_node_degree` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0066                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0046 (node_degree), RFC-0025 (batch_symbol_info) |

## Summary

Add `Store::batch_node_degree(ids)` — returns `NodeDegree` for each of the
given `NodeId`s in input order — and expose it as
`mycelium_batch_node_degree`.

`node_degree` (RFC-0046) provides the full in/out degree breakdown across all
four `EdgeKind`s for a single symbol.  `batch_node_degree` makes this
available for up to 50 symbols in one call, eliminating N round trips when
analysing a set of related symbols.

## Design

### Store method

```rust
pub fn batch_node_degree(&self, ids: &[NodeId]) -> Vec<NodeDegree>
```

- Returns one `NodeDegree` per id, in input order.
- IDs that are not present in the synapse return `NodeDegree::default()` (all
  counts zero).

### MCP tool — `mycelium_batch_node_degree`

Request:
```json
{ "paths": ["src/a.rs>A", "src/b.rs>B"] }
```

- `paths`: up to 50 symbol paths.

Response:
```json
{
  "degrees": [
    {
      "path": "src/a.rs>A",
      "in_calls": 3, "out_calls": 5,
      "in_imports": 0, "out_imports": 2,
      "in_extends": 0, "out_extends": 1,
      "in_implements": 0, "out_implements": 0
    },
    { "path": "src/unknown.rs>X", "error": "path not found" }
  ],
  "count": 2
}
```

- Known paths return the full degree breakdown.
- Unknown paths return `{ "path": "...", "error": "path not found" }`.
- Results in input order.
- `count` is the total number of entries (including error entries).

## Acceptance Criteria

- [ ] `Store::batch_node_degree(ids)` returns one `NodeDegree` per id in input order.
- [ ] Unknown (not-in-synapse) ids return `NodeDegree::default()`.
- [ ] `mycelium_batch_node_degree`: known paths return full degree breakdown.
- [ ] `mycelium_batch_node_degree`: unknown paths return `{ path, error }`.
- [ ] Results in input order.
- [ ] `count` equals the number of paths supplied (up to 50).
- [ ] All prior tests pass.
