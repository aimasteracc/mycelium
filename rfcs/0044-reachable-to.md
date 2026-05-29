# RFC-0044 — `Store::reachable_to` + `mycelium_get_reachable_to` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0044                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0043 (reachable_from), RFC-0039 (cross_refs) |

## Summary

Add `Store::reachable_to(id, kind, max_depth)` — flat BFS reachability
**into** a node following incoming edges of a given `EdgeKind` up to a
depth limit — and expose it as `mycelium_get_reachable_to`.

| Method | Direction | Question answered |
|---|---|---|
| `reachable_from` | outgoing | What does this symbol transitively depend on? |
| `reachable_to` | incoming | What transitively depends on this symbol? |

Together they give complete forward and backward reachability in two calls,
enabling impact analysis ("if I change X, who is affected?").

## Design

### Store method

```rust
pub fn reachable_to(
    &self,
    id: NodeId,
    kind: EdgeKind,
    max_depth: usize,
) -> Vec<String>
```

BFS from `id` following **incoming** `kind` edges.  The starting node is
excluded from the result.  Visits each node at most once (cycle-safe).
`max_depth` is capped at 20 internally.  Results sorted lexicographically.

### MCP tool — `mycelium_get_reachable_to`

Request:
```json
{ "path": "src/utils.rs>helper", "edge_kind": "calls", "max_depth": 5 }
```

`edge_kind` must be `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
`max_depth` is optional, defaults to 10, capped at 20.

Response:
```json
{
  "reachable": ["src/app.rs>App", "src/main.rs>main"],
  "count": 2
}
```

Unknown path returns `{ "error": "path not found: ..." }`.
Unknown edge_kind returns `{ "error": "unknown edge_kind: ..." }`.

## Acceptance Criteria

- [x] `Store::reachable_to(id, kind, max_depth)` performs BFS via incoming `kind` edges.
- [x] Starting node excluded from result.
- [x] Each node visited at most once (cycle-safe).
- [x] `max_depth` respected; 0 returns empty list.
- [x] Results sorted lexicographically.
- [x] `mycelium_get_reachable_to`: known path + valid edge_kind returns `{ reachable, count }`.
- [x] `mycelium_get_reachable_to`: unknown path returns `{ error }`.
- [x] `mycelium_get_reachable_to`: unknown edge_kind returns `{ error }`.
- [x] `max_depth` defaults to 10 when omitted; capped at 20.
- [x] All prior tests pass.
