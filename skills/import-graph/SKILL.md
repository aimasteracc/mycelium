---
name: import-graph
description: Navigate the import graph — direct imports, import trees, importers (who pulls this in).
allowed-tools:
  - mcp__mycelium__get_imports
  - mcp__mycelium__get_import_tree
  - mcp__mycelium__get_importers_tree
---

# `import-graph` — module dependencies, in both directions

This Skill bundles the three `Imports` edge-kind tools. Reach for it when the question is "what does this file pull in" or "who depends on this module".

Particularly useful for TypeScript, Python, JavaScript projects where the import statements are the load-bearing dependency contract. Rust projects have implicit `use` statements that this Skill also tracks — but you'll get more value out of `call-graph` for Rust.

## When to invoke this Skill

Use **when**:

- The user asks "who imports `X`", "what does `X` import", "is this module used".
- Planning a module split — you need to see the import surface clearly.
- Tracking down a circular import.

Do **NOT** use when:

- The relationship is `Calls` (use `call-graph`), `Extends`/`Implements` (use `inheritance`).
- The user wants symbol-level not module-level relationships — drop down to `call-graph`.

## Capabilities under this umbrella

### `get_imports` — direct imports of a file/module

```
mcp__mycelium__get_imports({ "path": "src/api/routes.rs" })
→ { "imports": ["src/auth/session.rs", "src/db.rs", "axum"], "count": 3 }
```

External crates (no path in the local index) appear as bare identifiers.

### `get_import_tree` — recursive imports

**When**: "what does this file transitively pull in" — depth-bounded tree.

```
mcp__mycelium__get_import_tree({ "path": "src/main.rs", "max_depth": 3 })
```

Returns a nested `{ path, children: [...] }` structure. Cycles render as `{ path, cycle: true }` leaves. Default depth 3, capped at 10.

### `get_importers_tree` — recursive who-imports-me

**When**: "who depends on this module, directly or transitively" — the import-edge analog of `get_caller_tree`.

```
mcp__mycelium__get_importers_tree({ "path": "src/auth/session.rs", "max_depth": 3 })
```

## Common chains

- **"Can I move this file?"** → `get_importers_tree` to see the full set of files that would need updating.
- **"What's this module's external dependency surface?"** → `get_imports` and filter for external (non-path) entries.
- **"Find a circular import"** → `get_import_tree` and look for `cycle: true` leaves.

## Equivalent CLI

```bash
mycelium get-imports "src/api/routes.rs" --format=json
mycelium get-import-tree "src/main.rs" --max-depth 3
mycelium get-importers-tree "src/auth/session.rs" --max-depth 3
```

## Parity contract

Per [RFC-0090](../../rfcs/0090-cli-mcp-skill-parity.md): each CLI ↔ MCP pair is byte-identical. `tests/parity.test.json` asserts byte-equality for one input per capability.

## Cross-references

- Related Skill: `call-graph` — when the relationship is `Calls` not `Imports`.
- Related Skill: `inheritance` (planned) — for `Extends` / `Implements`.
- Related Skill: `reachability` — for multi-edge-kind reachability.
