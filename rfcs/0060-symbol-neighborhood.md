# RFC-0060 — `Store::symbol_neighborhood` + `mycelium_get_symbol_neighborhood` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0060                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0039 (cross_refs), RFC-0041 (outgoing_refs), RFC-0059 (two_hop_neighbors) |

## Summary

Add `Store::symbol_neighborhood(id, kind)` — the ego-graph of a symbol:
the symbol itself plus all direct incoming and outgoing neighbours for
a given `EdgeKind` — and expose it as `mycelium_get_symbol_neighborhood`.

Complements `cross_refs` (RFC-0039, all kinds, incoming only) and
`outgoing_refs` (RFC-0041, all kinds, outgoing only). `symbol_neighborhood`
focuses on one `EdgeKind` but gives a complete bidirectional view in a
single call, making it ideal for local impact analysis.

## Design

### `SymbolNeighborhood` struct

```rust
pub struct SymbolNeighborhood {
    pub path: String,
    pub incoming: Vec<String>,  // callers / importers / extenders / implementors
    pub outgoing: Vec<String>,  // callees / imports / parents / interfaces
}
```

Both `incoming` and `outgoing` sorted ascending.

### Store method

```rust
pub fn symbol_neighborhood(&self, id: NodeId, kind: EdgeKind) -> SymbolNeighborhood
```

Returns `SymbolNeighborhood` for `id`:
- `path`: the symbol's own path (empty string if not found)
- `incoming`: all direct incoming neighbours for `kind`, resolved to paths,
  sorted ascending, file nodes included (no filter)
- `outgoing`: all direct outgoing neighbours for `kind`, resolved to paths,
  sorted ascending, file nodes included (no filter)

Returns `SymbolNeighborhood { path: "".into(), incoming: vec![], outgoing: vec![] }`
for unknown `id`.

### MCP tool — `mycelium_get_symbol_neighborhood`

Request:
```json
{ "path": "src/service.rs>Service", "edge_kind": "calls" }
```

`edge_kind` must be `"calls"`, `"imports"`, `"extends"`, or `"implements"`.

Response:
```json
{
  "path": "src/service.rs>Service",
  "incoming": ["src/main.rs>main"],
  "outgoing": ["src/util.rs>format", "src/util.rs>parse"],
  "incoming_count": 1,
  "outgoing_count": 2
}
```

Unknown path returns `{ "path": "", "incoming": [], "outgoing": [], "incoming_count": 0, "outgoing_count": 0 }`.
Unknown `edge_kind` returns `{ "error": "unknown edge_kind: ..." }`.

## Acceptance Criteria

- [x] `SymbolNeighborhood` struct exported from `mycelium_core::store`.
- [x] `Store::symbol_neighborhood(id, kind)` returns correct ego-graph.
- [x] `incoming` sorted ascending; `outgoing` sorted ascending.
- [x] Returns empty neighborhood for unknown `id`.
- [x] `mycelium_get_symbol_neighborhood`: unknown path returns empty neighborhood JSON.
- [x] `mycelium_get_symbol_neighborhood`: unknown edge_kind returns `{ error }`.
- [x] `mycelium_get_symbol_neighborhood`: valid request returns `{ path, incoming, outgoing, incoming_count, outgoing_count }`.
- [x] All prior tests pass.
