# RFC-0050 — `Store::shortest_path` + `mycelium_get_shortest_path` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0050                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0043 (reachable_from), RFC-0044 (reachable_to) |

## Summary

Add `Store::shortest_path(from, to, kind)` — BFS shortest path between two
symbol nodes along edges of a given `EdgeKind` — and expose it as
`mycelium_get_shortest_path`.

Answers "how does A reach B through calls/imports?" in a single call.
Complements `reachable_from` (all reachable nodes) with the specific path
to a target.

## Design

### Store method

```rust
pub fn shortest_path(&self, from: NodeId, to: NodeId, kind: EdgeKind) -> Option<Vec<String>>
```

BFS from `from` following outgoing `kind` edges.  Returns `Some(path)` where
`path` is the sequence of node paths from `from` to `to` (both endpoints
included), or `None` if no path exists.  If `from == to`, returns
`Some(vec![path_of(from)])`.  Cycles are safe (visited set).

### MCP tool — `mycelium_get_shortest_path`

Request:
```json
{ "from": "src/a.rs>main", "to": "src/b.rs>helper", "edge_kind": "calls" }
```

`edge_kind` must be `"calls"`, `"imports"`, `"extends"`, or `"implements"`.

Response (path found):
```json
{
  "path": ["src/a.rs>main", "src/b.rs>helper"],
  "length": 1
}
```

Response (no path):
```json
{ "path": null, "length": null }
```

Unknown `edge_kind` returns `{ "error": "unknown edge_kind: ..." }`.
Unknown `from`/`to` path returns `{ "error": "path not found: ..." }`.

## Acceptance Criteria

- [x] `Store::shortest_path(from, to, kind)` returns the shortest hop sequence via outgoing `kind` edges.
- [x] `from == to` returns `Some(vec![path_of(from)])` (zero-hop path).
- [x] No path returns `None`.
- [x] Cycle-safe (visited set prevents infinite loops).
- [x] `mycelium_get_shortest_path`: path found → `{ path, length }`.
- [x] `mycelium_get_shortest_path`: no path → `{ path: null, length: null }`.
- [x] `mycelium_get_shortest_path`: unknown edge_kind → `{ error }`.
- [x] `mycelium_get_shortest_path`: unknown from/to → `{ error }`.
- [x] All prior tests pass.
