# RFC-0040 — `Store::nodes_in_cycles` + `mycelium_detect_cycles` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0040                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0039 (cross_refs), RFC-0023 (imports), RFC-0011 (calls) |

## Summary

Add `Store::nodes_in_cycles(edge_kind, prefix)` — returns all node paths that
participate in at least one cycle in the edge-kind subgraph — and expose it
as `mycelium_detect_cycles`.  Circular dependency detection in one call.

## Motivation

Circular imports (A imports B imports A), circular extends chains, and
mutual Calls cycles are code-quality problems.  No existing Mycelium tool
identifies them.  An agent doing quality analysis must be able to ask
"are there any circular dependencies in my import graph?"

## Design

### Algorithm

Iterative post-order DFS with a per-traversal `in_stack` bit-set:

```
for each unvisited node n:
    DFS(n):
        push n onto in_stack
        for each neighbor m of n:
            if m in in_stack:
                mark m as cycle_member
                mark all nodes between m and n in the DFS stack as cycle_member
            elif m not yet visited:
                DFS(m)
        pop n from in_stack
        mark n as visited
```

All nodes marked `cycle_member` are returned, sorted lexicographically.

### Store method

```rust
pub fn nodes_in_cycles(&self, edge_kind: EdgeKind, prefix: Option<&str>) -> Vec<String>
```

### MCP tool — `mycelium_detect_cycles`

Request: `{ "edge_kind": "imports", "path_prefix": "src/" }` (path_prefix optional)

Response:
```json
{
  "cycle_nodes": ["src/a.rs>a", "src/b.rs>b"],
  "count": 2
}
```

Empty result: `{ "cycle_nodes": [], "count": 0 }`.

`edge_kind` must be one of `"calls"`, `"imports"`, `"extends"`, `"implements"`.
Unknown edge_kind returns `{ "error": "unknown edge_kind: ..." }`.

## Acceptance Criteria

- [ ] `Store::nodes_in_cycles(edge_kind, prefix)` returns nodes that are part of a cycle.
- [ ] Nodes not in any cycle are excluded.
- [ ] Prefix filter applies to paths.
- [ ] Results are sorted lexicographically.
- [ ] `mycelium_detect_cycles`: valid edge_kind returns `{ cycle_nodes, count }`.
- [ ] `mycelium_detect_cycles`: unknown edge_kind returns `{ error }`.
- [ ] All prior tests pass.
