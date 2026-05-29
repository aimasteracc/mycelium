# RFC-0036 — `Store::importers_tree` + `mycelium_get_importers_tree` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0036                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0024 (import_tree), RFC-0032 (subclasses_tree), RFC-0035 (implementors_tree) |

## Summary

Add a depth-limited DFS tree over **incoming** `EdgeKind::Imports` edges and
expose it as `mycelium_get_importers_tree`.  Mirrors the `subclasses_tree` /
`implementors_tree` pattern — where `import_tree` walks outgoing (what this
file imports), `importers_tree` walks incoming (who imports this file).

## Motivation

`mycelium_get_imports` returns direct `imported_by` neighbors.
`mycelium_get_import_tree` (RFC-0024) walks outgoing Imports edges.
Neither answers "show me every module that (transitively) depends on
this module."  An agent doing impact analysis for a library change needs
a single call that returns the full reverse-dependency tree.

## Design

### Store types

```rust
pub struct ImporterNode {
    pub id: NodeId,
    pub importers: Vec<Self>,
}
```

DFS over **incoming** `EdgeKind::Imports` edges.  Cycle-safe via
path-tracking visited set with backtrack removal.

### Store method

```rust
pub fn importers_tree(&self, id: NodeId, max_depth: usize) -> ImporterNode
```

### MCP tool — `mycelium_get_importers_tree`

Request: `{ "path": "src/utils.ts>utils", "max_depth": 4 }`

Response:
```json
{
  "root": {
    "path": "src/utils.ts>utils",
    "importers": [
      {
        "path": "src/app.ts>app",
        "importers": []
      }
    ]
  }
}
```

Unknown path returns `{ "error": "path not found: ..." }`.
`max_depth` defaults to 4, capped at 10.

## Acceptance Criteria

- [ ] `ImporterNode` struct with `id` and `importers` fields defined in core.
- [ ] `Store::importers_tree` returns a leaf node for `max_depth = 0`.
- [ ] DFS follows **incoming** Imports edges.
- [ ] Cycles produce leaf nodes (not infinite recursion).
- [ ] `mycelium_get_importers_tree`: known path returns `{ root: { path, importers } }`.
- [ ] `mycelium_get_importers_tree`: unknown path returns `{ error }`.
- [ ] All prior tests pass.
