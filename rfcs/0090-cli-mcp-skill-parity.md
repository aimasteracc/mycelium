# RFC-0090 — The 1:1:1 Rule: CLI / MCP / Skill Feature Parity

| 字段 | 值 |
|------|----|
| RFC  | 0090 |
| 状态 | Accepted |
| 作者 | orchestrator (Hive AI agent) |
| 日期 | 2026-05-30 |
| 参考 | Charter §5.13 (new), ADR-0007 |

## 摘要

Every Mycelium feature MUST be reachable through three equally-weighted
surfaces:

1. **CLI** — a `mycelium <command>` subcommand for the human at a terminal.
2. **MCP** — a tool exposed by `mycelium serve --mcp` for the autonomous
   AI agent.
3. **Skill** — a `skills/<feature-name>/SKILL.md` bundle that teaches a
   Claude Code (or compatible) agent **when** to call the MCP tool and
   **how** to interpret the result.

These three surfaces are 1:1:1: one capability ↔ one CLI command ↔ one
MCP tool ↔ one Skill. The PR that introduces the capability ships all
three or it does not ship.

## 背景与动机

Mycelium today has 90+ MCP tools, 8 CLI subcommands, and 0 Skills. The
delta exists because adding capability through MCP is the cheapest path
when the AI agent is the primary user. But it has three problems:

1. **Discoverability collapse.** A human user with a terminal cannot
   discover what the AI agent can do. The product surface is invisible
   from outside the agent.
2. **Skill bundles are the new install vector.** Claude Code, Cursor,
   and Codex all support skill bundles. Without them, Mycelium reaches
   AI agents only when the user manually wires up MCP. With them,
   installation is one command.
3. **Capability drift.** A CLI command implemented after the MCP tool
   often diverges (different flags, different defaults, different
   output shape) because the two are written by different sprints.

The 1:1:1 rule eliminates all three by making the three surfaces a
**single deliverable** rather than three separable artifacts.

## 设计

### Definition of "feature"

A feature is one user-observable capability. Examples:

- `index` — build an index of a project. (1 capability)
- `query` — execute a Hyphae query. (1 capability)
- `callers` / `callees` / `impact` — **three** capabilities (each its
  own command, tool, and skill — they are not one "graph" feature).

Refactors, performance fixes, and internal-only changes are NOT
features and are NOT subject to the rule.

### Mandatory surfaces

For every feature `<feat>`:

| Surface | Location | Naming |
|---|---|---|
| CLI | `crates/mycelium-cli/src/<feat>.rs` | `mycelium <feat>` |
| MCP | `crates/mycelium-mcp/src/tools/<feat>.rs` | tool name `<feat>` (snake_case) |
| Skill | `skills/<feat>/SKILL.md` (+ `skills/<feat>/examples/`) | folder name = CLI subcommand |

**Naming is canonical.** If the CLI is `mycelium graph-diff`, the MCP
tool is `graph_diff`, the skill directory is `skills/graph-diff/`.
Aliases (e.g. `mycelium gd`) are allowed for the CLI only.

### Skill bundle structure

```
skills/<feat>/
├── SKILL.md              # frontmatter + when-to-invoke + invocation pattern
├── examples/             # 2+ worked examples
│   ├── basic.md
│   └── advanced.md
└── tests/                # smoke tests that assert the MCP tool agrees with the CLI
    └── parity.test.json  # input → expected output, run by skill-parity CI job
```

`SKILL.md` follows the Claude Code skill specification:

```yaml
---
name: <feat>
description: <one-line summary; must match CLI --help one-liner exactly>
allowed-tools:
  - mcp__mycelium__<feat>
---
```

### Three-way parity invariants

These MUST hold for every feature and are CI-enforced (see Enforcement):

1. **Surface-name parity.** The CLI subcommand, the MCP tool name, and
   the skill folder name agree (up to case convention).
2. **Description parity.** The one-line description in `--help`, the
   MCP tool `description` field, and the SKILL.md `description`
   frontmatter are **identical strings**.
3. **Argument parity.** Every required argument to the CLI is a
   required field in the MCP tool input schema and is documented in
   SKILL.md. Optional CLI flags map to optional MCP fields.
4. **Output parity.** For the same inputs, the CLI's machine-readable
   output (`--format=json`) and the MCP tool's structured response are
   byte-for-byte equal modulo timestamps. The skill's `parity.test.json`
   asserts at least one input pair.

### Permitted exceptions

Two narrow exceptions, both requiring explicit RFC opt-out:

