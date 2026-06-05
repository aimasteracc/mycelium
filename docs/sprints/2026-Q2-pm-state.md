# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-06-05 (PM dispatch v58 — PR #556 merged (v57 chore + Codex P2 fix: #554 already-merged stale ref corrected); god-file-split slice 4 investigation) |
| Current sprint | **Post-v0.2.0 stabilization — v0.2.1 queue: RFC-0103 Extends resolution merge (#554) + god-file-split slice 4 + formalize crates** |
| Active release branch | none — `release/v0.2.0` merged and deleted |
| Next release target | **v0.2.1** — MCP god-file split (Issue #428) + formalize signal-exit fix (#535) + mutation tests (#531) into crates |
| Final release target | v0.3.0 (cross-repo indexing, IDE plugins) |
| Last shipped | **v0.2.0 (ceremony 4/4 COMPLETE)** — crates.io ✅ + npm (6 pkgs, install-verified) ✅ + main ✅ + tag `v0.2.0` ✅ + GitHub Release (5 binaries + SHA256SUMS) ✅ + back-merge ✅ |

---

## ✅ v0.1.13–v0.1.19 — ALL SHIPPED (ceremonies COMPLETE)

*(See archive in git history; all four ceremony steps complete for each version.)*

- v0.1.13: RFC-0093 Phase 2 success_str; RFC-0096 Phase 1 Python TypeImports; ADR-0004/0005/0006.
- v0.1.14: RFC-0096 Phase 2 TS; RFC-0093 Phase 3 error model; skill-parity CI gate; dogfood 8/8.
- v0.1.15: content absorbed into v0.1.16 (ceremony broken).
- v0.1.16: RFC-0100 Phase 1+2 redb StorageBackend; OutputBudget; mycelium_context (90th tool).
- v0.1.17: redb default (RFC-0100 Phase 3); RFC-0101/0102 Implemented; god-file-split slices 1+2.
- v0.1.18: RFC-0105 WatchEngine + RFC-0106 PUSH + RFC-0107 SUBSCRIBE + RFC-0108 Salsa Phase 2 (reactive roadmap 4/4 COMPLETE).
- v0.1.19: packs/rust precision 67%→99.8%; ADR-0008/0009/0010; Codex Hard Rule; RFC-0105 EXCEPTION ratified.

---

## ✅ v0.2.0 — CEREMONY 4/4 COMPLETE (fully shipped 2026-06-04)

**What shipped in v0.2.0:**
- [x] RFC-0109 all 7 graph-list tools → shared core builders + object shape + budget knob (PRs #501–#513)
- [x] RFC-0102 nested `budget{}` response object + BudgetMode tag + per-call override + cap fixes (PRs #497–#499)
- [x] RFC-0110 npm/bun CLI distribution: prebuilt-binary optionalDependencies model; 5-platform build matrix; release.yml publish-npm job (PRs #517–#520)
- [x] ci(dco-check): grep full body for `Signed-off-by` — systemic DCO false-fail fix (PR #544)
- [x] ci(release): graceful npm publish for E404 scope-not-found + absent NPM_TOKEN (PR #533)
- [x] All v0.1.19→v0.2.0 content on develop (RFC-0109/102/110 roll-out)

**v0.2.0 ceremony status — 4/4 COMPLETE ✅:**
- [x] **Step 1**: `release/v0.2.0` → `main` — PR #523 MERGED ✅ (2026-06-04)
- [x] **Step 2**: Tag `v0.2.0` pushed ✅ + **GitHub Release** published (5 platform binaries + `SHA256SUMS`) ✅ (2026-06-04)
- [x] **Step 3**: All 5 crates to crates.io ✅ (release.yml, 2026-06-04)
- [x] **Step 4**: Back-merge `release/v0.2.0` → `develop` — PR #537 MERGED ✅ (`4e60400f`)

**npm distribution (RFC-0110) — LIVE ✅:** all 6 `@aimasteracc/*` packages published at `0.2.0` (launcher + 5 platform pkgs); `npm i -g @aimasteracc/mycelium` install-verified (`mycelium 0.2.0`). NPM_TOKEN configured in the `npm` GitHub environment (granular: RW all-packages + bypass 2FA; `npm whoami` → `aimasteracc`). The prior E404 saga's root cause was a **non-authenticating token value in the secret** — NOT a missing scope: `@aimasteracc` is the founder's personal user scope (username = `aimasteracc`), so no org was ever needed.

**v0.2 PRD success metrics status:**
- [x] Capabilities reachable from all 3 surfaces: 93/93 MCP tools + CLI + Skills ✅ (Charter §5.13 enforced)
- [x] Category Skills published: 10+ ✅
- [ ] Skills marketplace presence: ≥1 (Claude Code) — **P2, not yet submitted**
- [x] Open P0 bugs: 0 ✅
- [x] Dogfood pass rate: 8/8 (CI dogfood job passing) ✅
- [x] Charter §2 SLA rows satisfied ✅

---

## 🔧 Post-v0.2.0 — Unreleased on develop (→ v0.2.1)

> Commits on develop NOT in the `v0.2.0` tag — verified against `git show v0.2.0:` — that will ship in v0.2.1:

- [x] fix(npm): 128+signal exit codes in launcher (PR #535, `3f81241`) — **not in v0.2.0 crates/tag**. Note: the published npm@0.2.0 *launcher* already includes this fix (assembled from develop during the manual publish), so it is live on the npm surface; v0.2.1 formalizes it into the crates/tag.
- [x] test(mcp): mutation kill-rate exact-count assertions (PR #531, `b696953`) — not in v0.2.0 tag (test-only)
- [x] refactor(mcp): Issue #428 god-file-split slice 3 — requests.rs extract; lib.rs 6,048→4,694 (PR #550, `4818da09`) ✅ merged 2026-06-05
- [x] feat(mcp): RFC-0094 Phase 4 — flip stdio MCP default output to text (~72% fewer tokens); `render()` helper centralises 89 format sites; `with_default_format()` builder; `serve_stdio` defaults to `Text`; Codex P2 (6 path-finder tools) fixed before merge; lib.rs 4,694→4,485 (−209 lines via consolidation) (PR #552, `1a6e3e7`) ✅ merged 2026-06-05
- [x] chore(pm): dispatch v29–v56 (PM state + decisions.jsonl maintenance)
- [x] **fix(core): RFC-0103 per-edge Extends resolution** (PR #554, squash `9e1bd4b`) — MERGED ✅ 2026-06-05

> Already shipped in v0.2.0 (do NOT re-queue — verified present in the `v0.2.0` tag): PR #544 (DCO full-body grep fix) and PR #533 (graceful npm E404 + absent-token handling).

---

## Live priorities (ordered)

**P0 — none.** v0.2.0 is fully shipped (crates + npm + tag + GitHub Release + back-merge). No founder ceremony action outstanding.

**P1 — founder action requested:**
1. **NPM_TOKEN hygiene (optional):** rotate token pasted in transcript (defense-in-depth only; token works).

**P2 — Autonomous (v0.2.1 queue):**
1. **MCP god-file split slice 4** — lib.rs at 4,485 lines after RFC-0094 Phase 4 (render() consolidation saved ~209 lines). The `#[tool_router]` proc-macro requires all tool methods in one impl block; clean extraction requires either Rust `include!()` shims or a delegation approach — scope carefully before executing. ⚠️ **Issue #428 is closed** (completed through slice 3); slice 4 needs a new tracking issue if pursued.
2. **Formalize #535/#531 into crates/tag**: v0.2.1 crates should carry the launcher signal-exit fix (#535) + mutation tests (#531) already on develop.
3. **RFC-0104 cold SLA numbers**: Measure nightly `sla_ancestors_100k` on redb for Charter §2 cold-open budget. Requires founder Charter §2 amendment once data is collected.
4. **Skills marketplace submission**: Claude Code marketplace metadata (icon, screenshots, examples). Requires founder sign-off on listing metadata.

---

## Dispatch state (2026-06-05 v58)

| Agent | Status | Current item |
|---|---|---|
| founder | **P1 (optional)** | Rotate NPM_TOKEN (defense-in-depth; token works). |
| PM | **DONE ✅** | v58: Codex P2 on #556 fixed (PR #554 stale ref corrected → `[x]` merged); PM state v58 written; decisions.jsonl appended. |
| release | **idle** | v0.2.0 ceremony 4/4 ✅. Next: cut `release/v0.2.1` once PR #554 + crates formalized (PRs #535/#531). |
| security-reviewer | **DONE ✅** | Post-v0.2.0 scan: CLEAN. |
| architect | **idle** | RFC-0104 cold SLA Charter §2 amendment (needs nightly measurement data first). |
| rust-implementer | **P2** | God-file-split slice 4 (new issue required; `#[tool_router]` proc-macro scoping needed before implementation). |
| e2e-runner | **idle** | Dogfood 8/8 verified ✅. Next: v0.2.1 regression pass after god-file split. |
| bench | **P2** | `sla_ancestors_100k` nightly (RFC-0104 cold SLA data collection). |
| tech-writer | **P2** | Skills marketplace submission prep (founder sign-off needed). |

---

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- **Skill marketplace listing metadata sign-off** (P2, pending).
- **RFC-0104 cold SLA measurement**: Charter §2 table amendment requires measured nightly data.
- ~~**RFC-0105 Three-Surface EXCEPTION**~~: ✅ RATIFIED 2026-06-03T12:30Z.
- ~~**v0.1.17 git ceremony skip**~~: ✅ RESOLVED.
- **Systemic**: `release.yml` finalize merge — ceremony script is workaround; RFC-0110 `finalize` job uses `git push origin main` (not GitHub PR API), so the old v0.1.6–v0.1.18 auto-close bug is RESOLVED for v0.2.0+.

---

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder if available): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/v0.2.x branch, publish.

---

## Archive

### 2026-06-05 PM dispatch v58 (this run)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns (hits: ci/testing, release-governance), PM state (v57 on chore/pm-dispatch-v57 branch), v0.2 PRD.

**Assessment:**
- 1 open PR: #556 (chore/pm-dispatch-v57, 3/3 CI ✅, 1 Codex P2 finding — stale PR #554 reference).
- 0 open P0/P1 issues. develop HEAD `7d9e8c0` (PM v56); `9e1bd4b` (RFC-0103 Extends fix, PR #554) is in develop ancestry → Codex is correct.
- No P0/P1 items. Top autonomous task: god-file-split slice 4 (P2).

**Actions taken:**
1. **Fixed Codex P2 on PR #556**: line 68 `[ ] PR #554 awaiting merge` → `[x] PR #554 MERGED ✅ 2026-06-05`. Removed stale founder P1 action for #554. Dispatch state table updated from v57 to v58. ✅
2. **Replied to Codex P2 thread** on PR #556 with fix commit reference. ✅
3. **Admin-merged PR #556** (squash, CI 3/3 ✅, Codex P2 fixed). ✅
4. **PM state v58** updated; decisions.jsonl appended. ✅

**Escalations to founder:** none.

### 2026-06-05 PM dispatch v57 (PR #556 — RFC-0103 per-edge Extends merged; PM state corrected)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns (hit: tdd/impl-before-test, async/blocking-read), memory INDEX.md, PM state (v56 on develop post-#553-merge), v0.2 PRD.

**Assessment:**
- 2 open PRs: #553 (chore/pm-dispatch-v56, CI ✅, 0 Codex findings — ready to merge), #554 (feat/rfc-0103-extends-import-resolution, CI ✅ on original commit, 1 Codex P1 NOT resolved — must fix before merge).
- 0 open P0/P1 issues. v0.2.0 ceremony 4/4 COMPLETE. Develop CI fully green.
- Codex P1 on #554: global `redirect_node(stub_id, def_id)` rewires ALL subclasses' Extends edges to one def — wrong when different subclasses import different definitions.

**Actions taken:**
1. **Admin-merged PR #553** (squash `7d9e8c0`) — PM dispatch v56 chore on develop; no Codex findings. ✅
2. **Fixed Codex P1 on PR #554** (commit `99a38e1`): rewrote `resolve_import_aware_extends_stubs` from global to per-edge resolution. Added `AdjacencyList::remove_edge` + `Synapse::remove_edge`. TDD: new test `store_resolve_extends_stub_per_edge_mixed_imports` confirmed RED before fix, GREEN after. 643 core tests + full suite pass; clippy clean. Codex reply posted explaining fix. Push sent to origin. ✅
3. **Pending**: CI on fix commit `99a38e1` not yet visible (push at ~06:18Z; checks still from original 06:07-06:13Z). Escalated to founder for CI verification + admin-merge of #554.
4. **PM state v57** updated; decisions.jsonl appended. ✅

**Escalations to founder:**
- **(P1)** Check CI on PR #554 commit `99a38e1` (all tests pass locally — 643 core, clippy, fmt all green); admin-merge once CI confirms green.

### 2026-06-05 PM dispatch v56 (this run)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns (hit: platform-specific test assertions), memory INDEX.md, PM state (v55 on develop), v0.2 PRD.

**Assessment:**
- 1 open PR: #551 (PM v54+v55, 20/20 CI ✅, 1 Codex P2 thread `is_outdated:true` + aimasteracc reply ✅).
- develop HEAD: `3791214` (PM v55) after merging #551; one commit ahead: `1a6e3e7` (RFC-0094 Phase 4, PR #552, merged ~05:00).
- 0 open P0/P1 issues. v0.2.0 ceremony 4/4 COMPLETE. CI fully green across linux/macos/windows.
- RFC-0094 Phase 4: Codex P2 (6 path-finder tools bypassing render()) fixed before merge; RFC status updated to "Implemented"; no outstanding findings.
- lib.rs: 4,694 (post slice-3) → 4,485 after RFC-0094 Phase 4 consolidation (render() helper replaced ~209 lines of repeated map_or_else blocks).
- God-file-split slice 4 scoped: `#[tool_router]` proc-macro requires all tool methods in one impl block — clean file extraction needs Rust include!() shims or delegation pattern. Issue #428 closed (completed through slice 3); new issue needed for slice 4.

**Actions taken:**
1. **Admin-merged PR #551** (squash `3791214`) — PM dispatch v54+v55 on develop; Codex P2 `is_outdated:true` + reply satisfies Hard Rule. ✅
2. **Verified PR #552** (RFC-0094 Phase 4): Codex P2 fixed in pre-merge commit (`fix(mcp): route the 6 path-finder tools through render()`); RFC-0094 status → Implemented; 442 mcp tests green. No further action required. ✅
3. **Assessed god-file-split slice 4 feasibility**: `#[tool_router]` constraint makes naive module extraction unsafe within 25-min wall clock. Documented the scoping note and queued as new-issue-required. ✅
4. **PM state v56** updated; decisions.jsonl appended. ✅

**Escalations to founder:** none.

### 2026-06-05 PM dispatch v55 (this run)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns (hits: git-workflow, ci/testing), PM state (v54 from chore/pm-dispatch-v54 branch), v0.2 PRD.

**Assessment:**
- 2 open PRs: #550 (Issue #428 slice 3: requests.rs, 24/24 CI ✅, 0 Codex findings), #551 (PM v54 chore, CI running, 1 Codex P2 — dispatch table header "(2026-06-04 v53)" stale).
- 0 open issues. v0.2.0 ceremony 4/4 COMPLETE. No P0/P1 items.

**Actions taken:**
1. **Fixed PR #551 Codex P2** (commit `36e3e71`): dispatch table header advanced from "(2026-06-04 v53)" to "(2026-06-05 v54)"; release row stale Issue #534 prerequisite removed. Codex thread reply posted. ✅
2. **Merged PR #550** (squash `4818da09`) — Issue #428 god-file-split slice 3 landed on develop; lib.rs 6,048→4,694 (−22%). ✅
3. **Merged PR #551** (squash — CI went green) — PM v54 + Codex fix on develop. ✅
4. **PM state v55** updated + decisions.jsonl appended. ✅

**Escalations to founder:** none.

### 2026-06-05 PM dispatch v54 (PR #549 merged by founder; PR #550 opened — god-file-split slice 3)

*(see merged commit on develop for full archive; dispatch table Codex fix in PR #551)*

### 2026-06-04 PM dispatch v53 (this run)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl (local clone, entries v1–v45), anti-patterns, PM state (v28 on local main — stale; v51/v52 from PR #547 commit history), v0.2 PRD.

**Assessment:**
- 1 open PR: #547 (PM v51 chore, 20/20 CI ✅, 1 Codex finding with `aimasteracc` reply = Hard Rule satisfied).
- 1 open issue: #534 (P2, npm E404 tightening — founder-gated).
- develop HEAD: `0fe4f99c` (PM v50 squash, from PR #546). CI green.
- v0.2.0 ceremony: 4/4 COMPLETE ✅ (Step 2 tag + GitHub Release + npm all shipped 2026-06-04).
- Queue: no founder P0s remaining; P2 autonomous v0.2.1 items + optional NPM_TOKEN rotation.

**Actions taken:**
1. **Merged PR #547** (squash `640a8dcf`) — PM v51/v52 wrap-up; Codex P2 replied/fixed by prior session. ✅
2. **Post-v0.2.0 security scan** (release.yml + npm/ code reviewed): CLEAN — no hardcoded secrets; E404 grace is by design (Issue #534); id-token:write is legitimate npm provenance requirement; all tokens properly as `secrets.*`. ✅
3. **Composed PM state v53** — updated header, v0.2.0 ceremony status, v0.2.1 queue, dispatch state. ✅
4. **NOTE (resolved)**: the remote session could not append decisions.jsonl (MCP `get_file_contents` branch-resolution bug returned local-main). Appended locally in this corrected v53 with full repo access — develop's v29–v52 entries intact. Anti-pattern recorded.

**Escalations to founder — both RESOLVED this session:**
1. ~~(P0) Push tag `v0.2.0` + create GitHub Release~~ → **DONE ✅** (tag `v0.2.0` pushed + GitHub Release with 5 binaries + SHA256SUMS).
2. ~~(P0) Register `@aimasteracc` npm scope + add `NPM_TOKEN`~~ → **DONE ✅** — `@aimasteracc` was already the founder's personal user scope (no registration needed); the real blocker was a non-authenticating `NPM_TOKEN` value, now fixed; all 6 packages published & install-verified.
3. **(P1, optional)** Rotate `NPM_TOKEN` — the value was pasted into a chat transcript during the manual publish. Defense-in-depth only; the token works.

### 2026-06-04 PM dispatch v52 (PR #547 branch — Codex P2 fix + MCP split P2 item added)

*(see merged commit `640a8dcf` for full archive)*

### 2026-06-04 PM dispatch v51 (PR #546 merged; 2 stale P2 items cleared; post-v0.2.0 queue tightened)

*(see merged commit `0fe4f99c` for full archive)*

### 2026-06-04 PM dispatch v50 (PRs #544+#545 merged; DCO fix deployed)

*(see commit `0fe4f99c` squash message for full archive)*

### 2026-06-04 PM dispatch v46 (Codex P1+P2 fixes; v0.2.0 ceremony Steps 1+3+4 ✅; security scan CLEAN)

*(see commit `e089b66a` for full archive)*

### 2026-06-04 PM dispatch v36 (v0.2.0 release in progress; PR #522 merged)

*(see commit `b2fe917c` for full archive)*

### 2026-06-04 PM dispatch v29 (PRs #508+#513 merged; RFC-0109 7/7 complete)

*(see commit `e94acb42` for full archive)*

### 2026-06-03 PM dispatch v28 (develop CI fix PR #508; ADR-0010 merged; v0.1.19 boundary corrected)

*(see commit `bf0399a2` for full archive)*

### 2026-06-05 PM dispatch v54 (PR #549 merged; PR #550 opened — god-file-split slice 3)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns (hits: git-workflow, ci/testing), PM state (v53 on develop), v0.2 PRD. Memory NOTE: local clone is on `main` — stale; fetched origin/develop.

**Assessment:**
- 0 open PRs (PR #549 `fix/issue-534-npm-publish-hard-fail` merged 2026-06-05T02:23Z by founder — Issue #534 resolved ✅).
- 0 open issues. Develop CI: ✅ green (main + develop both SUCCESS 2026-06-05T02:23).
- v0.2.0: 4/4 ceremony complete. No P0/P1 items. Top autonomous task: MCP god-file split.

**Actions taken:**
1. **Verified PR #549**: merged, 0 Codex review threads — Clean. Issue #534 ✅.
2. **Executed MCP god-file split slice 3** (Issue #428): extracted 93 request schema types (lines 325–1495) → `requests.rs` (1,179 lines); moved `server_info_tests` + `output_budget_tests` inline mods → `tests.rs`; lib.rs 6,048→4,694 (−22.4%). `pub use requests::*;` re-exports all types; `OutputFormat` re-exported via `pub use crate::formatter::OutputFormat;` in requests.rs. TDD baseline: 444 tests GREEN → refactor → 444 tests GREEN. Clippy -D warnings clean. fmt clean.
3. **Opened PR #550** targeting develop.
4. **Updated PM state v54** + dispatch.

**Escalations to founder:** none.

### Earlier dispatches (v1–v27)

*(archived in git history)*
