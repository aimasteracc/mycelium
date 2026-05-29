# RFC-0072 — `Store::biconnected_components` + `mycelium_get_biconnected_components` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0072                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0071 (bridge_edges), RFC-0070 (articulation_points) |

## Summary

Add `Store::biconnected_components(kind)` — partitions the undirected symbol
graph into **biconnected components** (2-connected subgraphs) for a given
`EdgeKind` — and expose it as `mycelium_get_biconnected_components`.

A biconnected component (BCC) is a maximal subgraph with no articulation
point: removing any single vertex leaves it connected.  BCCs reveal tightly
coupled code clusters — modules that are so interdependent that no single one
is a single point of failure within the cluster.  BCCs containing only a
bridge edge (2 nodes, 1 edge) are fine-grained; larger BCCs represent
cohesive, cycle-rich subsystems.

The algorithm is Tarjan's iterative BCC detection via an edge stack, O(V+E).
Edges are treated as undirected.  File nodes are excluded.

## Design

### Store method

```rust
pub fn biconnected_components(&self, kind: EdgeKind) -> Vec<Vec<String>>
```

- Treats all `kind` edges as undirected.
- Returns groups of symbol node paths, one group per BCC.
- Each group contains ≥ 2 nodes (singletons excluded).
- Each group is sorted ascending.
- Groups sorted by size descending, ties broken by first element ascending.
- File nodes excluded.

### MCP tool — `mycelium_get_biconnected_components`

Request:
```json
{ "edge_kind": "calls" }
```

Response:
```json
{
  "components": [["src/a.rs>A", "src/b.rs>B", "src/c.rs>C"], ["src/d.rs>D", "src/e.rs>E"]],
  "component_count": 2,
  "total_symbols": 5
}
```

- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [ ] `Store::biconnected_components(kind)` returns groups sorted by size desc.
- [ ] A triangle (cycle of 3) forms a single BCC of 3 nodes.
- [ ] A bridge edge's endpoints each appear in their own 2-node BCC.
- [ ] Singleton nodes (degree 0) are excluded.
- [ ] File nodes excluded.
- [ ] A "bowtie" (two triangles sharing one vertex) produces two BCCs of 3.
- [ ] `mycelium_get_biconnected_components`: valid edge_kind returns `{ components, component_count, total_symbols }`.
- [ ] `mycelium_get_biconnected_components`: unknown edge_kind returns `{ error }`.
- [ ] All prior tests pass.
