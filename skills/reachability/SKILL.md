---
name: reachability
description: Multi-hop graph navigation — shortest paths, reachable sets, k-hop neighborhoods, cross-references.
allowed-tools:
  - mcp__mycelium__get_reachable
  - mcp__mycelium__get_reachable_to
  - mcp__mycelium__get_k_hop_neighbors
  - mcp__mycelium__get_two_hop_neighbors
  - mcp__mycelium__get_shortest_path
  - mcp__mycelium__get_symbol_neighborhood
  - mcp__mycelium__get_cross_refs
  - mcp__mycelium__get_outgoing_refs
  - mcp__mycelium__get_dependency_depth
  - mcp__mycelium__get_reachable_set
  - mcp__mycelium__get_reaches_into
  - mcp__mycelium__get_singly_referenced
  - mcp__mycelium__get_mutual_reachability
  - mcp__mycelium__get_common_reachable
---

# `reachability` — multi-hop navigation across edge kinds

This Skill is the multi-hop layer. `call-graph` and `import-graph` cover direct relationships and trees on a single edge kind; this Skill answers the harder questions: shortest paths between two symbols, all symbols reachable from a starting set, k-hop neighborhoods.

Includes `get_shortest_path` — one of the top-3 most useful tools per the v0.1.1 external evaluation.

## When to invoke this Skill

Use **when**:

- The user asks "how does X reach Y", "what's the chain from X to Y".
- Building a refactor or debugging plan that needs blast-radius across edge kinds.
- Picking a starting set of symbols for a deeper analysis.

Do **NOT** use when:

- The relationship is single-edge-kind direct → use `call-graph` or `import-graph` (cheaper).
- The user wants ranking/aggregates → use `centrality`.

## Capabilities under this umbrella

### `get_shortest_path` — concrete call/ref chain between two symbols ⭐

The single highest-leverage tool in this Skill. Returns the actual symbol-to-symbol chain, not just a reachability flag.

```
mcp__mycelium__get_shortest_path({
  "from": "src/api/handler.rs>handle_login",
  "to": "src/db.rs>users>find_by_email",
  "edge_kind": "calls"
})
→ { "path": ["src/api/handler.rs>handle_login", "src/auth/session.rs>AuthService>login", "src/db.rs>users>find_by_email"], "length": 3 }
```

Returns `{ "path": null }` if no path exists.

### `get_reachable` — forward reachable set

"If I start here and follow `edge_kind` edges, where can I get?"

```
mcp__mycelium__get_reachable({
  "path": "src/main.rs>main",
  "edge_kind": "calls",
  "max_depth": 10
})
```

### `get_reachable_to` — backward reachable set

"Which symbols can reach this one by following `edge_kind` edges?" Complement of `get_reachable`.

### `get_k_hop_neighbors` / `get_two_hop_neighbors`

Bounded-depth neighborhoods. `get_two_hop_neighbors` is a fast path for `k=2`. Both return symbols at *exactly* k hops away (not within k).

### `get_symbol_neighborhood` — a dense local view

Returns the symbol's direct neighbors across all four edge kinds (calls, imports, extends, implements), in/out, with counts. Useful for "give me a one-call snapshot of this symbol's surroundings".

### `get_cross_refs` — all incoming references (any edge kind)

```
mcp__mycelium__get_cross_refs({ "path": "src/auth/session.rs>AuthService>login" })
→ { "callers": [...], "importers": [...], "subclasses": [...], "implementors": [...] }
```

Tied for top-10 most useful. One call replaces four separate `get_<edge>_to` calls.

### `get_outgoing_refs` — all outgoing references (any edge kind)

Symmetric counterpart of `get_cross_refs`.

### `get_dependency_depth` — distance from leaves

Returns the longest path from the given symbol down to a leaf (no outgoing `edge_kind` edges). Useful as a complexity proxy.

### `get_reachable_set` — reach from a multi-symbol seed

"What's collectively reachable from any of these starting points" — accepts up to 20 paths, deduplicates the union.

### `get_reaches_into` — which paths reach into the given subtree

Counterpart of `get_reachable_set` but path-prefix-scoped: "which symbols reach into anything under `src/auth/`?"

### `get_singly_referenced` — symbols with exactly one incoming edge

Useful for identifying tightly-coupled callers — symbols whose sole consumer is one specific other symbol.

## Common chains

- **"Connect these two dots"** → `get_shortest_path` with the appropriate `edge_kind`.
- **"What does this transitively touch?"** → `get_reachable` with depth bound.
- **"What's the blast radius across all edge kinds?"** → `get_cross_refs` (incoming) + `get_outgoing_refs` (outgoing).
- **"One-call neighborhood snapshot"** → `get_symbol_neighborhood`.

## Equivalent CLI

```bash
mycelium get-shortest-path --from "A" --to "B" --edge-kind calls --format=json
mycelium get-reachable "src/main.rs>main" --edge-kind calls --max-depth 10
mycelium get-cross-refs "src/auth/session.rs>AuthService>login" --format=json
```

## Parity contract

Per [RFC-0090](../../rfcs/0090-cli-mcp-skill-parity.md). `tests/parity.test.json` has one case per capability against a small fixture graph that exercises all four edge kinds.

## Cross-references

- Related Skill: `call-graph`, `import-graph`, `inheritance` (planned) — for single-edge-kind queries.
- Related Skill: `centrality` (planned) — when the user wants importance scores, not connectivity.
- Related Skill: `batch-ops` (planned) — for `batch_reachable_from` / `batch_reachable_to`.
