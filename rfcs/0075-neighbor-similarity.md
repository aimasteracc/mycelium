# RFC-0075 — `Store::neighbor_similarity` + `mycelium_get_neighbor_similarity` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0075                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0060 (symbol_neighborhood), RFC-0052 (common_callers) |

## Summary

Add `Store::neighbor_similarity(id1, id2, kind)` — Jaccard similarity between
the combined neighbor sets of two symbol nodes for a given `EdgeKind` — and
expose it as `mycelium_get_neighbor_similarity`.

Jaccard similarity = |N(u) ∩ N(v)| / |N(u) ∪ N(v)|, where `N(x)` is the
union of outgoing and incoming neighbors of `x` for `kind`.

A score of 1.0 means the two symbols have identical structural roles (same
callers, callees, importers, etc.); 0.0 means no structural overlap.  This
answers "which modules play similar roles in the dependency graph?" — a
useful signal for refactoring candidates and duplicate detection.

O(max_degree) per call.

## Design

### Store method

```rust
pub fn neighbor_similarity(&self, id1: NodeId, id2: NodeId, kind: EdgeKind) -> f64
```

Returns Jaccard ∈ [0.0, 1.0].  Two isolated nodes (both degree 0) return 0.0
(undefined similarity rather than 1.0 — they share no context).

### MCP tool — `mycelium_get_neighbor_similarity`

Request:
```json
{ "path1": "src/a.rs>A", "path2": "src/b.rs>B", "edge_kind": "calls" }
```

Response:
```json
{ "similarity": 0.75, "shared": 3, "total": 4 }
```

- Unknown path or unknown `edge_kind` → `{ "error": "..." }`.

## Acceptance Criteria

- [x] `neighbor_similarity` returns 1.0 for two nodes with identical neighbors.
- [x] `neighbor_similarity` returns 0.0 for two isolated nodes.
- [x] `neighbor_similarity` returns 0.0 for nodes with no overlapping neighbors.
- [x] `neighbor_similarity(id, id, kind)` returns 1.0 when id has neighbors.
- [x] File nodes excluded from neighbor sets.
- [x] `mycelium_get_neighbor_similarity`: valid paths return `{ similarity, shared, total }`.
- [x] `mycelium_get_neighbor_similarity`: unknown path returns `{ error }`.
- [x] `mycelium_get_neighbor_similarity`: unknown edge_kind returns `{ error }`.
- [x] All prior tests pass.
