# RFC-0018 — `mycelium_get_files` MCP Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0018                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0004 (MCP server), RFC-0005 (descendants tool) |

## Summary

Add `mycelium_get_files` — an MCP tool that enumerates all source files
currently materialized in the index, with an optional path-prefix filter.

## Motivation

AI coding agents frequently need to know which files are indexed before
searching for symbols or traversing the call graph.  Currently there is no
way to list the indexed files — the agent must guess paths or search for
file-level nodes via `mycelium_search_symbol`.  Adding a dedicated tool
closes this gap cleanly.

## Design

### Store layer

`Store::all_file_paths() -> Vec<String>`

Returns all trunk paths that contain no `>` separator (i.e. file-level
nodes), sorted lexicographically.  Implemented as a one-liner on top of
the existing `all_paths()` iterator.

### Request

```json
{
  "path_prefix": "src/"
}
```

`path_prefix` is optional.  When absent, all indexed files are returned.

### Response

```json
{
  "files": [
    "src/auth.rs",
    "src/db.rs",
    "src/main.rs"
  ]
}
```

Always sorted lexicographically.  Empty array when no files match.

### Implementation

1. Add `Store::all_file_paths()` — filter `all_paths()` to paths without `>`, collect into sorted `Vec<String>`.
2. Add `GetFilesRequest { path_prefix: Option<String> }`.
3. Add `mycelium_get_files` to the `#[tool_router]` impl.

## Acceptance Criteria

- [x] `Store::all_file_paths()` returns only file-level paths (no `>`).
- [x] `Store::all_file_paths()` returns paths in sorted order.
- [x] `mycelium_get_files` with no prefix returns all indexed files.
- [x] `mycelium_get_files` with a prefix filters by prefix match.
- [x] Symbol-level paths (containing `>`) are excluded from the result.
- [x] All prior tests pass.
