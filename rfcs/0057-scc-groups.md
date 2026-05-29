# RFC-0057 — `Store::scc_groups` + `mycelium_get_scc_groups` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0057                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0040 (nodes_in_cycles — per-node cycle membership) |

## Summary

Add `Store::scc_groups(kind)` — Tarjan's Strongly Connected Components
algorithm over symbol nodes for a given `EdgeKind`, returning groups of
size ≥ 2 — and expose it as `mycelium_get_scc_groups`.

Complements `nodes_in_cycles` (RFC-0040) which returns individual nodes
in cycles. `scc_groups` groups those nodes into their actual dependency
clusters, making it easy to see which specific symbols form tight
mutually-recursive cycles with each other.

## Design

### Store method

```rust
pub fn scc_groups(&self, kind: EdgeKind) -> Vec<Vec<String>>
```

Runs Tarjan's iterative SCC algorithm over all symbol nodes (paths
containing `>`). Returns only groups with size ≥ 2 (singletons excluded).
Groups sorted by size descending (largest first), then by the first path
within the group ascending. Paths within each group sorted ascending.

### MCP tool — `mycelium_get_scc_groups`

Request:
```json
{ "edge_kind": "calls" }
```

`edge_kind` must be `"calls"`, `"imports"`, `"extends"`, or `"implements"`.

Response:
```json
{
  "groups": [
    ["src/a.rs>a", "src/b.rs>b", "src/c.rs>c"],
    ["src/x.rs>x", "src/y.rs>y"]
  ],
  "group_count": 2,
  "total_symbols": 5
}
```

Unknown `edge_kind` returns `{ "error": "unknown edge_kind: ..." }`.

## Acceptance Criteria

- [ ] `Store::scc_groups(kind)` finds all SCCs of size ≥ 2 for symbol nodes.
- [ ] File nodes excluded (paths not containing `>`).
- [ ] Singletons (size 1) excluded from output.
- [ ] Groups sorted by size descending, ties by first path ascending.
- [ ] Paths within each group sorted ascending.
- [ ] `mycelium_get_scc_groups`: valid edge_kind returns `{ groups, group_count, total_symbols }`.
- [ ] `mycelium_get_scc_groups`: unknown edge_kind returns `{ error }`.
- [ ] All prior tests pass.
