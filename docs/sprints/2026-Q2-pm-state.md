# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-06-03 (PM dispatch v17 — PR #467 RFC-0107 merged; PR #468 URL-encoding fix opened; v0.1.17 CI still blocked — URL bug still on release branch) |
| Current sprint | **v0.1.17 — RELEASE BRANCH CUT** (`release/v0.1.17`, PR #452 Quality Gate ✅ — `publish to crates.io` STILL RED: URL encoding bug persists; see PR #468) |
| Active release branch | `release/v0.1.17` — PR #452 (→ main, founder-gated) + PR #453 (→ develop, back-merge) |
| Next release target | **v0.1.17** — redb default (RFC-0100 Phase 3), CLI twin (RFC-0101), OutputBudget-core (RFC-0102), Charter §2 SLA (RFC-0104), god-file-split slices 1+2 |
| Final release target | v0.2.0, ETA 2026-07-15 |
| Last shipped | **v0.1.16 — RFC-0100 Phase 1+2 redb backend, MCP routing instructions, journal persistence, memory budget, dep bumps (salsa 0.26, logos 0.16, redb 4.1), crates.io publish fix** (tag v0.1.16, GitHub Release published 2026-06-02T01:27Z) |

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

## 🚀 v0.1.17 — IN PROGRESS

**Content already on develop (post-v0.1.16):**
- [x] RFC-0101 Phase 2: `mycelium context` CLI twin — Three-Surface Rule fully satisfied (PR #414)
- [x] RFC-0102 Implemented: OutputBudget moved to `mycelium-core`; CLI+MCP byte-identical (PR #438)
- [x] RFC-0100 Phase 3: **redb is now the default storage backend** (PR #448)
- [x] RFC-0104: Charter §2 warm/cold SLA split — founder-approved 2026-06-02 (PR #444)
- [x] Issue #428 god-file-split slice 1: redb value codecs → `store::redb_codec` (PR #441)
- [x] Issue #428 god-file-split slice 2: `mod tests` → `src/tests.rs` (PR #442, `lib.rs` 12191→5627 lines, −54%)
- [x] 100k-node redb SLA gate + env-guarded nightly benchmark (PR #440)
- [x] Orphan `BoundedStore`/`MemoryBudget`/`FileAccessTracker` LRU removed (PR #440)
- [x] Repo hygiene: orphan `.claude/worktrees/` gitlinks removed + `.gitignore` updated (PR #449)
- [x] Vision scorecard updated to v0.1.16+ reality (PR #450)

**v0.1.17 ceremony status — CI RE-RUNNING (max_version fix pushed to release branch):**
- [x] **Pre-release**: Security scan post-v0.1.16 — CLEAN ✅ (v12 + re-confirmed v14)
- [x] **Pre-release**: CHANGELOG Unreleased section verified ✅ (v12)
- [ ] **Step 1**: `release/v0.1.17` → `main` (founder authorization required)
  - PR #452: Quality Gate ✅ SUCCESS (required branch-protection check)
  - ⚠️ **`publish to crates.io` STILL FAILING** — v16 fix (`abdb570` max_version Python check) addressed wrong symptom. **Actual root cause (v17 diagnosis)**: `crate_published()` uses `tr '-' '_'` to build the crates.io API URL → `/crates/mycelium_rcig_pack` (underscores) → **404** → curl -sf exits non-zero → Python json.load fails on empty stdin → function ALWAYS returns "not published". Fix: **PR #468** (`fix/release-crates-url-encoding`) drops the `tr` and uses the correct hyphenated URL.
  - **To unblock v0.1.17**: founder cherry-pick PR #468 commit to `release/v0.1.17` and re-trigger CI — OR use `scripts/release-ceremony.sh` directly (does not depend on release.yml publish check).
  - ⚠️ Do NOT push to release branch without founder review — with RELEASE_BOT_TOKEN set, a successful publish-crates run triggers the `finalize` job which auto-merges to main.
- [ ] **Step 2**: Tag `v0.1.17` pushed
- [ ] **Step 3**: GitHub Release published + 5 crates on crates.io
- [ ] **Step 4**: Back-merge `release/v0.1.17` → `develop` (PR #453)

---

## Live priorities (ordered)

**P0:** none.

**P1 (v0.1.17 release gates):**
1. ✓ Security scan post-v0.1.16 — COMPLETE ✅
2. ✓ Cut `release/v0.1.17` — COMPLETE ✅ (PR #452 + PR #453 open)
3. **PR #468 — URL encoding fix for release.yml** (on develop; CI pending). Merge then cherry-pick to `release/v0.1.17` to fix publish-crates CI check.
4. **Wait for PR #452 all CI SUCCESS/SKIPPED** after cherry-pick → founder authorizes `scripts/release-ceremony.sh` Steps 1–4. **OR** founder uses ceremony script NOW (bypasses release.yml) if confirmed 5 crates already on crates.io.

**P1 (reactive roadmap):**
5. **Founder: ratify or reject RFC-0105 Three-Surface EXCEPTION** — `feature/rfc-0105-watch-engine` branch CI SUCCESS (SHA `05654d96`), implementation complete. Awaiting founder EXCEPTION decision to merge.
6. **PR #467 (RFC-0107) merged ✅** — 5 founder-gated decisions (D1–D5) pending ratification before SUBSCRIBE implementation begins.

**P1 (post-v0.1.17):**
6. **RFC-0104 cold SLA numbers** — nightly `sla_ancestors_100k` benchmark results needed before Charter §2 table is amended.
7. **ADR-0008** for redb as default backend (required before v0.2.0).
8. **Dogfood re-run with redb-as-default** — validate 8/8 CLI commands.

**P2 (v0.2.0 scope):**
9. Issue #428 god-file-split remaining slices.
10. Skill marketplace submission to Claude Code marketplace.
11. "First 5 minutes" walkthrough validation.
12. `release.yml` finalize merge step systemic fix (v0.2.0 blocker).

---

## Dispatch state (2026-06-03 v17 — PR #467 merged; PR #468 URL fix opened; v0.1.17 CI still blocked)

| Agent | Status | Current item |
|---|---|---|
| founder | **action requested** | (1) **v0.1.17 ceremony**: PR #452 `publish to crates.io` STILL RED (URL encoding bug, not max_version). Options: (a) cherry-pick PR #468 commit to `release/v0.1.17` + re-trigger CI — then authorize ceremony; (b) use `scripts/release-ceremony.sh` directly if 5 crates confirmed on crates.io. (2) After Steps 1+2+3: admin-merge PR #453 (back-merge, Step 4). (3) **Ratify RFC-0105 Three-Surface EXCEPTION** — implementation complete, CI green. (4) **Ratify 5 RFC-0107 SUBSCRIBE decisions** (D1–D5 in PR #467 description). |
| PM | **DONE ✅** | PR #467 merged; PR #468 opened; PM state v17; decisions.jsonl appended. |
| security-reviewer | **DONE ✅** | Post-v0.1.16 scan: CLEAN (v12 + re-confirmed v14). |
| release | **DONE ✅** | `release/v0.1.17` branch cut; PR #452 + PR #453 open. Awaiting founder auth. |
| rust-implementer | **WAITING** | RFC-0105 WatchEngine built (branch `feature/rfc-0105-watch-engine`, CI SUCCESS). Gated on founder EXCEPTION ratification. |
| bench | **P1** | Run `sla_ancestors_100k` nightly for RFC-0104 cold SLA numbers. |
| e2e-runner | **P1** | Dogfood re-run with redb-as-default (8/8 CLI commands under new storage default). |
| architect | idle | ADR-0008: redb as default backend (required before v0.2.0). |
| tech-writer | idle | Skill marketplace submission prep (P2). |

---

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- Skill marketplace listing metadata sign-off.
- **RFC-0104 cold SLA measurement**: Charter §2 table amendment (warm/cold split) requires measured nightly data.
- **RFC-0105 Three-Surface EXCEPTION**: WatchEngine implementation built; merge blocked on founder ratification.
- **Systemic**: `release.yml` finalize merge step — partially fixed by PR #455 + `abdb570`. v0.1.17 uses ceremony script workaround.

---

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder if available): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/v0.1.x branch, publish.

---

## Archive

### 2026-06-03 PM dispatch v17 (this run — PR #467 merged; PR #468 URL fix; v0.1.17 CI still blocked)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-5 (latest: 2026-06-03T14:00Z rust-implementer rfc0106-impl), anti-patterns, PM state (stale v16 — develop at 54cc9b6), v0.2 PRD.

**Assessment:**
- develop HEAD: `54cc9b6` (RFC-0106 PUSH merged). 3 open PRs: #452 (release→main, CI ❌), #453 (back-merge→develop, CI same), #467 (RFC-0107 doc, 20/20 ✅). 0 open issues.
- PR #452 CI: Quality Gate ✅ SUCCESS, but `publish to crates.io` ❌ FAILURE. Root-cause re-diagnosis: v16's `abdb570` fixed the Python `max_version` field (vs old `versions[]`) but the `crate_published()` URL still uses `tr '-' '_'` → `/crates/mycelium_rcig_pack` → 404 → Python json.load fails → always "not published".
- PR #467 (RFC-0107 SUBSCRIBE): 20/20 CI SUCCESS. Doc-only with 5 founder-gated decisions. Safe to merge.

**Actions taken:**
1. **Merged PR #467** (docs(rfc): RFC-0107 SUBSCRIBE scoped per-batch delta, 20/20 CI green, squash). 5 founder decisions (D1–D5) added to founder action queue.
2. **Fixed release.yml URL encoding bug** (`tr '-' '_'` → direct use of hyphenated crate name) on `fix/release-crates-url-encoding` branch from develop HEAD. **Opened PR #468**.
3. Did NOT push URL fix to `release/v0.1.17` — with RELEASE_BOT_TOKEN set, `finalize` would auto-merge to main, bypassing Charter §5.12 founder authorization.
4. Updated PM state + decisions.jsonl.

**Escalations:**
- **v0.1.17 release**: `publish to crates.io` CI still RED after v16's abdb570 fix. PR #468 is the actual fix. Founder must cherry-pick + re-trigger CI OR use ceremony script directly.
- **RFC-0105 EXCEPTION**: still awaiting founder ratification.
- **RFC-0107 decisions D1–D5**: founder must ratify before SUBSCRIBE implementation begins.

### 2026-06-03 PM dispatch v16 (this run — max_version fix pushed to release/v0.1.17; fix PR + chore PR opened)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20 (latest: 2026-06-03T12:30Z rust-implementer rfc0105-merge-resolution), anti-patterns (no domain hits), PM state (stale — develop at 6cd5996d, PM state v14), v0.2 PRD.

**Assessment:**
- 3 open PRs: #452 (release/v0.1.17→main, Quality Gate ✅, `publish to crates.io` ❌ FAILURE), #453 (back-merge), #460 (PM chore v15, merge-conflict — stale).
- 0 open issues.
- `feature/rfc-0105-watch-engine` branch CI SUCCESS (SHA `05654d96`) — WatchEngine implementation built by rust-implementer; no open PR; awaiting RFC-0105 EXCEPTION ratification.
- PR #452 `publish to crates.io` failure: deeper diagnosis reveals `versions[]` API iteration is the root cause (not just timeout). Previous PR #455 fix only extended 12→36 attempts but kept the slow `versions[]` check. Fix on main (`cd66278`) uses `max_version` field which updates immediately after publish. This fix was never cherry-picked to release/v0.1.17.
- PR #460: stale (merge conflict in decisions.jsonl + pm-state.md) — superseded by this run.

**Actions taken:**
1. **Pushed `abdb570`** (max_version fix + 12-attempt loop) to `release/v0.1.17`. Release workflow re-triggered. ✅
2. **Closed PR #460** (stale, merge-conflict, superseded by v16). ✅
3. **Created branches** `fix/release-yml-crates-io-max-version` + `chore/pm-dispatch-2026-06-03-v16` from develop HEAD `6cd5996d`. ✅
4. **Pushed release.yml fix** to `fix/release-yml-crates-io-max-version` (same max_version fix for develop). ✅
5. **Pushed PM state v16** + decisions.jsonl to `chore/pm-dispatch-2026-06-03-v16`. ✅
6. **Opened PRs** for both branches (see artifacts). ✅

**Escalations:**
- Founder: PR #452 CI re-running (release.yml with max_version fix). Once all checks SUCCESS/SKIPPED → authorize `scripts/release-ceremony.sh` Steps 1–4.
- Founder: `feature/rfc-0105-watch-engine` is built and CI-green — ratify or reject RFC-0105 Three-Surface EXCEPTION to unblock merge.

### 2026-06-03 PM dispatch v15 (PR #459 merged; release.yml fix cherry-picked to release/v0.1.17; CI re-triggered)

**Actions taken:**
1. Merged PR #459 (chore PM dispatch v14, 22/22 CI SUCCESS, squash). ✅
2. Cherry-picked `ef5e19a` (ci: crates.io wait 12→36 + finalize gated) onto release/v0.1.17 as `121225f`. ✅
3. Updated PM state v15. **Note**: this v15 fix only extended the wait loop (360s) but did NOT fix the root cause (`versions[]` vs `max_version`). Root cause fixed in v16 via `abdb570`.

### 2026-06-03 PM dispatch v14 (PRs #455+#456 merged; PR #457 closed; PR #458 opened; security CLEAN)

**Actions taken:**
1. Merged PR #455 (release.yml: crates.io wait 120s→360s + finalize gated on workflow_dispatch). ✅
2. Merged PR #456 (PM chore v13). ✅
3. Closed PR #457 (conflict); opened PR #458 (RFC-0105 clean rebase). ✅
4. Security scan re-confirmed CLEAN. ✅

### 2026-06-02 PM dispatch v13 (PR #454 merged; PR #452 CI failure diagnosed; PR #455 release.yml fix opened)

**Actions taken:**
1. Merged PR #454 (chore PM dispatch v12). ✅
2. Diagnosed PR #452 `publish to crates.io` FAILURE: wait_for_crate loop timed out (12×10s=120s). Opened PR #455 (wait 12→36 + finalize gated). ✅
3. Updated PM state v13.

### 2026-06-02 PM dispatch v12 (security scan CLEAN; release/v0.1.17 CUT; PRs #452+#453 opened)

**Actions taken:**
1. Merged PR #451 (chore PM dispatch v11). ✅
2. Security scan post-v0.1.16: CLEAN. ✅
3. Cut release/v0.1.17; opened PR #452 (→main, founder-gated) + PR #453 (→develop, back-merge). ✅

### 2026-06-02 PM dispatch v11 (v0.1.16 SHIPPED confirmed; v0.1.17 sprint defined)

- v0.1.16 ceremony: ALL 4 STEPS COMPLETE. 10 commits on develop post-v0.1.16. PM state corrected.

### 2026-06-02 PM dispatch (RFC-0101 Phase 2 CLI twin; PR #414 opened)

- RFC-0101 Phase 2 TDD: mycelium context CLI twin implemented. Three-Surface violation resolved.

### Earlier dispatches (2026-06-01)

- PRs #395, #397-#401, #405 merged. PR #395: 90th MCP tool (mycelium_context + OutputBudget). Dep bumps: CI action bumps merged; salsa/redb/logos deferred. Issue #375 resolved.

### 2026-05-31 dispatches

- v0.1.13/v0.1.14 shipped, ceremonies complete. RFC-0093 Phase 3 CHANGELOG. v0.1.14 sprint criteria (6/6). Security scans CLEAN.

### 2026-05-30 dispatches

- v0.1.10/v0.1.11/v0.1.12 shipped. Skills CI gate. Dogfood 8/8.

### 2026-05-29 PM run (v0.1.4 close)

v0.1.4 sprint declared complete. All 7 exit criteria met.
