# RFC-0090 — The Three-Surface Rule: CLI ↔ MCP parity + Skill coverage

| 字段 | 值 |
|------|----|
| RFC  | 0090 |
| 状态 | Implemented |
| 作者 | orchestrator (Hive AI agent) |
| 日期 | 2026-05-30 |
| 参考 | Charter §5.13, ADR-0007 |
| 别名 | "1:1:1 rule" (colloquial) |

## 摘要

Every Mycelium capability ships on three surfaces. The cardinalities are
**not** symmetric:

- **CLI ↔ MCP is 1 : 1 — strict.** Each capability has exactly one CLI
  subcommand and exactly one MCP tool. Name, description, argument
  schema, and JSON output are byte-identical across the two.
- **(CLI, MCP) ↔ Skill is N : 1 — covered.** Every (CLI, MCP) pair
  MUST be referenced by **at least one** Skill bundle. A single Skill
  MAY cover multiple related capabilities (a "category" or "topic"
  skill). A capability with zero Skill coverage is an **orphan** and
  fails CI.

In one line: **CLI and MCP are co-twins; Skills are umbrellas that
must shelter every twin pair.**

## 背景与动机

Mycelium today has 88 MCP tools, 8 CLI subcommands, and 0 Skills.
Three concrete failure modes:

1. **Discoverability collapse.** A human user with a terminal cannot
   discover what the AI agent can do. The product surface is invisible
   from outside the agent.
2. **Skill bundles are the install vector.** Claude Code, Cursor, and
   Codex all consume skill bundles. Without them, Mycelium reaches
   AI-using developers only when they hand-wire MCP. With them,
   installation is one command.
3. **Capability drift.** A CLI command implemented after the MCP tool
   often diverges (different flags, different defaults, different
   output shape) because the two are written by different sprints.

An earlier draft of this RFC tried to force 1 : 1 : 1 across all
three surfaces. That was rejected as over-prescriptive: a 90-MCP-tool
graph-intelligence library does not naturally decompose into 90 isolated
Skills — agents reach for "this kind of analysis" (a category), not
"this exact tool". The corrected rule preserves the parity invariant
where it matters (CLI ↔ MCP, where drift causes user-visible bugs) and
relaxes it where it does not (Skills, where bundling improves
discoverability).

## 设计

### Definition of "capability"

A **capability** is one user-observable function with one input/output
contract. Examples:

| Capability | CLI | MCP tool |
|---|---|---|
| Resolve who calls a function | `mycelium callers` | `callers` |
| Resolve who a function calls | `mycelium callees` | `callees` |
| Blast-radius of a change | `mycelium impact` | `impact` |
| Build an index | `mycelium index` | `index` |

Refactors, performance fixes, and internal-only changes are NOT
capabilities and are NOT subject to the rule.

### Surface obligations

For every capability `<cap>`:

| Surface | Cardinality | Location | Identifier |
|---|---|---|---|
| CLI | exactly 1 | `crates/mycelium-cli/src/<cap>.rs` | `mycelium <cap>` |
| MCP | exactly 1 | `crates/mycelium-mcp/src/tools/<cap>.rs` | tool name `<cap>` (snake_case) |
| Skill | ≥ 1 | referenced from any `skills/<skill>/SKILL.md` whose `allowed-tools` lists this MCP tool | — |

CLI subcommand name = MCP tool name (modulo case convention). Skill
folder names follow their own category vocabulary (e.g.
`skills/code-navigation/`, `skills/centrality-analysis/`,
`skills/indexing/`).

### Skill bundle structure

A Skill is a **category** that bundles one or more capabilities. It
teaches the agent *when* to reach for these capabilities and *how* to
interpret the result.

```
skills/<skill-name>/
├── SKILL.md              # frontmatter + when-to-invoke + per-capability sections
├── examples/             # 2+ worked examples per covered capability
│   ├── callers-basic.md
│   ├── callees-basic.md
│   └── ...
└── tests/
    └── parity.test.json  # CLI ↔ MCP byte-equality assertions
```

