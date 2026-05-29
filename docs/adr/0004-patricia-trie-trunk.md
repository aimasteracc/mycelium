# 0004. Patricia Trie as the Trunk data structure

- **Status**: accepted
- **Date**: 2026-05-29
- **RFC**: RFC-0089

## Context

The Trunk v0.1 used `HashMap<String, NodeId>` (forward) + `HashMap<NodeId, String>` (reverse).
Benchmark at 100k nodes showed:
- `lookup_path`: 8.2 ns (excellent)
- `descendants`: **256 µs — O(N) full-table scan** (unacceptable for graph traversal)
- `remove_subtree`: O(N)

The `descendants` operation is on the hot path for BFS traversal, SCC, PageRank,
and every other graph algorithm. An O(N) scan makes those algorithms O(N²) in practice.

## Decision

Replace the dual-HashMap implementation with a **Patricia Trie** (compressed radix trie)
where path segments (split on `>`) are the edge labels.

Keep `by_id: HashMap<NodeId, String>` as a reverse index for O(1) `path_of` lookups.

## Consequences

| Operation | HashMap | Patricia Trie |
|-----------|---------|---------------|
| `lookup_path` | 8 ns | 50 ns |
| `descendants` | 256 µs | **392 ns (653× faster)** |
| `remove_subtree` | O(N) | O(K) |
| Memory | Flat maps | Structured tree |

`lookup_path` regressed from 8 ns to 50 ns, but both satisfy the Charter < 5 ms SLA.
The `descendants` win is decisive for all graph traversal algorithms.

No `unsafe` code. Public API unchanged. All 459 core tests pass unmodified.
