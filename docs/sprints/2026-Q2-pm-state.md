# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-06-04 (PM dispatch v46 — PR #541 merged (squash `e089b66a`); Codex P1+P2 fixed; security scan post-v0.2.0 CLEAN; Step 2 tag push awaits founder) |
| Current sprint | **v0.2.0 ceremony STEPS 1+3+4 COMPLETE** — Step 1 ✅ (PR #523→main); Step 3 ✅ (crates.io published); Step 4 ✅ (PR #537 back-merge `4e60400f`); Step 2 (tag push) awaits founder. |
| Active release branch | none — `release/v0.2.0` back-merged to develop ✅; Step 2 (tag) pending |
| Next release target | **v0.2.1** — npm scope registration + E404 tightening (Issue #534), post-v0.2.0 backlog |
| Final release target | v0.2.0 ceremony closing; v0.3.0 ETA TBD |
| Last shipped | **v0.1.19 (ceremony COMPLETE)** — v0.2.0 Steps 1+3+4 done; Step 2 (tag push) pending founder. |

---

## ✅ v0.1.13 — SHIPPED (ceremony COMPLETE)

**What shipped:**
- [x] RFC-0093 Phase 2: `success_str` exported from error module; all 101 MCP success-return sites unified
- [x] RFC-0096 Phase 1 (Python): `EdgeKind::TypeImports` for `if TYPE_CHECKING:` imports
- [x] TypeScript relative-import resolver bug fix (`@reference.import` now dispatches to TS resolver for .ts/.js files)
- [x] ADR-0004: Patricia Trie for Trunk documented
- [x] ADR-0005: MessagePack wire format documented
- [x] ADR-0006: Hyphae CSS-selector grammar style documented
- [x] Post-v0.1.12 security scan: CLEAN

**v0.1.13 ceremony status — ALL FOUR STEPS COMPLETE ✅:**
- [x] **Step 1**: `release/v0.1.13` → `main` — PR #332 MERGED ✅ (founder authorized 2026-05-31)
- [x] **Step 2**: Tag `v0.1.13` pushed ✅
- [x] **Step 3**: GitHub Release published ✅
- [x] **Step 4**: Back-merge `release/v0.1.13` → `develop` — PR #333 MERGED ✅

---

## ✅ v0.1.14 — SHIPPED (ceremony 4/4 COMPLETE)

**What shipped:**
- [x] RFC-0096 Phase 2 TypeScript: `import type` → TypeImports edges + TS resolver bug fix
- [x] RFC-0093 Phase 3 (BREAKING): all 89 MCP tools → `is_error: Some(true)` per MCP spec
- [x] Skills INDEX.md CI gate: `skill-parity` promoted to required Quality Gate
- [x] Store::merge R1 parallel-index primitive (step 1/2)
- [x] Dogfood pass rate 8/8: all 8 core CLI commands green

**v0.1.14 ceremony status — ALL FOUR STEPS COMPLETE ✅:**
- [x] **Step 1**: `release/v0.1.14` → `main` — PR #352 MERGED ✅
- [x] **Step 2**: Tag `v0.1.14` pushed ✅
- [x] **Step 3**: GitHub Release published ✅
- [x] **Step 4**: Back-merge `release/v0.1.14` → `develop` — PR #349 MERGED ✅

---

## ✅ v0.1.15 — CONTENT DONE; CEREMONY BROKEN (superseded by v0.1.16)

**v0.1.15 ceremony status — BROKEN ⚠️ (orphan tag; content absorbed into v0.1.16):**
- ❌ Steps 1–4: all failed (release.yml CRATES_IO_TOKEN failure; orphan tag; PRs #361/#362 closed unmerged)
- **Resolution**: v0.1.15 content absorbed into v0.1.16 release.

---

## ✅ v0.1.16 — SHIPPED (ceremony 4/4 COMPLETE — 2026-06-02)

**What shipped:**
- [x] RFC-0100 Phase 1+2: redb `StorageBackend` trait + `InMemoryBackend` + `RedbBackend` (feature-flagged)
- [x] RFC-0101 draft, RFC-0102 draft, RFC-0103 draft
- [x] MCP server routing instructions + primary tool-selection decision tree
- [x] Incremental persistence journal (Issue #343)
- [x] Memory budget / bounded store (Issue #344)
- [x] Release ceremony script `scripts/release-ceremony.sh`
- [x] Dep bumps: redb 2.6.3→4.1, logos 0.14→0.16, salsa 0.18→0.26
- [x] mycelium_context (90th MCP tool) + OutputBudget + import-aware stub resolution

**v0.1.16 ceremony status — ALL FOUR STEPS COMPLETE ✅:**
- [x] **Step 1**: `release/v0.1.16` → `main` — commit `0d27c5a` 2026-06-02T01:27Z ✅
- [x] **Step 2**: Tag `v0.1.16` pushed ✅
- [x] **Step 3**: GitHub Release published 2026-06-02T01:27:33Z ✅
- [x] **Step 4**: Back-merge `release/v0.1.16` → `develop` — commit `cb31814` 2026-06-02T01:28Z ✅

---

## ⚠️ v0.1.17 — CRATES PUBLISHED; GIT CEREMONY SUPERSEDED BY v0.1.18

**v0.1.17 ceremony status — PARTIAL (crates only; git superseded by v0.1.18):**
- [x] **Pre-release**: `publish to crates.io/npm/PyPI` ✅ — all 5 crates at v0.1.17.
- [x] **Step 4**: Back-merge `release/v0.1.17` → `develop` — **PR #477 MERGED ✅** 2026-06-03T07:54Z
- [x] **Retro-tag**: `v0.1.17` pushed at `6aa1bed` (2026-06-03T12:30Z) for traceability ✅
- ✅ Git ceremony superseded: main jumps v0.1.16 → v0.1.18 → v0.1.19. Founder confirmed.

---

## ✅ v0.1.18 — SHIPPED (ceremony 4/4 COMPLETE — 2026-06-03)

**What shipped in v0.1.18:**
- [x] **RFC-0107 SUBSCRIBE**: `mycelium_subscribe`, `mycelium_unsubscribe`, `mycelium_subscription_status` (3 new MCP tools = 93 total). `mycelium watch --subscribe` CLI face.
- [x] **RFC-0108 Salsa Phase 2**: `mycelium/queryResultChanged` reactive query subscriptions. BLAKE3-128 hash. 5 query kinds. 2s quiet-period, 200ms eval-budget.
- [x] **fix(subscribe)**: Replace `RwLock::blocking_read()` with `try_read()` in async watch paths (PR #479).
- [x] **fix(packs/rust)**: Capture `Type::method()` and `crate::mod::func()` call sites (PR #474).
- Reactive-completion roadmap: **4/4 COMPLETE** (watch ✅ push ✅ subscribe ✅ salsa ✅).

**v0.1.18 ceremony status — ALL FOUR STEPS COMPLETE ✅ (2026-06-03):**
- [x] **Step 1**: PR #490 merged `release/v0.1.18` → main ✅
- [x] **Step 2**: Tag `v0.1.18` pushed ✅ (SHA e429a224, 2026-06-03T12:30Z)
- [x] **Step 3**: GitHub Release v0.1.18 created ✅ (2026-06-03T12:30Z)
- [x] **Step 4**: Back-merge PR #483 MERGED to develop ✅ (2026-06-03T09:10:56Z)
- [x] RFC-0105 EXCEPTION ratified by founder — PR #491 (2026-06-03)

---

## ✅ v0.1.19 — SHIPPED (ceremony 4/4 COMPLETE — 2026-06-03T15:49Z)

> **⚠️ Content boundary note (Codex audit 2026-06-03):** PRs #497–#501 were verified
> via `git log 8ffcad9..bb685def --first-parent` to have landed on develop **after**
> the v0.1.19 release merge (`8ffcad9 #494`). They are **not** in v0.1.19; they belong
> in the post-v0.1.19 unreleased section below.

**What shipped in v0.1.19 (release branch content only):**
- [x] fix(packs/rust): extractor precision 67% → 99.8% recall — 5 additive queries.scm patterns (PR #492)
- [x] docs(adr): ADR-0008 redb as default backend (PR #485); ADR-0009 numbering fix (PR #486)
- [x] docs(rules): Codex review Hard Rule added to CLAUDE.md (PR #488); vision scorecard updated (PR #489)
- [x] RFC-0105 EXCEPTION: WatchEngine Three-Surface exception ratified (PR #491)

**v0.1.19 ceremony status — ALL FOUR STEPS COMPLETE ✅:**
- [x] **Step 1**: `release/v0.1.19` → `main` — founder ceremony ✅
- [x] **Step 2**: Tag `v0.1.19` pushed ✅ (SHA 55761a85, 2026-06-03)
- [x] **Step 3**: GitHub Release v0.1.19 created ✅ (2026-06-03T15:49Z)
- [x] **Step 4**: Back-merge PR #493 MERGED ✅ (develop HEAD = `55761a85`)

---

## ⚠️ v0.1.20 — CRATES PUBLISHED; GIT CEREMONY SUPERSEDED BY v0.2.0

**v0.1.20 ceremony status — SUPERSEDED BY v0.2.0 ⚠️:**
- [x] Release branch `release/v0.1.20` cut from develop
- [x] **crates.io v0.1.20 published** ✅ (orphan, 2026-06-04T01:17Z)
- [x] **npm v0.1.20 published** ✅ (orphan)
- [x] **PyPI v0.1.20 published** ✅ (orphan)
- [x] **PR #515 closed** as superseded (PM dispatch v36)
- ✅ Git ceremony superseded: main jumps v0.1.19 → v0.2.0. Founder decision.
- ❌ **Steps 2, 3, 4**: skipped per supersession strategy.

**Resolution**: v0.1.20 content (RFC-0109 7/7, RFC-0102 budget, RFC-0110 npm) absorbed into v0.2.0.

---

## ✅ RFC-0110 — npm/bun CLI distribution (ALL 3 INCREMENTS COMPLETE on develop)

**Goal:** `npm i -g @aimasteracc/mycelium && mycelium --version` works on machines without Cargo.

- [x] **Increment 1** (PR #517, merged 2026-06-04T02:15Z): npm package scaffolding
- [x] **Increment 2** (PR #519, merged 2026-06-04T02:26Z): `release.yml` cross-compile matrix
- [x] **Increment 3** (PR #520, merged 2026-06-04T02:56Z): `publish-npm` job rewired + CI smoke test

**Status:** RFC-0110 **Implemented** on develop. Goes live at **v0.2.0**.

---

## 🔥 v0.2.0 — "The Three-Surface Release" CEREMONY STEPS 1+4 DONE

**Founder-cut 2026-06-04T05:26:18Z** — `release/v0.2.0` branched from develop.

**What ships in v0.2.0:**
- [x] **RFC-0109** — graph-list CLI↔MCP output parity 7/7 tools COMPLETE
- [x] **RFC-0102** — adaptive output budget roll-out COMPLETE
- [x] **RFC-0110** — npm/bun CLI distribution (Increments 1+2+3)
- [x] CHANGELOG [Unreleased] sealed + consolidated into [0.2.0]; version bump 0.1.19→0.2.0
- [x] README: npm/bun install documented (coming soon wording; live once Issue #534 resolved)
- [x] DCO sign-off fixed: all 21 non-merge commits carry `Signed-off-by`

**v0.2.0 ceremony status — STEP 2 PENDING (founder action required):**
- [x] **Step 1**: PR #523 MERGED → `main` ✅ (2026-06-04T10:41:45Z)
- [ ] **Step 2**: Tag `v0.2.0` pushed — **founder action required**
- [x] **Step 3**: All 5 crates to crates.io ✅ (release.yml, 2026-06-04)
- [x] **Step 4**: PR #537 MERGED → `develop` ✅ (squash `4e60400f`, 2026-06-04T14:07Z)

**Note on npm/PyPI:** PyPI ✅ published. npm `@aimasteracc` scope not yet registered; `publish to npm` exits 0 gracefully (Issue #534). Draft GitHub Release at `untagged-eb9b123` will be published by founder when tag is pushed.

---

## Live priorities (ordered)

**P0 (v0.2.0 ceremony — founder action required):**
1. **Push tag `v0.2.0`** (Charter §5.12 Step 2 — sole remaining ceremony gate; Steps 1+3+4 done ✅). GitHub Release follows in the same UX action but is not a ceremony gate.
2. **Register `@aimasteracc` npm scope** on npmjs.com (Issue #534) — enables real npm publish for v0.2.1+.

**P1 (quality — post v0.2.0 ceremony):**
4. ~~**Security scan post-v0.2.0**~~ — ✅ DONE (dispatch v46, CLEAN).
5. **Dogfood re-run** — RFC-0109 object shapes + RFC-0110 npm launcher + redb-as-default + watch --subscribe (8/8 CLI).
6. **RFC-0104 cold SLA numbers** — nightly `sla_ancestors_100k` for Charter §2 cold-open budget.
7. **Add NPM_TOKEN secret** to `npm` environment — enables npm publish on next release.

**P2 (post-v0.2.0):**
8. Issue #534 — npm scope E404 tightening once @aimasteracc scope registered.
9. `release.yml` systemic auto-close fix (ceremony script is current workaround).
10. **Systemic DCO fix** (for v0.3.0+): update `dco-check` script in `ci.yml` to grep full commit message body.
11. Issue #428 god-file-split remaining slices.
12. Skill marketplace submission to Claude Code marketplace.
13. "First 5 minutes" walkthrough validation.

---

## Dispatch state (2026-06-04 v45 — PR #537 merged (Step 4 ✅); #539/#540 closed)

| Agent | Status | Current item |
|---|---|---|
| founder | **action requested (P0)** | **(P0)** Push tag `v0.2.0` (Charter §5.12 Step 2 — only remaining ceremony gate). **(P0)** Register `@aimasteracc` npm scope (Issue #534). |
| PM | **DONE ✅** | v46: PR #541 merged (Codex P1+P2 fixed); security scan CLEAN; #542 open. |
| release | **WAITING** | v0.2.0 ceremony: Steps 1+3+4 ✅. Step 2 (tag push) founder-gated. |
| security-reviewer | **DONE ✅** | Post-v0.2.0 scan: CLEAN (dispatch v46). |
| architect | **DONE ✅** | ADR-0009 ✅, ADR-0010 ✅. |
| e2e-runner | **P1** | Dogfood re-run: RFC-0109 object shapes + RFC-0110 npm + redb-as-default + watch --subscribe. |
| bench | **P1** | `sla_ancestors_100k` nightly for RFC-0104 cold SLA. |
| tech-writer | **P1** | Marketplace submission (v0.2.0 ships npm — right time to submit). |
| rust-implementer | idle | Next sprint backlog (P2 items). |

---

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- Skill marketplace listing metadata sign-off.
- **RFC-0104 cold SLA measurement**: Charter §2 table amendment requires measured nightly data.
- ~~**RFC-0105 Three-Surface EXCEPTION**~~: ✅ RATIFIED 2026-06-03T12:30Z.
- ~~**v0.1.17 git ceremony skip**~~: ✅ RESOLVED.
- ~~**v0.1.20 ceremony**~~: SUPERSEDED by v0.2.0. Founder confirmed.
- **v0.2.0 ceremony**: Step 1 ✅ (PR #523). Step 4 ✅ (PR #537 `4e60400f`). **Founder: push tag `v0.2.0` + create GitHub Release.**
- **Register `@aimasteracc` npm scope**: npmjs.com account creation + org scope registration. One-time founder action.
- **Systemic DCO config**: update `dco-check` script in `ci.yml` (same issue will recur on every future release with squash-merged commits).

---

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder if available): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/vX.Y branch, publish.

---

## Archive

### 2026-06-04 PM dispatch v46 (this run — PR #541 merged; Codex P1+P2 fixed; security scan CLEAN)

**Pre-flight:** CHARTER §2/§5.1/§5.10/§5.12/§5.13, _orchestrator, decisions tail-20, anti-patterns (1 new hit: MCP resource-prefix), PM state v45 (PR #541 open), v0.2 PRD.

**Assessment:**
- 1 open PR: #541 (chore pm-dispatch-v45, CI 22/22 ✅ on original commit; 2 Codex findings open).
- 0 open issues.
- v0.2.0 ceremony: Steps 1+3+4 done; Step 2 (tag) awaits founder.

**Actions taken:**
1. **Fixed Codex P2** (Hard Rule violation): reverted decisions.jsonl line 1 — MCP GitHub tool prepended resource-reference prefix to `DECISIONS_CONTENT_PLACEHOLDER`, rewriting an append-only line. Reverted. ✅
2. **Fixed Codex P1** (ceremony tracking): corrected Step 3 label from "GitHub Release" (wrong) to "crates.io publish" (Charter §5.12 correct). Step 3 already done; Step 2 (tag) is sole remaining founder action. ✅
3. **Replied to both Codex threads** on PR #541 (P1: fixed in `858af01`; P2: fixed in `858af01`). ✅
4. **Security scan post-v0.2.0**: CLEAN — no secrets, unsafe blocks documented, no shell injection, npm launcher secure. ✅
5. **Recorded anti-pattern**: MCP GitHub tool read prepends resource-reference prefix; use local Read tool for append-only memory files. ✅
6. **Squash-merged PR #541** (commit `e089b66a`) — 19/19 CI ✅. ✅

**Escalations to founder:**
- **(P0)** Push tag `v0.2.0` (Charter §5.12 Step 2; Steps 1+3+4 done ✅). GitHub Release creation follows from the same UX action but is not itself a ceremony gate.
- **(P0)** Register `@aimasteracc` npm scope on npmjs.com (Issue #534).

---

### 2026-06-04 PM dispatch v45 (PR #537 merged (Step 4); #539/#540 closed)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns (no new domain hits), PM state v42 (from develop HEAD `4e60400f` post-#537 merge), v0.2 PRD.

**Assessment:**
- 3 open PRs: #537 (back-merge CI 20/20 ✅, Codex P2 fixed in `7a5987a`), #539 (PM v44, CI 22/22 ✅), #540 (back-merge duplicate, CI 20/20, Codex P2 LIVE — same README npm issue).
- 1 open issue: #534 (P2 npm scope E404 tightening, not blocking).
- Key finding: duplicate back-merge PRs (#537 and #540 both target Charter §5.12 Step 4). #537 is clean; #540 has live Codex P2 (same README npm install issue that #537 already fixed in commit `7a5987a`).

**Actions taken:**
1. **Replied to Codex P2 on PR #540**: finding is valid; closing as superseded by #537 which carries identical fix. ✅
2. **Closed PR #540** as superseded by #537. ✅
3. **Squash-merged PR #537** (commit `4e60400f`) — Charter §5.12 Step 4 COMPLETE ✅
4. **PR #539** (PM v44) conflicted after #537 back-merge advanced develop; closed as superseded by v45. ✅
5. **PM state v45 written** + decisions.jsonl appended. ✅

**Escalations to founder:**
- **(P0)** Push tag `v0.2.0` + create GitHub Release (ceremony Steps 2+3).
- **(P0)** Register `@aimasteracc` npm scope on npmjs.com (Issue #534).

### 2026-06-04 PM dispatch v44 (PR #535 merged; Codex P2 on #537+#538 addressed)

**Summary:** PR #535 MERGED (fix(npm): 128+signal exit code `3f812410`) → Issue #525 CLOSED. Codex P2 on PR #537 (README npm install) fixed in commit `7a5987a` (reverted to "coming soon"). Codex P2 on PR #538 (pm-state "Last shipped") corrected. PR #538 superseded by v45. v0.2.0 Step 1 ✅ confirmed (PR #523 → main 2026-06-04T10:41:45Z).

### 2026-06-04 PM dispatch v43 (PRs #537+#538 opened)

**Summary:** Back-merge PR #537 opened (Charter §5.12 Step 4). PM state correction PR #538 opened (Last shipped: v0.1.19 until Steps 2+3 complete). Ceremony Step 1 confirmed merged.

### 2026-06-04 PM dispatch v42 (PR #533 merged; Issue #526 closed; Issue #534 created; PR #535 opened)

*(See full archive in closed PR #539 and decisions.jsonl entries for v42.)*

### Earlier dispatches (v1–v41)

*(archived in older versions of this file and decisions.jsonl)*

