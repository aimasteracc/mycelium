# RFC-0024 — `mycelium_get_import_tree` MCP Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0024                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0023 (get_imports), RFC-0020 (callee_tree) |

## Summary

Add `mycelium_get_import_tree` — a depth-limited transitive import
dependency tree walking outgoing `Imports` edges.  Completes the
transitive-tree triad: `callee_tree` (RFC-0020), `caller_tree`
(RFC-0021), and now `import_tree` for module-level dependencies.

## Motivation

`mycelium_get_imports` returns only direct imports.  Agents doing
full dependency analysis (e.g. identifying circular imports or the full
transitive closure of a module's dependencies) must iterate
`get_imports` recursively, accumulating state.  `mycelium_get_import_tree`
does this in one call, up to `max_depth` hops.

## Design

### Tree node

```json
{
  "path": "src/auth.rs",
  "imports": [
    {
      "path": "hashlib",
      "imports": []
    }
  ]
}
```

Cycles are broken with the same DFS path-tracking mechanism: visited set
with backtrack removal.

### Request

```json
{
  "path": "src/auth.rs",
  "max_depth": 3
}
```

`max_depth` defaults to 4, capped at 10.

### Response (found)

```json
{ "root": { "path": "...", "imports": [...] } }
```

### Response (unknown path)

```json
{ "error": "path not found: src/auth.rs" }
```

### Implementation

```rust
pub struct ImportNode {
    pub id: NodeId,
    pub imports: Vec<Self>,
}
```

`Store::import_tree(id: NodeId, max_depth: usize) -> ImportNode`

Mirrors `callee_tree_inner` but uses `synapse.outgoing(id, EdgeKind::Imports)`.

## Acceptance Criteria

- [x] Direct imports appear as level-1 imports.
- [x] Transitive imports appear at depth 2+.
- [x] `max_depth` limits traversal; nodes at the depth limit appear as leaves.
- [x] Cycles produce a leaf (not infinite recursion).
- [x] Unknown `path` returns `{ error }`.
- [x] All prior tests pass.
