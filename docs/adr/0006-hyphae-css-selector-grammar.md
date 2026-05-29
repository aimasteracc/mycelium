# 0006. Hyphae uses CSS-selector-inspired grammar

- **Status**: accepted
- **Date**: 2026-05-29
- **RFC**: RFC-0003

## Context

Mycelium needs a query language (Hyphae) that:
1. Lets AI agents query the symbol graph with far fewer tokens than JSON API calls
2. Is learnable from a one-page SKILL.md description
3. Encodes structural relationships naturally (parent/child, sibling, descendant)

## Decision

Hyphae borrows CSS selector syntax:
- `#name` — name selector
- `.kind` — kind selector (`.function`, `.class`, etc.)
- `>` — direct child combinator
- `~` — sibling combinator
- ` ` (space) — descendant combinator
- `:pseudo(arg)` — relationship pseudo-classes (`:calls()`, `:imports()`)

Example: `.function:calls(.function)` = "all functions that call other functions"

Implemented as: `logos`-based lexer + hand-written recursive descent parser (RFC-0003).
Executor against Store is RFC-0004 (next phase).

## Alternatives rejected

- **LALR (lalrpop)**: overkill for this grammar; adds complexity
- **S-expression style** (like tree-sitter queries): unfamiliar to AI agents in prompt context
- **GraphQL**: too verbose; doesn't match the CSS-selector precedent AI agents already know

## Consequences

The CSS metaphor is intentionally loose — Hyphae is not DOM manipulation.
Users familiar with CSS will recognize the surface but must not assume CSS semantics.

The grammar is intentionally simple enough that an AI can learn it from 20 examples.
This is the primary mechanism for achieving Charter §2 ≤ 30% token efficiency.
