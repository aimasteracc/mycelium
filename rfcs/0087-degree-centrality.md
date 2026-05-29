# RFC-0087 — `Store::degree_centrality` + `mycelium_get_degree_centrality` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0087                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0082 (page_rank), RFC-0085 (betweenness_centrality) |

## Summary

Add `Store::degree_centrality(kind)` — returns normalized in-degree and
out-degree centrality scores for every symbol node — and expose it as
`mycelium_get_degree_centrality`.

Degree centrality is the simplest centrality measure:
- **In-degree centrality**: how many symbols call/import/extend/implement this
  node.  High in-degree = widely used dependency (fan-in hub).
- **Out-degree centrality**: how many symbols this node calls/imports/extends/
  implements.  High out-degree = wide surface area (fan-out hub).

Both scores are normalized by `(n-1)` (max possible degree in a directed graph
with `n` nodes).  `n < 2` → all scores 0.0.

Complements `PageRank` (global importance with damping) and
`betweenness_centrality` (bridge nodes on shortest paths).  Degree centrality
is O(V + E) — one pass over edges.

## Design

### Store method

```rust
pub fn degree_centrality(&self, kind: EdgeKind) -> Vec<DegreeCentralityEntry>
```

where:
```rust
pub struct DegreeCentralityEntry {
    pub path: String,
    pub in_degree: usize,
    pub out_degree: usize,
    pub in_centrality: f64,   // in_degree / (n-1), ∈ [0.0, 1.0]
    pub out_centrality: f64,  // out_degree / (n-1), ∈ [0.0, 1.0]
}
```

- Returns entries for all symbol nodes.
- Sorted descending by `in_centrality`, then descending by `out_centrality`,
  then alphabetically by path.
- `n < 2` → all centrality scores 0.0.
- File nodes excluded.

### MCP tool — `mycelium_get_degree_centrality`

Request:
```json
{ "edge_kind": "calls", "top_n": 10, "sort_by": "in" }
```

Response:
```json
{
  "nodes": [
    { "path": "src/core.rs>Core", "in_degree": 5, "out_degree": 2,
      "in_centrality": 0.5, "out_centrality": 0.2 }
  ],
  "symbol_count": 11,
  "top_n": 10,
  "sort_by": "in"
}
```

- `top_n` optional, defaults to 10.
- `sort_by` optional: `"in"` (default) or `"out"`.
- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.
- Unknown `sort_by` value → `{ "error": "unknown sort_by: <value>" }`.

## Acceptance Criteria

- [ ] Empty graph → empty result.
- [ ] Single node → score 0.0 for both centralities.
- [ ] Two nodes, A→B: B has in_degree=1, in_centrality=1.0; A has out_degree=1, out_centrality=1.0.
- [ ] Scores normalized to [0.0, 1.0].
- [ ] Default sort is by `in_centrality` descending.
- [ ] `sort_by: "out"` sorts by `out_centrality` descending.
- [ ] File nodes excluded.
- [ ] `mycelium_get_degree_centrality`: valid edge_kind returns `{ nodes, symbol_count, top_n, sort_by }`.
- [ ] `mycelium_get_degree_centrality`: unknown edge_kind returns `{ error }`.
- [ ] `mycelium_get_degree_centrality`: unknown sort_by returns `{ error }`.
- [ ] All prior tests pass.
