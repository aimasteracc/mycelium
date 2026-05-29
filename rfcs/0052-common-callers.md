# RFC-0052 — `Store::common_callers` + `mycelium_get_common_callers` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0052                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0019 (rank_symbols), RFC-0046 (node_degree) |

## Summary

Add `Store::common_callers(target_ids, kind)` — symbol nodes that appear as
an incoming neighbour for ALL of the given target nodes for the specified
`EdgeKind` — and expose it as `mycelium_get_common_callers`.

Answers "which symbols depend on ALL of these targets simultaneously?",
useful for finding shared entry-points, cross-cutting dependencies, or
symbols that must be refactored when any of the targets change.

## Design

### Store method

```rust
pub fn common_callers(&self, target_ids: &[NodeId], kind: EdgeKind) -> Vec<String>
```

For each target, collect its `incoming(id, kind)` set.  Intersect all sets.
Return sorted paths for surviving nodes.  Empty `target_ids` returns empty
`Vec`.

### MCP tool — `mycelium_get_common_callers`

Request:
```json
{ "paths": ["src/a.rs>A", "src/b.rs>B"], "edge_kind": "calls" }
```

`paths` — list of target node paths (1–20 entries).
`edge_kind` must be `"calls"`, `"imports"`, `"extends"`, or `"implements"`.

Response:
```json
{
  "callers": ["src/main.rs>main"],
  "count": 1
}
```

Empty `paths` array returns `{ "callers": [], "count": 0 }`.
Unknown path returns `{ "error": "path not found: ..." }`.
Unknown `edge_kind` returns `{ "error": "unknown edge_kind: ..." }`.

## Acceptance Criteria

- [ ] `Store::common_callers(targets, kind)` returns intersection of all incoming-neighbour sets.
- [ ] Empty targets returns empty `Vec`.
- [ ] Single target equivalent to `incoming(target, kind)` resolved to paths.
- [ ] Results sorted ascending by path.
- [ ] `mycelium_get_common_callers`: valid request returns `{ callers, count }`.
- [ ] `mycelium_get_common_callers`: unknown path returns `{ error }`.
- [ ] `mycelium_get_common_callers`: unknown edge_kind returns `{ error }`.
- [ ] All prior tests pass.
