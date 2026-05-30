# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-05-30 (post-v0.1.4 ship) |
| Current sprint | **v0.1.5 COMPLETE on develop, awaiting cut** |
| Active release branch | none — PR #195 (release/v0.1.4 → develop back-merge) open |
| Next release target | **v0.1.5** (cut after PR #195 back-merge), then v0.2.0 |
| Final release target | v0.2.0, ETA 2026-07-15 |
| Last shipped | **v0.1.4 — Perf + CLI parity batches 1-2 + Three-Surface CI scaffold** (tag v0.1.4, crates.io published 2026-05-30) |

---

## ✅ v0.1.4 SHIPPED — 2026-05-30

- PR #176 merged to main (commit 279834b)
- Tag `v0.1.4` pushed
- 5 crates published to crates.io: `mycelium-rcig-{core,hyphae,pack,mcp,cli}@0.1.4`
- GitHub Release: https://github.com/aimasteracc/mycelium/releases/tag/v0.1.4
- PR #195 (back-merge release/v0.1.4 → develop) **open and awaiting founder admin merge**
- After back-merge: cut release/v0.1.5 with bump 0.1.4 → 0.1.5, ship all v0.1.5 sprint work in one release

---

## v0.1.5 Sprint — ALL EXIT CRITERIA MET ✅

All items confirmed complete as of develop `34b989610`:

- [x] CLI parity batch 2: 7 subcommands (get-descendants … get-all-symbols, server-status). PR #175
- [x] CLI parity batch 3: 7 call-graph subcommands. PR #177
- [x] CLI parity batch 4: 3 import-graph subcommands. PR #178
- [x] CLI parity batch 5: 8 inheritance subcommands. PR #179
- [x] CLI parity batch 6: 12 reachability subcommands. PR #182
- [x] CLI parity batch 7: 14 centrality subcommands. PR #183
- [x] CLI parity batch 8: 14 graph-structure subcommands. PR #185
- [x] CLI parity batch 9: 4 batch-ops subcommands. PR #186
- [x] CLI parity batch 10 (FINAL): 10 misc subcommands + 4 MCP-only exceptions filed. PR #187
- [x] **RFC-0091 jQuery-style Hyphae selectors** (8 pseudo-classes + attribute selectors). PR #184
- [x] **89/89 capabilities Three-Surface compliant** (100% — Three-Surface Rule complete).
- [x] skills/INDEX.md flushed: all 89 rows ✅ (this PM run).
- [x] RFC-0090 marked Implemented (this PM run).
- [ ] parity.yml flipped to required — **blocked on v0.1.4 main merge** (PR #176).
- [ ] Marketplace metadata + asciinema — stretch, not blocking v0.1.5.

---

## Live priorities (ordered)

**P0: none** — issue queue is empty (0 open issues).

**Decision gates (require founder — treat as P0 blocker):**
1. **PR #176** → `main`: v0.1.4 release. Founder admin merge required. Unblocks v0.1.5 cut.

**P1 (v0.2.0 prep, start after v0.1.4 ships):**
2. `parity.yml` flip from informational to required (PR, rust-implementer).
3. Charter §2 SLA — add 100K-node heavy-graph rows (architect, PR #168 covered 1K+10K only).
4. Skill marketplace submission metadata: icon, screenshots, category examples (tech-writer).
5. End-to-end "first 5 minutes" walkthrough / asciinema recording (tech-writer).
6. RFC-0091 e2e tests — fixture tests against real repos for new selector forms (e2e-runner).

**P2 (v0.2.0 stretch):**
7. README badges + `cargo install` line updated to v0.1.4 / v0.1.5 (doc-updater).
8. Security scan — routine post-v0.1.4 window (security-reviewer).

---

## Dispatch state (2026-05-30, post-batch-10)

| Agent | Status | Current item |
|---|---|---|
| release | **blocked** | Waiting for founder to merge PR #176. |
| rust-implementer | idle | Next: parity.yml `--strict` promotion (post-v0.1.4 merge). |
| architect | idle | Charter §2 SLA 100K-node row. |
| tech-writer | idle | Marketplace metadata + asciinema. |
| code-reviewer | idle | Blocks on PR opens. |
| security-reviewer | idle | Routine post-v0.1.4 scan. |
| e2e-runner | idle | RFC-0091 selector e2e tests. |
| doc-updater | idle | README badges + install line. |

---

## v0.1.4 Sprint — COMPLETE on develop ✅ (awaiting main merge)

All items confirmed complete as of develop `be317da` (merged into release/v0.1.4):

- [x] All 6 timeout tools < 2 s on 1K-node graph. PR #168
- [x] New SLA rows in Charter §2 for heavy-graph algorithms. PR #168
- [x] Benchmarks in `benches/` for each of the 6 tools. PR #168
- [x] `parity.yml` CI workflow live (informational). PR #170
- [x] `mycelium init` resolved — keeping hidden. PR #154
- [x] PR template Three-Surface Self-Check section. PR #149
- [x] CLI parity backfill batch 1: `search-symbol`, `get-symbol-info`, `get-ancestors`. PR #172

---

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder if available): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/v0.1.x branch, publish.

---

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- Skill marketplace listing metadata sign-off.
- **Merging any `release/*` branch to `main`** (Charter §5.12 — GPG-signed founder approval).

---

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

### 2026-05-29 PM run (v0.1.4 kickoff)

- #153 ✅ graph-algorithm timeouts fixed (PR #168)
- RFC-0090 Phase 1 ✅ parity.yml (PR #170)
- Confirmed all Phase 2/2.3 from v0.1.3 complete (89/89 test coverage)

### 2026-05-30 PM call (v0.1.2 era — superseded)

1. v0.1.0 + v0.1.1 shipped and on crates.io. Three-Surface Rule is law.
2. External eval (glm5.1) found 4 issues; orchestrator dogfood found 2 more. All filed.
3. Priority order set. Sprint v0.1.2 kicked off on issue #150.
4. PRD for v0.2 at [`docs/prd/v0.2-the-three-surface-release.md`](../prd/v0.2-the-three-surface-release.md).
5. No blocker from founder at this checkpoint. Begin dispatch.
