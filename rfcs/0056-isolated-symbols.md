# RFC-0056 — `Store::isolated_symbols` + `mycelium_get_isolated_symbols` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0056                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0037 (dead_symbols — zero incoming Calls/Imports), RFC-0049 (leaf_symbols — zero outgoing for one kind) |

## Summary

Add `Store::isolated_symbols(prefix)` — symbol nodes that have zero
incoming **and** zero outgoing edges across **all four** `EdgeKind` variants
(Calls, Imports, Extends, Implements) — and expose it as
`mycelium_get_isolated_symbols`.

Complements `dead_symbols` (RFC-0037, zero incoming Calls+Imports) and
`leaf_symbols` (RFC-0049, zero outgoing for one kind). Isolated symbols
are completely disconnected from the rest of the graph — strong candidates
for deletion.

## Design

### Store method

```rust
pub fn isolated_symbols(&self, prefix: Option<&str>) -> Vec<String>
```

Iterates all symbol nodes (paths containing `>`). A node is isolated when
`node_degree(id)` has all counts equal to zero. Optional `prefix` filter
applied before the degree check. Results sorted lexicographically.

### MCP tool — `mycelium_get_isolated_symbols`

Request:
```json
{ "path_prefix": "src/" }
```

`path_prefix` is optional.

Response:
```json
{
  "isolated_symbols": ["src/orphan.rs>orphan"],
  "count": 1
}
```

## Acceptance Criteria

- [x] `Store::isolated_symbols(prefix)` returns symbol nodes with all-zero degree across all EdgeKinds.
- [x] File nodes excluded (paths not containing `>`).
- [x] Optional prefix filter applied.
- [x] Results sorted ascending by path.
- [x] `mycelium_get_isolated_symbols`: returns `{ isolated_symbols, count }`.
- [x] `mycelium_get_isolated_symbols`: optional `path_prefix` filters results.
- [x] All prior tests pass.
