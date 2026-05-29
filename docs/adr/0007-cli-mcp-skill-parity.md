# 0007. The Three-Surface Rule (CLI ↔ MCP parity + Skill coverage)

- **Status**: accepted
- **Date**: 2026-05-30
- **RFC**: RFC-0090
- **Charter**: §5.13
- **Colloquial name**: "1:1:1 rule" (founder coinage)

## Context

Mycelium addresses three audiences:

1. The human developer who types `mycelium <command>` in a terminal.
2. The autonomous AI agent that calls MCP tools over stdio/HTTP.
3. The Claude-Code-class skill-bundle consumer that installs a packaged
   capability and gets agent + prompt + tool wiring in one drop.

The project has been built MCP-first: 88 MCP tools, 8 CLI subcommands,
0 Skills. The asymmetry has three concrete costs:

1. **Discoverability**: a terminal user cannot see what the AI can do.
2. **Distribution**: skill bundles are the install path for Claude
   Code / Cursor users; without them, Mycelium does not exist for that
   audience.
3. **Drift**: features added through MCP first and CLI later diverge
   (different flags, defaults, output shapes).

An earlier draft of this ADR proposed a strict 1:1:1 mapping. That was
rejected once it became clear the surfaces have different natural
cardinalities: CLI and MCP are 1:1 (they describe the same contract for
two different consumer shapes), but Skills are categorically *shaped*
(an agent reaches for "centrality analysis", not for one specific tool).
This ADR records the corrected decision.

## Decision

Every Mycelium **capability** lives on three surfaces with asymmetric
cardinalities:

- **CLI ↔ MCP is 1 : 1** — strict. Name, description, argument schema,
  and JSON output are byte-identical across the two.
- **(CLI, MCP) ↔ Skill is N : 1** — covered. Every (CLI, MCP) pair must
  appear in at least one Skill's `allowed-tools`. A single Skill may
  bundle multiple related capabilities. A capability with zero Skill
  coverage is an **orphan** and fails CI.

Skills live at `skills/<category>/SKILL.md` following the Claude Code
skill specification. The coverage matrix lives at `skills/INDEX.md` —
generated, CI-gated, single source of truth for which Skills cover
which capabilities.

CI enforces five invariants:

1. **Name parity** — CLI subcommand = MCP tool name (mod case).
2. **Description parity** — byte-identical one-line `description`.
3. **Argument parity** — required CLI args = required MCP fields.
4. **Output parity** — CLI `--format=json` = MCP structured response,
   asserted by `skills/<skill>/tests/parity.test.json`.
5. **Coverage** — every (CLI, MCP) pair is in ≥ 1 Skill's `allowed-tools`;
   every `allowed-tools` entry resolves to a real (CLI, MCP) pair.

Exceptions: `EXCEPTION: CLI-only` (debug/trace internals),
`EXCEPTION: MCP-only` (multi-agent coordination, BDFL signoff).
There is no Skill-only exception.

## Alternatives rejected

- **Strict 1:1:1 (one Skill per capability).** Would produce 88+
  near-duplicate Skills for the existing MCP surface; agents would have
  cognitive overhead picking the right Skill; the per-Skill maintenance
  cost does not amortize. Rejected in favor of category-shaped Skills.
- **Aspirational rule, no CI enforcement.** Rejected because the 90/8/0
  drift happened under exactly the aspirational model.
- **CLI is the source of truth; MCP and Skill auto-generated.** MCP
  partially derivable; Skills are not (when-to-invoke prose and
  category grouping require domain judgment). Future optimization for
  MCP only; does not change the rule.
- **Skill is the source of truth; CLI and MCP derived.** Skill bundles
  are tutorial-shaped, not contract-shaped. They would omit edge cases
  and rare flags.

## Consequences

**Positive:**

- Three audiences reachable from one PR.
- CLI ↔ MCP drift becomes a CI failure, not a slow discovery.
- Skill bundles are first-class distribution, organized by topic.
- Adding a capability to an existing Skill category is cheap; only the
  first capability in a new category pays the Skill-authoring cost.

**Negative:**

- Per-capability PR size grows ~1.3x (CLI + MCP + Skill umbrella update).
- A brand-new Skill category PR is ~1.7x.
- We must build and maintain the parity-check CI job + the coverage
  matrix generator.

**Neutral:**

- v0.1.0 + v0.1.1 ship in a non-compliant state. v0.1.x is the
  backfill window; v0.2.0 is the first release that ships compliant
  end-to-end.
- A well-organized Skill set will likely cover all 88 capabilities
  with 10–15 Skills, not 90. This is a design constraint on Skill
  grouping, not a regression.
