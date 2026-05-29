# Worked example: 30-second onboarding into a new codebase

**Scenario:** The user just cloned an open-source Rust project they've never seen and asks "what should I read first?"

**Step 1 — top files:**

```
mcp__mycelium__get_top_files({ "limit": 5 })
```

Returns the 5 most-leveraged files (high in-degree, central, large surface). These are the "skeleton" of the project.

```json
[
  { "file": "src/lib.rs", "score": 0.42 },
  { "file": "src/parser.rs", "score": 0.31 },
  { "file": "src/ast.rs", "score": 0.28 },
  { "file": "src/store.rs", "score": 0.24 },
  { "file": "src/cli/main.rs", "score": 0.19 }
]
```

The agent surfaces these to the user as the "read first" list.

**Step 2 — top symbols:**

```
mcp__mycelium__rank_symbols({ "limit": 10 })
```

Combines fan-in, betweenness, and PageRank into one ranking. The first few hits are usually:

- The main entry-point function
- Top-level public traits / interfaces
- The data structure types everyone touches

**Step 3 (optional) — bottlenecks:**

```
mcp__mycelium__betweenness_centrality({ "edge_kind": "calls", "limit": 5 })
```

These are the choke points. If you understand them, you understand the call flow. *Warning*: on 10K+ node graphs, this can take seconds. See [#153](https://github.com/aimasteracc/mycelium/issues/153) for performance bounds.

**Step 4 — hubs to watch when refactoring:**

```
mcp__mycelium__get_hub_symbols({ "limit": 10 })
```

High in-degree symbols. Changes here ripple through many callers.

**Follow-up:** Once the agent has the top files + symbols, it can chain into the `basic-queries` Skill (`get_symbol_info` for each top symbol) and surface kind + signature inline. Five tool calls give the user a real onboarding rather than a `ls`.
