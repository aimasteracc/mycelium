# RFC-0059 — `Store::two_hop_neighbors` + `mycelium_get_two_hop_neighbors` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0059                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0043 (reachable_from), RFC-0011 (call graph) |

## Summary

Add `Store::two_hop_neighbors(id, kind)` — symbols reachable in exactly 2 outgoing
hops (direct neighbors of direct neighbors, excluding `id` itself and direct
neighbors) — and expose it as `mycelium_get_two_hop_neighbors`.

Complements `reachable_from` (RFC-0043) which returns all reachable nodes up to a
depth limit. `two_hop_neighbors` is a focused bridge detector: it reveals
what a symbol reaches indirectly through its immediate callees, without
traversing the full reachability set.

## Design

### Store method

```rust
pub fn two_hop_neighbors(&self, id: NodeId, kind: EdgeKind) -> Vec<String>
```

- Returns symbol paths reachable from `id` in exactly 2 outgoing steps for `kind`.
- Excludes `id` itself.
- Excludes direct (1-hop) neighbors of `id`.
- Includes only symbol nodes (paths containing `>`).
- Results sorted ascending.
- Returns empty `Vec` if `id` has no outgoing edges or none of its neighbors have outgoing edges.

### MCP tool — `mycelium_get_two_hop_neighbors`

Request:
```json
{ "path": "src/service.rs>Service", "edge_kind": "calls" }
```

`edge_kind` must be `"calls"`, `"imports"`, `"extends"`, or `"implements"`.

Response:
```json
{
  "neighbors": ["src/util.rs>format", "src/util.rs>parse"],
  "count": 2
}
```

Unknown path returns `{ "neighbors": [], "count": 0 }`.
Unknown `edge_kind` returns `{ "error": "unknown edge_kind: ..." }`.

## Acceptance Criteria

- [x] `Store::two_hop_neighbors(id, kind)` returns symbols exactly 2 hops away.
- [x] Direct (1-hop) neighbors excluded from result.
- [x] `id` itself excluded from result.
- [x] File nodes excluded (paths not containing `>`).
- [x] Results sorted ascending.
- [x] Empty result if source has no outgoing edges.
- [x] `mycelium_get_two_hop_neighbors`: unknown path returns `{ neighbors: [], count: 0 }`.
- [x] `mycelium_get_two_hop_neighbors`: unknown edge_kind returns `{ error }`.
- [x] `mycelium_get_two_hop_neighbors`: valid request returns `{ neighbors, count }`.
- [x] All prior tests pass.
