---
name: architecture-context
description: One-shot architecture tracing — get entry points, call graph neighborhood, and source snippets for any natural-language question about how code works.
allowed-tools:
  - mcp__mycelium__context
  - mycelium_context
category: navigation
icon: 🏗️
marketplace_examples:
  - query: "How does request routing work?"
    tool: mcp__mycelium__context
  - query: "Trace handle_request to get_user"
    tool: mcp__mycelium__context
  - query: "Show how auth reaches token verification"
    tool: mcp__mycelium__context
---

# `architecture-context` — one-shot architecture tracing

This Skill exposes the `mycelium_context` tool (MCP) and `mycelium context`
CLI command: a single call that accepts a natural-language task description and
returns the most relevant entry-point symbols, their call-graph neighborhood,
and source location spans.

Use this **before** chaining lower-level tools like `mycelium_search_symbol`,
`mycelium_get_callers`, and `mycelium_get_callees`. One `mycelium_context` call
typically saves 5–20 round-trips.

## When to invoke this Skill

Use **when**:

- The user asks "how does X work?" or "trace A to B".
- You need a broad orientation in an unfamiliar area of the codebase.
- You want entry points + neighbors + source spans in a single call.

Do **NOT** use when:

- You already know the exact symbol path and only need its direct callers/callees
  → use the `call-graph` Skill.
- You want a precise multi-hop graph query → use the `hyphae-query` Skill.

## Tool reference

### MCP: `mycelium_context`

```
task            (string)  Natural-language architecture question or "trace A to B".
max_nodes       (int?)    Maximum graph nodes returned. Default: 30.
max_code_blocks (int?)    Maximum source snippets. Default: 6.
output_format   (string?) "json" (default), "text", or "msgpack".
```

### CLI: `mycelium context`

```
mycelium context --task "trace ServeHTTP to HandlerFunc"
mycelium context --task "how does auth work" --format json
mycelium context --task ".function:calls(#AuthService)" --max-nodes 50
```

```
--task TEXT           Natural-language task or Hyphae selector (required)
--root PATH           Project root (default: current directory)
--max-nodes INT       Maximum graph nodes (default: 30, max: 100)
--max-code-blocks INT Maximum source snippets (default: 6, max: 25)
--format json|text    Output format (default: json)
```

Both surfaces return `entry_points`, `nodes`, `edges`, `code_blocks`, `stats`,
and `agent_summary`. The `agent_summary.next_step` field tells you whether to
proceed from the returned context or chain a narrower tool.
