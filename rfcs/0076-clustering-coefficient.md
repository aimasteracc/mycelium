# RFC-0076 — `Store::clustering_coefficient` + `mycelium_get_clustering_coefficient` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0076                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0075 (neighbor_similarity), RFC-0060 (symbol_neighborhood) |

## Summary

Add `Store::clustering_coefficient(id, kind)` — local clustering coefficient
for a symbol node — and expose it as `mycelium_get_clustering_coefficient`.

For a node u, let N(u) be its combined neighbor set (outgoing ∪ incoming,
self excluded, file nodes excluded).  The local clustering coefficient is:

```
CC(u) = #{(a,b) : a ∈ N(u), b ∈ N(u), a→b exists for kind} / (|N(u)| * (|N(u)| - 1))
```

Returns 0.0 when `|N(u)| < 2` (undefined — not enough neighbors to form a pair).

A score of 1.0 means every neighbor of u calls every other neighbor of u
(maximum local density); 0.0 means no two neighbors are connected.  High
clustering identifies nodes embedded in tight cohesive clusters vs. loose
hub-and-spoke topologies.  Complements RFC-0075's neighbor similarity.

O(degree²) per call.

## Design

### Store method

```rust
pub fn clustering_coefficient(&self, id: NodeId, kind: EdgeKind) -> f64
```

- Returns CC ∈ [0.0, 1.0].
- `|N(u)| < 2` → 0.0.
- File nodes excluded from N(u) and from edge counting.

### MCP tool — `mycelium_get_clustering_coefficient`

Request:
```json
{ "path": "src/a.rs>A", "edge_kind": "calls" }
```

Response:
```json
{ "coefficient": 0.67, "neighbor_count": 6, "neighbor_edge_count": 8 }
```

- `neighbor_edge_count` = number of directed edges among N(u) for `kind`.
- `neighbor_count` = |N(u)|.
- Unknown path → `{ "error": "unknown path: <value>" }`.
- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [x] `clustering_coefficient` returns 0.0 for a node with fewer than 2 neighbors.
- [x] `clustering_coefficient` returns 1.0 when all neighbors form a complete directed graph.
- [x] `clustering_coefficient` returns correct fractional value for partial connectivity.
- [x] File nodes excluded from neighbor set and edge counting.
- [x] `mycelium_get_clustering_coefficient`: valid path returns `{ coefficient, neighbor_count, neighbor_edge_count }`.
- [x] `mycelium_get_clustering_coefficient`: unknown path returns `{ error }`.
- [x] `mycelium_get_clustering_coefficient`: unknown edge_kind returns `{ error }`.
- [x] All prior tests pass.
