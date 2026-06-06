---
name: graph-structure
description: Structural graph analysis — cycles, SCCs, topological order, articulation points, bridges, k-core, dependency layers.
allowed-tools:
  - mcp__mycelium__get_stats
  - mcp__mycelium__get_graph_metrics
  - mcp__mycelium__detect_cycles
  - mcp__mycelium__get_scc_groups
  - mcp__mycelium__topological_sort
  - mcp__mycelium__find_articulation_points
  - mcp__mycelium__find_bridge_edges
  - mcp__mycelium__get_biconnected_components
  - mcp__mycelium__get_k_core
  - mcp__mycelium__get_dependency_layers
  - mcp__mycelium__get_strongly_connected_components
  - mcp__mycelium__get_wcc
  - mcp__mycelium__get_degree_histogram
  - mcp__mycelium__find_cycle_members
  - mcp__mycelium__project_health
category: analysis
icon: 🕸️
marketplace_examples:
  - query: "Are there circular dependencies in my imports?"
    tool: mcp__mycelium__detect_cycles
  - query: "Find the strongly connected components in the call graph"
    tool: mcp__mycelium__get_scc_groups
  - query: "Topologically sort all modules by dependency order"
    tool: mcp__mycelium__topological_sort
---

# `graph-structure` — what shape is this graph?

This Skill bundles 14 structural-analysis tools. Where `centrality` ranks nodes by importance, `graph-structure` answers questions about the graph's overall shape: cycles, connected components, articulation points, layering.

Includes `detect_cycles` and `get_dependency_layers` — two of the top-10 most useful tools per the v0.1.1 external evaluation.

## When to invoke this Skill

Use **when**:

- The user asks "are there circular dependencies", "what's the build order", "what would split this codebase into modules".
- Code-health audits: cycle count, dead components, modularity scores.
- Migration planning: which parts of the codebase are safe to touch independently.

Do **NOT** use when:

- The user wants per-node scores → `centrality`.
- The user wants single-symbol relationships → `call-graph` or `reachability`.

## Quick examples

| Developer question | Tool |
|---|---|
| "Are there circular dependencies in my imports?" | `mcp__mycelium__detect_cycles` |
| "Find the strongly connected components in the call graph" | `mcp__mycelium__get_scc_groups` |
| "Topologically sort all modules by dependency order" | `mcp__mycelium__topological_sort` |
| "Which nodes are articulation points — removing one breaks the graph?" | `mcp__mycelium__find_articulation_points` |
| "Give me overall graph metrics for this codebase" | `mcp__mycelium__get_graph_metrics` |

## Capabilities under this umbrella

### `get_stats` — high-level summary

Node count, edge count, edge-kind histogram, language breakdown. The "is the index sane" sanity check.

```
mcp__mycelium__get_stats({})
→ { "nodes": 12453, "edges": 47821, "languages": ["rust", "python"], "by_edge_kind": { "calls": 28301, "imports": 11320, "extends": 213, "implements": 7987 } }
```

### `get_graph_metrics` — density, diameter, average degree

⚠️ Performance: see [#153](https://github.com/aimasteracc/mycelium/issues/153). On graphs > 5k nodes, may exceed default time budget — v0.1.4 adds `time_budget_ms`.

### `detect_cycles` ⭐ — find every cycle

The single most-recommended structural tool. Returns the list of cycles as ordered node sequences.

```
mcp__mycelium__detect_cycles({ "edge_kind": "imports", "limit": 50 })
→ { "cycles": [["src/a.py", "src/b.py", "src/a.py"], ...], "count": 3 }
```

### `get_scc_groups` — strongly connected components

A cycle in the graph corresponds to an SCC of size ≥ 2. Returns the components, sorted by size.

### `topological_sort` — dependency order

Returns nodes in topological order. Fails (or returns an error envelope) if the graph has cycles. Use `detect_cycles` first to confirm acyclicity.

### `find_articulation_points` — single-node bottlenecks

Nodes whose removal disconnects the graph. Useful for identifying refactor-risk choke points.

### `find_bridge_edges` — single-edge bottlenecks

Edges whose removal disconnects the graph. Often correspond to load-bearing imports or call chains.

### `get_biconnected_components` — robust modules

Maximal subgraphs with no articulation points internally. These are the "robust modules" of the codebase — chunks that stay connected even when individual nodes are removed.

### `get_k_core` — densely-connected subgraph

The k-core decomposition — useful for finding the densely-connected "hot" parts of the codebase.

### `get_dependency_layers` ⭐ — layered ordering

Stratifies nodes by dependency depth (0 = leaves, 1 = depends only on leaves, etc.). The build-order view. One of the top-10 most useful tools.

### `get_scc` — SCC membership for one symbol

Cheaper than `get_scc_groups` if you only need to know "which cycle does this symbol belong to".

### `get_wcc` — weakly connected components

⚠️ Performance: see [#153](https://github.com/aimasteracc/mycelium/issues/153).

### `get_degree_histogram` — degree distribution

⚠️ Performance: see [#153](https://github.com/aimasteracc/mycelium/issues/153).

### `find_cycle_members` — which symbols are in cycles

Given an edge kind, lists every symbol that's part of any cycle.

## Common chains

- **"Are there circular dependencies?"** → `detect_cycles --edge-kind imports`.
- **"What order should I build/load these?"** → `topological_sort` (after confirming no cycles).
- **"What does the project decompose into?"** → `get_scc_groups` + `get_biconnected_components`.
- **"Where are the bottlenecks?"** → `find_articulation_points` + `find_bridge_edges`.
- **"How deep is this code stack?"** → `get_dependency_layers`.

## Performance

Four tools (`get_graph_metrics`, `get_wcc`, `get_degree_histogram`, `find_articulation_points`) currently exceed budget on the 926-node Mycelium-core index per [#153](https://github.com/aimasteracc/mycelium/issues/153). v0.1.4 introduces `time_budget_ms` + partial-result envelopes.

## Equivalent CLI

```bash
mycelium detect-cycles --edge-kind imports --limit 50 --format=json
mycelium get-dependency-layers --format=json
mycelium topological-sort --edge-kind calls
```

## Parity contract

Per [RFC-0090](../../rfcs/0090-cli-mcp-skill-parity.md). `tests/parity.test.json` uses a 5-node fixture with a small cycle to exercise cycle / SCC / topo / articulation in one place.

## Cross-references

- Related Skill: `centrality` — for per-node importance scores.
- Related Skill: `call-graph`, `import-graph`, `inheritance` — for the edges that this Skill analyses.