`SKILL.md` frontmatter (Claude Code skill spec):

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
```

Each capability listed in `allowed-tools` gets its own section in the
`SKILL.md` body covering:

- **When to invoke** (situations where this capability is the right reach)
- **Invocation pattern** (MCP call + equivalent CLI for human reading)
- **Result interpretation** (field meanings, common follow-ups)

### Invariants (CI-enforced)

#### Pair invariants (CLI ↔ MCP, four checks)

1. **Name parity.** CLI subcommand name = MCP tool name, modulo case
   convention (kebab-case CLI, snake_case MCP).
2. **Description parity.** The one-line description in CLI `--help` and
   the MCP tool `description` field are **byte-identical**.
3. **Argument parity.** Required CLI arguments ↔ required MCP input
   fields. Optional CLI flags ↔ optional MCP fields.
4. **Output parity.** For the same inputs, CLI `--format=json` output
   = MCP structured response, byte-for-byte modulo timestamps.
   `skills/*/tests/parity.test.json` asserts at least one input pair
   per capability.

#### Coverage invariant (Skill umbrella)

5. **No orphan.** Every (CLI, MCP) pair MUST appear in at least one
   `SKILL.md`'s `allowed-tools` list. The inverse is also CI-enforced:
   every MCP tool listed in any `allowed-tools` MUST correspond to a
   real CLI + MCP pair.

The coverage matrix lives at `skills/INDEX.md` — a generated table of
`capability → covering skill(s)`. CI regenerates it on every PR and
fails on diff.

### Permitted exceptions

#### CLI-only

Low-level trace/debug commands that operate on the index file format
or runtime internals (e.g. `mycelium dump-index`,
`mycelium repair-store`). Carry an `EXCEPTION: CLI-only` line in the
governing RFC. These do not need an MCP twin or Skill coverage.

#### MCP-only

Multi-agent state coordination with no single-shot human equivalent.
Requires BDFL signoff in the governing RFC and an
`EXCEPTION: MCP-only` line. We currently have **zero** such
capabilities and do not anticipate any pre-v0.3.

#### No "Skill-only"

A Skill without at least one (CLI, MCP) capability behind it is
marketing copy, not a feature. Forbidden.

### Migration plan for existing surface gaps

| Existing state | Action | Tracked by |
|---|---|---|
| MCP tool with CLI counterpart, no Skill coverage | Backfill into an appropriate Skill before next minor release. | `parity-backfill` label, grouped by proposed Skill category |
| MCP tool with no CLI | Backfill CLI + ensure Skill coverage, OR file `EXCEPTION: MCP-only` RFC. | `parity-backfill` label |
| CLI command with no MCP | Backfill MCP + ensure Skill coverage, OR file `EXCEPTION: CLI-only` RFC. | `parity-backfill` label |

Backfill is grouped by **category** to amortize Skill-authoring cost:
e.g. all 8 centrality-analysis MCP tools get backfilled into one
`skills/centrality-analysis/` skill in one PR.

The release-blocking gate is set so v0.2.0 cannot ship with any
unbackfilled gaps that do not have a filed exception RFC.

## 验收标准 (Acceptance Criteria)

- [x] `skills/` directory exists at the repo root with a `README.md`,
      a `_template/` category-style scaffold, and `INDEX.md`.
- [x] `skills/INDEX.md` is generated by a script and lists every
      (capability → covering skill) pair.
- [x] `scripts/check_skill_parity.py` walks MCP functions and Skill
      `allowed-tools`, enforces I1 and I2 invariants, reports I4 deferred.
- [x] CHARTER.md §5.13 amended with the Three-Surface Rule.
- [x] CLAUDE.md "Hard Rules" updated.
- [x] ADR-0007 records the architectural decision.
- [x] The `parity-backfill` audit is seeded with one issue per
      existing un-covered capability, grouped by proposed Skill
      category.
- [x] PR template gains a "Three-Surface Self-Check" section.
- [x] `.github/workflows/parity.yml` runs the parity check on every
      PR touching `crates/mycelium-cli/`, `crates/mycelium-mcp/`,
      or `skills/` (Phase 1: informational).

## Alternatives considered

### (A) Strict 1 : 1 : 1 (one skill per capability)

The original draft of this RFC. Rejected because:

- 88 MCP tools would produce 90+ Skills. Most would be near-duplicates
  differing only in tool name. Cognitive overhead for the agent (which
  Skill do I load?) defeats the purpose.
- Real agent reasoning is category-shaped, not capability-shaped. The
  agent thinks "I need centrality analysis", picks one Skill, and the
  Skill teaches it which specific tool fits.
- Skill maintenance cost is per-Skill not per-capability. Categories
  amortize.

### (B) Aspirational rule, no CI enforcement

Rejected. The 90 / 8 / 0 drift happened under exactly the aspirational
model. Without enforcement the rule is decorative.

### (C) CLI is the source of truth; MCP and Skill auto-generated

MCP partially derivable from CLI argument schemas. Skills are not — the
*when-to-invoke* prose and category grouping require domain judgment.
This is a future optimization for the MCP half (≤ 30% gain) but does
not change the rule.

### (D) Skill is the source of truth; CLI and MCP derived

Rejected. Skill bundles are tutorial-shaped, not contract-shaped. They
omit edge cases and rare flags that CLI/MCP must support.

## Consequences

**Positive:**

- The product is reachable from three audiences (human dev, MCP agent,
  Skill-bundle agent) with one PR per capability.
- CLI ↔ MCP drift becomes a CI failure rather than a slow discovery.
- Skill bundles are first-class distribution, organized by topic.
- Adding a capability to an existing Skill category is cheap; only the
  first capability in a new category pays the Skill-authoring cost.

**Negative:**

- Every capability PR is roughly 1.3x larger (CLI + MCP + at least an
  `allowed-tools` line in an existing Skill).
- Creating a brand-new Skill category is a roughly 1.7x PR.
- We must build and maintain the parity-check CI job + the coverage
  matrix generator.

**Neutral:**

- The Skill umbrella structure means one well-organized Skill set may
  cover the entire capability surface with 10–15 Skills rather than
  88. This is a design constraint on how we group capabilities, not
  a regression.

## 实施计划

| 阶段 | 内容 | 负责人 |
|---|---|---|
| Phase 0 (this RFC) | Charter amendment, ADR, CLAUDE.md, `skills/` scaffold, INDEX.md template | orchestrator |
| Phase 1 (v0.1.2) | Parity-check CI, PR template update, coverage matrix generator, backfill issues filed | rust-implementer |
| Phase 2 (v0.1.3) | Backfill Skills for the 8 existing CLI commands; consolidate the 88 MCP tools into ~12 category Skills | rust-implementer + tech-writer |
| Phase 3 (v0.2.0) | All capabilities have Skill coverage; CI parity gate is required on `main`. First Skill-bundle release on a Claude Code marketplace. | hive |
| Phase 4 (v0.2.1+) | New capabilities ship Three-Surface from day one; rule enforced by CI | (default mode) |

## 备注

This RFC was rewritten after the founder pointed out that the original
1 : 1 : 1 formulation conflated two different invariants. The correct
mental model:

> CLI and MCP are co-twins. Skills are umbrellas under which co-twins shelter.
> An umbrella may shelter many twins. A twin must shelter under at least one umbrella.

The colloquial name "1 : 1 : 1 rule" is retained because the founder
coined it, but the formal name is **the Three-Surface Rule**.
