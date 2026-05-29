# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-05-29 (PM run — #153 merged, parity checker shipped) |
| Current sprint | v0.1.4 (performance hardening + CLI parity backfill) |
| Active release branch | none (between releases) |
| Next release target | v0.1.4, ETA 2026-06-20 |
| Final release target | v0.2.0, ETA 2026-07-15 |
| Last shipped | **v0.1.3 — Hyphae lands + Skill umbrella sprint 1** (backmerge: develop commit 36d250d) |

## Live priorities (ordered)

P1:
*(none — #153 closed in this PM run)*

P2 (governance / v0.1.4 remaining):
1. `mycelium init` — still hidden/unimplemented per #154. Decision: keep hidden until v0.2 scoping. No new work needed; exit criteria satisfied by v0.1.2 hide.
2. CLI parity backfill — ~87 CLI subcommands still pending; systematic batch landing. **Large; likely spans v0.1.4 → v0.1.5.**
3. PR template "Three-Surface Self-Check" section — the 1 remaining RFC-0090 Phase 1 checkbox.

Sprint v0.1.4 backlog:
4. `mycelium index` benchmarks against 10K/100K node repos — new SLA row (1K done in PR #168).
5. Parity checker `--strict` promotion — flip `parity.yml` from informational to required; coordinate with CLI backfill.

Completed in this PM run (2026-05-29):
- [#153](https://github.com/aimasteracc/mycelium/issues/153) ✅ — graph-algorithm timeouts fixed (PR #168). 8 perf tests, Criterion benches, Charter §2 SLA extended.
- RFC-0090 Phase 1 ✅ — `parity.yml` CI workflow + `scripts/check_skill_parity.py` (PR #170). Also fixed 12 Skill naming mismatches; confirmed 89/89 mechanical coverage.

Completed in v0.1.3 (2026-05-29):
- [#151](https://github.com/aimasteracc/mycelium/issues/151) ✅ — `mycelium query` Hyphae CLI wired (PR #159)
- RFC-0090 Phase 2 ✅ — 9 category Skills: hyphae-query, basic-queries, call-graph, import-graph, reachability, centrality, inheritance, graph-structure, batch-ops (73/88 caps in PRs #159–#162)
- RFC-0090 Phase 2.3 ✅ — 16 remaining capabilities triaged; index-management Skill created; 89/89 coverage (PR #166)

## Dispatch state (2026-05-29, post-PM-run)

| Agent | Status | Current item |
|---|---|---|
| rust-implementer | **next-up** | CLI parity backfill — start with batch 1: implement `mycelium search-symbol`, `mycelium get-symbol-info`, `mycelium get-ancestors` (3 high-value CLI twins). |
| architect | idle | next: finalize Charter §2 SLA for 10K/100K graph rows (PR #168 added 1K + 10K; 100K row still TODO) |
| tech-writer | **next-up** | Add "Three-Surface Self-Check" section to `.github/PULL_REQUEST_TEMPLATE.md` (last RFC-0090 Phase 1 item) |
| code-reviewer | idle | blocks on PR opens |
| security-reviewer | idle | next: routine post-sprint scan (post-v0.1.3) |
| e2e-runner | idle | done — perf tests shipped in PR #168 |

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder if available): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/v0.1.x branch, publish.

## Sprint v0.1.4 exit criteria

- [x] All 6 timeout tools complete on 1K-node graph in < 2 s wall-clock. (PR #168)
- [x] New SLA rows in Charter §2 for heavy-graph algorithms. (PR #168)
- [x] Benchmarks in `benches/` for each of the 6 tools. (PR #168)
- [x] `parity.yml` CI workflow live (informational). (PR #170) *Promote to required in v0.2.0.*
- [x] `mycelium init` resolved — keeping hidden per v0.1.2; no further work needed in v0.1.4.
- [x] PR template Three-Surface self-check section. (already shipped in PR #149 — confirmed 2026-05-29)
- [x] CLI parity backfill batch 1: `search-symbol`, `get-ancestors`, `get-symbol-info`.

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- Skill marketplace listing metadata sign-off.

## Archive

### 2026-05-30 PM call (v0.1.2 era — superseded)

1. v0.1.0 + v0.1.1 are shipped and on crates.io. Three-Surface Rule is law.
2. External eval (glm5.1) found 4 issues; orchestrator dogfood found 2 more. All filed.
3. Priority order is set. Sprint v0.1.2 kicks off the moment rust-implementer picks up [#150](https://github.com/aimasteracc/mycelium/issues/150).
4. PRD for v0.2 is at [`docs/prd/v0.2-the-three-surface-release.md`](../prd/v0.2-the-three-surface-release.md). 5 sprints to v0.2.0.
5. No blocker from the founder needed at this checkpoint. Begin dispatch.
