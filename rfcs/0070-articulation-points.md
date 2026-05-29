# RFC-0070 — `Store::articulation_points` + `mycelium_find_articulation_points` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0070                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0068 (WCC), RFC-0067 (cycle_members) |

## Summary

Add `Store::articulation_points(kind)` — returns the paths of symbol nodes
that are **articulation points** (cut vertices) in the undirected version of
the symbol graph for a given `EdgeKind` — and expose it as
`mycelium_find_articulation_points`.

An articulation point is a node whose removal splits a connected component
into two or more smaller components.  These are the single points of
structural failure in the dependency graph: if an articulation point module
is removed or broken, the codebase graph fragments.

The algorithm is Tarjan's iterative DFS (discovery time + low-link values),
O(V + E).  Edges are treated as undirected.  File nodes are excluded.

## Design

### Store method

```rust
pub fn articulation_points(&self, kind: EdgeKind) -> Vec<String>
```

- Treats all `kind` edges as undirected (union of outgoing + incoming).
- Returns paths of symbol nodes that are articulation points, sorted ascending.
- Nodes in singleton components (no neighbors) are never articulation points.
- File nodes excluded.

### MCP tool — `mycelium_find_articulation_points`

Request:
```json
{ "edge_kind": "calls" }
```

Response:
```json
{ "points": ["src/b.rs>B", "src/d.rs>D"], "count": 2 }
```

- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [ ] `Store::articulation_points(kind)` returns paths sorted ascending.
- [ ] Removing an articulation point disconnects its WCC.
- [ ] Non-articulation-point nodes (their removal leaves the graph connected) excluded.
- [ ] Singleton nodes (degree 0) never returned.
- [ ] File nodes excluded.
- [ ] `mycelium_find_articulation_points`: valid edge_kind returns `{ points, count }`.
- [ ] `mycelium_find_articulation_points`: unknown edge_kind returns `{ error }`.
- [ ] All prior tests pass.
