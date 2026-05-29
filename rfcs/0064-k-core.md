# RFC-0064 — `Store::k_core` + `mycelium_get_k_core` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0064                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0048 (most_connected), RFC-0057 (scc_groups), RFC-0058 (dependency_layers) |

## Summary

Add `Store::k_core(kind, k)` — the **k-core decomposition** of the symbol
graph for a given `EdgeKind` — and expose it as `mycelium_get_k_core`.

The k-core is the **maximal induced subgraph** in which every node has total
degree (in + out within the subgraph) ≥ k.  It is computed by iteratively
peeling off all nodes whose degree drops below k as other nodes are removed.

### Use cases

- **Hard-to-refactor core**: symbols so tightly interconnected that each one
  has ≥ k edges to the others; any refactoring must touch the whole k-core.
- **Architectural clusters**: increasing k reveals progressively denser cores,
  from a broad 2-core to a tight, high-k core.
- **Risk assessment**: if a PR touches symbols in the high-k core, expect
  cascading review and test burden.

## Design

### Algorithm (peeling / bucket queue)

1. Restrict to symbol nodes (paths containing `>`).
2. Compute `degree[id] = in_degree(id, kind) + out_degree(id, kind)` for all
   symbols, counting only edges whose **both** endpoints are symbol nodes.
3. Queue all nodes with `degree[id] < k`.
4. While queue non-empty:
   a. Remove node `u` from working set.
   b. For each neighbour `v` of `u` (incoming or outgoing, for `kind`):
      - If `v` is still in the working set, decrement `degree[v]`.
      - If `degree[v]` drops below `k`, enqueue `v`.
5. Return paths of all nodes remaining in the working set, sorted ascending.

The result is empty if no symbol satisfies the k-core condition.

### Store method

```rust
pub fn k_core(&self, kind: EdgeKind, k: usize) -> Vec<String>
```

- `k = 0` returns all symbol nodes (everything is trivially in the 0-core).
- Results sorted ascending.
- File nodes excluded.

### MCP tool — `mycelium_get_k_core`

Request:
```json
{ "edge_kind": "calls", "k": 2 }
```

`k` must be ≥ 0 and defaults to 2 if omitted.
`edge_kind` must be `"calls"`, `"imports"`, `"extends"`, or `"implements"`.

Response:
```json
{
  "core": ["src/a.rs>A", "src/b.rs>B"],
  "count": 2,
  "k": 2
}
```

Unknown `edge_kind` returns `{ "error": "unknown edge_kind: ..." }`.

## Acceptance Criteria

- [ ] `Store::k_core(kind, k)` returns the maximal subgraph where every node has total degree ≥ k.
- [ ] Degree counts only edges within the symbol subgraph (file nodes ignored).
- [ ] `k = 0` returns all symbol nodes.
- [ ] Results sorted ascending.
- [ ] File nodes excluded.
- [ ] `mycelium_get_k_core`: valid request returns `{ core, count, k }`.
- [ ] `mycelium_get_k_core`: unknown edge_kind returns `{ error }`.
- [ ] `k` defaults to 2 if omitted in the MCP request.
- [ ] All prior tests pass.
