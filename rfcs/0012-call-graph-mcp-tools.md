# RFC-0012 — MCP Tools: mycelium_get_callers and mycelium_get_callees

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0012                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0011 (Calls edges), RFC-0004 (MCP server) |

## Summary

Expose the call graph populated by RFC-0011 through two new MCP tools:

- `mycelium_get_callers` — returns all symbols that call a given path
- `mycelium_get_callees` — returns all symbols that a given path calls

## Motivation

RFC-0011 writes `Calls` edges into the Synapse, but those edges are only
accessible through the Rust API. AI agents using the MCP server have no way
to query the call graph. These two tools close that gap.

## Design

### `mycelium_get_callees`

Input: `{ "path": "src/main.py>process" }`

1. Look up `path` in the store. Return error if not found.
2. Call `store.outgoing(id, EdgeKind::Calls)`.
3. Map each `NodeId` to its path string via `store.path_of(id)`.
4. Return sorted list of path strings.

Response:
```json
{ "callee_paths": ["src/main.py>validate", "src/utils.py>log"] }
```

### `mycelium_get_callers`

Input: `{ "path": "src/utils.py>log" }`

1. Look up `path` in the store.
2. Call `store.incoming(id, EdgeKind::Calls)`.
3. Map each `NodeId` to its path string.
4. Return sorted list.

Response:
```json
{ "caller_paths": ["src/main.py>process", "src/auth.py>login"] }
```

### Error response

If the path is not in the store, return:
```json
{ "error": "path not found: src/missing.py>fn" }
```

## Acceptance Criteria

- [ ] `mycelium_get_callees` returns outgoing Calls edges for a known path.
- [ ] `mycelium_get_callers` returns incoming Calls edges for a known path.
- [ ] Both tools return a sorted, deduplicated list of path strings.
- [ ] Both tools return an error JSON for unknown paths.
- [ ] MCP tool doc table updated to list 9 tools.
- [ ] All prior tests pass.
