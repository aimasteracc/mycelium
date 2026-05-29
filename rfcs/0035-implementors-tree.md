# RFC-0035 — `Store::implementors_tree` + `mycelium_get_implementors_tree` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0035                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0032 (subclasses_tree), RFC-0034 (implements_tree), RFC-0026 (get_implements) |

## Summary

Add a depth-limited DFS tree over **incoming** `EdgeKind::Implements` edges
and expose it as `mycelium_get_implementors_tree`.  Mirrors the
`subclasses_tree` pattern — where `implements_tree` walks to the interfaces a
class satisfies, `implementors_tree` walks to the classes that satisfy an
interface.

## Motivation

`mycelium_get_implements` returns direct `implemented_by` neighbors.
`mycelium_get_implements_tree` (RFC-0034) walks outgoing edges (class → interfaces).
Neither answers "show me every class that implements Interface X, recursively."
An agent doing impact analysis on an interface needs a single call that returns
the full implementor forest.

## Design

### Store types

```rust
pub struct ImplementorNode {
    pub id: NodeId,
    pub implementors: Vec<Self>,
}
```

DFS over **incoming** `EdgeKind::Implements` edges.  Cycle-safe via
path-tracking visited set with backtrack removal.

### Store method

```rust
pub fn implementors_tree(&self, id: NodeId, max_depth: usize) -> ImplementorNode
```

### MCP tool — `mycelium_get_implementors_tree`

Request: `{ "path": "src/iface.ts>IFace", "max_depth": 4 }`

Response:
```json
{
  "root": {
    "path": "src/iface.ts>IFace",
    "implementors": [
      { "path": "src/cls.ts>Cls", "implementors": [] }
    ]
  }
}
```

Unknown path returns `{ "error": "path not found: ..." }`.
`max_depth` defaults to 4, capped at 10.

## Acceptance Criteria

- [x] `ImplementorNode` struct with `id` and `implementors` fields defined in core.
- [x] `Store::implementors_tree` returns a leaf node for `max_depth = 0`.
- [x] DFS follows **incoming** Implements edges.
- [x] Cycles produce leaf nodes (not infinite recursion).
- [x] `mycelium_get_implementors_tree`: known path returns `{ root: { path, implementors } }`.
- [x] `mycelium_get_implementors_tree`: unknown path returns `{ error }`.
- [x] All prior tests pass.
