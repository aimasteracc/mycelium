# RFC-0088 — `Store::closeness_centrality` + `mycelium_get_closeness_centrality` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0088                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0078 (harmonic_centrality), RFC-0085 (betweenness_centrality), RFC-0087 (degree_centrality) |

## Summary

Add `Store::closeness_centrality(kind)` — Wasserman-Faust normalized closeness
centrality for all symbol nodes — and expose it as
`mycelium_get_closeness_centrality`.

Classical closeness centrality of node `v` (Bavelas, 1950):
  CC(v) = (n-1) / Σ_{u≠v} d(v, u)

where `d(v, u)` is the shortest-path distance (BFS hop count) from `v` to `u`,
and the sum is only over reachable nodes.  Nodes that reach no other node get
score 0.0.

The **Wasserman-Faust** normalization accounts for disconnected graphs by
adjusting for the fraction of nodes actually reachable:
  CC_WF(v) = [(n_reach / (n-1))^2] * (n_reach / Σ d(v, u))

where `n_reach` is the number of nodes `v` can reach (excluding itself).

Identifies **well-connected hubs** that can propagate influence quickly through
the dependency graph.  Complementary to harmonic centrality (RFC-0078) and
betweenness centrality (RFC-0085).

O(V × (V + E)) — one BFS per source node.

## Design

### Store method

```rust
pub fn closeness_centrality(&self, kind: EdgeKind) -> Vec<ClosenessCentralityEntry>
```

where:
```rust
pub struct ClosenessCentralityEntry {
    pub path: String,
    pub score: f64,   // Wasserman-Faust normalized ∈ [0.0, 1.0]
}
```

- Returns entries for all symbol nodes sorted descending by score.
- Nodes that reach zero other nodes → score 0.0.
- `n < 2` → all scores 0.0 (single node); `n == 0` → empty `Vec`.
- File nodes excluded.

### MCP tool — `mycelium_get_closeness_centrality`

Request:
```json
{ "edge_kind": "calls", "top_n": 10 }
```

Response:
```json
{
  "nodes": [{ "path": "src/core.rs>Core", "score": 0.82 }, ...],
  "symbol_count": 50,
  "top_n": 10
}
```

- `top_n` optional, defaults to 10.
- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [x] Empty graph → empty result.
- [x] Single node → score 0.0.
- [x] Two nodes, no edge → both scores 0.0.
- [x] Two nodes, A→B: A has positive score (reaches B), B has score 0.0 (reaches no one).
- [x] Linear chain A→B→C: A has highest score, B has second, C has 0.0.
- [x] Scores ∈ [0.0, 1.0].
- [x] File nodes excluded.
- [x] `mycelium_get_closeness_centrality`: valid edge_kind returns `{ nodes, symbol_count, top_n }`.
- [x] `mycelium_get_closeness_centrality`: unknown edge_kind returns `{ error }`.
- [x] All prior tests pass.
