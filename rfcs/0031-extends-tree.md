# RFC-0031 — `Store::extends_tree` + `mycelium_get_extends_tree` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0031                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0020 (callee_tree), RFC-0024 (import_tree), RFC-0026 (get_extends) |

## Summary

Add a depth-limited DFS tree over outgoing `EdgeKind::Extends` edges and
expose it as `mycelium_get_extends_tree`.  Mirrors the `callee_tree` /
`import_tree` pattern for class hierarchy traversal.

## Motivation

`mycelium_get_extends` returns only direct neighbors.
`mycelium_find_extends_path` (RFC-0030) finds a path between two known
endpoints.  Neither answers "show me the full superclass chain starting
from this class."  An agent exploring a deep class hierarchy needs a
single call that returns the entire tree up to a configurable depth.

## Design

### Store types

```rust
pub struct ExtendsNode {
    pub id: NodeId,
    pub parents: Vec<Self>,
}
```

DFS over outgoing `EdgeKind::Extends` edges (each node extends its
parents).  Cycle-safe via the same path-tracking visited set with
backtrack removal used by `callee_tree` and `import_tree`.

### Store method

```rust
pub fn extends_tree(&self, id: NodeId, max_depth: usize) -> ExtendsNode
```

### MCP tool — `mycelium_get_extends_tree`

Request: `{ "path": "src/child.ts>Child", "max_depth": 4 }`

Response:
```json
{
  "root": {
    "path": "src/child.ts>Child",
    "parents": [
      {
        "path": "src/mid.ts>Mid",
        "parents": [
          { "path": "src/base.ts>Base", "parents": [] }
        ]
      }
    ]
  }
}
```

Unknown path returns `{ "error": "path not found: ..." }`.
`max_depth` defaults to 4, capped at 10.

## Acceptance Criteria

- [x] `ExtendsNode` struct with `id` and `parents` fields defined in core.
- [x] `Store::extends_tree` returns a leaf node for `max_depth = 0`.
- [x] DFS follows outgoing Extends edges.
- [x] Cycles produce leaf nodes (not infinite recursion).
- [x] `mycelium_get_extends_tree`: known path returns `{ root: { path, parents } }`.
- [x] `mycelium_get_extends_tree`: unknown path returns `{ error }`.
- [x] All prior tests pass.
