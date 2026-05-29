# 0007. Every feature ships CLI + MCP + Skill (the 1:1:1 rule)

- **Status**: accepted
- **Date**: 2026-05-30
- **RFC**: RFC-0090
- **Charter**: §5.13

## Context

Mycelium addresses three audiences:

1. The human developer who types `mycelium <command>` in a terminal.
2. The autonomous AI agent that calls MCP tools over stdio/HTTP.
3. The Claude-Code-class skill-bundle consumer that installs a packaged
   capability and gets agent + prompt + tool wiring in one drop.

Until now the project has been built MCP-first: 90+ MCP tools, 8 CLI
subcommands, 0 skills. The asymmetry has three concrete costs:

1. **Discoverability**: a terminal user cannot see what the AI can do.
2. **Distribution**: skill bundles are the install path for Claude
   Code / Cursor users; without them, Mycelium does not exist for that
   audience.
3. **Drift**: features added through MCP first and CLI later diverge
   (different flags, defaults, output shapes).

The decision below is the architectural commitment that prevents all
three.

## Decision

Every Mycelium feature MUST ship on all three surfaces — CLI, MCP, and
Skill — in the same PR. The three are 1:1:1: one capability ↔ one CLI
command ↔ one MCP tool ↔ one Skill folder.

- Naming is canonical and identical (modulo case convention).
- The one-line `description` string is byte-identical across all three.
- Required CLI arguments are required MCP input fields, documented in
  SKILL.md.
- CLI `--format=json` output and MCP structured response are
  byte-for-byte equal modulo timestamps, asserted by per-feature
  `skills/<feat>/tests/parity.test.json`.

Skill bundles live at `skills/<feat>/` and follow the Claude Code skill
specification (`SKILL.md` with YAML frontmatter + `allowed-tools`).

CI enforces all four parity invariants (name, description, args,
output) on every PR touching `crates/mycelium-cli/`,
`crates/mycelium-mcp/`, or `skills/`.

Two narrow exceptions exist (CLI-only debug tools, MCP-only multi-agent
coordination) and require explicit opt-out in the governing RFC. There
is no skill-only exception.

## Alternatives rejected

- **CLI is the source of truth; MCP/Skill auto-generated.** Tempting,
  but MCP tool schemas and SKILL.md *when-to-invoke* prose do not
  derive mechanically from `clap` definitions. The 30-50% of SKILL.md
  that *can* be auto-generated is a future optimization, not the rule.
- **Aspirational rule, no CI enforcement.** Rejected because the 90/8/0
  drift happened under exactly the aspirational model. Without
  enforcement the rule is decorative.
- **MCP-only, CLI-only, or Skill-only as default; multi-surface only
  when requested.** Rejected because it preserves the current drift
  dynamics. The default mode of work must produce the invariant.

## Consequences

**Positive:**
- Three audiences reachable from one PR.
- Drift becomes a CI failure, not a slow discovery.
- Skill bundles are first-class, not afterthought.
- The product surface is internally consistent.

**Negative:**
- Per-feature PR size grows ~1.5x.
- The minimum unit of "ship a feature" gets larger; ad-hoc MCP tools
  are no longer cheap.
- We must build and maintain a parity-check CI job.

**Neutral:**
- v0.1.0 + v0.1.1 ship in a non-compliant state. v0.1.x is the
  backfill window; v0.2.0 is the first release that ships compliant
  end-to-end.
