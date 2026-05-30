---
name: basic-queries
description: Look up symbols, kinds, ancestors, descendants, siblings — the foundation tools any agent reaches for first.
allowed-tools:
  - mcp__mycelium__search_symbol
  - mcp__mycelium__get_symbol_info
  - mcp__mycelium__get_ancestors
  - mcp__mycelium__get_descendants
  - mcp__mycelium__get_node_kind
  - mcp__mycelium__get_symbols_by_kind
  - mcp__mycelium__get_source_span
  - mcp__mycelium__get_siblings
  - mcp__mycelium__get_all_symbols
  - mcp__mycelium__server_status
  - mcp__mycelium__get_files
  - mcp__mycelium__get_node_degree
  - mcp__mycelium__get_symbol_count_by_kind
category: navigation
icon: 🔍
marketplace_examples:
  - query: "Find all symbols named login"
    tool: mcp__mycelium__search_symbol
  - query: "What symbols are defined in src/auth/session.rs?"
    tool: mcp__mycelium__get_all_symbols
  - query: "What kind of symbol is AuthService?"
    tool: mcp__mycelium__get_node_kind
  - query: "What file and line range does render live at?"
    tool: mcp__mycelium__get_source_span
  - query: "List every function in the project"
    tool: mcp__mycelium__get_symbols_by_kind
---

# `basic-queries` — the foundation layer

This Skill bundles the 10 lowest-level capabilities — the ones an agent reaches for *before* deciding whether to do a graph traversal, a Hyphae query, or a centrality analysis. Every higher-level Skill assumes the agent already knows how to use these.

If you're new to Mycelium, this is the first Skill to load.

## When to invoke this Skill

Use **when**:

- The user names a specific symbol (`get_user`, `AuthService.login`) and you need to verify it exists and learn its kind, location, or context.
- You're starting any multi-step analysis and need an entry-point symbol.
- The user asks "what's in this file/class/module?" — these tools give you the structural inventory.

Do **NOT** use when:

- The user wants *set-based* matching by pattern → use the `hyphae-query` Skill.
- The user asks "what calls X" → use the `call-graph` Skill.
- The user wants ranking, centrality, or aggregate statistics → those are different Skills.

## Quick examples

| Developer question | Tool |
|---|---|
| "Find all symbols named login" | `mcp__mycelium__search_symbol` |
| "What symbols are defined in src/auth/session.rs?" | `mcp__mycelium__get_all_symbols` |
| "What kind of symbol is AuthService?" | `mcp__mycelium__get_node_kind` |
| "What file and line range does render live at?" | `mcp__mycelium__get_source_span` |
| "List every function in the project" | `mcp__mycelium__get_symbols_by_kind` |

## Capabilities under this umbrella

### `search_symbol` — find symbols by name fragment

**When**: agent has a name or partial name and needs to discover concrete symbol paths.

```
mcp__mycelium__search_symbol({ "query": "login", "limit": 20 })
```

**Result**: array of full symbol paths whose final segment contains the query, case-insensitive. Sorted lexicographically.

### `get_symbol_info` — one-shot symbol summary

**When**: most useful general-purpose tool. Returns kind, file, source span, language, parent chain, and degree counts for one symbol in a single call.

```
mcp__mycelium__get_symbol_info({ "path": "src/auth/session.rs>AuthService>login" })
```

### `get_ancestors` — walk up the containment chain

**When**: agent has a deeply-nested symbol path and wants to know its container chain (method → class → file → module).

```
mcp__mycelium__get_ancestors({ "path": "src/a.rs>App>render" })
→ ["src/a.rs>App", "src/a.rs"]
```

### `get_descendants` — walk down the containment chain

**When**: agent has a container and wants every symbol nested inside (recursive). For *direct* children only, use `get_siblings` on a known child instead.

```
mcp__mycelium__get_descendants({ "path": "src/a.rs>App" })
→ ["src/a.rs>App>init", "src/a.rs>App>render", ...]
```

### `get_node_kind` — what kind of symbol is this

**When**: agent has a path string but doesn't know if it's a function, class, module, etc. Cheaper than `get_symbol_info` when you only need the kind.

```
mcp__mycelium__get_node_kind({ "path": "src/a.rs>App" })
→ "class"
```

### `get_symbols_by_kind` — every symbol of one kind

**When**: agent wants "every function in the project" or "every class in `src/auth/`" — a kind-only filter.

```
mcp__mycelium__get_symbols_by_kind({ "kind": "function", "limit": 100 })
```

For combined kind + name patterns, prefer the `hyphae-query` Skill (one DSL call vs. a fetch-then-filter loop).

### `get_source_span` — get the file + line range

**When**: agent needs to surface code to the user, or to chain into a file-reader. Returns `{file, start_line, end_line}` for the symbol.

```
mcp__mycelium__get_source_span({ "path": "src/a.rs>App>render" })
```

### `get_siblings` — direct children of the same parent

**When**: agent wants "what else is in this class/file?" — the symbols that share a direct parent with the given path. Excludes the path itself. Root nodes return empty.

```
mcp__mycelium__get_siblings({ "path": "src/a.rs>App>render" })
→ ["src/a.rs>App>init", "src/a.rs>App>destroy"]
```

### `get_all_symbols` — the whole inventory

**When**: rare — only for global-scope analyses. Returns every symbol path in the index, optionally filtered by path-prefix or kind. Can be large.

```
mcp__mycelium__get_all_symbols({ "path_prefix": "src/auth/", "kind": null, "limit": 1000 })
```

### `server_status` — the index is healthy

**When**: before starting any analysis, confirm the index loaded. Returns engine version, node/edge counts, last-load source.

```
mcp__mycelium__server_status({})
→ { "version": "0.1.3", "nodes": 12453, "edges": 47821, "languages": ["rust", "python", ...] }
```

## Equivalent CLI

Each capability has a matching `mycelium <cap>` subcommand with identical name, description, and JSON output. Examples:

```bash
mycelium search-symbol login --limit 20
mycelium get-symbol-info "src/auth/session.rs>AuthService>login"
mycelium get-descendants "src/a.rs>App" --format=json
```

## Parity contract

This Skill covers the 10 listed capabilities per [RFC-0090](../../rfcs/0090-cli-mcp-skill-parity.md):

- Each CLI ↔ MCP pair is byte-identical in name, description, argument schema, and JSON output.
- `tests/parity.test.json` asserts byte-equality for at least one input per capability.

## Cross-references

- Related Skill: `hyphae-query` — when the agent wants set-based pattern matching instead of one-shot lookups.
- Related Skill: `call-graph` — when the agent wants relationship traversal (callers, callees).
- Related Skill: `reachability` (planned) — for multi-hop graph navigation.
- Implementation: search the codebase for `mycelium_get_symbol_info`, `mycelium_search_symbol`, etc. in `crates/mycelium-mcp/src/lib.rs`.
