# RFC-0020 — `mycelium_get_callee_tree` MCP Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0020                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0011 (Call graph), RFC-0017 (find_call_path) |

## Summary

Add `mycelium_get_callee_tree` — an MCP tool that returns a depth-limited
tree of all transitive callees reachable from a given symbol.  Each node
in the tree records its path and its own direct children, producing a
nested JSON structure that agents can read without further BFS traversal.

## Motivation

`mycelium_get_callees` returns only immediate callees.  Agents that need
to understand "what does this function ultimately call?" must iterate
`mycelium_get_callees` repeatedly, accumulating state across many calls.
`mycelium_get_callee_tree` does this in one call, up to `max_depth` hops,
and returns a clean nested tree.

## Design

### Tree node

```json
{
  "path": "src/auth.rs>AuthService>login",
  "children": [
    {
      "path": "src/db.rs>query",
      "children": []
    }
  ]
}
```

Cycles are broken via a per-traversal visited set: a node that has
already appeared in the current tree is represented with `"children": []`
(leaf, not expanded further).

### Request

```json
{
  "path": "src/main.rs>main",
  "max_depth": 3
}
```

`max_depth` is optional (default 4, capped at 10).

### Response (found)

```json
{
  "root": { "path": "...", "children": [...] }
}
```

### Response (unknown path)

```json
{ "error": "path not found: src/main.rs>main" }
```

### Implementation

`Store::callee_tree(id: NodeId, max_depth: usize) -> CalleeNode`

Recursive DFS.  Cycle guard: pass a `&mut HashSet<NodeId>` of already-visited
IDs through the recursion; if a node is already visited, return it as a
leaf.  `CalleeNode` is a plain struct (not serialized by the store layer;
serialization happens in the MCP tool).

```rust
pub struct CalleeNode {
    pub id: NodeId,
    pub children: Vec<CalleeNode>,
}
```

The MCP tool converts `CalleeNode` to JSON by resolving `id` → path via
`Store::path_of`.

## Acceptance Criteria

- [x] Direct callees appear as level-1 children.
- [x] Transitive callees appear at depth 2+.
- [x] `max_depth` limits traversal; nodes at the depth limit appear as leaves.
- [x] Cycles produce a leaf (not infinite recursion).
- [x] Unknown `path` returns `{ error }`.
- [x] All prior tests pass.
