# RFC-0077 — `Store::eccentricity` + `mycelium_get_eccentricity` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0077                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0050 (shortest_path), RFC-0076 (clustering_coefficient) |

## Summary

Add `Store::eccentricity(id, kind)` — the maximum BFS distance from a node
to any symbol node reachable from it for a given `EdgeKind` — and expose it
as `mycelium_get_eccentricity`.

Eccentricity measures "how deep is this node's reach?" in the directed graph:
the longest shortest path from the node to any reachable node.  A node that
can reach everything in 2 hops has eccentricity 2.  An isolated node or a
node with no outgoing reachability has eccentricity 0.

File nodes are excluded from traversal and from the reachable set.

O(V + E) per call (single BFS).

## Design

### Store method

```rust
pub fn eccentricity(&self, id: NodeId, kind: EdgeKind) -> usize
```

- Returns the maximum BFS distance from `id` to any reachable symbol node.
- Isolated node (no reachable nodes) → 0.
- `id` itself → distance 0, not counted.
- File nodes excluded.

### MCP tool — `mycelium_get_eccentricity`

Request:
```json
{ "path": "src/a.rs>A", "edge_kind": "calls" }
```

Response:
```json
{ "eccentricity": 3, "reachable_count": 15 }
```

- `reachable_count` = number of symbol nodes reachable from `path` (BFS
  result set, not counting `path` itself).
- Unknown path → `{ "error": "unknown path: <value>" }`.
- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [ ] `eccentricity` returns 0 for an isolated node.
- [ ] `eccentricity` returns 1 for a node that only directly reaches others.
- [ ] `eccentricity` returns the correct max distance for a chain.
- [ ] `eccentricity` counts directed reachability (not undirected).
- [ ] File nodes excluded from traversal and reachable count.
- [ ] `mycelium_get_eccentricity`: valid path returns `{ eccentricity, reachable_count }`.
- [ ] `mycelium_get_eccentricity`: unknown path returns `{ error }`.
- [ ] `mycelium_get_eccentricity`: unknown edge_kind returns `{ error }`.
- [ ] All prior tests pass.
