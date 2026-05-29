# RFC-0071 — `Store::bridge_edges` + `mycelium_find_bridge_edges` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0071                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0070 (articulation_points), RFC-0068 (WCC) |

## Summary

Add `Store::bridge_edges(kind)` — returns the `(from_path, to_path)` pairs of
edges that are **bridges** (cut edges) in the undirected version of the symbol
graph for a given `EdgeKind` — and expose it as `mycelium_find_bridge_edges`.

A bridge edge is an edge whose removal splits a connected component into two or
more smaller components.  These are the fragile single-link connections in the
dependency graph: if a bridge dependency is severed, the codebase graph
fragments.  Complements articulation points (RFC-0070): where APs are vertex
cut-points, bridges are edge cut-points.

The algorithm is Tarjan's iterative bridge-finding DFS (disc + low-link
values), O(V + E).  Edges are treated as undirected.  File nodes are excluded.

## Design

### Store method

```rust
pub fn bridge_edges(&self, kind: EdgeKind) -> Vec<(String, String)>
```

- Treats all `kind` edges as undirected (union of outgoing + incoming).
- Returns `(from_path, to_path)` pairs where `from < to` (canonical order),
  sorted ascending by `(from, to)`.
- Multigraph-safe: parallel edges between the same pair of nodes are **not**
  bridges (removing one still leaves the other).
- File nodes excluded.

### MCP tool — `mycelium_find_bridge_edges`

Request:
```json
{ "edge_kind": "calls" }
```

Response:
```json
{ "bridges": [{ "from": "src/a.rs>A", "to": "src/b.rs>B" }], "count": 1 }
```

- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [ ] `Store::bridge_edges(kind)` returns `(from, to)` pairs with `from < to`, sorted ascending.
- [ ] Removing a bridge edge disconnects its WCC.
- [ ] Non-bridge edges (removing them leaves the graph connected) excluded.
- [ ] Singleton nodes (degree 0) never produce bridges.
- [ ] File nodes excluded.
- [ ] Parallel edges between the same node pair are not bridges.
- [ ] `mycelium_find_bridge_edges`: valid edge_kind returns `{ bridges, count }`.
- [ ] `mycelium_find_bridge_edges`: unknown edge_kind returns `{ error }`.
- [ ] All prior tests pass.
