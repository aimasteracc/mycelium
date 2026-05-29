# RFC-0010 — Accurate Node and Edge Counts in server_status

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0010                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0007 (server_status)           |

## Summary

`mycelium_server_status` currently returns `node_count` but no `edge_count`.
The existing `Store::node_count()` is accurate. This RFC adds `Store::edge_count()`
and surfaces both values in `mycelium_server_status`.

## Motivation

Clients (AI agents, dashboards) need to know the density of the graph —
node count alone is insufficient. Edge count is the primary indicator of
how well the call-graph and containment edges are populated.

## Design

### `Synapse::edge_count()`

`AdjacencyList::edge_count()` already exists (sums forward adjacency list lengths).
Add a corresponding method on `Synapse` that sums over all `EdgeKind` buckets:

```rust
pub fn edge_count(&self) -> usize {
    self.by_kind.values().map(AdjacencyList::edge_count).sum()
}
```

### `Store::edge_count()`

Delegate to `self.synapse.edge_count()`.

### `mycelium_server_status` response

Add `"edge_count": <usize>` to the JSON response alongside the existing
`"node_count"`, `"indexed_root"`, and `"is_loaded"` fields.

## Acceptance Criteria

- [x] `Synapse::edge_count()` returns the sum of all directed edges across all kinds.
- [x] `Store::edge_count()` delegates to `Synapse::edge_count()`.
- [x] `mycelium_server_status` JSON includes `"edge_count"` key.
- [x] Unit tests cover the new methods.
- [x] All existing tests pass.
