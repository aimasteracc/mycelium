# RFC-0085 — `Store::betweenness_centrality` + `mycelium_get_betweenness_centrality` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0085                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0082 (page_rank), RFC-0078 (harmonic_centrality) |

## Summary

Add `Store::betweenness_centrality(kind)` — Brandes' O(V×(V+E)) betweenness
centrality algorithm over all symbol nodes for a given `EdgeKind` — and
expose it as `mycelium_get_betweenness_centrality`.

Betweenness centrality of node `v`:
  BC(v) = Σ_{s≠t≠v} σ(s,t|v) / σ(s,t)

where `σ(s,t)` is the number of shortest paths from `s` to `t`, and
`σ(s,t|v)` is the count of those paths passing through `v`.

Identifies **bridge nodes**: utilities that lie on many critical dependency
paths.  A node with high betweenness is a bottleneck — its removal or
change ripples through many call chains.  Complements PageRank (in-degree
importance) and harmonic centrality (average closeness).

Normalized by dividing by `(n-1)×(n-2)` (directed graph denominator).
`n < 3` returns zero for all nodes.  File nodes excluded.

O(V × (V + E)) — one BFS + backward accumulation per source node.

## Design

### Store method

```rust
pub fn betweenness_centrality(&self, kind: EdgeKind) -> Vec<BetweennessEntry>
```

where:
```rust
pub struct BetweennessEntry {
    pub path: String,
    pub score: f64,   // normalized ∈ [0.0, 1.0]
}
```

- Returns entries for all symbol nodes sorted descending by score.
- `n < 2` → empty `Vec`.
- `n == 2` → all scores 0.0.
- Normalized: divide raw BC by `(n-1)×(n-2)`.
- File nodes excluded.

### MCP tool — `mycelium_get_betweenness_centrality`

Request:
```json
{ "edge_kind": "calls", "top_n": 10 }
```

Response:
```json
{
  "nodes": [{ "path": "src/core.rs>Core", "score": 0.42 }, ...],
  "symbol_count": 50,
  "top_n": 10
}
```

- `top_n` optional, defaults to 10.
- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [x] Empty graph → empty result.
- [x] Single node → empty result.
- [x] Two nodes → both scores 0.0.
- [x] Linear chain A→B→C → B has highest betweenness.
- [x] Scores normalized to [0.0, 1.0].
- [x] `mycelium_get_betweenness_centrality`: valid edge_kind returns `{ nodes, symbol_count, top_n }`.
- [x] `mycelium_get_betweenness_centrality`: unknown edge_kind returns `{ error }`.
- [x] All prior tests pass.
