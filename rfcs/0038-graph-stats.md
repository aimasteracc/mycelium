# RFC-0038 — `Store::graph_stats` + `mycelium_get_stats` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0038                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0007 (server_status), RFC-0010 (edge_count), RFC-0028 (NodeKind) |

## Summary

Add `Store::graph_stats()` — returns comprehensive per-kind node and edge
counts — and expose it as the `mycelium_get_stats` MCP tool.  Extends the
existing `mycelium_server_status` (total counts) with a breakdown by
`NodeKind` and `EdgeKind`.

## Motivation

`mycelium_server_status` tells an agent how many nodes and edges exist in
total.  It cannot answer "how many functions vs. classes?" or "how many
Calls edges vs. Imports edges?"  Agents doing architectural analysis or
quality gates need the per-kind breakdown in one call.

## Design

### Store types

```rust
pub struct GraphStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub nodes_by_kind: BTreeMap<String, usize>,
    pub edges_by_kind: BTreeMap<String, usize>,
}
```

`BTreeMap<String, usize>` (not `NodeKind`/`EdgeKind` keys) so the struct
serialises cleanly to JSON without needing custom impls.  Keys are the
wire-format strings used elsewhere (e.g. `"Function"`, `"Calls"`).

### Store method

```rust
pub fn graph_stats(&self) -> GraphStats
```

Iterates all nodes to count by kind, iterates all `EdgeKind` variants to
count edges per kind.

### MCP tool — `mycelium_get_stats`

Request: `{}` (no parameters)

Response:
```json
{
  "total_nodes": 120,
  "total_edges": 85,
  "nodes_by_kind": {
    "Class": 4,
    "File": 12,
    "Function": 80,
    "Method": 24
  },
  "edges_by_kind": {
    "Calls": 60,
    "Contains": 20,
    "Imports": 5
  }
}
```

Kinds with zero count are omitted from the maps.

## Acceptance Criteria

- [ ] `GraphStats` struct with `total_nodes`, `total_edges`, `nodes_by_kind`, `edges_by_kind`.
- [ ] `Store::graph_stats()` counts nodes grouped by `NodeKind`.
- [ ] `Store::graph_stats()` counts edges grouped by `EdgeKind`.
- [ ] Kinds with zero count are omitted from the maps.
- [ ] `total_nodes` equals the sum of all `nodes_by_kind` values.
- [ ] `total_edges` equals the sum of all `edges_by_kind` values.
- [ ] `mycelium_get_stats`: returns the full stats object.
- [ ] All prior tests pass.
