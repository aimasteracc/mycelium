# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-05-30 |
| Current sprint | none cut yet — v0.1.2 pending kickoff |
| Active release branch | none |
| Next release target | v0.1.2 (hotfix), ETA 2026-06-06 |
| Final release target | v0.2.0, ETA 2026-07-15 |

## Live priorities (ordered)

P0:
1. [#150](https://github.com/aimasteracc/mycelium/issues/150) — MCP stdout pollution. Blocks every well-behaved client.
2. [#151](https://github.com/aimasteracc/mycelium/issues/151) — `mycelium query` placeholder. Breaks README lead example.

P1:
3. [#152](https://github.com/aimasteracc/mycelium/issues/152) — `edge_kind` case sensitivity.
4. [#153](https://github.com/aimasteracc/mycelium/issues/153) — Graph-algorithm timeouts on 1 K-node graph.

P2:
5. [#154](https://github.com/aimasteracc/mycelium/issues/154) — `mycelium init` placeholder.

Governance / process:
6. RFC-0090 Phase 1 — `parity.yml` CI workflow.
7. RFC-0090 Phase 2 — 8 category Skills authored.
8. RFC-0090 Phase 2.5 — INDEX.md generator script.

## Dispatch state (today)

| Agent | Status | Current item |
|---|---|---|
| rust-implementer | idle | next: pick up [#150](https://github.com/aimasteracc/mycelium/issues/150) |
| architect | idle | next: review Charter §2 SLA additions (heavy-graph rows) |
| tech-writer | idle | next: draft `skills/basic-queries/SKILL.md` |
| code-reviewer | idle | blocks on PR opens |
| security-reviewer | idle | next: routine post-sprint scan |
| e2e-runner | idle | next: write stdout-purity test for [#150](https://github.com/aimasteracc/mycelium/issues/150) |
| doc-updater | idle | next: refresh "90+" → "88" across README/CHANGELOG/RFC-0090 |

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder if available): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/v0.1.x branch, publish.

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- Skill marketplace listing metadata sign-off.

## Today's PM call (2026-05-30)

Read this verbatim into the next cadence check:

1. v0.1.0 + v0.1.1 are shipped and on crates.io. Three-Surface Rule is law.
2. External eval (glm5.1) found 4 issues; orchestrator dogfood found 2 more. All filed.
3. Priority order is set. Sprint v0.1.2 kicks off the moment rust-implementer picks up [#150](https://github.com/aimasteracc/mycelium/issues/150).
4. PRD for v0.2 is at [`docs/prd/v0.2-the-three-surface-release.md`](../prd/v0.2-the-three-surface-release.md). 5 sprints to v0.2.0.
5. No blocker from the founder needed at this checkpoint. Begin dispatch.

## Archive

(none yet — this file is fresh)
