# RFC-0017 — `mycelium_find_call_path` MCP Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0017                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0011 (Call graph), RFC-0016 (Symbol info) |

## Summary

Add `mycelium_find_call_path` — a BFS-based MCP tool that finds the
shortest call chain from a `from` symbol to a `to` symbol through
`EdgeKind::Calls` edges.

## Motivation

AI coding agents regularly need to answer "how does request handling reach
the database?" or "what's the call chain from `main` to this crash site?".
The existing tools expose direct callers/callees but require the agent to
manually traverse the graph — BFS in the prompt is expensive and error-prone.

## Design

### Algorithm

BFS from `from_path`, following outgoing `Calls` edges, until `to_path` is
reached or all reachable nodes are exhausted.

- Returns the first (shortest) path found.
- Maximum search depth: `max_depth` (default 10, capped at 20).
- Cycle-safe: visited set prevents revisiting nodes.

### Request

```json
{
  "from_path": "src/main.rs>handle_request",
  "to_path": "src/db.rs>query",
  "max_depth": 10
}
```

`max_depth` is optional (default 10).

### Response (found)

```json
{
  "path": [
    "src/main.rs>handle_request",
    "src/auth.rs>AuthService>verify",
    "src/db.rs>query"
  ],
  "hops": 2
}
```

### Response (not found)

```json
{
  "path": [],
  "hops": null,
  "message": "no call path found within depth 10"
}
```

### Response (unknown path)

```json
{ "error": "path not found: src/main.rs>handle_request" }
```

### Implementation

`Store::find_call_path(from: NodeId, to: NodeId, max_depth: usize) -> Option<Vec<NodeId>>`

BFS using a `VecDeque`. Returns `Some(vec)` with the path including both
endpoints, or `None` if unreachable within `max_depth`.

## Acceptance Criteria

- [x] Direct call `A → B`: path is `[A, B]`, hops = 1.
- [x] Transitive call `A → B → C`: path is `[A, B, C]`, hops = 2.
- [x] No path: returns `path: []`, `hops: null`.
- [x] Unknown `from_path` or `to_path`: returns `error`.
- [x] `max_depth` limits traversal depth.
- [x] Cycles do not cause infinite loops.
- [x] All prior tests pass.
