# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-05-29 (PM run — v0.1.4 sprint COMPLETE; v0.1.4 release queued) |
| Current sprint | v0.1.5 (CLI parity batches 2–N + marketplace prep) |
| Active release branch | **release/v0.1.4 needed** — all exit criteria met, awaiting cut |
| Next release target | v0.1.4, ETA 2026-06-20 (ready now) |
| Final release target | v0.2.0, ETA 2026-07-15 |
| Last shipped | **v0.1.3 — Hyphae lands + Skill umbrella sprint 1** (tag v0.1.3) |

## v0.1.4 Sprint — ALL EXIT CRITERIA MET ✅

All items confirmed complete as of develop `be317da`:

- [x] All 6 timeout tools complete on 1K-node graph in < 2 s. (PR #168)
- [x] New SLA rows in Charter §2 for heavy-graph algorithms. (PR #168)
- [x] Benchmarks in `benches/` for each of the 6 tools. (PR #168)
- [x] `parity.yml` CI workflow live (informational). (PR #170)
- [x] `mycelium init` resolved — keeping hidden. (PR #154, confirmed no action needed)
- [x] PR template Three-Surface Self-Check section. (PR #149, confirmed already shipped)
- [x] CLI parity backfill batch 1: `search-symbol`, `get-symbol-info`, `get-ancestors`. (PR #172)

**→ Next run action: cut release/v0.1.4 from develop, bump to 0.1.4, publish 5 crates.**

## Live priorities (ordered)

P0:
1. Cut **v0.1.4 release** — all exit criteria met; develop is clean. Assign to release agent.

P1 (v0.1.5 sprint):
2. CLI parity backfill batch 2 — next 5 subcommands (suggested: `get-descendants`,
   `get-callees`, `get-callers`, `get-callee-tree`, `get-caller-tree`).
3. CLI parity backfill batch 3 — next 5 (suggested: `get-imports`, `get-import-tree`,
   `get-stats`, `detect-cycles`, `rank-symbols`).
4. Charter §2 SLA — 100K-node benchmark row. (PR #168 covered 1K+10K; 100K row still TODO)
5. Parity checker `--strict` promotion — flip `parity.yml` from informational to required
   when CLI parity reaches ≥ 50 % (currently 4/89 = ~5 %).

P2 (v0.1.5 / governance):
6. `skills/INDEX.md` status flip — mark landed CLI rows from 🟡 to ✅ as each batch lands.
7. Security scan — routine post-sprint-4 check (post-v0.1.3 window).

## Dispatch state (2026-05-29, post-PM-run)

| Agent | Status | Current item |
|---|---|---|
| release | **P0 — next-up** | Cut release/v0.1.4; bump to 0.1.4; CHANGELOG date; PR to develop+main; tag; publish crates.io |
| rust-implementer | **next-up** | CLI parity batch 2 (5 subcommands). Branch from develop post-v0.1.4. |
| architect | idle | Charter §2 SLA 100K-node row — open PR to add the row after batch 2 lands |
| tech-writer | idle | Update skills/INDEX.md as CLI rows flip from 🟡 to ✅ after each batch |
| code-reviewer | idle | blocks on PR opens |
| security-reviewer | idle | next: routine post-sprint scan |
| e2e-runner | idle | next: extend cli_basic_queries.rs tests once batch 2 lands |

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder if available): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/v0.1.x branch, publish.

## Sprint v0.1.5 exit criteria (draft)

- [ ] CLI parity batch 2 (5 subcommands): `get-descendants`, `get-callees`, `get-callers`, `get-callee-tree`, `get-caller-tree`.
- [ ] CLI parity batch 3 (5 subcommands): `get-imports`, `get-import-tree`, `get-stats`, `detect-cycles`, `rank-symbols`.
- [ ] Charter §2 SLA 100K-node benchmark row added.
- [ ] `skills/INDEX.md` rows flipped to ✅ for all landed CLI batches.
- [ ] Security scan complete (no high-severity findings).

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- Skill marketplace listing metadata sign-off.

## Archive

### 2026-05-29 PM run (v0.1.4 close)

v0.1.4 sprint declared complete. All 7 exit criteria met:
- PR #168 — perf hardening + heavy-graph SLA + Criterion benches
- PR #170 — parity.yml + check_skill_parity.py + 12 naming fixes; 89/89 coverage
- PR #172 — CLI batch 1 (search-symbol, get-symbol-info, get-ancestors + 8 integration tests)
- PR #149 — PR template Three-Surface Self-Check (confirmed already live from RFC-0090 launch)
- PR #154 — mycelium init kept hidden (no new work needed)

This PM run attempted to independently implement CLI batch 1 (PR #173) before discovering
PR #172 already merged concurrently. PR #173 was closed as superseded. Anti-pattern note:
concurrent PM runs can duplicate work; inter-run state synchronisation depends on this file.

### 2026-05-29 PM run (v0.1.4 kickoff — prior run)

- #153 ✅ graph-algorithm timeouts fixed (PR #168)
- RFC-0090 Phase 1 ✅ parity.yml (PR #170)
- Confirmed all Phase 2/2.3 from v0.1.3 complete (89/89 coverage)

### 2026-05-30 PM call (v0.1.2 era — superseded)

1. v0.1.0 + v0.1.1 are shipped and on crates.io. Three-Surface Rule is law.
2. External eval (glm5.1) found 4 issues; orchestrator dogfood found 2 more. All filed.
3. Priority order is set. Sprint v0.1.2 kicks off on issue #150.
4. PRD for v0.2 is at [`docs/prd/v0.2-the-three-surface-release.md`](../prd/v0.2-the-three-surface-release.md).
5. No blocker from the founder needed at this checkpoint. Begin dispatch.
