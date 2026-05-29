# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-05-30 (after v0.1.2 ship) |
| Current sprint | v0.1.3 (Hyphae lands + Skill umbrella sprint 1) |
| Active release branch | none (between releases) |
| Next release target | v0.1.3, ETA 2026-06-13 |
| Final release target | v0.2.0, ETA 2026-07-15 |
| Last shipped | **v0.1.2 — Sprint 1 hotfix** (https://github.com/aimasteracc/mycelium/releases/tag/v0.1.2) |

## Live priorities (ordered)

P0:
1. [#151](https://github.com/aimasteracc/mycelium/issues/151) — `mycelium query` placeholder. Breaks README lead example. **v0.1.3 lead item.**

P1:
2. [#153](https://github.com/aimasteracc/mycelium/issues/153) — Graph-algorithm timeouts on 1 K-node graph. **v0.1.4.**

P2: (none — all v0.1.x P2 items shipped in v0.1.2)

Governance / process:
3. RFC-0090 Phase 1 — `parity.yml` CI workflow. **v0.1.3.**
4. RFC-0090 Phase 2 — first 3 category Skills (`basic-queries`, `call-graph`, `hyphae-query`). **v0.1.3.**
5. RFC-0090 Phase 2.5 — INDEX.md generator script. **v0.1.3.**

Closed in v0.1.2:
- [#150](https://github.com/aimasteracc/mycelium/issues/150) ✅
- [#152](https://github.com/aimasteracc/mycelium/issues/152) ✅
- [#154](https://github.com/aimasteracc/mycelium/issues/154) ✅

## Dispatch state (today, post-v0.1.2)

| Agent | Status | Current item |
|---|---|---|
| rust-implementer | next-up | [#151](https://github.com/aimasteracc/mycelium/issues/151) Hyphae CLI wiring + MCP twin + parity test |
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
