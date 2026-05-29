# RFC-0084 — `Store::k_hop_neighbors` + `mycelium_get_k_hop_neighbors` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0084                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0080 (reachable_set), RFC-0050 (shortest_path) |

## Summary

Add `Store::k_hop_neighbors(id, kind, k)` — returns all symbol paths
reachable from `id` in **exactly** `k` BFS hops — and expose it as
`mycelium_get_k_hop_neighbors`.

Answers "what is directly reachable at depth k?" without pulling in the
full transitive closure.  A BFS frontier at depth exactly k; useful for
"what does this function call (k=1)?", "what are the transitive callees two
levels out (k=2)?", etc.

O(V + E). File nodes excluded. `id` excluded from results even if a cycle
brings it back to depth k. Results sorted alphabetically.

## Design

### Store method

```rust
pub fn k_hop_neighbors(&self, id: NodeId, kind: EdgeKind, k: usize) -> Vec<String>
```

- `k == 0` → empty `Vec` (no zero-hop self-result).
- `k == 1` → direct outgoing neighbors (symbol nodes only).
- Nodes at depth < k are not included.
- A node first reached at depth < k cannot appear again at depth k.
- File nodes excluded from traversal and results.
- Results sorted alphabetically.

### MCP tool — `mycelium_get_k_hop_neighbors`

Request:
```json
{ "path": "src/a.rs>A", "edge_kind": "calls", "k": 2 }
```

Response:
```json
{ "neighbors": ["src/b.rs>B", "src/c.rs>C"], "count": 2, "k": 2 }
```

- Unknown path → `{ "error": "unknown path: <value>" }`.
- Unknown `edge_kind` → `{ "error": "unknown edge kind: <value>" }`.

## Acceptance Criteria

- [ ] `k == 0` → empty result.
- [ ] `k == 1` → direct outgoing symbol neighbors.
- [ ] `k == 2` → neighbors two hops away (not including hop-1 nodes).
- [ ] Nodes reachable at depth < k are not re-reported at k.
- [ ] `id` itself excluded even if a cycle returns it at exactly hop k.
- [ ] Results sorted alphabetically.
- [ ] File nodes excluded.
- [ ] `mycelium_get_k_hop_neighbors`: valid path returns `{ neighbors, count, k }`.
- [ ] `mycelium_get_k_hop_neighbors`: unknown path returns `{ error }`.
- [ ] `mycelium_get_k_hop_neighbors`: unknown edge_kind returns `{ error }`.
- [ ] All prior tests pass.
