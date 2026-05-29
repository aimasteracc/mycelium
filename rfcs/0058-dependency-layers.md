# RFC-0058 — `Store::dependency_layers` + `mycelium_get_dependency_layers` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0058                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0049 (entry_points — zero in-degree), RFC-0057 (scc_groups — cycle detection) |

## Summary

Add `Store::dependency_layers(kind)` — Kahn's BFS topological layering of symbol
nodes for a given `EdgeKind` — and expose it as `mycelium_get_dependency_layers`.

Layer 0 contains leaf/utility symbols (no outgoing edges for `kind`).
Layer k+1 contains symbols whose all direct dependencies (outgoing neighbours)
are in layers 0..=k. Symbols in cycles are excluded from the result
(they have no valid topological position).

Complements `entry_points` (RFC-0049) which returns zero-in-degree symbols,
and `scc_groups` (RFC-0057) which identifies cycle members. Together they give a
complete picture of graph topology: utilities at layer 0, orchestrators at the
top layer, and mutual-recursion groups via `scc_groups`.

## Design

### Store method

```rust
pub fn dependency_layers(&self, kind: EdgeKind) -> Vec<Vec<String>>
```

Kahn's BFS layering over symbol nodes only (paths containing `>`).
Returns layers sorted from lowest to highest (layer 0 first).
Paths within each layer sorted ascending.
Empty output if no symbol nodes exist.
Symbols in cycles (out-degree never reaches 0) are silently excluded.

### MCP tool — `mycelium_get_dependency_layers`

Request:
```json
{ "edge_kind": "calls" }
```

`edge_kind` must be `"calls"`, `"imports"`, `"extends"`, or `"implements"`.

Response:
```json
{
  "layers": [
    ["src/util.rs>helper", "src/util.rs>parse"],
    ["src/service.rs>process"],
    ["src/main.rs>main"]
  ],
  "layer_count": 3,
  "total_symbols": 4,
  "cycle_excluded_count": 0
}
```

`total_symbols` = sum of lengths of all layers.
`cycle_excluded_count` = all symbol nodes − `total_symbols`.
Unknown `edge_kind` returns `{ "error": "unknown edge_kind: ..." }`.

## Acceptance Criteria

- [x] `Store::dependency_layers(kind)` returns symbols in Kahn BFS layers.
- [x] Layer 0 contains only symbols with zero outgoing edges (for `kind`) within the symbol subgraph.
- [x] Layer k+1 contains only symbols all of whose outgoing symbol-subgraph neighbours are in layers 0..=k.
- [x] File nodes excluded (paths not containing `>`).
- [x] Symbols in cycles excluded from all layers.
- [x] Paths within each layer sorted ascending.
- [x] `mycelium_get_dependency_layers`: valid edge_kind returns `{ layers, layer_count, total_symbols, cycle_excluded_count }`.
- [x] `mycelium_get_dependency_layers`: unknown edge_kind returns `{ error }`.
- [x] `cycle_excluded_count` correctly reports symbols not assigned to any layer.
- [x] All prior tests pass.
