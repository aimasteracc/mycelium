# RFC-0022 — `mycelium_get_entry_points` MCP Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0022                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0011 (Call graph), RFC-0019 (rank_symbols) |

## Summary

Add `mycelium_get_entry_points` — an MCP tool that returns all indexed
symbols (non-file nodes) that have zero incoming `Calls` edges.  These
are either genuine entry points (e.g. `main`, test functions, public API
handlers called from outside the indexed codebase) or potentially dead
code.  Complements `mycelium_rank_symbols` which returns the most-called
end of the spectrum.

## Motivation

Agents doing dead-code analysis, architectural mapping, or test discovery
currently need to iterate `mycelium_rank_symbols` at full limit and filter
on the client side.  `mycelium_get_entry_points` does this server-side in
one call and returns only the zero-caller symbols.

## Design

### Request

```json
{
  "path_prefix": "src/handlers"
}
```

`path_prefix` is optional.  When present only symbols whose path starts
with the prefix are included.

### Response

```json
{
  "entry_points": [
    "src/main.rs>main",
    "src/handlers/auth.rs>login",
    "src/handlers/auth.rs>logout"
  ]
}
```

Results are sorted lexicographically.  File-level nodes (paths without
`>`) are always excluded — they have no callers by definition and
including them would create noise.

### Implementation

`Store::entry_points(prefix: Option<&str>) -> Vec<String>`

Iterates all paths that contain `>` (symbol nodes).  For each, checks
whether the incoming `Calls` edge list is empty.  Collects and sorts.
Optional prefix filter applied before the caller-count check.

## Acceptance Criteria

- [ ] Returns symbols with 0 incoming Calls edges.
- [ ] Excludes file-level paths (no `>`).
- [ ] Optional `path_prefix` filter applied correctly.
- [ ] Results sorted lexicographically.
- [ ] Empty graph returns `{ entry_points: [] }`.
- [ ] All prior tests pass.
