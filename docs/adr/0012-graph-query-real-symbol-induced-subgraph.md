# ADR-0012: Graph-theory queries operate on the real-symbol induced subgraph

**Status**: Accepted
**Date**: 2026-06-08
**RFC**: [RFC-0118](../../rfcs/0118-resolver-receiver-disambiguation.md) (Part A.2)
**Deciders**: rust-implementer, founder (auditor)

---

## Context

RFC-0118 Part A introduced `NodeKind::Unresolved` — a dedicated marker for the
resolver's *phantom* nodes: synthetic callee/receiver stubs minted for calls
that cannot be statically resolved (e.g. a bare `unwrap`, or a qualified
`Db>upsert_node` whose receiver type is unknown). The extractor links a `Calls`
edge from the caller to each phantom so the caller is not falsely reported
"dead". Part A then gated the **three** queries that demonstrably leaked these
phantoms as if they were real code: `all_symbols`, `page_rank`, and
`rank_symbols` (`top_callee_symbols` / `top_symbols_by_incoming`), using the
authoritative `Store::is_real_symbol(id)` predicate (= `kind != Unresolved`).

But the `store` module also exposes a large family of **graph-theory** queries —
the ones an agent reaches for to answer "what matters here": centrality,
cycles, strongly/weakly-connected components, k-core, dependency layers,
articulation points, bridges, biconnected components, topological order, hubs,
leaves, isolated nodes, most-connected, singly-referenced. Every one of these
built its node universe with the *string heuristic*

```rust
self.trunk.all_paths().filter(|p| p.contains('>')).filter_map(|p| self.trunk.lookup_path(p))
```

which admits `>`-qualified phantoms (e.g. `Db>upsert_node`) into the universe,
and several of them counted *raw* edge degree — so a phantom could appear in a
result list, inflate a real node's degree, sit on a shortest path, or join a
cycle/component. This makes the structural picture an agent perceives wrong in
exactly the place RFC-0118 set out to fix.

Per Charter §3, each of these is a public output-contract change and therefore
needs a decision record.

---

## Decision

1. **Single source of truth for the node universe.** Add
   `Store::symbol_universe(&self) -> Vec<NodeId>` = the `>`-qualified symbol
   nodes filtered by `is_real_symbol`. All gated graph-theory queries construct
   their universe through it. The three Part-A queries are also routed through
   it where it is a behaviour-preserving no-op (`page_rank`).

2. **Operate on the real-symbol *induced subgraph*, not just a filtered node
   list.** When an algorithm walks edges (degree, BFS, SCC, union-find,
   Tarjan), it skips any edge whose other endpoint is not in the real universe.
   Most of these queries already restricted traversal to a `sym_set` / `idx`
   built from the universe, so once the universe excludes phantoms the induced
   subgraph falls out for free; the degree-style queries that read *raw*
   `synapse` degree were changed to count only real-endpoint edges. Centrality
   normalization denominators use `|real symbols|`, not `|all nodes|`.

3. **Back-compatible by construction.** `is_real_symbol` is a *negative* gate
   (exclude iff `Unresolved`), so a programmatic/legacy store that never
   recorded kinds has no `Unresolved` nodes and is therefore unchanged.

4. **ADR documents the per-query disposition** (below). Each gated query is a
   Charter §3 output-contract change.

---

## Per-query disposition

