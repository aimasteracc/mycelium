# RFC-0086 — `Store::strongly_connected_components` + `mycelium_get_strongly_connected_components` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0086                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0080 (reachable_set), RFC-0081 (reaches_into) |

## Summary

Add `Store::strongly_connected_components(kind)` — Tarjan's O(V+E) strongly
connected components (SCC) algorithm over all symbol nodes for a given
`EdgeKind` — and expose it as `mycelium_get_strongly_connected_components`.

A **strongly connected component** is a maximal set of nodes where every node
is reachable from every other node via directed edges.  In a dependency graph:

- **Singleton SCC** (size 1): a node with no cycle through itself — normal.
- **Non-trivial SCC** (size ≥ 2): a set of nodes that mutually depend on each
  other — a circular dependency.

Identifying SCCs answers: *"Which functions / modules are in a dependency
cycle?"*  Circular dependencies are a common source of build failures and
architectural debt.  Complements `reachable_set` (what a node reaches) and
`betweenness_centrality` (bridge nodes).

O(V + E) — single DFS pass with a stack (Tarjan's algorithm).

## Design

### Store method

```rust
pub fn strongly_connected_components(&self, kind: EdgeKind) -> Vec<SccEntry>
```

where:
```rust
pub struct SccEntry {
    pub members: Vec<String>,  // symbol paths, sorted alphabetically
    pub size: usize,
}
```

- Returns one `SccEntry` per component.
- Components with `size == 1` (singleton, no self-edge) are included.
- Results sorted descending by `size`, then alphabetically by first member.
- File nodes excluded.
- `n == 0` → empty `Vec`.

### MCP tool — `mycelium_get_strongly_connected_components`

Request:
```json
{ "edge_kind": "calls", "min_size": 2 }
```

Response:
```json
{
  "components": [
    { "members": ["src/a.rs>A", "src/b.rs>B"], "size": 2 }
  ],
  "total_components": 5,
  "symbol_count": 10,
  "min_size": 2
}
```

- `min_size` optional, defaults to 1 (return all components including singletons).
- `min_size: 2` filters to only non-trivial SCCs (cycles).
- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [x] Empty graph → empty result.
- [x] Single node → one singleton SCC.
- [x] Two nodes, no edges → two singleton SCCs.
- [x] Two nodes, one directed edge A→B → two singleton SCCs.
- [x] Two nodes, mutual edges A→B and B→A → one SCC with both members.
- [x] Linear chain A→B→C (no back-edges) → three singleton SCCs.
- [x] Cycle A→B→C→A → one SCC with three members.
- [x] `min_size: 2` filters out singleton SCCs.
- [x] Members within each SCC are sorted alphabetically.
- [x] Results sorted descending by size.
- [x] File nodes excluded.
- [x] `mycelium_get_strongly_connected_components`: valid edge_kind returns `{ components, total_components, symbol_count, min_size }`.
- [x] `mycelium_get_strongly_connected_components`: unknown edge_kind returns `{ error }`.
- [x] All prior tests pass.
