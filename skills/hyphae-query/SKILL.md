---
name: hyphae-query
description: Execute a Hyphae DSL selector against the project's index.
allowed-tools:
  - mcp__mycelium__query
---

# `hyphae-query` — query the Mycelium symbol graph with one DSL expression

This Skill is the umbrella for **one** capability (`query`) because Hyphae is itself a query language — every selector the user might want to write is the same MCP tool with different arguments. Bundling more capabilities here would be misleading.

This is the canonical *example* of a single-capability Skill under the Three-Surface Rule (RFC-0090). It exists to prove the rule and to teach the agent the marquee Mycelium feature.

## When to invoke this Skill

Use **when**:

- The user wants a *set* of symbols matching a structural pattern, not a single named symbol.
- The agent has already indexed a project (`.mycelium/index.rmp` exists) or just ran `mycelium index`.
- The query shape is naturally CSS-selector-style: a name, a kind, a parent-child relationship, or a combination.

Do **NOT** use when:

- The user asks for *one* specific symbol by exact name → reach for `mycelium_search_symbol` (`basic-queries` Skill) which is more focused.
- The user asks "what calls X" — that's a call-graph traversal, use `mycelium_get_callers` from the `call-graph` Skill.
- The project hasn't been indexed yet — first run `mycelium index <root>` (will be wrapped in `indexing` Skill).

## Capabilities under this umbrella

### `query` — execute a Hyphae selector

**When**: structural multi-match questions like "all functions in `src/auth/`", "every class with a `render` method", "callers of `get_user` that aren't in tests".

**MCP invocation:**

```
mcp__mycelium__query({
  "expr": "#login"          // name selector — matches symbols named `login`
})
```

```
mcp__mycelium__query({
  "expr": ".function"       // kind selector — matches all function symbols
})
```

```
mcp__mycelium__query({
  "expr": ".class>.method"  // direct-child combinator — methods of classes
})
```

**Equivalent CLI (for human reading or shell scripting):**

```
mycelium query "#login"
mycelium query ".function" --format=json
mycelium query ".class>.method" --root /path/to/project
```

**Result shape:**

```json
{ "matches": ["src/a.rs>login", "src/b.rs>login"], "count": 2 }
```

The CLI text format prints one match per line. The CLI `--format=json` is a JSON array of strings — equivalent to the MCP `matches` field, asserted byte-for-byte by `tests/parity.test.json`.

**Result on parse error:**

```json
{ "error": "hyphae parse error: …" }
```

The agent should retry with a corrected selector. The `error` field always contains the literal substring `hyphae` for easy detection.

**Typical follow-ups:**

- Got a too-large result set? Refine with `:in(path-pattern)` or kind selectors.
- Need the caller relationships, not just the match set? Pass each match into `mycelium_get_callers` (`call-graph` Skill).
- Need source for one match? `mycelium_get_symbol_info` (`basic-queries` Skill).

## Hyphae DSL cheat-sheet

### Base selectors

| Syntax | Meaning |
|---|---|
| `#name` | Symbol named `name` (any kind, any depth). |
| `.kind` | All symbols of a given kind: `.function`, `.class`, `.method`, `.module`, `.struct`, `.enum`, `.interface`/`.trait`, `.constant`/`.const`, `.type`, `.variable`. |
| `*` | Universal — every indexed symbol. |

### Combinators

| Syntax | Meaning |
|---|---|
| ` ` (space) | Descendant combinator — any descendant. |
| `>` | Direct child combinator. |
| `~` | Sibling combinator (same parent). |
| `,` | Selector list — union. |

### Relationship pseudo-classes (RFC-0003)

| Syntax | Meaning |
|---|---|
| `:calls(X)` | Outgoing `Calls` edge to a node matching `X`. |
| `:callers(X)` | Incoming `Calls` edge from a node matching `X`. |
| `:imports(X)` | Outgoing `Imports` edge. |
| `:extends(X)` | Outgoing `Extends` edge. |
| `:implements(X)` | Outgoing `Implements` edge (RFC-0091). |

### jQuery-style pseudo-classes (RFC-0091)

| Syntax | Meaning |
|---|---|
| `:not(X)` | Set-difference — candidates NOT matching `X`. |
| `:has(X)` | Candidates that contain at least one descendant matching `X`. |
| `:in(path-prefix)` | Candidates whose path starts with the given prefix. |
| `:first-child` / `:last-child` / `:only-child` | Positional filters within siblings. |
| `:nth-child(N)` | 1-indexed sibling position. |

### Attribute selectors (RFC-0091)

| Syntax | Meaning |
|---|---|
| `[language=rust]` | Filter by source language (`rust`, `python`, `typescript`, `javascript`, `go`, `java`, `c`, `cpp`, `csharp`, `ruby`). |
| `[kind=function]` | Filter by `NodeKind` wire string. |
| `[file=src/lib.rs]` | Filter by file path. |

### Composition examples

```
.function:not(#main):in(src/auth/)
.class:has(.method:calls(#log))[language=python]
.struct[file=src/lib.rs]:implements(#Repository)
.method:first-child:in(src/handlers/)
*:nth-child(3)
```

Full grammar: [RFC-0003](../../rfcs/0003-hyphae-query-language.md) + [RFC-0091](../../rfcs/0091-hyphae-jquery-selectors.md).

## Parity contract

This Skill, the CLI `mycelium query`, and the MCP tool `mycelium_query` are 1:1 per [RFC-0090](../../rfcs/0090-cli-mcp-skill-parity.md). The parity test `tests/parity.test.json` asserts:

- Identical input expression → identical output `matches` array (byte-for-byte, sorted).
- Same parse error envelope contains `hyphae` in `error`.

## Cross-references

- Related Skill: `basic-queries` (when you want one specific symbol by name).
- Related Skill: `call-graph` (when you've found symbols and want to traverse their relationships).
- Source RFC: [RFC-0003](../../rfcs/0003-hyphae-query-language.md) — Hyphae grammar.
- Source RFC: [RFC-0004](../../rfcs/0004-hyphae-executor.md) — executor semantics.
- Implementation: `crates/mycelium-cli/src/query.rs`, `crates/mycelium-mcp/src/lib.rs` (search for `mycelium_query`).
