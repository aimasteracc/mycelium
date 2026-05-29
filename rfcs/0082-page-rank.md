# RFC-0082 — `Store::page_rank` + `mycelium_page_rank` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0082                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0078 (harmonic_centrality), RFC-0077 (eccentricity) |

## Summary

Add `Store::page_rank(kind, damping, iterations)` — iterative PageRank
over symbol nodes for a given `EdgeKind` — and expose it as
`mycelium_page_rank`.

Identifies globally important symbols: entry points, heavily-called
utilities, and hub nodes that many other symbols depend on.  Complements
local metrics (harmonic centrality, eccentricity) with a global importance
ranking.

O(iterations × (V + E)). File nodes excluded.

## Design

### Algorithm

Standard power-iteration PageRank:
- Initialize each of the `n` symbol nodes with score `1.0 / n`.
- Each iteration: for every symbol node `v` with out-degree `k > 0`,
  distribute `score(v) / k` to each successor.  Dangling nodes (out-degree
  0) distribute their mass uniformly to all nodes.
- Apply damping: `new_score(v) = (1 - d) / n + d * in_flow(v)`.
- Repeat for `iterations` rounds (convergence not required; caller controls).
- Return sorted descending by score.

### Store method

```rust
pub struct PageRankEntry {
    pub path: String,
    pub score: f64,
}

pub fn page_rank(
    &self,
    kind: EdgeKind,
    damping: f64,       // typically 0.85; clamped to [0.0, 1.0]
    iterations: usize,  // typically 20; 0 → return uniform scores
) -> Vec<PageRankEntry>  // sorted descending by score
```

- `n < 1` → empty `Vec`.
- `n == 1` → single entry with score 1.0.
- `damping` clamped to `[0.0, 1.0]`.
- File nodes excluded from nodes and edges.

### MCP tool — `mycelium_page_rank`

Request:
```json
{ "edge_kind": "calls", "damping": 0.85, "iterations": 20 }
```

Response (top 10 by default):
```json
{
  "nodes": [
    { "path": "src/core.rs>Core", "score": 0.152 },
    ...
  ],
  "symbol_count": 42,
  "top_n": 10
}
```

- `top_n` defaults to 10; accepts optional `top_n` request field.
- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [x] Empty graph → empty result.
- [x] Single isolated node → score 1.0.
- [x] Uniform graph (no edges) → all scores equal `1/n`.
- [x] Star graph (one hub) → hub has highest score after convergence.
- [x] Chain A→B→C → C has higher score than A (C is more depended-upon).
- [x] Damping 0.0 → all scores `1/n` (teleportation only).
- [x] `mycelium_page_rank`: valid edge_kind returns `{ nodes, symbol_count, top_n }`.
- [x] `mycelium_page_rank`: unknown edge_kind returns `{ error }`.
- [x] All prior tests pass.
