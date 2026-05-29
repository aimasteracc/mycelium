---
name: batch-ops
description: Batch-query equivalents of common single-symbol tools — one MCP call instead of 20, for token efficiency.
allowed-tools:
  - mcp__mycelium__batch_symbol_info
  - mcp__mycelium__batch_node_degree
  - mcp__mycelium__batch_reachable_from
  - mcp__mycelium__batch_reachable_to
---

# `batch-ops` — N symbols, 1 round trip

This Skill bundles the batched variants of common single-symbol tools. The point is **token efficiency**: when an agent has 10–20 symbols to inspect, one batch call beats 10–20 individual calls.

Charter §2 SLA targets ≤ 30% of the JSON-MCP token count for graph queries — batch tools are a major part of how Mycelium hits that target.

## When to invoke this Skill

Use **when**:

- The agent already has a *set* of symbol paths (e.g., from a Hyphae query, a search, or a previous tool's output) and needs to inspect them.
- The repeated single calls would otherwise hit the same store for the same kind of data.
- The user-facing latency budget is tight.

Do **NOT** use when:

- You have one symbol — the single-call tool is simpler and cheaper.
- The set is larger than 50 — split and call in chunks.

## Capabilities under this umbrella

### `batch_symbol_info` — N symbols' info in one call

Single-call equivalent of N × `get_symbol_info`. Returns a list of info records keyed by path. Unknown paths get `{ "path": "...", "error": "not found" }` entries — no partial-call failures.

```
mcp__mycelium__batch_symbol_info({ "paths": ["src/a.rs>App", "src/a.rs>App>render", "..."] })
```

Cap: 50 paths.

### `batch_node_degree` — N symbols' degree breakdown in one call

Single-call equivalent of N × `get_node_degree`. Returns the four-edge-kind in/out degree counts per symbol.

```
mcp__mycelium__batch_node_degree({ "paths": ["src/a.rs>App>render", "src/b.rs>Service"] })
→ [
    { "path": "src/a.rs>App>render", "in_calls": 3, "out_calls": 5, "in_imports": 0, ... },
    { "path": "src/b.rs>Service",    "in_calls": 1, "out_calls": 8, "in_imports": 12, ... }
  ]
```

Cap: 50 paths.

### `batch_reachable_from` — union of reachable sets from multiple seeds

Single-call equivalent of "for each seed, compute the reachable set, then union". Excludes the seed paths from the result.

```
mcp__mycelium__batch_reachable_from({
  "paths": ["src/main.rs>main", "src/cli.rs>cli_main"],
  "edge_kind": "calls",
  "max_depth": 10
})
→ { "reachable": [...], "count": N }
```

Cap: 20 paths. `max_depth` defaults to 10, capped at 20.

### `batch_reachable_to` — union of "who reaches these"

The reverse of `batch_reachable_from`. Returns the union of every symbol that can reach any of the seeds.

```
mcp__mycelium__batch_reachable_to({
  "paths": ["src/db.rs>users>find_by_email", "src/db.rs>users>create"],
  "edge_kind": "calls",
  "max_depth": 10
})
```

Cap: 20 paths.

## Common chains

- **Hyphae query → batch enrich**: `query .function` returns matches → `batch_symbol_info` enriches all matches in one call.
- **Top-N ranking → batch inspect**: `rank_symbols --limit 10` → `batch_symbol_info` on the 10.
- **Multi-entrypoint reachability**: `batch_reachable_from` on every entry-point.

## Equivalent CLI

```bash
mycelium batch-symbol-info --paths "src/a.rs>App,src/a.rs>App>render" --format=json
mycelium batch-reachable-from --paths "src/main.rs>main,src/cli.rs>cli_main" --edge-kind calls --max-depth 10
```

CLI accepts comma-separated paths in a single `--paths` arg or repeated `--path` flags.

## Parity contract

Per [RFC-0090](../../rfcs/0090-cli-mcp-skill-parity.md). `tests/parity.test.json` asserts byte-equality for one batch input per capability against the diamond fixture (reused from `reachability` Skill).

## Cross-references

- Related Skill: `basic-queries` — for single-symbol equivalents of these batch tools.
- Related Skill: `reachability` — for single-seed reachable / reachable-to variants.
- Related Skill: `hyphae-query` — common upstream that feeds match sets into batch tools.
