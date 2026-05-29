# RFC-0068 — `Store::weakly_connected_components` + `mycelium_get_wcc` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0068                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0067 (cycle_members / SCC), RFC-0064 (k_core) |

## Summary

Add `Store::weakly_connected_components(kind)` — groups symbol nodes into
weakly-connected components (WCCs) for a given `EdgeKind`, treating edges as
undirected — and expose it as `mycelium_get_wcc`.

An SCC (RFC-0067) requires a directed path in both directions.  A WCC only
requires *any* path (ignoring direction).  WCCs surface isolated clusters of
code: modules with no dependency links to the rest of the codebase, orphaned
utility files, or logical subsystems that are self-contained.

## Design

### Store method

```rust
pub fn weakly_connected_components(&self, kind: EdgeKind) -> Vec<Vec<String>>
```

- Union-Find (path-compressed) over symbol nodes, unioning endpoints for every
  `kind` edge regardless of direction.
- Returns one `Vec<String>` per component (paths, sorted ascending).
- Components sorted by size descending; ties broken by first element ascending.
- File nodes excluded.

### MCP tool — `mycelium_get_wcc`

Request:
```json
{ "edge_kind": "calls", "min_size": 2 }
```

- `edge_kind`: one of `calls`, `imports`, `extends`, `implements`.
- `min_size` (optional, default 1): only return components with at least this
  many symbols. Set to 2 to hide singleton isolated nodes.

Response:
```json
{
  "components": [
    ["src/a.rs>A", "src/b.rs>B", "src/c.rs>C"],
    ["src/x.rs>X", "src/y.rs>Y"]
  ],
  "component_count": 2,
  "total_symbols": 5
}
```

- `components`: list of symbol-path lists (each component sorted ascending,
  components sorted by size descending).
- `component_count`: number of components returned (after `min_size` filter).
- `total_symbols`: total symbols across all returned components.
- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [x] `Store::weakly_connected_components(kind)` returns one Vec<String> per
      component, symbols sorted ascending, components sorted by size desc.
- [x] Edges treated as undirected (both endpoints in same component regardless
      of direction).
- [x] File nodes excluded.
- [x] `mycelium_get_wcc`: valid edge_kind returns `{ components, component_count, total_symbols }`.
- [x] `min_size` filter applied before returning.
- [x] `mycelium_get_wcc`: unknown edge_kind returns `{ error }`.
- [x] All prior tests pass.
