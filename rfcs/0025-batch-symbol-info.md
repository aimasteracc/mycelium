# RFC-0025 — `mycelium_batch_symbol_info` MCP Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0025                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0016 (get_symbol_info) |

## Summary

Add `mycelium_batch_symbol_info` — a batch variant of `mycelium_get_symbol_info`
that accepts a list of paths and returns symbol info for all of them in
one MCP call.  Eliminates N round-trips for agents analyzing several
related symbols at once.

## Motivation

`mycelium_get_symbol_info` is the most useful single-symbol query, but
agents routinely need to analyze clusters of related symbols.  Calling
it N times for N symbols is N round-trips.  A batch variant amortises
this to one call with the same server-side work.

## Design

### Request

```json
{ "paths": ["src/auth.rs>login", "src/db.rs>query"] }
```

Maximum 50 paths per call (configurable cap).

### Response

```json
{
  "symbols": [
    {
      "path": "src/auth.rs>login",
      "ancestors":   ["src/auth.rs"],
      "descendants": [],
      "callers":     ["src/main.rs>main"],
      "callees":     ["src/db.rs>query"]
    },
    {
      "path": "no/such>path",
      "error": "path not found"
    }
  ]
}
```

Results appear in the same order as the input `paths`.  Unknown paths
produce `{ path, error }` rather than failing the whole request.

### Implementation

No new Store methods needed.  The MCP tool iterates `req.paths` and for
each calls the existing `Store::ancestors_of_path`, `descendants_of_path`,
`incoming(id, Calls)`, `outgoing(id, Calls)` methods.

### Limits

- Maximum 50 paths per request (excess paths silently truncated).

## Acceptance Criteria

- [x] Each found path returns `{ path, ancestors, descendants, callers, callees }`.
- [x] Each not-found path returns `{ path, error }`.
- [x] Results in same order as input.
- [x] More than 50 paths → only first 50 processed.
- [x] All prior tests pass.
