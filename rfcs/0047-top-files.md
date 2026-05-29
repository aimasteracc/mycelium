# RFC-0047 — `Store::top_files` + `mycelium_get_top_files` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0047                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0019 (rank_symbols), RFC-0038 (graph_stats) |

## Summary

Add `Store::top_files(limit)` — top-N source files ranked by direct
symbol count (number of direct children in the containment tree) — and
expose it as `mycelium_get_top_files`.

Answers "which files are the most complex?" by counting the symbols
directly nested inside each file node.  Identifies god-files and helps
prioritise refactoring effort.

## Design

### Store method

```rust
pub fn top_files(&self, limit: usize) -> Vec<(String, usize)>
```

1. Collect all file-level paths (no `>` in path).
2. For each file, count its direct children (paths of the form
   `{file}>*` that contain exactly one `>`, i.e. depth-1 symbols).
3. Sort descending by count, then ascending by path for ties.
4. Return up to `limit` entries (cap 100 internally).

### MCP tool — `mycelium_get_top_files`

Request: `{ "limit": 10 }` (optional, defaults to 10, capped at 100)

Response:
```json
{
  "files": [
    { "path": "src/god.rs", "symbol_count": 42 },
    { "path": "src/heavy.rs", "symbol_count": 18 }
  ],
  "count": 2
}
```

Empty graph returns `{ "files": [], "count": 0 }`.

## Acceptance Criteria

- [ ] `Store::top_files(limit)` counts direct child symbols per file node.
- [ ] Files without symbols are excluded (count > 0 only).
- [ ] Results sorted descending by count, then ascending by path for ties.
- [ ] `limit` respected; capped at 100 internally.
- [ ] `mycelium_get_top_files`: returns `{ files, count }`.
- [ ] `mycelium_get_top_files`: limit defaults to 10, capped at 100.
- [ ] Empty graph returns `{ files: [], count: 0 }`.
- [ ] All prior tests pass.
