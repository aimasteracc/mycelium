# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-06-04 (PM dispatch v37 — PR #524 merged; Codex P2 on PR #523 addressed; Issue #525+#526 filed) |
| Current sprint | **v0.2.0 — IN PROGRESS** (PR #523 CI running; founder merge + tag + GitHub Release pending) |
| Active release branch | `release/v0.2.0` — PR #523 open (CI in_progress: binary builds + release workflow running) |
| Next release target | **v0.2.0** — RFC-0109 parity + RFC-0102 budget + RFC-0110 npm/bun distribution |
| Final release target | v0.2.0 (THIS release), v0.3.0 ETA 2026-09-01 |
| Last shipped | **v0.1.18 (ALL 4 STEPS COMPLETE)** — PR #490 founder-merged 2026-06-03T12:32Z, tags v0.1.17+v0.1.18 pushed, GitHub Releases created. |

---

## ✅ v0.1.13 — SHIPPED (ceremony COMPLETE)

- RFC-0093 Phase 2, RFC-0096 Phase 1 (Python), TS relative-import fix, ADR-0004/0005/0006.
- Ceremony: all 4 steps complete.

---

## ✅ v0.1.14 — SHIPPED (ceremony 4/4 COMPLETE)

- RFC-0096 Phase 2 TypeScript, RFC-0093 Phase 3 (BREAKING MCP is_error), Skills INDEX CI gate, dogfood 8/8.
- Ceremony: all 4 steps complete.

---

## ✅ v0.1.15 — CONTENT DONE; CEREMONY BROKEN (superseded by v0.1.16)

- Steps 1–4 all failed. Content absorbed into v0.1.16.

---

## ✅ v0.1.16 — SHIPPED (ceremony 4/4 COMPLETE — 2026-06-02)

**What shipped:** RFC-0100 Phase 1+2 (redb StorageBackend), RFC-0101/0102/0103 drafts, ceremony script, dep bumps, mycelium_context (90th MCP tool) + OutputBudget.

**Ceremony status — ALL FOUR STEPS COMPLETE ✅:**
- [x] **Step 1**: PR #332-equivalent merged → main ✅
- [x] **Step 2**: Tag `v0.1.16` pushed ✅
- [x] **Step 3**: GitHub Release published 2026-06-02T01:27:33Z ✅
- [x] **Step 4**: Back-merge → develop ✅

---

## ✅ v0.1.17 — CRATES PUBLISHED; GIT CEREMONY MERGED VIA v0.1.18 PR #490

**Content:** RFC-0101 Phase 2 (`mycelium context` CLI twin), RFC-0102 Implemented, RFC-0100 Phase 3 (redb default), RFC-0104, god-file-split slices 1+2, 100k SLA gate, ADR-0008/0009.

**Ceremony status — COMPLETE (via retro-tag):**
- [x] **Step 1**: Content merged to main via PR #490 (v0.1.18 PR with -X ours, 2026-06-03T12:32Z) ✅
- [x] **Step 2**: Tag `v0.1.17` retro-pushed at `6aa1bed` (crates.io v0.1.17 published) ✅
- [x] **Step 3**: GitHub Release created ✅
- [x] **Step 4**: PR #477 back-merged → develop ✅

---

## ✅ v0.1.18 — SHIPPED (ceremony ALL 4 STEPS COMPLETE — 2026-06-03)

**What shipped:**
- [x] RFC-0107 SUBSCRIBE: `mycelium_subscribe`, `mycelium_unsubscribe`, `mycelium_subscription_status` (93 MCP tools total). `mycelium watch --subscribe` CLI.
- [x] RFC-0108 Salsa Phase 2: `mycelium/queryResultChanged` reactive query subscriptions. BLAKE3-128 hash dedup. 5 query kinds.
- [x] fix(subscribe): `RwLock::blocking_read()` → `try_read()` P1 safety (PR #479).
- [x] fix(packs/rust): `Type::method()` + `crate::mod::func()` call sites (PR #474).
- [x] Reactive-completion roadmap: **4/4 COMPLETE** (watch ✅ push ✅ subscribe ✅ salsa ✅).
- [x] RFC-0105 EXCEPTION ratified by founder 2026-06-03T12:30Z.
- [x] New Hard Rule: Codex review findings must be addressed before merge.

**Ceremony status — ALL FOUR STEPS COMPLETE ✅:**
- [x] **Step 1**: PR #490 merged → main by founder 2026-06-03T12:32Z (-X ours strategy) ✅
- [x] **Step 2**: Tag `v0.1.18` pushed ✅
- [x] **Step 3**: GitHub Release created ✅
- [x] **Step 4**: PR #483 back-merged → develop 2026-06-03T09:10:56Z ✅

---

## ⚠️ v0.1.19 / v0.1.20 — INTERMEDIATE; v0.1.20 SUPERSEDED BY v0.2.0

**Content shipped on develop (post-v0.1.18, PM dispatches v26–v36):**
- [x] RFC-0109 Graph-list CLI↔MCP parity: `get_callees`, `get_callers`, `get_dead_symbols`, `get_isolated_symbols`, `get_reachable`, `get_reachable_to`, `get_all_symbols` — `--format json` emits byte-identical named-object output. Shared `mycelium_core::queries` builder.
- [x] RFC-0102 Phase 2 (budget roll-out COMPLETE): `budget_ms` per-call override on all 7 RFC-0109 tools.
- [x] RFC-0090 status: **Implemented** — Three-Surface Rule fully satisfied across all 93+ capabilities.
- [x] Security scans post-v0.1.18: CLEAN.
- [x] ADR-0009 (redb-storage-engine) renaming complete.

**v0.1.20 ceremony status — SUPERSEDED:**
- ❌ Git ceremony skipped (same systemic release.yml auto-close pattern). crates.io v0.1.20 orphan-published.
- PR #515 closed as superseded by v0.2.0.
- v0.2.0 incorporates all v0.1.20 content + RFC-0110.

---

## 🔥 v0.2.0 — IN PROGRESS ("The Three-Surface Release")

**What ships in v0.2.0 (from PR #523):**
- [x] **RFC-0109** — Graph-list CLI↔MCP output parity (7/7 tools). `--format json` byte-identical to MCP output.
- [x] **RFC-0102** — Adaptive output budget (COMPLETE roll-out). `budget_ms` knob on all 7 RFC-0109 tools.
- [x] **RFC-0110** — npm/bun CLI distribution: `npx mycelium-rcig` / `bunx mycelium-rcig` works out-of-the-box. 5 platform binaries.
- [x] **Version bump**: 0.1.19 → 0.2.0 (semver major milestone; BREAKING `--format json` output shape per RFC-0109 Option A).
- [x] **RFC-0090**: marked Implemented. Three-Surface Rule fully enforced by CI.

**v0.2 PRD success metrics:**
| Metric | Target | Status |
|---|---|---|
| Three-Surface Rule | 88/88 capabilities | ✅ CI-enforced |
| Dogfood pass rate | 8/8 CLI commands | ✅ E2E CI green |
| npm/bun distribution | marketplace | ✅ RFC-0110 |
| RFC-0090 status | Implemented | ✅ this release |

**v0.2.0 ceremony status — IN PROGRESS ⚠️:**
- [x] Release branch `release/v0.2.0` cut from develop by founder 2026-06-04T05:26:18Z
- [x] CI: Quality Gate ✅ SUCCESS (all checks green on latest run)
- [x] Codex P2 finding addressed: Issue #525 + reply on PR #523 ✅
- ⚠️ `publish to crates.io` via release.yml: running (in_progress, may orphan-publish per systemic pattern)
- ⚠️ `preflight (npm token present)`: **FAILED** — `NPM_TOKEN` secret missing → npm package NOT published by release workflow. **Founder must add NPM_TOKEN secret OR publish manually.**
- ⏳ **Step 1**: PR #523 CI still running (binary builds in_progress). Merge pending founder (Charter §5.12: CI must be green).
- ❌ **Step 2**: Tag `v0.2.0` NOT created yet.
- ❌ **Step 3**: GitHub Release NOT created yet.
- ❌ **Step 4**: Back-merge `release/v0.2.0` → `develop` — PM will open after Step 1.
- **Repair**: Once CI fully green → founder merges PR #523 → push tag `v0.2.0` → create GitHub Release. If release.yml auto-closes again, use `scripts/release-ceremony.sh`.

---

## Live priorities (ordered)

**P0 (v0.2.0 ceremony — CI completing, founder merge imminent):**
1. **⏳ WAIT**: PR #523 CI binary builds in_progress (~5 min ETA). All quality checks ✅ already.
2. **Founder: merge PR #523** once ALL CI checks SUCCESS/SKIPPED (Charter §5.12 gate).
3. **Founder: push tag `v0.2.0`** + create GitHub Release.
4. **PM: open back-merge PR** `release/v0.2.0` → `develop` after Step 1.
5. **⚠️ NPM_TOKEN missing**: npm publish skipped. Founder must add `NPM_TOKEN` repo secret OR publish `@aimasteracc/mycelium-rcig@0.2.0` manually.

**P0 done this run ✅ (v37):**
- PR #524 (PM v36 chore, 22/22 CI green) MERGED ✅ (`b2fe917`)
- Issue #525 created: fix(npm) 128+signal exit code (Codex P2 deferred to v0.2.1) ✅
- Issue #526 created: P1 mutation kill-rate < 70% nightly gate ✅
- Codex P2 on PR #523 replied + spin-off issue: pre-merge checklist satisfied ✅

**P1 (post-v0.2.0 quality):**
6. **Issue #526**: mutation testing kill-rate < 70% on nightly (Charter §2 gate). Investigate survived mutants in RFC-0109/RFC-0108 paths (rust-implementer).
7. **Dogfood re-run** post-v0.2.0 (e2e-runner; 8/8 CLI including new `--format json`).
8. **RFC-0104 cold SLA numbers** — nightly `sla_ancestors_100k` (bench agent).
9. **Issue #525**: fix(npm) 128+signal exit code in mycelium.cjs (rust-implementer, v0.2.1).

**P2 (v0.3.0 scope):**
10. Skill marketplace submission to Claude Code marketplace.
11. "First 5 minutes" walkthrough validation.
12. `release.yml` systemic DCO + auto-close fix (before v0.3.0).
13. Issue #428 god-file-split remaining slices.

---

## Dispatch state (2026-06-04 v37 — PR #524 merged; PR #523 CI running; v0.2.0 ceremony imminent)

| Agent | Status | Current item |
|---|---|---|
| founder | **action requested** | **(1)** Merge PR #523 once all CI green (Charter §5.12). **(2)** Push tag `v0.2.0` + create GitHub Release. **(3)** Add `NPM_TOKEN` repo secret for npm publish OR publish `@aimasteracc/mycelium-rcig@0.2.0` manually. |
| PM | **DONE ✅** | v37 complete: PR #524 merged; Issues #525+#526 filed; Codex P2 addressed; PM state v37 pushed. |
| release | **WAITING** | PR #523 CI completing; ceremony steps 1–3 founder-gated. |
| rust-implementer | **P1** | Issue #526: mutation kill-rate fix (investigate survived mutants, add targeted assertions). |
| e2e-runner | **P1** | Dogfood re-run post-v0.2.0 (8/8 CLI with `--format json`). |
| bench | **P1** | `sla_ancestors_100k` nightly for RFC-0104 cold SLA. |
| security-reviewer | **idle** | Post-v0.2.0 security scan (queue after ceremony complete). |
| architect | **idle** | ADR-0010 for RFC-0109 output-shape decision (if needed). |
| tech-writer | **P2** | Skill marketplace submission prep. |

---

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- **Skill marketplace listing metadata** sign-off.
- **RFC-0104 cold SLA measurement**: Charter §2 warm/cold split amendment requires measured nightly data.
- **NPM_TOKEN**: Add repo secret for npm publish to work in release.yml. Required for RFC-0110 distribution.
- **Systemic**: `release.yml` finalize merge step — fix or bypass. P2 before v0.3.0.

---

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/vX.Y.Z branch, publish.

---

## Archive

### 2026-06-04 PM dispatch v37 (PR #524 merged; Codex P2 addressed; Issues #525+#526 filed)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl (entries 1-28, note: entries for dispatches v26-v36 are on develop post-PR #524), anti-patterns (no new domain hits for this run's tasks), PM state (develop v36 — local disk stale at v25; authoritative state from GitHub develop branch post-PR #524 merge), v0.2 PRD.

**Assessment:**
- 2 open PRs: #523 (release/v0.2.0 → main, CI mostly green — windows test done, binary builds in_progress), #524 (PM v36 chore, 22/22 CI SUCCESS ✅ — merged this run).
- 0 open P0/P1 issues (pre-run). 0 open labeled issues.
- CI recent runs: Release workflow in_progress (runs #26936160034 + #26935413616). Quality Gate ✅ from earlier run. Nightly run #26934880069: `mutation testing (kill-rate >= 70%)` FAILED (all other nightly jobs SUCCESS).
- PR #523: 1 Codex P2 review (`chatgpt-codex-connector`, `discussion_r3353893253`) — signal handling in npm/mycelium/bin/mycelium.cjs line 71 always exits 1 instead of 128+signal.
- `preflight (npm token present)` job FAILED in release.yml run — `NPM_TOKEN` repo secret missing → npm packages won't publish autonomously.

**Actions taken:**
1. **Merged PR #524** (chore/pm-dispatch-2026-06-04-v34, 22/22 CI green, squash `b2fe917`) ✅
2. **Created Issue #525** (`fix(npm): use 128+signal exit code in mycelium.cjs`, P2, labels: P2/npm/good-first-issue) ✅
3. **Replied to Codex P2** on PR #523 (`discussion_r3353893253`): acknowledged finding valid; not blocking v0.2.0; tracked as Issue #525 for v0.2.1 ✅
4. **Created Issue #526** (`P1: nightly mutation testing kill-rate below 70% gate`, P1, labels: P1/quality/testing) ✅
5. **Updated PM state v37**: reflects v0.1.18 ceremony COMPLETE (founder 2026-06-03), v0.1.19/v0.1.20 intermediate superseded by v0.2.0, v0.2.0 IN PROGRESS, new P1 issue, new P2 issue. ✅
6. **Appended decisions.jsonl** entry (this run). ✅

**Sprint status:** v0.2.0 content 100% on release branch. PR #523 CI nearly complete. The only autonomous blocker remaining is Charter §5.12 green-CI gate before founder merge.

**Escalations to founder:**
- **(1) PR #523 ceremony (P0)**: Once CI fully green → merge + tag `v0.2.0` + GitHub Release.
- **(2) NPM_TOKEN missing (P1)**: Add `NPM_TOKEN` repo secret. Without it, `@aimasteracc/mycelium-rcig` npm package won't publish in release.yml. RFC-0110 value proposition requires this.
- **(3) mutation kill-rate (P1)**: Issue #526 — rust-implementer dispatch needed.

### 2026-06-03 PM dispatches v26-v36 (founder-driven: v0.1.18 ceremony, RFC-0109/0102/0110, v0.2.0 branch)

**Summary (condensed from individual run archives now on develop):**
- v26-v27: Post-v0.1.18 dogfood + RFC-0109 initial work (graph-list parity, 7 tools).
- v28-v30: RFC-0109 TDD complete; RFC-0102 Phase 2 (OutputBudget rollout to all 7 RFC-0109 tools).
- v31-v32: RFC-0110 npm/bun distribution drafted + implemented; v0.1.19/v0.1.20 release attempts.
- v33: PR #522 merged (PM dispatch v33). PR #515 closed (v0.1.20 superseded).
- v34-v35: v0.2.0 release branch cut by founder 2026-06-04T05:26:18Z; PR #523 opened; PR #515 closed.
- v36 (PR #524): PR #522 merged; v0.1.20 superseded noted; PR #523 CI running; PM state v36.

**Key decisions in this period:**
- **Founder action 2026-06-03T12:32Z**: PR #490 merged `release/v0.1.18` → `main` (-X ours), tags v0.1.17+v0.1.18 retro-pushed, GitHub Releases created. v0.1.18 ceremony COMPLETE.
- **RFC-0105 EXCEPTION** ratified by founder 2026-06-03T12:30Z.
- **Hard Rule added** (2026-06-03T22:00Z approx): Codex review findings must be addressed before merge.
- **RFC-0090**: marked Implemented — Three-Surface Rule fully satisfied for all 93+ capabilities.

### 2026-06-03 PM dispatch v25 (PRs #485+#486 merged; ADR-0009 renaming; v0.1.18 ceremony escalated)

- Merged PRs #485 (ADR-0008 docs) + #486 (PM chore v24). Fixed ADR-0008→0009 numbering collision. Updated CHANGELOG. Escalated v0.1.18 ceremony to founder.

### Earlier dispatches (2026-06-02 to 2026-06-03 v14–v24)

- v14–v18: release.yml `crates_published` URL fix (tr '-' '_' → direct hyphen), max_version field fix, cherry-picks to release/v0.1.17. Multiple PRs merged (fixes, PM chores).
- v19–v22: RFC-0107 CI fixed (rustdoc + coverage), rebased, merged. RFC-0108 doc-only PR #473 merged. Back-merge PR #477 opened. Reactive roadmap 4/4 COMPLETE confirmed. INDEX.md subscribe rows added.
- v23–v24: v0.1.18 release branch cut. PR #482 AUTO-CLOSED (systemic). PR #484 merged. PR #452 closed (superseded). Security scan CLEAN. ADR-0008 drafted (PR #485).

### Earlier dispatches (2026-05-29 to 2026-06-01)

- v0.1.4 sprint closed. v0.1.10–v0.1.14 shipped (ceremonies complete). Skills CI gate. Dogfood 8/8. RFC-0093/0096/0100/0101/0102/0103 implemented.
