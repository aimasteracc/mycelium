# RFC-0069 — `Store::topological_sort` + `mycelium_topological_sort` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0069                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0067 (cycle_members), RFC-0058 (dependency_layers) |

## Summary

Add `Store::topological_sort(kind)` — returns a valid topological ordering
of the symbol graph for a given `EdgeKind` using Kahn's BFS algorithm — and
expose it as `mycelium_topological_sort`.

`dependency_layers` (RFC-0058) groups symbols into broad layers.
`topological_sort` goes further: it returns a full linear ordering where every
symbol appears after all its dependencies.  Cycle members — nodes that cannot
be ordered because they form a directed cycle — are reported separately.

## Design

### Return type

```rust
pub struct TopologicalOrder {
    /// Symbols in dependency order: each symbol appears after all its
    /// predecessors.  Sources (in-degree 0) come first.
    pub order: Vec<String>,
    /// Symbols that are part of a directed cycle and could not be placed in
    /// the linear order.  Sorted ascending.
    pub cycle_members: Vec<String>,
}
```

### Store method

```rust
pub fn topological_sort(&self, kind: EdgeKind) -> TopologicalOrder
```

- Kahn's algorithm (BFS / in-degree queue) over the symbol sub-graph.
- Sources (symbols with in-degree 0 within the symbol subgraph) are processed
  first; ties broken by path ascending for determinism.
- Symbols that are never reachable from the queue (those in cycles) populate
  `cycle_members`, sorted ascending.
- File nodes excluded.

### MCP tool — `mycelium_topological_sort`

Request:
```json
{ "edge_kind": "calls" }
```

Response:
```json
{
  "order": ["src/a.rs>A", "src/b.rs>B", "src/c.rs>C"],
  "cycle_members": ["src/x.rs>X", "src/y.rs>Y"],
  "ordered_count": 3,
  "cycle_count": 2
}
```

- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [ ] `Store::topological_sort(kind)` returns `TopologicalOrder` with `order`
      and `cycle_members`.
- [ ] Each symbol in `order` appears after all its `kind`-predecessors.
- [ ] Symbols forming directed cycles appear in `cycle_members`, not `order`.
- [ ] `cycle_members` sorted ascending; `order` is a valid topological ordering.
- [ ] File nodes excluded from both lists.
- [ ] `mycelium_topological_sort`: valid edge_kind returns `{ order, cycle_members, ordered_count, cycle_count }`.
- [ ] `mycelium_topological_sort`: unknown edge_kind returns `{ error }`.
- [ ] All prior tests pass.
