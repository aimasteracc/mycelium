# RFC-0043 — `Store::reachable_from` + `mycelium_get_reachable` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0043                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0041 (outgoing_refs), RFC-0017 (find_call_path) |

## Summary

Add `Store::reachable_from(id, kind, max_depth)` — flat BFS reachability
set from a node following outgoing edges of a given `EdgeKind` up to a
depth limit — and expose it as `mycelium_get_reachable`.

The existing tree methods (`callee_tree`, `import_tree`, …) return nested
structures that are verbose to traverse.  Agents doing transitive dependency
analysis ("what does this function ultimately depend on?") need a flat sorted
list, not a tree.

| Method | Returns | Use case |
|---|---|---|
| `callee_tree` | nested tree | visualising call hierarchy |
| `find_call_path` | shortest path between two nodes | tracing a specific dependency |
| `reachable_from` | flat sorted list | all transitive dependencies |

## Design

### Store method

```rust
pub fn reachable_from(
    &self,
    id: NodeId,
    kind: EdgeKind,
    max_depth: usize,
) -> Vec<String>
```

BFS from `id` following outgoing `kind` edges.  The starting node is
excluded from the result.  Visits each node at most once (cycle-safe).
`max_depth` is capped at 20 internally.  Results sorted lexicographically.

### MCP tool — `mycelium_get_reachable`

Request:
```json
{ "path": "src/app.rs>App", "edge_kind": "calls", "max_depth": 5 }
```

`edge_kind` must be `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
`max_depth` is optional, defaults to 10, capped at 20.

Response:
```json
{
  "reachable": ["src/a.rs>helper", "src/b.rs>util"],
  "count": 2
}
```

Unknown path returns `{ "error": "path not found: ..." }`.
Unknown edge_kind returns `{ "error": "unknown edge_kind: ..." }`.

## Acceptance Criteria

- [ ] `Store::reachable_from(id, kind, max_depth)` performs BFS via outgoing `kind` edges.
- [ ] Starting node excluded from result.
- [ ] Each node visited at most once (cycle-safe).
- [ ] `max_depth` respected; 0 returns empty list.
- [ ] Results sorted lexicographically.
- [ ] `mycelium_get_reachable`: known path + valid edge_kind returns `{ reachable, count }`.
- [ ] `mycelium_get_reachable`: unknown path returns `{ error }`.
- [ ] `mycelium_get_reachable`: unknown edge_kind returns `{ error }`.
- [ ] `max_depth` defaults to 10 when omitted; capped at 20.
- [ ] All prior tests pass.
