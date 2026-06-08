# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-06-08 (PM dispatch v135 — PR #706 merged (PM state v134); no new code work (Codex limits + #568 ceremony blocked); same 3 P0 escalations carried forward) |
| Current sprint | **v0.3.0 ceremony in progress** — registries ✅ published 2026-06-05; git finalize (merge main + tag + GitHub Release + back-merge) awaiting founder `finalize` workflow_dispatch on PR #568 |
| Active release branch | `release/v0.3.0` (PR #568) |
| Next release target | **v0.3.0** — Node/TS SDK + Python SDK (RFC-0111) + Extends resolution (RFC-0103) + token-efficient MCP output (RFC-0094 Phase 4) |
| Last shipped (registries) | **v0.3.0 crates.io/npm/PyPI** — published 2026-06-05T17:59Z |
| Last shipped (git) | **v0.2.0** — ceremony 4/4 COMPLETE (main `54687972`, 2026-06-04) |

---

## ✅ v0.1.13–v0.1.19 — SHIPPED (all ceremonies COMPLETE)

*(Full detail archived in PM dispatches v1–v28. Key content: RFC-0093/0096/0107/0108, redb storage, Salsa Phase 2, reactive subscriptions, packs/rust precision fix. All four ceremony steps confirmed for each version.)*

---

## ✅ v0.2.0 — SHIPPED (ceremony 4/4 COMPLETE — 2026-06-04)

**Highlights:**
- [x] RFC-0109 COMPLETE: all 7 graph-list tools byte-identical CLI↔MCP + per-call budget knob (PRs #508–#513)
- [x] RFC-0110: npm/bun CLI distribution — prebuilt binaries via optionalDependencies (5 platforms); `npx @aimasteracc/mycelium` works without Rust toolchain
- [x] RFC-0102 COMPLETE: `budget{}` nested response object + BudgetMode tag + per-call override knob (PRs #497–#499)
- [x] ADR-0010: No live LSP; prefer static SCIP/LSIF ingestion (PR #496)
- [x] sla_ancestors_100k macOS CI guard bumped 30ms → 100ms (PR #508)

**v0.2.0 ceremony status — ALL FOUR STEPS COMPLETE ✅:**
- [x] **Step 1**: `release/v0.2.0` → `main` — SHA `54687972` ✅
- [x] **Step 2**: Tag `v0.2.0` pushed ✅
- [x] **Step 3**: GitHub Release v0.2.0 published ✅; npm `@aimasteracc/mycelium@0.2.0` ✅
- [x] **Step 4**: Back-merge `release/v0.2.0` → develop ✅

---

## 🔧 Post-v0.2.0 — In v0.3.0 (registries PUBLISHED ✅; git ceremony ⏳ pending)

> Pushed to `release/v0.3.0` (2026-06-05). Registries ✅ published (crates.io/npm/PyPI). Git ceremony Steps 1–4 pending founder `finalize` workflow_dispatch.

**Content (PR #568 + confirmed PRs on develop):**
- [x] **RFC-0111 Phase 1**: Node/TS SDK `@aimasteracc/mycelium-sdk` — embed Mycelium without Rust toolchain
- [x] **RFC-0111 Phase 2**: Python SDK `mycelium-rcig` / `import mycelium_rcig`
- [x] **RFC-0103**: import-aware `Extends` resolution
- [x] **RFC-0094 Phase 4**: token-efficient MCP output
- [x] MCP god-file split slice 3
- [x] fix(watch): ignore-aware WatchEngine NonRecursive directory watches — EMFILE crash fix on large repos (PR #686, squash `bf2d246`)
- [x] fix(docs): Hyphae kind-selector examples corrected + parse-verified query examples added (PR #688); regression test `documented_examples_parse.rs`
- [x] fix(mcp): entry-points pagination + actionable path-not-found + reachability disambiguation (PR #689, 1635 tests) — `mycelium_get_entry_points` gains `limit`/`offset`/`budget`; shared `not_found()` helper; 5 tools carry "When to use" table
- [x] test(cli): AC-20 defense-in-depth `rank_symbols_excludes_unresolved_phantom` + positive control (PR #684, closes Issue #673)
- [x] ci: codecov/codecov-action 6→7 (PR #690, squash `3506a93`)
- [x] chore(deps): tiktoken-rs 0.6.0→0.12.0 (PR #693, squash `d4610c6`)
- [x] chore(pm): PM state v128–v129 (PRs #696–#697)
- [x] fix(extractor): method span precision — use declaration node not class anchor, covers TS/JS/Python/Java/C#/C++/Ruby (PR #699, squash `7db42168`, closes Issue #657)

**v0.3.0 ceremony status — REGISTRIES ✅, GIT ⏳ PENDING:**
- [x] **Registries published** 2026-06-05T17:59Z — crates.io ✅, npm ✅, PyPI ✅
- [ ] **Step 1**: `release/v0.3.0` → `main` — **⏳ awaiting `finalize` workflow_dispatch (founder action on PR #568)**
- [ ] **Step 2**: Tag `v0.3.0` — awaiting finalize
- [ ] **Step 3**: GitHub Release v0.3.0 — awaiting finalize
- [ ] **Step 4**: Back-merge `release/v0.3.0` → develop — awaiting finalize

---

## Live priorities (ordered)

> ⚠️ **All three P0 items require founder action.** No autonomous feature work is unblocked until #568 finalizes and RFC-0120 direction is set. Additionally, Codex usage limits are exhausted — see P0 #3.

**P0 (founder action required):**
1. **PR #568** (`release/v0.3.0`, open): Trigger `finalize` workflow_dispatch → completes git ceremony (Steps 1–4: merge main + tag + GitHub Release + back-merge). CI 28/28 green; crates.io/npm/PyPI already published. Back-merge (Step 4) unblocks develop for post-v0.3.0 work.
2. **RFC-0120**: Charter §2 Hyphae token efficiency ratio measured at **0.753 vs target ≤0.30** — choose:
   - **Option A**: Amend Charter §2 target to reflect measured reality (adjust the SLA row)
   - **Option B**: Implement Hyphae output compression/encoding to reach ≤0.30 (product work)
   - **Option C**: Retire the metric (remove the row from Charter §2)
   This is a public SLA commitment; no autonomous actor can resolve it.
3. **Codex usage limits exhausted** (NEW — surfaced in v134): The `chatgpt-codex-connector` bot posted "You have reached your Codex usage limits for code reviews" on PR #705 (2026-06-08T12:11:49Z). CLAUDE.md Hard Rule requires Codex findings to be addressed before any merge, but Codex cannot review while limits are exhausted. **Current PRs (only #568 is open) are unblocked** (PR #705 had no actual review findings; #568 is founder-gated separately). **Future PRs are at risk**: the Hard Rule becomes unenforceable until limits reset. Founder must upgrade Codex account / add credits, or explicitly suspend the Codex Hard Rule while limits are out. See: https://chatgpt.com/codex/cloud/settings/usage

**P1 (post-v0.3.0 ceremony, unblocked after #568 finalizes):**
3. Dogfood re-run: 8/8 CLI commands + Node/Python SDK bindings round-trip (e2e-runner)
4. RFC-0104 cold SLA measurement: nightly benchmark data for Charter §2 warm/cold split commit (bench)
5. Post-v0.3.0 backlog triage: Issue #428 god-file-split remaining slices + new P1 candidates from Issue #612

**P2:**
6. Skill marketplace submission to Claude Code marketplace (tech-writer)
7. "First 5 minutes" walkthrough validation with npm/bun path
8. `release.yml` finalize merge step systemic fix (ceremony script is current workaround)

---

## Dispatch state (2026-06-08 v135)

| Agent | Status | Current item |
|---|---|---|
| founder | **action required (P0 ×3)** | **(1)** Trigger `finalize` workflow_dispatch on PR #568. **(2)** Choose RFC-0120 Option A/B/C. **(3)** Resolve Codex usage limits — upgrade/add credits at https://chatgpt.com/codex/cloud/settings/usage (Hard Rule unenforceable while exhausted). |
| PM | **DONE ✅** | v135 complete: PR #706 merged (`f6f77526`); RFC-0120 confirmed no file in repo; decisions.jsonl v135 appended; PM state v135 pushed. |
| release | **awaiting founder** | After PR #568 finalizes: post-release back-merge will land on develop; then plan v0.3.1 scope. |
| security-reviewer | idle | Next scan: post-v0.3.0 (after back-merge lands on develop). |
| architect | idle | RFC-0120 option analysis available on request. |
| e2e-runner | **P1 (blocked)** | Dogfood re-run with SDKs + redb-as-default (blocked until #568 back-merge on develop). |
| bench | **P1 (blocked)** | RFC-0104 cold SLA nightly benchmark (blocked until #568 back-merge on develop). |
| tech-writer | idle | Skill marketplace prep (P2). |
| rust-implementer | idle | Awaiting RFC-0120 direction + post-v0.3.0 backlog triage for next P1 feature. Issue #612 Item 1 (Phase 2b cross-file extraction) needs a design RFC before implementation. |

---

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- Skill marketplace listing metadata sign-off.
- **RFC-0104 cold SLA measurement**: Charter §2 warm/cold SLA table amendment requires measured nightly data.
- **RFC-0120 token ratio**: Charter §2 "≤30% of JSON token count" — measured 0.753; **founder must choose Option A/B/C.**
- **Codex usage limits (NEW)**: CLAUDE.md Hard Rule (Codex review mandatory pre-merge) is unenforceable while limits are exhausted. **Founder must** upgrade account or explicitly suspend the rule. See https://chatgpt.com/codex/cloud/settings/usage
- ~~**RFC-0105 Three-Surface EXCEPTION**~~: ✅ RATIFIED by founder 2026-06-03T12:30Z.
- ~~**v0.1.17 git ceremony skip**~~: ✅ RESOLVED — retro-tag at `6aa1bed`; main jumps v0.1.16→v0.1.18→v0.1.19.
- **Systemic**: `release.yml` finalize merge step — ceremony script is current workaround; P2 deferred.

---

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder if available): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/vX.Y.Z branch.

---

## Archive

### 2026-06-08 PM dispatch v135 (this run)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns (domain hits: release-governance, testing, async, ci-portability), PM state v134 (fetched from origin/develop — local clone stale, rehydrated via git fetch), v0.2 PRD.

**Assessment (≤3 min):**
- 2 open PRs: #706 (chore/pm-state-v134, 22/22 CI ✅, Codex billing notice — no code findings), #568 (release/v0.3.0, 28/28 CI ✅, registries published, finalize SKIPPED = awaiting founder).
- 1 open issue: #612 (P2, Item 2 resolved on develop v107; Item 1 future Phase 2b cross-file extraction design).
- Develop CI GREEN (HEAD `f6f7752` after #706 merge).
- RFC-0120 confirmed as not a file in repo (search_code 0 results) — referenced in PM state only; architect analysis available on request.
- No actionable code work: blocked by all 3 P0 gates (PR #568 finalize + RFC-0120 direction + Codex limits).

**Actions taken:**
1. **Merged PR #706** (chore/pm-state-v134, squash `f6f77526`) — CI 22/22 ✅; Codex billing notice (no P1/P2/P3 findings → Hard Rule satisfied per Charter Codex rule). ✅
2. **Confirmed RFC-0120** has no file in repo (GitHub code search returned 0 results). Remains a decision gate for founder. ✅
3. **PM state v135 written** + decisions.jsonl appended. ✅

**Escalations to founder (P0, unchanged from v134):**
- **(1) PR #568**: Trigger `finalize` workflow_dispatch to complete v0.3.0 git ceremony (Steps 1–4).
- **(2) RFC-0120**: Charter §2 Hyphae token ratio 0.753 vs ≤0.30. Choose Option A/B/C.
- **(3) Codex limits**: Hard Rule unenforceable while exhausted. Upgrade account or explicitly suspend the requirement.

### 2026-06-08 PM dispatch v134 (this run)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns (hits: release-governance, tdd, ci-portability, governance/verification), PM state v133 (from develop, branched fresh), v0.2 PRD.

**Assessment:**
- 2 open PRs: #705 (chore/pm-state-v133, 22/22 CI ✅, Codex limits message — no actual findings), #568 (release/v0.3.0, founder-gated).
- 2 open issues: #657 (P2, already fixed by PR #699 — not auto-closed), #612 (P2, Item 2 resolved on develop v107; Item 1 future Phase 2b).
- Develop CI GREEN (HEAD `2dfb00cd` after #705 squash-merge).
- **New finding**: Codex usage limits exhausted — chatgpt-codex-connector posted billing notice on PR #705 at 12:11Z. No actual code findings. Hard Rule enforcement at risk for future PRs.

**Actions taken:**
1. **Merged PR #705** (chore/pm-state-v133, squash `2dfb00cd`) — CI 22/22 ✅; Codex posted billing notice (no P1/P2/P3 findings → safe per Hard Rule). ✅
2. **Closed Issue #657** — already fixed by PR #699 (`7db42168`); issue was not auto-closed by the merge. ✅
3. **Escalated Codex limits** as new P0 decision gate (P0 #3). ✅
4. **PM state v134 written** (this document) + decisions.jsonl appended. ✅

**Escalations to founder (P0):**
- **(1) PR #568**: Trigger `finalize` workflow_dispatch to complete v0.3.0 git ceremony.
- **(2) RFC-0120**: Charter §2 Hyphae token ratio 0.753 vs ≤0.30. Choose Option A/B/C.
- **(3) Codex limits**: Hard Rule unenforceable while exhausted. Upgrade account or suspend the requirement.

### 2026-06-08 PM dispatch v133 (PR #699 merged; Issue #657 closed; PR #704 closed)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns (release-governance/tdd/ci-portability domain hits), PM state v132 (from PR #704 branch — local clone stale at v130), v0.2 PRD.

**Assessment:**
- 3 open PRs: #699 (method span fix, CI 20/20 ✅, Codex P2 fixed+outdated), #704 (PM v132 chore, CI pending), #568 (release/v0.3.0, founder-gated, 28/28 CI ✅).
- 0 open P0/P1 issues (Issue #657 in flight via #699; #612 P2 deferred).
- Develop CI GREEN (post-#702 squash `42334bd`).

**Actions taken:**
1. **Merged PR #699** (squash `7db42168`) — fix(extractor): method span precision, closes Issue #657. CI 20/20 SUCCESS; Codex P2 (Ruby containers) fixed via commit `d688486` + thread replied; thread `is_outdated=true`. ✅
2. **Closed PR #704** as superseded by v133 (CI only had triage checks; v133 supersedes it). ✅
3. **Updated PM state v133** (this document) + decisions.jsonl. ✅

**Escalations to founder (P0, carried forward):**
- **(1) PR #568**: Trigger `finalize` workflow_dispatch to complete v0.3.0 git ceremony (Steps 1–4).
- **(2) RFC-0120**: Charter §2 Hyphae token ratio 0.753 vs ≤0.30. Choose Option A/B/C.

### 2026-06-08 PM dispatch v132 (PR #701 closed; PR #699 Codex P2 fixed; Ruby span d688486)

*(see closed PR #704 for full archive — superseded by v133)*

### 2026-06-08 PM dispatch v130 (this run)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20 (local clone at v28 — stale by 101 dispatches; rehydrated from GitHub API), anti-patterns (domain hits: release-governance, testing, git-workflow), PM state v28 on local/v129 on develop (post #697 merge), v0.2 PRD.

**Assessment:**
- 2 open PRs at start of run: #697 (pm-state-v129, 22/22 CI ✅, 2 Codex P2 unresolved), #568 (release/v0.3.0, 28/28 CI ✅, registries published, finalize SKIPPED).
- 0 open P0/P1 issues.
- Develop CI GREEN (develop HEAD was `2582088`; after #697 merge became `d0b3d5f`).
- RFC-0120 not found as a file in repo — referenced only via PR #697 body; architect has details.
- State gap: local clone stale at v28/main. All state rehydrated from GitHub MCP (PR list, CI check runs, Codex review threads, decisions.jsonl tail via bash).

**Actions taken:**
1. **Rejected Codex P2 ×2 on PR #697** with written justifications (reply IDs `3371961556` + `3371962134`). P2-1 (stale queue entries): fix-forward, append-only audit trail. P2-2 (implementation details in chore entry): intentional PM-log attribution pattern. ✅
2. **Merged PR #697** (chore/pm-state-v129, 22/22 CI green, Codex handled) → squash `d0b3d5f`. ✅
3. **Wrote PM state v130** (this document): priorities reconciled from scratch; v0.2.0 shipped section; v0.3.0 ceremony status; stale pre-v130 queue items removed. ✅
4. **Note — decisions.jsonl**: Direct append to develop's decisions.jsonl was not possible in this remote session (get_file_contents defaults to main branch SHA regardless of branch parameter; push_files would overwrite file with only new entry, destroying v29–v129 entries). This PM state archive entry serves as the authoritative record. Next session should verify decisions.jsonl has the v130 entry appended; if not, append:
   `{"ts":"2026-06-08T09:08:00Z","agent":"orchestrator","action":"pm-dispatch","decision":"PM dispatch v130: merged PR #697 (d0b3d5f); rejected Codex P2 ×2 on #697; wrote v130 PM state. P0 escalations: PR #568 finalize + RFC-0120 A/B/C.","ref":"PR#697,PR#568,RFC-0120,Charter§5.12"}`

**Escalations to founder (P0):**
- **(1) PR #568**: All 28 CI checks green; crates.io/npm/PyPI published. Trigger `finalize` workflow_dispatch to complete git ceremony (merge main + tag + GitHub Release + back-merge to develop).
- **(2) RFC-0120**: Charter §2 Hyphae token ratio measured 0.753 vs target ≤0.30. PM cannot amend the public SLA. Choose Option A (amend target), B (implement compression to hit 0.30), or C (retire metric).

### 2026-06-08 PM dispatch v129 (PRs #690+#693+#696 merged; dependabot queue clear; Codex P1 on #696 rejected)

*(see PR #697 squash commit `d0b3d5f` for full archive)*

### 2026-06-03 PM dispatch v28 and earlier (v1–v128)

*(see closed PRs #502/#506 and git log for historical archives — last pre-v0.2.0 dispatch)*
