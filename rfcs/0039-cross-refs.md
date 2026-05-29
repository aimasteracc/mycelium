# RFC-0039 — `Store::cross_refs` + `mycelium_get_cross_refs` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0039                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0012 (callers/callees), RFC-0023 (imports), RFC-0026 (extends/implements) |

## Summary

Add `Store::cross_refs(id)` — returns all incoming edges to a symbol,
grouped by edge kind — and expose it as `mycelium_get_cross_refs`.
This is the unified "who references this symbol?" primitive, answering
a common impact-analysis question in one call.

## Motivation

An agent preparing to modify a symbol must currently make 4+ calls:
`mycelium_get_callers`, `mycelium_get_imports`, `mycelium_get_extends`,
`mycelium_get_implements`.  A single `mycelium_get_cross_refs` reduces
this to one call and ensures no edge kind is forgotten.

## Design

### Store type

```rust
pub struct CrossRefs {
    pub callers: Vec<String>,
    pub importers: Vec<String>,
    pub extended_by: Vec<String>,
    pub implemented_by: Vec<String>,
}
```

All lists sorted lexicographically.  Empty lists included (not omitted).

### Store method

```rust
pub fn cross_refs(&self, id: NodeId) -> CrossRefs
```

Collects incoming edges for each of the four primary `EdgeKind` variants
and resolves them to path strings.

### MCP tool — `mycelium_get_cross_refs`

Request: `{ "path": "src/lib.rs>MyClass" }`

Response:
```json
{
  "callers": [],
  "importers": ["src/app.rs>app"],
  "extended_by": ["src/child.rs>Child"],
  "implemented_by": []
}
```

Unknown path returns `{ "error": "path not found: ..." }`.

## Acceptance Criteria

- [x] `CrossRefs` struct with `callers`, `importers`, `extended_by`, `implemented_by`.
- [x] All four lists sorted lexicographically.
- [x] Empty lists are present (not omitted) in the response.
- [x] `mycelium_get_cross_refs`: known path returns `{ callers, importers, extended_by, implemented_by }`.
- [x] `mycelium_get_cross_refs`: unknown path returns `{ error }`.
- [x] All prior tests pass.
