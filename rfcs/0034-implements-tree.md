# RFC-0034 — `Store::implements_tree` + `mycelium_get_implements_tree` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0034                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0024 (import_tree), RFC-0031 (extends_tree), RFC-0033 (find_implements_path), RFC-0026 (get_implements) |

## Summary

Add a depth-limited DFS tree over **outgoing** `EdgeKind::Implements` edges
and expose it as `mycelium_get_implements_tree`.  Mirrors the `extends_tree`
pattern for the implements relationship — answers "show me all interfaces
(recursively) that this class implements."

## Motivation

`mycelium_get_implements` returns only direct neighbors.
`mycelium_find_implements_path` (RFC-0033) finds a path between two known
endpoints.  Neither answers "show me the full interface hierarchy this class
satisfies, recursively."

## Design

### Store types

```rust
pub struct ImplementsNode {
    pub id: NodeId,
    pub interfaces: Vec<Self>,
}
```

DFS over **outgoing** `EdgeKind::Implements` edges.  Cycle-safe via
path-tracking visited set with backtrack removal.

### Store method

```rust
pub fn implements_tree(&self, id: NodeId, max_depth: usize) -> ImplementsNode
```

### MCP tool — `mycelium_get_implements_tree`

Request: `{ "path": "src/cls.ts>Cls", "max_depth": 4 }`

Response:
```json
{
  "root": {
    "path": "src/cls.ts>Cls",
    "interfaces": [
      {
        "path": "src/iface.ts>IFace",
        "interfaces": [
          { "path": "src/base.ts>BaseIFace", "interfaces": [] }
        ]
      }
    ]
  }
}
```

Unknown path returns `{ "error": "path not found: ..." }`.
`max_depth` defaults to 4, capped at 10.

## Acceptance Criteria

- [ ] `ImplementsNode` struct with `id` and `interfaces` fields defined in core.
- [ ] `Store::implements_tree` returns a leaf node for `max_depth = 0`.
- [ ] DFS follows outgoing Implements edges.
- [ ] Cycles produce leaf nodes (not infinite recursion).
- [ ] `mycelium_get_implements_tree`: known path returns `{ root: { path, interfaces } }`.
- [ ] `mycelium_get_implements_tree`: unknown path returns `{ error }`.
- [ ] All prior tests pass.
