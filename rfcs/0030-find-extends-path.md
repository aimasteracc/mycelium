# RFC-0030 — `Store::find_extends_path` + `mycelium_find_extends_path` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0030                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0017 (find_call_path), RFC-0027 (find_import_path), RFC-0026 (get_extends) |

## Summary

Add BFS shortest-path search over `EdgeKind::Extends` edges and expose it
as a new MCP tool `mycelium_find_extends_path`.  Completes the
`find_*_path` triad alongside `find_call_path` (RFC-0017) and
`find_import_path` (RFC-0027).

## Motivation

Agents that want to trace a class hierarchy ("how does `FileReader`
ultimately descend from `EventEmitter`?") currently have no way to find
the shortest inheritance chain without manually walking `get_extends` edges.
`find_extends_path` gives the same one-call BFS convenience as the two
existing path finders, applied to the inheritance graph.

## Design

### Store method

```rust
pub fn find_extends_path(&self, from: NodeId, to: NodeId, max_depth: usize) -> Option<Vec<NodeId>>
```

BFS over outgoing `EdgeKind::Extends` edges.  Mirrors `find_import_path`
exactly: same `VecDeque`/`HashSet` structure, same `max_depth` guard,
same `Some(vec![from])` self-path shortcut.

### MCP request struct

```rust
pub struct FindExtendsPathRequest {
    pub from_path: String,
    pub to_path: String,
    pub max_depth: Option<usize>,
}
```

### MCP tool — `mycelium_find_extends_path`

Request: `{ "from_path": "src/io.ts>ReadStream", "to_path": "src/base.ts>EventEmitter" }`

Response (found):
```json
{ "path": ["src/io.ts>ReadStream", "src/io.ts>Stream", "src/base.ts>EventEmitter"], "hops": 2 }
```

Response (unreachable):
```json
{ "path": [], "hops": null, "message": "no extends path found" }
```

Response (unknown path):
```json
{ "error": "path not found: ..." }
```

`max_depth` defaults to 8, capped at 20.

## Acceptance Criteria

- [x] `Store::find_extends_path` returns `Some(vec![id])` when `from == to`.
- [x] BFS finds direct single-hop extends chain.
- [x] BFS finds transitive multi-hop extends chain.
- [x] Returns `None` when unreachable.
- [x] `max_depth` limits hops correctly.
- [x] `mycelium_find_extends_path`: direct extends path returns `{ path, hops: 1 }`.
- [x] `mycelium_find_extends_path`: transitive path returns correct `hops`.
- [x] `mycelium_find_extends_path`: unreachable returns `{ path: [], hops: null, message }`.
- [x] `mycelium_find_extends_path`: unknown `from_path` returns `{ error }`.
- [x] All prior tests pass.
