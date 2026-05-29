# RFC-0065 — `Store::batch_reachable_from` + `mycelium_batch_reachable_from` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0065                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0043 (reachable_from), RFC-0063 (batch_reachable_to) |

## Summary

Add `Store::batch_reachable_from(ids, kind, max_depth)` — the **union** of all
symbols transitively reachable *from* any of the given source nodes via
outgoing `EdgeKind` edges — and expose it as `mycelium_batch_reachable_from`.

Symmetric complement of `batch_reachable_to` (RFC-0063):

| RFC   | Direction | Question answered |
|-------|-----------|-------------------|
| 0063  | incoming  | "What's the blast radius of changing *any* of these?" |
| 0065  | outgoing  | "What does the combined execution of *any* of these transitively touch?" |

## Design

### Store method

```rust
pub fn batch_reachable_from(
    &self,
    ids: &[NodeId],
    kind: EdgeKind,
    max_depth: usize,
) -> Vec<String>
```

- Runs BFS from all `ids` simultaneously via outgoing `kind` edges.
- Unions the results; deduplicates.
- Excludes any path that is itself one of the input `ids`.
- `max_depth` capped at 20 (same as `reachable_from`).
- Results sorted ascending.
- Returns `Vec<String>` of resolved path strings.

### MCP tool — `mycelium_batch_reachable_from`

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
  "reachable": ["src/util.rs>format", "src/util.rs>parse"],
  "count": 2
}
```

Unknown paths in `paths` are silently skipped.
Unknown `edge_kind` returns `{ "error": "unknown edge_kind: ..." }`.
Empty `paths` returns `{ "reachable": [], "count": 0 }`.

## Acceptance Criteria

- [ ] `Store::batch_reachable_from(ids, kind, max_depth)` returns union of individual `reachable_from` results.
- [ ] Input node ids excluded from result.
- [ ] `max_depth` capped at 20.
- [ ] Results sorted ascending.
- [ ] Deduplication: a path reachable from multiple inputs appears only once.
- [ ] `mycelium_batch_reachable_from`: valid request returns `{ reachable, count }`.
- [ ] `mycelium_batch_reachable_from`: unknown edge_kind returns `{ error }`.
- [ ] `mycelium_batch_reachable_from`: empty paths returns empty result.
- [ ] `mycelium_batch_reachable_from`: unknown paths silently skipped.
- [ ] `max_depth` defaults to 10; paths list capped at 20 entries.
- [ ] All prior tests pass.
