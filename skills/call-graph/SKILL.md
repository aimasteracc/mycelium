---
name: call-graph
description: Navigate the call graph — callers, callees, call trees, entry points, dead and isolated symbols.
allowed-tools:
  - mcp__mycelium__get_callees
  - mcp__mycelium__get_callers
  - mcp__mycelium__get_callee_tree
  - mcp__mycelium__get_caller_tree
  - mcp__mycelium__get_entry_points
  - mcp__mycelium__get_dead_symbols
  - mcp__mycelium__get_isolated_symbols
---

# `call-graph` — who calls who, and what stays in the dark

This Skill bundles the seven `Calls` edge-kind tools. Reach for it any time the user asks a "what calls X" or "who can reach Y" question, or wants reachability-based code-health diagnostics (dead code, entry points, isolated functions).

## When to invoke this Skill

Use **when**:

- The user asks "who calls `X`", "what does `X` call", "is `X` reachable", "what dies if I delete `X`".
- The agent is planning a refactor and needs blast-radius information.
- The user wants code-health views: entry points, dead symbols, isolated nodes.

Do **NOT** use when:

- The relationship is `Imports`, `Extends`, `Implements`, etc. — use `import-graph`, `inheritance` (planned).
- The user wants a *ranking* of callees by importance — see `centrality`.
- The user wants multi-hop set reachability across edge kinds — see `reachability` (planned).

## Capabilities under this umbrella

### `get_callees` — direct callees of a symbol

```
mcp__mycelium__get_callees({ "path": "src/auth/session.rs>AuthService>login" })
→ { "callees": ["src/db.rs>users>find_by_email", "src/crypto.rs>verify_password", ...], "count": 5 }
```

### `get_callers` — direct callers of a symbol

```
mcp__mycelium__get_callers({ "path": "src/auth/session.rs>AuthService>login" })
→ { "callers": ["src/api/routes.rs>handle_login", "src/cli/main.rs>cli_login"], "count": 2 }
```

### `get_callee_tree` — recursive callee tree

**When**: "what does my function transitively depend on?" — gives a tree, not a flat set.

```
mcp__mycelium__get_callee_tree({ "path": "src/auth/session.rs>AuthService>login", "max_depth": 3 })
```

Returns a nested `{ path, children: [...] }` structure. Default depth 3, capped at 10. Cycles are detected and rendered as `{ path, cycle: true }` leaves.

### `get_caller_tree` — recursive caller tree

**When**: "who transitively depends on my function?" — the refactor blast-radius view.

```
mcp__mycelium__get_caller_tree({ "path": "src/auth/session.rs>AuthService>login", "max_depth": 3 })
```

### `get_entry_points` — symbols with no incoming `Calls` edges

**When**: "what are the roots of the call graph?" — main functions, CLI commands, HTTP handlers. Combined with `get_dead_symbols`, gives a project-shape overview.

```
mcp__mycelium__get_entry_points({ "limit": 100 })
```

### `get_dead_symbols` — symbols with no incoming OR outgoing `Calls` edges

**When**: "what's unreachable from anywhere?" — dead-code detection. Note: test code often appears here because tests don't call each other. Filter with `--exclude-paths "tests/"` if you want production-only dead code.

```
mcp__mycelium__get_dead_symbols({ "exclude_paths": ["tests/"], "limit": 200 })
```

### `get_isolated_symbols` — singleton nodes in the call graph

**When**: similar to `get_dead_symbols` but stricter — symbols with zero edges of any kind. Often signals unused utility functions or generated code.

## Common chains

- **"Where is this code reached from?"** → `get_callers` → `get_caller_tree --max-depth 5`.
- **"What does this function transitively touch?"** → `get_callees` → `get_callee_tree --max-depth 5`.
- **"Is this code dead?"** → `get_callers` (empty?) + `get_callees` (empty?) → confirm via `get_dead_symbols` listing.
- **"What's the project's entry point?"** → `get_entry_points --limit 50`.

## Equivalent CLI

```bash
mycelium get-callers "src/auth/session.rs>AuthService>login" --format=json
mycelium get-caller-tree "src/auth/session.rs>AuthService>login" --max-depth 3
mycelium get-dead-symbols --exclude-paths tests/ --limit 200
```

## Parity contract

Per [RFC-0090](../../rfcs/0090-cli-mcp-skill-parity.md): each of the 7 CLI ↔ MCP pairs is byte-identical in name, description, argument schema, and JSON output. `tests/parity.test.json` asserts byte-equality for one input per capability against a fixture project with a small call graph.

## Cross-references

- Related Skill: `basic-queries` — to find the symbol path first.
- Related Skill: `import-graph` (planned) — for `Imports` edge traversal.
- Related Skill: `centrality` (planned) — for "most called" / "most calling" rankings instead of raw sets.
- Related Skill: `graph-structure` (planned) — for cycle detection and dependency layers in the call graph.
- Implementation: `crates/mycelium-mcp/src/lib.rs` (search for `mycelium_get_callers`, etc.).