`GATE-FULL` = node universe **and** edge induction both restricted to real
symbols (a phantom can neither appear nor affect any real node's metric).
`GATE-OUTPUT-ONLY` = phantom dropped from the universe/result; documented where
the edge-level induction is already implied or not applicable.

| Query | Disposition | Notes |
|---|---|---|
| `leaf_symbols` | GATE-FULL | universe via `symbol_universe`; out-degree counts real endpoints only |
| `isolated_symbols` | GATE-FULL | universe via `symbol_universe`; node-degree induced over real endpoints |
| `singly_referenced` | GATE-FULL | universe via `symbol_universe`; incoming counted over real callers only |
| `hub_symbols` | GATE-FULL | universe via `symbol_universe`; in/out degree over real endpoints only |
| `most_connected` | GATE-FULL | universe via `symbol_universe`; total degree over real endpoints only |
| `k_core` | GATE-FULL | universe via `symbol_universe`; subgraph degree already `sym_set`-restricted |
| `dependency_layers` | GATE-FULL | universe via `symbol_universe`; Kahn edges already `sym_set`-restricted |
| `topological_sort` | GATE-FULL | universe via `symbol_universe`; successors already `sym_set`-restricted |
| `nodes_in_cycles` | GATE-FULL | universe via `symbol_universe`; DFS edges restricted to real symbols |
| `cycle_members` | GATE-FULL | universe via `symbol_universe`; Kosaraju adjacency already `sym_set`-restricted |
| `scc_groups` | GATE-FULL | universe via `symbol_universe`; Tarjan edges already `sym_ids`-restricted |
| `strongly_connected_components` | GATE-FULL | universe via `symbol_universe`; Tarjan adjacency already `idx`-restricted |
| `weakly_connected_components` | GATE-FULL | universe via `symbol_universe`; union-find already `sym_set`-restricted |
| `articulation_points` | GATE-FULL | universe via `symbol_universe`; undirected adjacency already `sym_set`-restricted |
| `bridge_edges` | GATE-FULL | universe via `symbol_universe`; undirected adjacency already `sym_set`-restricted |
| `biconnected_components` | GATE-FULL | universe via `symbol_universe`; undirected adjacency already `sym_set`-restricted |
| `betweenness_centrality` | GATE-FULL | universe via `symbol_universe`; Brandes edges `idx`-restricted; norm = `(n-1)(n-2)` over real n |
| `closeness_centrality` | GATE-FULL | universe via `symbol_universe`; BFS edges `idx`-restricted; norm = `n-1` over real n |
| `harmonic_centrality_stats` | GATE-FULL | symbol_count = `|real symbols|`; BFS counts only real-symbol endpoints; norm = `n-1` over real n |

All 19 land as **GATE-FULL**. No query shipped GATE-OUTPUT-ONLY or DEFERRED:
the dominant pattern (universe + `sym_set`/`idx`-restricted traversal) made the
induced subgraph fall out of the universe swap for the algorithmic queries, and
the five raw-degree queries each had a single, local degree expression to
induce.

---

## Rationale

- **Correctness of the agent's structural perception.** A phantom on a real
  node's call list is the exact failure RFC-0118 targets; letting it survive in
  centrality/cycle/k-core output reintroduces the noise on a different surface.
- **One universe, one definition.** Centralizing the node set in
  `symbol_universe()` removes 19 copies of the `p.contains('>')` heuristic from
  the universe-construction sites and gives a single, authoritative,
  kind-based definition — replacing a fragile string test with a typed gate.
- **Induced subgraph, not just node filtering.** Filtering only the node list
  would still let a phantom edge inflate a real node's degree or carry shortest-
  path mass. Restricting traversal to real endpoints makes every metric a
  property of the real-symbol subgraph.
- **No new SLA risk.** The change adds at most one `is_real_symbol` check per
  node at universe build (O(V)) and reuses existing `sym_set`/`idx` membership
  tests on the edge path; no extra graph passes.

---

## Consequences

- **Output contract change (intended).** On a kind-annotated store, the 19
  queries no longer emit `Unresolved` phantoms and no longer count phantom
  edges. Snapshot/golden tests that previously captured phantom-inclusive
  output must be regenerated. Programmatic stores without kinds are unchanged.
- **CLI ↔ MCP parity preserved.** Both surfaces call the same core methods;
  gating in core keeps them byte-identical by construction (Charter §5.13).
- **Forward-safe.** Any future `NodeKind::Unresolved` node — even one with zero
  incoming edges — is excluded by kind, not by an edge heuristic.

---

## Alternatives considered

- **Filter the result list only (GATE-OUTPUT-ONLY everywhere).** Rejected:
  leaves phantom edges inflating real-node degree and shortest-path counts.
- **Drop phantoms in the extractor so they never enter the graph.** Rejected by
  RFC-0118: the `Calls` edge to the phantom is what keeps the caller from being
  falsely "dead"; the phantom must exist as a node, just not as a *real* one.
- **A `clippy`-style allowlist of which queries to gate.** Rejected: the
  authoritative predicate already exists (`is_real_symbol`); a single
  `symbol_universe()` is simpler and uniform.
