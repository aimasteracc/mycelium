# RFC-0019 — `mycelium_rank_symbols` MCP Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0019                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0011 (Call graph), RFC-0012 (call MCP tools) |

## Summary

Add `mycelium_rank_symbols` — an MCP tool that returns the top-N symbols
ranked by incoming `Calls` edge count (caller count).  Helps agents
identify architectural hot spots: widely-called utilities, high-coupling
functions, and potential refactoring targets.

## Motivation

After indexing a workspace, agents need a quick way to find which
functions are called most widely.  Currently they must call
`mycelium_get_callers` for each symbol individually and compare manually.
`mycelium_rank_symbols` does this in one pass over all nodes, making
hot-spot analysis a single call.

## Design

### Store layer

`Store::top_callee_symbols(limit: usize) -> Vec<(String, usize)>`

Returns up to `limit` `(path, caller_count)` pairs, sorted by
`caller_count` descending (ties broken by path ascending).  Only paths
with at least one caller are included.  Implemented by iterating all
materialized paths, counting incoming `Calls` edges, and sorting.

### Request

```json
{
  "limit": 10
}
```

`limit` is optional (default 10, capped at 100).

### Response

```json
{
  "symbols": [
    { "path": "src/db.rs>query",      "caller_count": 12 },
    { "path": "src/utils.rs>log",     "caller_count": 8  },
    { "path": "src/auth.rs>validate", "caller_count": 5  }
  ]
}
```

Always at most `limit` entries, sorted by `caller_count` descending.
Empty array when no Calls edges exist.

### Implementation

1. Add `Store::top_callee_symbols(limit)` in `store/mod.rs`.
2. Add `RankSymbolsRequest { limit: Option<usize> }`.
3. Add `mycelium_rank_symbols` to `#[tool_router]`.

## Acceptance Criteria

- [ ] Returns symbols sorted by caller count descending.
- [ ] Ties broken by path ascending.
- [ ] `limit` defaults to 10, capped at 100.
- [ ] Symbols with 0 callers are excluded.
- [ ] Empty store returns empty list.
- [ ] All prior tests pass.
