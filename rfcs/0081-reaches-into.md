# RFC-0081 — `Store::reaches_into` + `mycelium_get_reaches_into` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0081                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0080 (reachable_set), RFC-0079 (mutual_reachability) |

## Summary

Add `Store::reaches_into(id, kind)` — reverse BFS transitive closure:
returns all symbol paths that can reach `id` by following `kind` edges
(i.e. `id` is in their `reachable_set`) — and expose it as
`mycelium_get_reaches_into`.

Answers "what symbols transitively depend on / call / import this one?".
Symmetric companion to RFC-0080 `reachable_set`.

Returns sorted paths. `id` itself excluded.
O(V + E). File nodes excluded.

## Design

### Store method

```rust
pub fn reaches_into(&self, id: NodeId, kind: EdgeKind) -> Vec<String>
```

- Returns sorted list of symbol paths that can reach `id` via `kind` edges.
- `id` itself excluded.
- File nodes excluded.
- Isolated node (no incoming reachable symbols) → empty `Vec`.
- Uses reverse BFS: follows `synapse.incoming(node, kind)` edges.

### MCP tool — `mycelium_get_reaches_into`

Request:
```json
{ "path": "src/a.rs>A", "edge_kind": "calls" }
```

Response:
```json
{ "callers": ["src/b.rs>B", "src/c.rs>C"], "count": 2 }
```

- Unknown path → `{ "error": "unknown path: <value>" }`.
- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [x] `reaches_into` returns empty for an isolated node.
- [x] Returns direct callers for a single-hop graph.
- [x] Returns full transitive reverse closure for a chain A→B→C (reaches_into(C) = {A, B}).
- [x] Does not include `id` itself (even if there is a cycle back to it).
- [x] Results are sorted alphabetically.
- [x] File nodes excluded from results.
- [x] `mycelium_get_reaches_into`: valid path returns `{ callers, count }`.
- [x] `mycelium_get_reaches_into`: unknown path returns `{ error }`.
- [x] `mycelium_get_reaches_into`: unknown edge_kind returns `{ error }`.
- [x] All prior tests pass.
