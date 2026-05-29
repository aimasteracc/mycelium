# RFC-0080 — `Store::reachable_set` + `mycelium_get_reachable_set` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0080                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0079 (mutual_reachability), RFC-0050 (shortest_path) |

## Summary

Add `Store::reachable_set(id, kind)` — returns all symbol node paths
transitively reachable from `id` via `kind` edges (excluding `id` itself) —
and expose it as `mycelium_get_reachable_set`.

Answers "what does this symbol transitively depend on / call / import?".
Returns sorted paths so output is deterministic.

O(V + E) (single BFS). File nodes excluded from traversal and from results.
`id` itself is not included in the result set.

## Design

### Store method

```rust
pub fn reachable_set(&self, id: NodeId, kind: EdgeKind) -> Vec<String>
```

- Returns sorted list of symbol paths reachable from `id` via `kind` edges.
- `id` itself excluded.
- File nodes excluded.
- Isolated node (no outgoing reachable symbols) → empty `Vec`.

### MCP tool — `mycelium_get_reachable_set`

Request:
```json
{ "path": "src/a.rs>A", "edge_kind": "calls" }
```

Response:
```json
{ "reachable": ["src/b.rs>B", "src/c.rs>C"], "count": 2 }
```

- Unknown path → `{ "error": "unknown path: <value>" }`.
- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [ ] `reachable_set` returns empty for an isolated node.
- [ ] Returns direct neighbors for a single-hop graph.
- [ ] Returns full transitive closure for a chain A→B→C.
- [ ] Does not include `id` itself (even if there is a cycle back to it).
- [ ] Results are sorted alphabetically.
- [ ] File nodes excluded from results.
- [ ] `mycelium_get_reachable_set`: valid path returns `{ reachable, count }`.
- [ ] `mycelium_get_reachable_set`: unknown path returns `{ error }`.
- [ ] `mycelium_get_reachable_set`: unknown edge_kind returns `{ error }`.
- [ ] All prior tests pass.
