# RFC-0032 — `Store::subclasses_tree` + `mycelium_get_subclasses_tree` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0032                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0021 (caller_tree), RFC-0031 (extends_tree) |

## Summary

Add a depth-limited DFS tree over **incoming** `EdgeKind::Extends` edges and
expose it as `mycelium_get_subclasses_tree`.  Mirrors the `caller_tree` /
`extends_tree` pattern — where `extends_tree` walks up to ancestors,
`subclasses_tree` walks down to descendants.

## Motivation

`mycelium_get_extends` returns direct neighbors in both directions.
`mycelium_get_extends_tree` (RFC-0031) walks *outgoing* edges (a class → its
parents).  Neither answers "show me every class that inherits from Base,
recursively."  An agent doing impact analysis on a base class needs a single
call that returns the full subclass forest.

## Design

### Store types

```rust
pub struct SubclassNode {
    pub id: NodeId,
    pub subclasses: Vec<Self>,
}
```

DFS over **incoming** `EdgeKind::Extends` edges (each node is extended by its
subclasses).  Cycle-safe via the same path-tracking visited set with backtrack
removal used by `extends_tree` and `caller_tree`.

### Store method

```rust
pub fn subclasses_tree(&self, id: NodeId, max_depth: usize) -> SubclassNode
```

### MCP tool — `mycelium_get_subclasses_tree`

Request: `{ "path": "src/base.ts>Base", "max_depth": 4 }`

Response:
```json
{
  "root": {
    "path": "src/base.ts>Base",
    "subclasses": [
      {
        "path": "src/mid.ts>Mid",
        "subclasses": [
          { "path": "src/child.ts>Child", "subclasses": [] }
        ]
      }
    ]
  }
}
```

Unknown path returns `{ "error": "path not found: ..." }`.
`max_depth` defaults to 4, capped at 10.

## Acceptance Criteria

- [ ] `SubclassNode` struct with `id` and `subclasses` fields defined in core.
- [ ] `Store::subclasses_tree` returns a leaf node for `max_depth = 0`.
- [ ] DFS follows **incoming** Extends edges.
- [ ] Cycles produce leaf nodes (not infinite recursion).
- [ ] `mycelium_get_subclasses_tree`: known path returns `{ root: { path, subclasses } }`.
- [ ] `mycelium_get_subclasses_tree`: unknown path returns `{ error }`.
- [ ] All prior tests pass.
