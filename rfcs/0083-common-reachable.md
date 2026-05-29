# RFC-0083 — `Store::common_reachable` + `mycelium_get_common_reachable` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0083                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0080 (reachable_set), RFC-0081 (reaches_into) |

## Summary

Add `Store::common_reachable(id1, id2, kind)` — intersection of the
transitive reachable sets of two symbol nodes for a given `EdgeKind` — and
expose it as `mycelium_get_common_reachable`.

Answers "what symbols do both of these nodes transitively depend on
(call/import/extend)?".  Useful for refactoring analysis (finding shared
utility code), impact analysis (nodes that would affect both if changed),
and duplicate-detection (similar call graphs share many common reachable
nodes).

O(V + E) — two BFS traversals then a HashSet intersection.
`id1` and `id2` themselves are excluded from the result.
File nodes excluded.
Results sorted alphabetically.

## Design

### Store method

```rust
pub fn common_reachable(
    &self,
    id1: NodeId,
    id2: NodeId,
    kind: EdgeKind,
) -> Vec<String>
```

- Returns sorted paths in `reachable_set(id1) ∩ reachable_set(id2)`.
- `id1` and `id2` excluded even if reachable from the other.
- Both isolated → empty `Vec`.
- `id1 == id2` → same as `reachable_set(id1, kind)`.

### MCP tool — `mycelium_get_common_reachable`

Request:
```json
{ "path1": "src/a.rs>A", "path2": "src/b.rs>B", "edge_kind": "calls" }
```

Response:
```json
{ "common": ["src/util.rs>helper"], "count": 1 }
```

- Unknown path → `{ "error": "unknown path: <value>" }`.
- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [ ] Both isolated → empty result.
- [ ] No overlap in reachable sets → empty result.
- [ ] Single shared dependency → returns that one path.
- [ ] `id1 == id2` → same result as `reachable_set(id1, kind)`.
- [ ] `id1` and `id2` themselves excluded from result.
- [ ] Results sorted alphabetically.
- [ ] File nodes excluded.
- [ ] `mycelium_get_common_reachable`: valid paths returns `{ common, count }`.
- [ ] `mycelium_get_common_reachable`: unknown path returns `{ error }`.
- [ ] `mycelium_get_common_reachable`: unknown edge_kind returns `{ error }`.
- [ ] All prior tests pass.
