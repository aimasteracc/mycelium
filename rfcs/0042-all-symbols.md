# RFC-0042 — `Store::all_symbols` + `mycelium_get_all_symbols` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0042                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0028 (NodeKind), RFC-0018 (get_files) |

## Summary

Add `Store::all_symbols(prefix, kind)` — list all non-file symbol paths
with optional path-prefix and kind filters — and expose it as
`mycelium_get_all_symbols`.

`mycelium_get_symbols_by_kind` (RFC-0028) requires specifying a kind.
`mycelium_get_files` returns only file-level nodes.  Neither answers
"give me every symbol under `src/controllers/`" across all kinds.

## Design

### Store method

```rust
pub fn all_symbols(
    &self,
    prefix: Option<&str>,
    kind: Option<NodeKind>,
) -> Vec<String>
```

Returns all paths containing `>` (symbol nodes, not file nodes) that:
1. Start with `prefix` (if provided)
2. Have `NodeKind == kind` (if provided)

Results sorted lexicographically.

### MCP tool — `mycelium_get_all_symbols`

Request: `{ "path_prefix": "src/", "kind": "function" }` (both optional)

Response:
```json
{
  "symbols": ["src/a.rs>fn1", "src/b.rs>fn2"],
  "count": 2
}
```

Empty result: `{ "symbols": [], "count": 0 }`.

Unknown `kind` string returns `{ "error": "unknown kind: ..." }`.
No parameters returns all symbols.

## Acceptance Criteria

- [x] `Store::all_symbols(prefix, kind)` returns only symbol paths (containing `>`).
- [x] File-level nodes are excluded.
- [x] Optional `prefix` filter applies.
- [x] Optional `kind` filter applies (by `NodeKind` value).
- [x] Results sorted lexicographically.
- [x] `mycelium_get_all_symbols`: returns `{ symbols, count }`.
- [x] `mycelium_get_all_symbols`: unknown kind string returns `{ error }`.
- [x] All prior tests pass.
