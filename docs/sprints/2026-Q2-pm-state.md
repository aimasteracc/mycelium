# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-06-03 (PM dispatch v21 — RFC-0105/0106/0107 merged; RFC-0108 doc merged; v0.1.17 crates.io published; PR #477 back-merge conflict-resolved; PR #472/473 MERGED to develop) |
| Current sprint | **v0.1.17 — CEREMONY BLOCKED on founder auth (Steps 1+2+3); Step 4 ready (PR #477)** |
| Active release branch | `release/v0.1.17` — PR #452 (→ main, founder-gated, `dirty` in GitHub but fast-forward-mergeable) + **PR #477** (→ develop back-merge, conflict-resolved, supersedes PR #453) |
| Next release target | **v0.1.17** — redb default + CLI twin + OutputBudget-core + Charter §2 SLA + god-file-split (content COMPLETE; crates.io PUBLISHED; ceremony pending founder) |
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

**v0.1.17 ceremony status — CRATES PUBLISHED; Steps 1+2+3 blocked on founder:**
- [x] **Pre-release**: Security scan post-v0.1.16 — CLEAN ✅
- [x] **Pre-release**: CHANGELOG Unreleased section verified ✅
- [x] **Pre-release**: `publish to crates.io` ✅ — release.yml workflow run `26867945825` shows SUCCESS (sparse-index fix + continue fix both applied). All 5 crates at v0.1.17 on crates.io.
- [ ] **Step 1**: `release/v0.1.17` → `main` (founder authorization required)
  - PR #452: Quality Gate ✅ SUCCESS. `mergeable_state: dirty` in GitHub UI but `origin/release/v0.1.17..origin/main = 0 commits` — it is a clean fast-forward. Use `scripts/release-ceremony.sh` or direct merge.
- [ ] **Step 2**: Tag `v0.1.17` pushed (tag does NOT exist yet — latest tag is `v0.1.16`)
- [ ] **Step 3**: GitHub Release published (crates.io ✅ already published; GitHub Release itself not yet created)
- [ ] **Step 4**: Back-merge `release/v0.1.17` → `develop` — **PR #477** ✅ conflict-resolved (CHANGELOG + release.yml + Cargo.toml conflicts resolved; supersedes PR #453 which had unresolvable conflicts after RFC-0105/0106/0107 merged to develop)

---

## Live priorities (ordered)

**P0:** none.

**P1 (v0.1.17 ceremony — founder gates):**
1. ✓ Security scan, release branch cut, all CI fixes, crates.io published — COMPLETE ✅
2. **Step 1**: Founder merges PR #452 (`release/v0.1.17` → `main`). Use `scripts/release-ceremony.sh` or direct merge (it's a fast-forward).
3. **Step 2**: Founder pushes tag `v0.1.17` to origin.
4. **Step 3**: Founder creates GitHub Release for `v0.1.17` (crates.io already done ✅).
5. **Step 4**: Admin-merge PR #477 (back-merge → develop, all conflicts resolved). Can merge immediately after Steps 1–3.

**P1 (reactive roadmap — post-v0.1.17 sprint):**
6. ✓ RFC-0105 WatchEngine — MERGED to develop ✅
7. ✓ RFC-0106 graphChanged PUSH — MERGED to develop ✅
8. ✓ RFC-0107 SUBSCRIBE (93 MCP tools) — MERGED to develop ✅ (PR #472)
9. ✓ RFC-0108 reactive query subscriptions doc — MERGED to develop ✅ (PR #473)
10. **RFC-0108 D1–D4 implementation**: founder ratifies D1–D4 decisions (PR #473 body has recommendations; "全选推荐" applies). Once ratified, implementation PR opens (~250 LOC, 8 RED tests). Salsa Phase 2 final reactive step.
11. **Three-Surface coverage for subscribe/unsubscribe/subscription_status** (93 tools): verify skills/INDEX.md updated for the 3 new tools — check `index-management/SKILL.md` coverage. [⚠️ verify next run]

**P1 (post-v0.1.17 quality):**
12. **Dogfood re-run with redb-as-default** — validate 8/8 CLI commands.
13. **ADR-0008** for redb as default backend (required before v0.2.0).
14. **RFC-0104 cold SLA numbers** — nightly `sla_ancestors_100k` benchmark results needed.

**P2 (v0.2.0 scope):**
15. Issue #428 god-file-split remaining slices.
16. Skill marketplace submission to Claude Code marketplace.
17. "First 5 minutes" walkthrough validation.
18. `release.yml` finalize merge step systemic fix (v0.2.0 blocker).

---

## Dispatch state (2026-06-03 v21 — RFC-0105/106/107/108 merged; PR #477 opened; crates.io published)

| Agent | Status | Current item |
|---|---|---|
| founder | **action requested** | (1) **v0.1.17 ceremony**: crates.io already published ✅. Merge PR #452 → main (fast-forward; `dirty` is GitHub UI artifact). Push tag `v0.1.17`. Create GitHub Release. Then admin-merge PR #477 (back-merge, Step 4). Use `scripts/release-ceremony.sh` (Steps 2–4 only since crates already published). (2) **RFC-0108 D1–D4**: "全选推荐" → open implementation PR. |
| PM | **DONE ✅** | This run: PR #477 opened (back-merge conflict-resolved); PM state v21 updated. Subscribed to PR #472 CI. |
| rust-implementer | **IDLE — await founder** | RFC-0107 MERGED ✅. RFC-0108 implementation gated on founder D1-D4 ratification. |
| security-reviewer | **DONE ✅** | Post-v0.1.16 scan: CLEAN. Post-v0.1.17 scan needed after ceremony completes. |
| release | **WAITING** | PR #452 Quality Gate ✅. All CI green/skipped. Awaiting founder ceremony auth (Steps 1+2+3). PR #477 ready for Step 4. |
| bench | **P1** | Run `sla_ancestors_100k` nightly for RFC-0104 cold SLA numbers. |
| e2e-runner | **P1** | Dogfood re-run with redb-as-default (8/8 CLI commands). |
| architect | **P1** | ADR-0008: redb as default backend (required before v0.2.0). |
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

### 2026-06-03 PM dispatch v21 (this run — RFC-0107/108 merged; PR #477 opened; crates.io published)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20 (last entry 2026-06-02T00:00:00Z — stale), anti-patterns (no new hits), PM state v20, v0.2 PRD.

**Assessment:**
- develop HEAD `22eded7`: RFC-0108 merged (#473), RFC-0107 merged (#472), PM dispatch v20 (#476). Memory stale since 2026-06-02.
- Open PRs: #452 (release/v0.1.17 → main, `dirty`, founder auth required), #453 (back-merge, stale — develop advanced), #472 (RFC-0107, shown open in GitHub API but MERGED per local git — API lag), #473 (RFC-0108 doc, same).
- 0 open issues. CI: develop `7bf5006` + `22eded7` both green ✅. PR #452 Quality Gate ✅.
- v0.1.17 ceremony: crates.io published ✅ (workflow run `26867945825`, `publish to crates.io: success`). Tag v0.1.17: NOT created. GitHub Release: NOT created. Step 1 (→ main): NOT done. Step 4 (back-merge): PR #453 stale.
- PR #452 `dirty` state investigated: `origin/release/v0.1.17..origin/main = 0 commits` → it IS a fast-forward. `dirty` is GitHub UI artifact.
- PR #453 conflicts: 3 conflicts (CHANGELOG.md, release.yml, Cargo.toml) — all mechanical, resolved this run.

**Actions taken:**
1. **Subscribed to PR #472** CI notifications.
2. **Resolved back-merge conflicts**: merged `origin/release/v0.1.17` into local develop, resolved CHANGELOG (preserved [Unreleased] RFC-0105/0106/0107 entries above sealed [0.1.17]), release.yml (took sparse-index check from release branch), Cargo.toml (auto-resolved to 0.1.17).
3. **Opened PR #477** (`chore/v0.1.17-back-merge-develop` → develop): conflict-resolved back-merge, supersedes PR #453.
4. **Updated PM state v21**: RFC-0107/108 merged, crates published, ceremony status, dispatch table.
5. **Appended decisions.jsonl**.

**Escalations to founder:**
- **v0.1.17 ceremony**: crates.io published ✅. Merge PR #452 → main (fast-forward). Push tag. Create GitHub Release. Then admin-merge PR #477 (Step 4). Use `scripts/release-ceremony.sh` Steps 1–4 (will skip crates since already published due to idempotency guard).
- **RFC-0108 D1–D4**: "全选推荐" ratification to unblock implementation PR (~250 LOC, 8 tests, Salsa Phase 2 final reactive step).

### 2026-06-03 PM dispatch v20 (this run — PR #471 merged; PR #472 rebased; PR #452 publish running)

**Pre-flight:** Resumed from v19 context (summary). Read CHARTER.md, _orchestrator.md, anti-patterns, PM state v19, v0.2 PRD.

**Assessment:**
- 5 open PRs: #452 (release→main, Quality Gate ✅, publish in_progress), #453 (back-merge), #472 (RFC-0107, dirty/rebased), #473 (RFC-0108 doc-only, CI SUCCESS), #475 (PM chore v19, CI running).
- develop HEAD: `d3b3f1e` (v19 merged).

**Actions taken:**
1. **Merged PR #471** (continue fix, squash `8c225fd`) → develop. ✅
2. **Verified** `release/v0.1.17` already has `continue` fix via grep — no cherry-pick needed. ✅
3. **Rebased PR #472** (`feature/rfc-0107-subscribe`) onto develop: resolved 2 conflicts (decisions.jsonl vt18 entry append-only; RFC-0107 status kept HEAD "Accepted"). Force-pushed (`45ef29c`). CI triggered. ✅
4. **Merged PR #475** (PM chore v19, squash `d3b3f1e`) → develop. ✅
5. PR #452: Quality Gate ✅, `publish to crates.io` in_progress — first time this job is actually running with the correct URL + continue fixes applied. ✅

**Escalations:**
- Founder: PR #452 `publish to crates.io` running. Once all CI SUCCESS/SKIPPED, run `scripts/release-ceremony.sh` Steps 1–4.
- Founder: RFC-0107, RFC-0108 D1–D4, RFC-0105 EXCEPTION all pending.

### 2026-06-03 PM dispatch v18 (this run — PR #468 merged; URL fix cherry-picked to release/v0.1.17 as `62a2478`)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20 (latest: `2026-06-02T00:00:00Z` RFC-0101 Phase 2 CLI twin PR #414), anti-patterns (domain hits: `storage` + `release-governance` + `ci-portability`), PM state (v16 — stale header but body accurate), v0.2 PRD.

**Assessment:**
- 4 open PRs: #452 (release/v0.1.17 → main, `publish to crates.io` FAILURE despite v16 max_version fix), #453 (back-merge → develop, pending Steps 1–3), #468 (URL fix, CI 22/22 ✅), #469 (PM chore v17, only triage checks).
- 0 open issues.
- CI on develop: E2E + CI both SUCCESS ✅ (`b3208a5` develop HEAD after #468 merge). `release/v0.1.17` HEAD was `abdb570`; after this run it's `62a2478`.
- Root cause of PR #452 persistent failure: `crate_published()` in release.yml used `tr '-' '_'` to convert crate name, producing URL `/crates/mycelium_rcig_pack` (underscore) instead of `/crates/mycelium-rcig-pack` (hyphen). crates.io returns 404 for underscore form → `wait_for_crate` always times out. The v13/v15 wait-time fix and v16 max_version fix did NOT address this encoding. PR #468 (authored by previous PM run) correctly identified and fixed it.

**Actions taken:**
1. **Merged PR #468** (squash, 22/22 CI SUCCESS) — URL fix lands on develop. ✅
2. **Cherry-picked URL fix to release/v0.1.17** as commit `62a2478` — same 3-line change: remove `tr '-' '_'`, use `$1` directly. Pushed to origin. PR #452 CI re-triggered. ✅
3. **Updated PM state v18** — ceremony status, dispatch table, live priorities, archive. ✅
4. **Will append decisions.jsonl** + open chore PR v18 + merge if CI green.

**Escalations:**
- Founder: PR #452 CI re-running with definitive URL fix (`62a2478`). Once all CI SUCCESS/SKIPPED → run `scripts/release-ceremony.sh` Steps 1–4.
- Founder: RFC-0105 EXCEPTION decision still pending.
- Founder: RFC-0107 D1–D5 decisions pending.

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
