# RFC-0079 — `Store::mutual_reachability` + `mycelium_get_mutual_reachability` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0079                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0050 (shortest_path), RFC-0077 (eccentricity) |

## Summary

Add `Store::mutual_reachability(id1, id2, kind)` — bidirectional
reachability check between two symbol nodes for a given `EdgeKind` — and
expose it as `mycelium_get_mutual_reachability`.

Answers "are these two symbols connected, and in which direction(s)?"
Returns the forward BFS distance from `id1` to `id2`, the backward BFS
distance from `id2` to `id1`, and derived flags.

`id1 == id2` → both distances are 0, both directions are true.

O(V + E) total (at most two BFS traversals; the second is skipped if
`id1 == id2`).  File nodes excluded from traversal.

## Design

### Store method

```rust
pub struct MutualReachability {
    pub forward: bool,         // id1 can reach id2
    pub backward: bool,        // id2 can reach id1
    pub mutual: bool,          // both directions
    pub forward_distance: Option<usize>,   // BFS hops id1→id2; None if unreachable
    pub backward_distance: Option<usize>,  // BFS hops id2→id1; None if unreachable
}

pub fn mutual_reachability(
    &self,
    id1: NodeId,
    id2: NodeId,
    kind: EdgeKind,
) -> MutualReachability
```

### MCP tool — `mycelium_get_mutual_reachability`

Request:
```json
{ "path1": "src/a.rs>A", "path2": "src/b.rs>B", "edge_kind": "calls" }
```

Response (forward only):
```json
{ "forward": true, "backward": false, "mutual": false, "forward_distance": 3, "backward_distance": null }
```

Response (mutual):
```json
{ "forward": true, "backward": true, "mutual": true, "forward_distance": 1, "backward_distance": 2 }
```

- Unknown path → `{ "error": "unknown path: <value>" }`.
- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [x] `mutual_reachability(id, id, kind)` returns both directions true, distances 0.
- [x] Forward-only edge returns forward=true, backward=false.
- [x] No connection returns both false, both distances null.
- [x] Mutual connection returns both true, correct distances.
- [x] `mycelium_get_mutual_reachability`: valid paths return full struct.
- [x] `mycelium_get_mutual_reachability`: unknown path returns `{ error }`.
- [x] `mycelium_get_mutual_reachability`: unknown edge_kind returns `{ error }`.
- [x] All prior tests pass.
