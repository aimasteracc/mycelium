# RFC-0073 — `Store::degree_histogram` + `mycelium_get_degree_histogram` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0073                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0046 (node_degree), RFC-0053/54 (fan_out/fan_in_rank) |

## Summary

Add `Store::degree_histogram(kind)` — returns the in-degree and out-degree
frequency distributions (histograms) of all symbol nodes for a given
`EdgeKind` — and expose it as `mycelium_get_degree_histogram`.

The degree histogram reveals the **structural shape** of the dependency graph:
- Power-law distributions (many nodes with degree 0-1, a few "hubs" with high
  degree) indicate hub-spoke architecture.
- Uniform distributions suggest balanced, modular design.
- High in-degree concentration identifies hotspot symbols (already findable via
  `fan_in_rank`), while the histogram shows the full picture.

O(V) computation.  File nodes excluded.

## Design

### Store method

```rust
pub fn degree_histogram(&self, kind: EdgeKind) -> DegreeHistogram
```

```rust
#[derive(Debug, Clone, Default)]
pub struct DegreeHistogram {
    /// (in_degree, symbol_count) pairs sorted by degree ascending.
    pub in_degrees: Vec<(u64, u64)>,
    /// (out_degree, symbol_count) pairs sorted by degree ascending.
    pub out_degrees: Vec<(u64, u64)>,
}
```

- Iterates all symbol nodes (no file nodes) and buckets by in/out degree.
- Degree 0 is included when there are symbols with no edges.
- Sorted ascending by degree.

### MCP tool — `mycelium_get_degree_histogram`

Request:
```json
{ "edge_kind": "calls" }
```

Response:
```json
{
  "in_degrees":  [{"degree": 0, "count": 12}, {"degree": 1, "count": 5}],
  "out_degrees": [{"degree": 0, "count": 8},  {"degree": 1, "count": 9}],
  "total_symbols": 17
}
```

- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [x] `degree_histogram` returns sorted `in_degrees` and `out_degrees` vectors.
- [x] Degree 0 is included for symbols with no edges.
- [x] File nodes excluded.
- [x] Counts sum to the total symbol count.
- [x] `mycelium_get_degree_histogram`: valid edge_kind returns `{ in_degrees, out_degrees, total_symbols }`.
- [x] `mycelium_get_degree_histogram`: unknown edge_kind returns `{ error }`.
- [x] All prior tests pass.
