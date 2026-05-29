# RFC-0063 — `Store::batch_reachable_to` + `mycelium_batch_reachable_to` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0063                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0043 (reachable_from), RFC-0044 (reachable_to), RFC-0052 (common_callers) |

## Summary

Add `Store::batch_reachable_to(ids, kind, max_depth)` — the **union** of all
symbols that can transitively reach *any* of the given target nodes via
incoming `EdgeKind` edges — and expose it as `mycelium_batch_reachable_to`.

`reachable_to` (RFC-0044) answers "what depends on *this* symbol?" for a
single symbol.  `batch_reachable_to` answers "what is the **total blast radius**
if *any one* of these symbols changes?" — a single call for the entire impact
surface of a multi-file diff or a feature-level change.

## Design

### Store method

```rust
pub fn batch_reachable_to(
    &self,
    ids: &[NodeId],
    kind: EdgeKind,
    max_depth: usize,
) -> Vec<String>
```

- Runs `reachable_to(id, kind, max_depth)` for each id in `ids`.
- Unions the results; deduplicates.
- Excludes any path that is itself one of the input `ids`.
- `max_depth` capped at 20 (same as `reachable_to`).
- Results sorted ascending.
- Returns `Vec<String>` of resolved path strings.

### MCP tool — `mycelium_batch_reachable_to`

Request:
```json
{ "paths": ["src/a.rs>A", "src/b.rs>B"], "edge_kind": "calls", "max_depth": 5 }
```

- `paths`: up to 20 symbol paths (excess paths after index 20 are silently ignored).
- `edge_kind` must be `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
- `max_depth` defaults to 10, capped at 20.

Response:
```json
{
  "reachable": ["src/app.rs>App", "src/main.rs>main"],
  "count": 2
}
```

Unknown paths in `paths` are silently skipped (no contribution to result).
Unknown `edge_kind` returns `{ "error": "unknown edge_kind: ..." }`.
Empty `paths` returns `{ "reachable": [], "count": 0 }`.

## Acceptance Criteria

- [x] `Store::batch_reachable_to(ids, kind, max_depth)` returns union of individual `reachable_to` results.
- [x] Input node ids excluded from result.
- [x] `max_depth` capped at 20.
- [x] Results sorted ascending.
- [x] Deduplication: a path reachable from multiple inputs appears only once.
- [x] `mycelium_batch_reachable_to`: valid request returns `{ reachable, count }`.
- [x] `mycelium_batch_reachable_to`: unknown edge_kind returns `{ error }`.
- [x] `mycelium_batch_reachable_to`: empty paths returns empty result.
- [x] `mycelium_batch_reachable_to`: unknown paths silently skipped.
- [x] `max_depth` defaults to 10; paths list capped at 20 entries.
- [x] All prior tests pass.
