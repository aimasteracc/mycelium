# RFC-0051 — `Store::symbol_count_by_kind` + `mycelium_get_symbol_count_by_kind` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0051                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0038 (graph_stats), RFC-0042 (all_symbols) |

## Summary

Add `Store::symbol_count_by_kind()` — a breakdown of the total number of indexed
symbols grouped by `NodeKind` — and expose it as `mycelium_get_symbol_count_by_kind`.

Complements `graph_stats` (which gives total counts) with a per-kind histogram.
Answers "what is this codebase made of?" — how many functions, classes, methods,
interfaces, etc.

## Design

### Store method

```rust
pub fn symbol_count_by_kind(&self) -> Vec<(String, usize)>
```

Iterates all nodes with a known `NodeKind` entry in `kind_map`, groups by kind,
and returns a list of `(kind_wire_string, count)` pairs sorted alphabetically by
kind name.  Kinds with zero count are excluded.

### MCP tool — `mycelium_get_symbol_count_by_kind`

Request: *(no parameters)*

Response:
```json
{
  "kinds": [
    { "kind": "class", "count": 14 },
    { "kind": "function", "count": 87 },
    { "kind": "method", "count": 203 }
  ],
  "total": 304
}
```

Empty graph returns `{ "kinds": [], "total": 0 }`.

## Acceptance Criteria

- [x] `Store::symbol_count_by_kind()` counts nodes per `NodeKind`; only nodes in `kind_map` counted.
- [x] Returns `(wire_string, count)` pairs sorted alphabetically by wire string.
- [x] Kinds with count 0 are excluded.
- [x] `mycelium_get_symbol_count_by_kind`: returns `{ kinds: [{ kind, count }], total }`.
- [x] Empty graph returns `{ kinds: [], total: 0 }`.
- [x] All prior tests pass.
