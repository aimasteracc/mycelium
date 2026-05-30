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
  - mcp__mycelium__get_leaf_symbols
  - mcp__mycelium__find_call_path
  - mcp__mycelium__get_common_callers
  - mcp__mycelium__get_common_callees
category: analysis
icon: 📞
marketplace_examples:
  - query: "Who calls the login function?"
    tool: mcp__mycelium__get_callers
  - query: "What does AuthService.login transitively call?"
    tool: mcp__mycelium__get_callee_tree
  - query: "What are the entry points of this project?"
    tool: mcp__mycelium__get_entry_points
  - query: "What functions are dead code?"
    tool: mcp__mycelium__get_dead_symbols
  - query: "Is there a call path from cli_main to verify_password?"
    tool: mcp__mycelium__find_call_path
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

## Quick examples

| Developer question | Tool |
|---|---|
| "Who calls the login function?" | `mcp__mycelium__get_callers` |
| "What does AuthService.login transitively call?" | `mcp__mycelium__get_callee_tree` |
| "What are the entry points of this project?" | `mcp__mycelium__get_entry_points` |
| "What functions are dead code?" | `mcp__mycelium__get_dead_symbols` |
| "Is there a call path from cli_main to verify_password?" | `mcp__mycelium__find_call_path` |

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

## Known limitation: autouse conftest fixtures inflate caller counts

In pytest projects, `conftest.py` fixtures decorated with `autouse=True` run around every
test and commonly import and call methods across many modules (resetting singletons, clearing
caches, etc.). Mycelium records all of these as static call edges, so **every test file
appears to call every method touched by an autouse fixture** — even if that test file has
nothing to do with those methods.

Practical impact:
- `get_callers` caller counts are inflated for any symbol touched by autouse fixtures.
- `get_dead_symbols` / `get_isolated_symbols` may report code as "live" only because an
  autouse fixture reaches it, making dead-code detection unreliable.
- `get_callers "conftest.py>reset_singletons"` may show hundreds of test-file callers,
  obscuring actual direct callers.

**Workaround**: use `--exclude-paths "tests/"` (CLI) or `exclude_paths: ["tests/"]` (MCP)
to restrict analysis to production code. For true test-coverage measurement (which tests
actually exercise which code paths), use a runtime tool such as `pytest-cov` — mycelium's
static call graph cannot distinguish "transitively reachable through autouse fixtures" from
"directly exercised by this test".

This is a static analysis limitation, not a mycelium bug (issue #269).

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
