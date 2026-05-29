---
name: CATEGORY_NAME
description: ONE-LINE SUMMARY OF THE CATEGORY — e.g. "Navigate the symbol graph — callers, callees, definers, impact."
allowed-tools:
  - mcp__mycelium__CAPABILITY_A
  - mcp__mycelium__CAPABILITY_B
  # ... add one entry per (CLI, MCP) pair this Skill covers
---

# `CATEGORY_NAME` — short title

## When to invoke this Skill

A 2–4 sentence description of the situations that bring the agent to
this category. Be concrete:

- "Use when the user asks structural-navigation questions
  ('what calls X', 'what does Y impact')."
- "Do NOT use for graph-statistics questions — see the
  `graph-analysis` Skill for that."

## Capabilities under this umbrella

### `CAPABILITY_A` — short imperative description

**When**: 1–2 sentences on the exact trigger.

**MCP invocation:**

```
mcp__mycelium__CAPABILITY_A({
  arg1: "...",
  arg2: ...    // optional; default = ...
})
```

**Equivalent CLI (human reading):**

```
mycelium CAPABILITY_A --arg1 ... --arg2 ...
```

**Result shape:**

```json
{ "field1": "...", "field2": [...] }
```

**Typical follow-ups:** which tools the agent reaches for next.

### `CAPABILITY_B` — short imperative description

(Same structure as above.)

## Parity contract

This Skill covers the listed capabilities per
[RFC-0090](../../rfcs/0090-cli-mcp-skill-parity.md) §Invariants:

- CLI ↔ MCP for each listed capability is byte-identical (name,
  description, args, JSON output).
- `tests/parity.test.json` asserts at least one input pair per
  capability.

## Cross-references

- Related Skills: `<other-category>` (for…)
- Source RFC: e.g. RFC-00XX defining these capabilities
