# RFC-0067 — `Store::cycle_members` + `mycelium_find_cycle_members` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0067                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0064 (k_core), RFC-0063 (batch_reachable_to) |

## Summary

Add `Store::cycle_members(kind)` — returns the paths of all symbol nodes that
participate in at least one directed cycle for a given `EdgeKind` — and expose
it as `mycelium_find_cycle_members`.

Cycle membership is determined via Kosaraju's two-pass SCC algorithm over the
symbol sub-graph.  Any node whose strongly-connected component has size ≥ 2 is
a cycle member.  File nodes are excluded.

## Design

### Store method

```rust
pub fn cycle_members(&self, kind: EdgeKind) -> Vec<String>
```

- Returns paths of symbol nodes participating in any cycle for `kind`.
- Results sorted ascending.
- Returns `[]` if no cycles exist.

### Algorithm

Kosaraju's SCC (O(V + E)):

1. Restrict to symbol nodes (paths that contain `>`).
2. First DFS pass on the forward graph; push finish order onto a stack.
3. Second DFS pass on the reversed graph in finish-order; each tree is one SCC.
4. Collect nodes in SCCs with size ≥ 2.

### MCP tool — `mycelium_find_cycle_members`

Request:
```json
{ "edge_kind": "calls" }
```

- `edge_kind`: one of `calls`, `imports`, `extends`, `implements`.

Response:
```json
{
  "members": ["src/a.rs>foo", "src/b.rs>bar"],
  "count": 2
}
```

- `members`: symbol paths of cycle participants, sorted ascending.
- `count`: total number of cycle-member symbols.
- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [x] `Store::cycle_members(kind)` returns paths of symbols in cycles, sorted ascending.
- [x] Nodes in SCCs of size 1 (no self-loop, no mutual cycle) are excluded.
- [x] `cycle_members` returns `[]` when no cycles exist.
- [x] File nodes are excluded.
- [x] `mycelium_find_cycle_members`: valid edge_kind returns `{ members, count }`.
- [x] `mycelium_find_cycle_members`: unknown edge_kind returns `{ error }`.
- [x] All prior tests pass.
