# RFC-0027 — `mycelium_find_import_path` MCP Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0027                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0017 (find_call_path), RFC-0023 (get_imports), RFC-0024 (import_tree) |

## Summary

Add `mycelium_find_import_path` — a BFS shortest-path tool over `Imports`
edges that mirrors `mycelium_find_call_path` (RFC-0017) for import
dependency chains.  Completes the import-analysis triad alongside
`mycelium_get_imports` and `mycelium_get_import_tree`.

## Motivation

Agents investigating circular dependencies or the transitive dependency chain
between two files/modules can currently walk `get_import_tree` and
reconstruct the shortest path themselves.  `mycelium_find_import_path`
does this in one call and returns only the path, not the full tree.

## Design

### Request

```json
{ "from_path": "src/auth.rs", "to_path": "src/db.rs", "max_depth": 6 }
```

`max_depth` defaults to 8, capped at 20.

### Response (reachable)

```json
{ "path": ["src/auth.rs", "src/utils.rs", "src/db.rs"], "hops": 2 }
```

`hops` = number of edges traversed = `path.length - 1`.

### Response (unreachable)

```json
{ "path": [], "hops": null, "message": "no import path found within max_depth=8" }
```

### Response (unknown path)

```json
{ "error": "path not found: src/auth.rs" }
```

Either `from_path` or `to_path` being unknown triggers `{ error }`.

### Implementation

#### Store method

```rust
pub fn find_import_path(&self, from: NodeId, to: NodeId, max_depth: usize) -> Option<Vec<NodeId>>
```

BFS over `outgoing(id, EdgeKind::Imports)` edges.  Same algorithm as
`find_call_path` but with `EdgeKind::Imports`.

#### MCP request struct

```rust
pub struct FindImportPathRequest {
    pub from_path: String,
    pub to_path: String,
    pub max_depth: Option<usize>,
}
```

## Acceptance Criteria

- [x] Direct import (`from` imports `to`) returns path of length 2, hops 1.
- [x] Transitive path returns correct intermediate nodes in order.
- [x] Unreachable pair returns `{ path: [], hops: null, message }`.
- [x] `max_depth` limits BFS; path longer than `max_depth` hops returns unreachable.
- [x] Unknown `from_path` or `to_path` returns `{ error }`.
- [x] All prior tests pass.
