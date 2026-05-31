---
name: centrality
description: Rank symbols by importance — PageRank, betweenness, fan-in/out, hub detection, top-file diagnostics.
allowed-tools:
  - mcp__mycelium__rank_symbols
  - mcp__mycelium__get_top_files
  - mcp__mycelium__get_most_connected
  - mcp__mycelium__get_hub_symbols
  - mcp__mycelium__get_fan_out_rank
  - mcp__mycelium__get_fan_in_rank
  - mcp__mycelium__get_betweenness_centrality
  - mcp__mycelium__get_closeness_centrality
  - mcp__mycelium__get_degree_centrality
  - mcp__mycelium__get_clustering_coefficient
  - mcp__mycelium__get_eccentricity
  - mcp__mycelium__page_rank
  - mcp__mycelium__get_harmonic_centrality
  - mcp__mycelium__get_neighbor_similarity
category: analysis
icon: ⭐
marketplace_examples:
  - query: "Which symbols are most critical to this codebase?"
    tool: mcp__mycelium__rank_symbols
  - query: "What are the top 10 most-connected files?"
    tool: mcp__mycelium__get_top_files
  - query: "Rank all functions by fan-in"
    tool: mcp__mycelium__get_fan_in_rank
---

# `centrality` — who matters in this codebase

This Skill bundles 14 ranking and importance-scoring tools. Where `call-graph` and `reachability` answer "what's connected to what", centrality answers "*which of those connections matter most*".

Includes `rank_symbols` and `get_top_files` — two of the top-10 most useful tools per the v0.1.1 external evaluation.

## When to invoke this Skill

Use **when**:

- The user asks "what are the most important symbols in this codebase", "which files do I need to know first", "where are the hubs".
- Onboarding into an unfamiliar project — surface the top-ranked files and symbols first.
- Identifying refactor pressure points — high-betweenness symbols are bottlenecks.

Do **NOT** use when:

- The user wants raw connectivity, not scores → use `call-graph` or `reachability`.
- The user wants a specific symbol's neighborhood → use `get_symbol_neighborhood` from `reachability`.

## Quick examples

| Developer question | Tool |
|---|---|
| "Which symbols are most critical to this codebase?" | `mcp__mycelium__rank_symbols` |
| "What are the top 10 most-connected files?" | `mcp__mycelium__get_top_files` |
| "Rank all functions by fan-in" | `mcp__mycelium__get_fan_in_rank` |
| "Compute PageRank for this codebase" | `mcp__mycelium__page_rank` |
| "Find hub symbols with high in-degree" | `mcp__mycelium__get_hub_symbols` |

## Capabilities under this umbrella

### `rank_symbols` ⭐ — overall importance ranking

The single most-recommended tool for "what should I look at first" in any codebase. Combines centrality scores into one ranking.

```
mcp__mycelium__rank_symbols({ "limit": 20 })
```

### `get_top_files` ⭐ — most important files

Same idea, file-level. Surfaces the highest-leverage files for onboarding or audit.

```
mcp__mycelium__get_top_files({ "limit": 10 })
```

Caveat: a file appearing here may be a "god file" — high importance can mean high coupling, which is often a smell.

### `get_most_connected` — total-degree ranking

Symbols with the most edges across all kinds. Closely related to `degree_centrality` but conveniently sorted descending and limited.

### `get_hub_symbols` — high in-degree

Symbols that everyone depends on. Reverse of `get_dead_symbols` from `call-graph`. Combine with `get_top_files` to find the load-bearing parts of the codebase.

### `get_fan_out_rank` — symbols that call many things

Useful for "what's the most coupling-heavy code" — high fan-out often means a coordinator or orchestrator. Sometimes a refactor target.

### `get_fan_in_rank` — symbols that many things call

Inverse of fan-out. Symbols here are the most leveraged — changes break the most callers.

### `betweenness_centrality` — bottleneck detection

Returns the number of shortest paths passing through each symbol. High betweenness = bottleneck; if it breaks, many flows are disrupted.

```
mcp__mycelium__betweenness_centrality({ "edge_kind": "calls", "limit": 20 })
```

Expensive on large graphs — see [#153](https://github.com/aimasteracc/mycelium/issues/153) for performance bounds.

### `closeness_centrality` — distance from everything

Symbols with low average distance to every other symbol. The "well-connected" middle of the graph.

### `degree_centrality` — raw in/out degree

The simplest centrality. Often a good first pass before reaching for more expensive measures.

### `clustering_coefficient` — local triangle density

How tightly clustered is a symbol's neighborhood? High = cohesive module; low = bridge.

### `eccentricity` — farthest distance to any other symbol

The maximum shortest-path distance from the symbol to any other. Eccentricity is high for symbols "on the edge" of the call graph.

### `page_rank` — PageRank with edge kinds

Classic PageRank, edge-kind-parameterized. Heavier than `degree_centrality` but more informative for asymmetric graphs.

Expensive on large graphs — see [#153](https://github.com/aimasteracc/mycelium/issues/153).

### `harmonic_centrality` — robust closeness

Closeness centrality variant that gracefully handles disconnected graphs. Use this instead of `closeness_centrality` if the project has unreachable subgraphs.

### `neighbor_similarity` — find related symbols

Given a symbol, returns symbols with overlapping neighborhoods. Useful for "find me other things like this".

## Common chains

- **"Where do I start understanding this codebase?"** → `get_top_files` then `rank_symbols`.
- **"Where will breakage cascade if I change something?"** → `get_fan_in_rank` (downstream blast) + `betweenness_centrality` (cross-cut bottlenecks).
- **"Find similar code"** → `neighbor_similarity`.

## Performance notes

`page_rank`, `betweenness_centrality`, `wcc`-class algorithms are heavy on large graphs. Issue [#153](https://github.com/aimasteracc/mycelium/issues/153) tracks this; v0.1.4 introduces partial-result envelopes and a `time_budget_ms` parameter.

## Equivalent CLI

```bash
mycelium rank-symbols --limit 20 --format=json               # top callee symbols (Calls)
mycelium rank-symbols --edge-kind imports --limit 20          # most-imported symbols
mycelium rank-symbols --edge-kind extends --limit 10          # most-extended base classes
mycelium get-top-files --limit 10
mycelium page-rank --edge-kind calls --limit 20
```

## Parity contract

Per [RFC-0090](../../rfcs/0090-cli-mcp-skill-parity.md). `tests/parity.test.json` asserts byte-equality on a small fixture for each capability.

## Cross-references

- Related Skill: `call-graph`, `reachability` — for the connectivity these tools rank.
- Related Skill: `graph-structure` (planned) — for cycle and SCC detection that often combines with centrality.
