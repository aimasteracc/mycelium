# RFC-0016 — `mycelium_get_symbol_info` Composite MCP Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0016                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0004 (MCP server), RFC-0012 (Call graph tools) |

## Summary

Add a `mycelium_get_symbol_info` MCP tool that returns all structural
information about a symbol in a single call: its ancestors, descendants,
callers, and callees.

## Motivation

AI coding agents frequently need the full context for a symbol before
suggesting edits. The current tool set requires 4 sequential calls:
`mycelium_get_ancestors`, `mycelium_get_descendants`, `mycelium_get_callers`,
`mycelium_get_callees`. Each call has MCP round-trip overhead and complicates
the agent's prompt.

A single composite endpoint reduces latency and simplifies agent logic.

## Design

### Request

```json
{ "path": "src/auth.rs>AuthService>login" }
```

### Response (success)

```json
{
  "path": "src/auth.rs>AuthService>login",
  "ancestors": ["src/auth.rs>AuthService", "src/auth.rs"],
  "descendants": [],
  "callers": ["src/main.rs>main"],
  "callees": ["src/db.rs>query", "src/token.rs>sign"]
}
```

### Response (not found)

```json
{ "error": "path not found: src/auth.rs>AuthService>login" }
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `path` | string | The exact queried path |
| `ancestors` | string[] | Containing scope chain (child-to-root) |
| `descendants` | string[] | All nested symbols (sorted) |
| `callers` | string[] | All symbols calling this path (sorted) |
| `callees` | string[] | All symbols this path calls (sorted) |

Sorted lists allow stable diffing between index runs.

## Acceptance Criteria

- [x] `mycelium_get_symbol_info` returns all 4 relationship types in one call.
- [x] Returns `{"error": "..."}` for an unknown path.
- [x] All lists are sorted lexicographically.
- [x] All prior tests pass.
