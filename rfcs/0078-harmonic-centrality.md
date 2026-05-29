# RFC-0078 — `Store::harmonic_centrality` + `mycelium_get_harmonic_centrality` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0078                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0077 (eccentricity), RFC-0050 (shortest_path) |

## Summary

Add `Store::harmonic_centrality(id, kind)` — harmonic centrality for a
symbol node for a given `EdgeKind` — and expose it as
`mycelium_get_harmonic_centrality`.

Harmonic centrality = (1/(n-1)) × Σ_{v≠id, v reachable} (1 / d(id, v))

where n is the total symbol count, d(id, v) is the BFS shortest-path
distance from `id` to `v`, and the sum is over all reachable symbol nodes
(file nodes excluded).

Unreachable nodes contribute 0 (not infinity), making harmonic centrality
well-defined for directed graphs where some nodes may not be reachable.
A score near 1.0 means the node can reach all others in ~1 hop; 0.0 means
completely isolated.  Complements eccentricity (max distance) by measuring
average closeness.

O(V + E) per call (single BFS).

## Design

### Store method

```rust
pub fn harmonic_centrality(&self, id: NodeId, kind: EdgeKind) -> f64
```

- Returns harmonic centrality ∈ [0.0, 1.0].
- Isolated node (no reachable symbols) → 0.0.
- n = total symbol count (file nodes excluded); n < 2 → 0.0.
- File nodes excluded from BFS and from the denominator.

### MCP tool — `mycelium_get_harmonic_centrality`

Request:
```json
{ "path": "src/a.rs>A", "edge_kind": "calls" }
```

Response:
```json
{ "harmonic_centrality": 0.42, "reachable_count": 15, "symbol_count": 100 }
```

- `reachable_count` = symbol nodes BFS-reachable from `path`.
- `symbol_count` = total symbol nodes in the graph (denominator of normalization).
- Unknown path → `{ "error": "unknown path: <value>" }`.
- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [ ] `harmonic_centrality` returns 0.0 for an isolated node.
- [ ] Returns 1.0 when a node reaches all other symbols in exactly 1 hop.
- [ ] Returns correct fractional value for a chain (1-hop and 2-hop reachable).
- [ ] Unreachable nodes do not affect the result.
- [ ] File nodes excluded from BFS and from symbol_count.
- [ ] `mycelium_get_harmonic_centrality`: valid path returns `{ harmonic_centrality, reachable_count, symbol_count }`.
- [ ] `mycelium_get_harmonic_centrality`: unknown path returns `{ error }`.
- [ ] `mycelium_get_harmonic_centrality`: unknown edge_kind returns `{ error }`.
- [ ] All prior tests pass.
