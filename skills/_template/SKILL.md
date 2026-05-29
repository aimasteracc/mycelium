---
name: FEATURE_NAME
description: ONE-LINE SUMMARY — MUST match `mycelium FEATURE_NAME --help` one-liner and MCP tool description exactly.
allowed-tools:
  - mcp__mycelium__FEATURE_NAME
---

# `FEATURE_NAME` — short title

## When to invoke

A 2-4 sentence description of the situation where the agent should
reach for this tool. Examples:

- "Use when the user asks 'what calls function X' and you have an
  indexed Mycelium project at the cwd."
- "Do NOT use when X is a method on a polymorphic type without
  receiver narrowing — prefer `callers --polymorphic` for that."

## How to invoke

```
mcp__mycelium__FEATURE_NAME({
  arg1: "...",
  arg2: ...   // optional, default = ...
})
```

Equivalent CLI invocation (for the human reading this skill):

```
mycelium FEATURE_NAME --arg1 ... --arg2 ...
```

## How to interpret the result

The MCP tool returns a structured response:

```json
{
  "field1": "...",
  "field2": [...]
}
```

Guidance on what each field means and how to chain into other tools.

## Examples

See `examples/basic.md` and `examples/advanced.md`.

## Parity contract

This skill, the CLI `mycelium FEATURE_NAME`, and the MCP tool
`FEATURE_NAME` are 1:1:1 per [RFC-0090](../../rfcs/0090-cli-mcp-skill-parity.md).
`tests/parity.test.json` asserts byte-for-byte output equality.
