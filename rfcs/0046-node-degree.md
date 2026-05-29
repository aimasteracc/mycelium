# RFC-0046 — `Store::node_degree` + `mycelium_get_node_degree` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0046                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0039 (cross_refs), RFC-0041 (outgoing_refs) |

## Summary

Add `Store::node_degree(id)` — a fast edge-count summary (in-degree and
out-degree per `EdgeKind`) for a node — and expose it as
`mycelium_get_node_degree`.

`cross_refs` and `outgoing_refs` return full edge lists.  Agents doing
broad coupling analysis ("is this symbol highly connected?") need counts
only — not the full lists — to avoid large payloads.

## Design

### Store type

```rust
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NodeDegree {
    pub in_calls: usize,
    pub out_calls: usize,
    pub in_imports: usize,
    pub out_imports: usize,
    pub in_extends: usize,
    pub out_extends: usize,
    pub in_implements: usize,
    pub out_implements: usize,
}
```

### Store method

```rust
pub fn node_degree(&self, id: NodeId) -> NodeDegree
```

Returns incoming and outgoing counts for all four `EdgeKind`s.

### MCP tool — `mycelium_get_node_degree`

Request: `{ "path": "src/app.rs>App" }`

Response:
```json
{
  "in_calls": 5,
  "out_calls": 3,
  "in_imports": 2,
  "out_imports": 1,
  "in_extends": 0,
  "out_extends": 1,
  "in_implements": 0,
  "out_implements": 0
}
```

Unknown path returns `{ "error": "path not found: ..." }`.

## Acceptance Criteria

- [ ] `NodeDegree` struct with 8 fields (in/out × 4 kinds).
- [ ] `Store::node_degree(id)` returns correct counts for all kinds.
- [ ] Isolated node returns all-zero `NodeDegree`.
- [ ] `mycelium_get_node_degree`: known path returns `{ in_calls, out_calls, … }`.
- [ ] `mycelium_get_node_degree`: unknown path returns `{ error }`.
- [ ] All prior tests pass.
