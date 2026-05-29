# RFC-0074 — `Store::graph_metrics` + `mycelium_get_graph_metrics` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0074                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0038 (graph_stats), RFC-0073 (degree_histogram) |

## Summary

Add `Store::graph_metrics(kind)` — computes summary structural metrics for
the symbol graph for a given `EdgeKind` — and expose it as
`mycelium_get_graph_metrics`.

Where `graph_stats` (RFC-0038) gives raw counts, `graph_metrics` gives
**derived architectural indicators**:

- **density**: how "full" is the graph? (0 = empty, 1 = complete).
- **average degree**: mean edges per symbol node.
- **max in/out degree**: identifies hub nodes at a glance.

These give an instant health check of the dependency topology.

All O(V + E).  File nodes excluded.

## Design

### Store method

```rust
pub fn graph_metrics(&self, kind: EdgeKind) -> EdgeKindMetrics
```

```rust
#[derive(Debug, Clone, Default)]
pub struct EdgeKindMetrics {
    pub symbol_count: usize,
    pub directed_edge_count: usize,
    pub density: f64,   // directed: E / (V*(V-1)), 0.0 for V < 2
    pub avg_degree: f64, // directed_edge_count / symbol_count, 0.0 for V=0
    pub max_in_degree: usize,
    pub max_out_degree: usize,
}
```

- `density` uses directed edge formula: `E / (V * (V-1))` (0 for V < 2).
- `avg_degree` = `directed_edge_count / symbol_count` (0 for V = 0).
- `max_in_degree` / `max_out_degree` — maximum in- / out-degree seen.

### MCP tool — `mycelium_get_graph_metrics`

Request:
```json
{ "edge_kind": "calls" }
```

Response:
```json
{
  "symbol_count": 100,
  "directed_edge_count": 150,
  "density": 0.0152,
  "avg_degree": 1.5,
  "max_in_degree": 12,
  "max_out_degree": 8
}
```

- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [x] `graph_metrics` returns correct `symbol_count` and `directed_edge_count`.
- [x] `density` is 0 for V < 2 and 1 for a complete graph.
- [x] `avg_degree` is 0 for empty graph.
- [x] `max_in_degree` / `max_out_degree` correctly identify the highest-degree node.
- [x] File nodes excluded.
- [x] `mycelium_get_graph_metrics`: valid edge_kind returns the metrics struct.
- [x] `mycelium_get_graph_metrics`: unknown edge_kind returns `{ error }`.
- [x] All prior tests pass.
