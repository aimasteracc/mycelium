# RFC-0055 — `Store::common_callees` + `mycelium_get_common_callees` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0055                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0052 (common_callers — incoming intersection) |

## Summary

Add `Store::common_callees(source_ids, kind)` — symbol nodes that appear as
an outgoing neighbour for ALL of the given source nodes for the specified
`EdgeKind` — and expose it as `mycelium_get_common_callees`.

Symmetric complement to `common_callers` (RFC-0052): where `common_callers`
answers "which symbols depend on ALL of these targets?", `common_callees`
answers "which symbols are depended upon by ALL of these sources?". Useful
for finding shared downstream dependencies or common imports.

## Design

### Store method

```rust
pub fn common_callees(&self, source_ids: &[NodeId], kind: EdgeKind) -> Vec<String>
```

For each source, collect its `outgoing(id, kind)` set. Intersect all sets.
Return sorted paths for surviving nodes. Empty `source_ids` returns empty
`Vec`.

### MCP tool — `mycelium_get_common_callees`

Request:
```json
{ "paths": ["src/a.rs>A", "src/b.rs>B"], "edge_kind": "calls" }
```

`paths` — list of source node paths (1–20 entries).
`edge_kind` must be `"calls"`, `"imports"`, `"extends"`, or `"implements"`.

Response:
```json
{
  "callees": ["src/utils.rs>log"],
  "count": 1
}
```

Empty `paths` array returns `{ "callees": [], "count": 0 }`.
Unknown path returns `{ "error": "path not found: ..." }`.
Unknown `edge_kind` returns `{ "error": "unknown edge_kind: ..." }`.

## Acceptance Criteria

- [x] `Store::common_callees(sources, kind)` returns intersection of all outgoing-neighbour sets.
- [x] Empty sources returns empty `Vec`.
- [x] Single source equivalent to `outgoing(source, kind)` resolved to paths.
- [x] Results sorted ascending by path.
- [x] `mycelium_get_common_callees`: valid request returns `{ callees, count }`.
- [x] `mycelium_get_common_callees`: unknown path returns `{ error }`.
- [x] `mycelium_get_common_callees`: unknown edge_kind returns `{ error }`.
- [x] All prior tests pass.
