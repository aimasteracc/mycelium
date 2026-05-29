# RFC-0033 — `Store::find_implements_path` + `mycelium_find_implements_path` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0033                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0017 (find_call_path), RFC-0027 (find_import_path), RFC-0030 (find_extends_path), RFC-0026 (get_implements) |

## Summary

Add BFS shortest-path search over `EdgeKind::Implements` edges and expose
it as `mycelium_find_implements_path`.  Completes the `find_*_path` family
(calls / imports / extends / implements).

## Motivation

`mycelium_get_implements` returns only direct neighbors.
`mycelium_find_extends_path` (RFC-0030) finds a path over Extends edges.
Neither answers "is there any indirect implements chain from ClassA to
InterfaceB?"  An agent tracing interface conformance chains needs a single
call that returns the shortest path.

## Design

### Store method

```rust
pub fn find_implements_path(
    &self,
    from: NodeId,
    to: NodeId,
    max_depth: usize,
) -> Option<Vec<NodeId>>
```

BFS over **outgoing** `EdgeKind::Implements` edges.  Returns `Some(path)`
including both endpoints if `to` is reachable within `max_depth` hops, else
`None`.  Cycle-safe via standard visited set.

### MCP tool — `mycelium_find_implements_path`

Request: `{ "from_path": "src/foo.ts>Foo", "to_path": "src/iface.ts>IFace", "max_depth": 5 }`

Response (reachable):
```json
{ "path": ["src/foo.ts>Foo", "src/iface.ts>IFace"], "hops": 1 }
```

Response (unreachable):
```json
{ "path": [], "hops": null, "message": "no implements path found within max_depth=5" }
```

Unknown from/to path returns `{ "error": "path not found: ..." }`.
`max_depth` defaults to 8, capped at 20.

## Acceptance Criteria

- [x] `Store::find_implements_path(self_node, self_node, _)` returns `Some([self])`.
- [x] `Store::find_implements_path` finds a direct hop.
- [x] `Store::find_implements_path` finds a transitive path.
- [x] `Store::find_implements_path` returns `None` for unreachable targets.
- [x] `max_depth` limits the number of hops.
- [x] `mycelium_find_implements_path`: known path returns `{ path, hops }`.
- [x] `mycelium_find_implements_path`: unknown path returns `{ path: [], hops: null, message }`.
- [x] `mycelium_find_implements_path`: unknown from/to path returns `{ error }`.
- [x] All prior tests pass.
