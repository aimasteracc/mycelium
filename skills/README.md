# Mycelium Skills

This directory holds the **Skill** surface of the [Three-Surface Rule](../rfcs/0090-cli-mcp-skill-parity.md) (Charter §5.13).

A **Skill** is a *category-shaped* bundle that teaches AI agents *when* to reach for a group of related Mycelium capabilities and *how* to interpret the results. A single Skill can cover many capabilities — that's the point.

## Cardinality, at a glance

| Relation | Cardinality | Where the contract lives |
|---|---|---|
| CLI ↔ MCP | **1 : 1** (byte-identical name, description, args, JSON output) | enforced by `parity.yml` CI on every PR |
| (CLI, MCP) ↔ Skill | **N : 1** (every pair appears in ≥ 1 Skill's `allowed-tools`) | enforced by [`INDEX.md`](INDEX.md) coverage matrix on every PR |

**Mental model:** *CLI and MCP are co-twins. Skills are umbrellas. Every twin must shelter under at least one umbrella; an umbrella can shelter many twins.*

## Layout

```
skills/
├── README.md              ← you are here
├── INDEX.md               ← capability → covering skill(s) matrix (CI-generated)
├── _template/             ← copy when creating a new Skill category
│   ├── SKILL.md
│   ├── examples/
│   │   ├── capability-a-basic.md
│   │   └── capability-b-basic.md
│   └── tests/
│       └── parity.test.json
└── <category>/            ← one folder per Skill category (NOT per capability)
    ├── SKILL.md
    ├── examples/
    └── tests/
```

## SKILL.md structure

```yaml
---
name: code-navigation
description: Navigate the Mycelium symbol graph — callers, callees, definers, impact.
allowed-tools:
  - mcp__mycelium__callers
  - mcp__mycelium__callees
  - mcp__mycelium__definers
  - mcp__mycelium__impact
---

# Code Navigation

## When to invoke this Skill
…

## Capabilities under this umbrella

### `callers` — find what calls a given symbol
**When**: …  
**MCP**: `mcp__mycelium__callers({ symbol: "..." })`  
**CLI**: `mycelium callers <symbol>`  
**Result**: …

### `callees` — find what a symbol calls
…
```

## The five CI-enforced invariants

For every capability `<cap>`:

1. **Name parity** — CLI subcommand name == MCP tool name (modulo kebab/snake case).
2. **Description parity** — one-line description is byte-identical across CLI `--help`, MCP tool schema, and any Skill section heading.
3. **Argument parity** — required CLI args ↔ required MCP fields.
4. **Output parity** — CLI `--format=json` == MCP structured response (modulo timestamps). Asserted by `skills/<category>/tests/parity.test.json`.
5. **Coverage** — `<cap>` appears in ≥ 1 `SKILL.md`'s `allowed-tools`. Inverse: every `allowed-tools` entry resolves to a real (CLI, MCP) pair.

## Adding a new capability

If the capability **belongs to an existing category**:

```bash
# 1. Add CLI in crates/mycelium-cli/src/<cap>.rs
# 2. Add MCP in crates/mycelium-mcp/src/tools/<cap>.rs
# 3. Add the MCP tool name to skills/<category>/SKILL.md allowed-tools
# 4. Add a new section under "## Capabilities under this umbrella"
# 5. Add an example to skills/<category>/examples/
# 6. Add a case to skills/<category>/tests/parity.test.json
# 7. Run: cargo run -p mycelium-cli -- parity-check
```

If the capability **starts a new category**:

```bash
# 1. Copy the template
cp -r skills/_template skills/<new-category>
# 2. Fill SKILL.md, edit allowed-tools, fill examples + tests
# 3. Add the new category to skills/INDEX.md (script regenerates it)
# 4. cargo run -p mycelium-cli -- parity-check
```

## Reference

- [Charter §5.13](../CHARTER.md#513--the-three-surface-rule-cli--mcp-parity--skill-coverage) — the rule
- [RFC-0090](../rfcs/0090-cli-mcp-skill-parity.md) — full design
- [ADR-0007](../docs/adr/0007-cli-mcp-skill-parity.md) — architectural decision
- [Claude Code skill spec](https://docs.claude.com/en/docs/claude-code/skills) — upstream format
