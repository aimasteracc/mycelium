# RFC-0021 — `mycelium_get_caller_tree` MCP Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0021                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0011 (Call graph), RFC-0020 (callee tree) |

## Summary

Add `mycelium_get_caller_tree` — the symmetric complement to
`mycelium_get_callee_tree` (RFC-0020).  Returns a depth-limited tree of
all transitive callers that can reach a given symbol.  Each node records
its path and its own direct callers, producing a nested JSON structure.

## Motivation

`mycelium_get_callee_tree` answers "what does this function ultimately
call?".  Agents also need the reverse: "what code paths could invoke this
function?".  `mycelium_get_caller_tree` does this in one call, walking
incoming `Calls` edges up to `max_depth` hops.

Typical use case: an agent wants to understand the full blast radius
before touching a low-level utility — it calls `mycelium_get_caller_tree`
to get all entry points that transitively invoke that utility.

## Design

### Tree node

```json
{
  "path": "src/db.rs>query",
  "callers": [
    {
      "path": "src/auth.rs>AuthService>login",
      "callers": []
    }
  ]
}
```

Cycles are broken with the same per-traversal path-tracking set used by
RFC-0020: a node already present in the current DFS path is returned as a
leaf.  `visited.remove(&id)` on backtrack so sibling branches can still
expand the same node.

### Request

```json
{
  "path": "src/db.rs>query",
  "max_depth": 3
}
```

`max_depth` is optional (default 4, capped at 10) — matches RFC-0020.

### Response (found)

```json
{
  "root": { "path": "...", "callers": [...] }
}
```

### Response (unknown path)

```json
{ "error": "path not found: src/db.rs>query" }
```

### Implementation

`Store::caller_tree(id: NodeId, max_depth: usize) -> CallerNode`

Mirrors `callee_tree_inner` but uses `synapse.incoming(id, EdgeKind::Calls)`
instead of `outgoing`.

```rust
pub struct CallerNode {
    pub id: NodeId,
    pub callers: Vec<Self>,
}
```

The MCP tool converts `CallerNode` to JSON by resolving `id` → path via
`Store::path_of`.

## Acceptance Criteria

- [x] Direct callers appear as level-1 callers.
- [x] Transitive callers appear at depth 2+.
- [x] `max_depth` limits traversal; nodes at the depth limit appear as leaves.
- [x] Cycles produce a leaf (not infinite recursion).
- [x] Unknown `path` returns `{ error }`.
- [x] All prior tests pass.