- **CLI-only**: trace/debug tools that operate on the index file
  format or runtime internals (e.g. `mycelium dump-index`). Carry an
  `EXCEPTION: CLI-only` line in the RFC governing the feature.
- **MCP-only**: tools that require multi-agent state coordination
  with no single-shot human equivalent. As of this RFC's writing we
  do not believe such a tool exists in Mycelium's roadmap; an
  exception requires the founder's signoff in the RFC.

**There is no "skill-only" exception.** A skill without a CLI/MCP
counterpart is a marketing artifact, not a feature.

### Migration plan for existing surface gaps

| Existing state | Action |
|---|---|
| MCP tool with CLI counterpart, no skill | Backfill skill before next minor release. |
| MCP tool with no CLI, no skill | Backfill CLI + skill before next minor release, OR file exception RFC. |
| CLI command with no MCP, no skill | Backfill MCP + skill before next minor release. |

Backfill PRs are tracked under the `parity-backfill` label and grouped
by feature. The release blocking gate is set so v0.2.0 cannot ship
with any unbackfilled gaps that do not have a filed exception RFC.

## 验收标准 (Acceptance Criteria)

- [ ] `skills/` directory exists at the repo root with a `README.md`
      and a `_template/SKILL.md` scaffold.
- [ ] `crates/mycelium-cli/src/parity.rs` (or equivalent CI script)
      walks the three surface inventories and fails the build on
      any unaccounted-for gap.
- [ ] CHARTER.md §5.13 amended to include the 1:1:1 rule (this RFC's
      one-paragraph form).
- [ ] CLAUDE.md "Hard Rules" updated to include the 1:1:1 commitment.
- [ ] ADR-0007 records the architectural decision with alternatives
      considered (see ADR for detail).
- [ ] The parity-backfill audit (issue label `parity-backfill`) is
      seeded with one issue per existing MCP-only or CLI-only
      capability.
- [ ] PR template gains a "1:1:1 self-check" section (3 checkboxes).
- [ ] `.github/workflows/parity.yml` runs the parity check on every PR
      touching `crates/mycelium-cli/`, `crates/mycelium-mcp/`, or
      `skills/`.

## Alternatives considered

### (A) "CLI is the source of truth; MCP/Skill are generated"

Tempting because it eliminates drift by construction. Rejected because:
- MCP tools have a fundamentally different shape (structured input
  schemas, agent-friendly error envelopes) that does not derive
  mechanically from `clap` definitions.
- Skills require domain-specific *when-to-invoke* prose that no
  code-gen can produce.

### (B) "Aspirational rule, no CI enforcement"

Rejected. We saw the 90+ MCP / 0 Skill drift happen under exactly the
"aspirational" model. Without enforcement the rule is decorative.

### (C) "1:1:1 but Skills are auto-generated from MCP schemas"

Considered for v0.2. The MCP → SKILL.md auto-generation can cover
30-50% of the SKILL.md body (frontmatter, parameter docs). The
when-to-invoke section remains hand-written. This is a future
optimization that does not change the rule.

## Consequences

**Positive:**
- The product is reachable from three audiences (human dev, MCP agent,
  skill-bundle agent) with one PR per feature.
- Drift between surfaces becomes a CI failure rather than a slow
  discovery.
- Skill bundles become a first-class distribution surface, not an
  afterthought.

**Negative:**
- Every feature PR is roughly 1.5x larger.
- A small "add a quick MCP tool" change becomes a three-surface
  commitment. The minimum unit of feature work goes up.
- We must build and maintain a parity-check CI job.

**Neutral:**
- Some refactors that previously could "just touch MCP" now have to
  touch all three. The 1.5x cost is paid in exchange for invariant
  preservation.

## 实施计划

| 阶段 | 内容 | 负责人 |
|---|---|---|
| Phase 0 (this RFC) | Charter amendment, ADR, CLAUDE.md, skills/ scaffold | orchestrator |
| Phase 1 (v0.1.2) | Parity-check CI, PR template update, backfill issues filed | rust-implementer |
| Phase 2 (v0.1.3) | Backfill skills for the 8 existing CLI commands | rust-implementer |
| Phase 3 (v0.2.0) | All 90+ MCP tools have skills, OR exception RFCs filed | hive |
| Phase 4 (v0.2.1+) | New features ship 1:1:1 from day one; rule enforced by CI | (default mode) |

## 备注

This RFC is small in code but heavy in governance. It changes the
**minimum unit of shippable work** in Mycelium. The decision is
ratified by Charter amendment (§5.13) and by ADR-0007.
