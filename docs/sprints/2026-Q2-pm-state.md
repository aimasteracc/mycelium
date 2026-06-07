# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-06-07 (PM dispatch v119 — PR #671 merged (AC-20 tests squash `40ffbc6f`); PR #672 closed superseded; Issue #673 opened (CLI phantom defense-in-depth); next: Issue #636 (RFC-0118 Part B Phase 3 shadowed bindings)) |
| Current sprint | **v0.3.0 ceremony READY** (P0 — founder action) + **RFC-0120 Charter §2 governance event** (P0 — ratio 0.753 vs ≤0.30 claim). RFC-0118 Part B: **ALL 9 COMPLETE + rule-b + AC-20 ✅**. Three-Surface **94/94** ✅. Next P1 autonomous: Issue #636 (shadowed local bindings, RFC-0118 Part B Phase 3). |
| Active release branch | **`release/v0.3.0`** — PR #568 open (→ main); all registries published (crates.io ✅ npm ✅ PyPI ✅); **AWAITING FOUNDER FINALIZE** |
| Next release target | **v0.3.0** → ceremony imminent. **v0.4.0** = VS Code ext (RFC-0112 Ph1 on develop) + TSA-reuse feature set (RFC-0113–0117) + GitHub Action. |
| Final release target | v0.4.0 (IDE plugin Phase 1, TSA-reuse features, cross-repo indexing) |
| Last shipped | **v0.2.0 (ceremony 4/4 COMPLETE)** — crates.io ✅ + npm (6 pkgs, install-verified) ✅ + main ✅ + tag `v0.2.0` ✅ + GitHub Release (5 binaries + SHA256SUMS) ✅ + back-merge ✅. v0.2.1 superseded by v0.3.0. |

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

**v0.2.0 ceremony status — 4/4 COMPLETE ✅:**
- [x] **Step 1**: `release/v0.2.0` → `main` — PR #523 MERGED ✅ (2026-06-04)
- [x] **Step 2**: Tag `v0.2.0` pushed ✅ + **GitHub Release** published (5 platform binaries + `SHA256SUMS`) ✅
- [x] **Step 3**: All 5 crates to crates.io ✅
- [x] **Step 4**: Back-merge `release/v0.2.0` → `develop` — PR #537 MERGED ✅ (`4e60400f`)

**npm distribution (RFC-0110) — LIVE ✅:** all 6 `@aimasteracc/*` packages published at `0.2.0`.

---

## 🚧 v0.3.0 — CEREMONY READY (crates.io ✅ npm ✅ PyPI ✅ — all registries published)

**What ships in v0.3.0:**
- [x] **RFC-0111 Phase 1 — Node/TS SDK** `@aimasteracc/mycelium-sdk` (PR #559, `19fb6f1`)
- [x] **RFC-0111 Phase 2 — Python SDK** `mycelium-rcig` / import `mycelium_rcig` (PR #565, `64e865f`)
- [x] RFC-0103 per-edge `Extends` stub resolution (PR #554, `9e1bd4b`)
- [x] RFC-0094 Phase 4: stdio MCP default → `text` (~72% fewer tokens); `render()` helper (PR #552, `1a6e3e7`)
- [x] Issue #428 god-file-split slice 3: `requests.rs` extract; lib.rs 6,048→4,694 (−22%) (PR #550, `4818da09`)
- [x] fix(npm): 128+signal exit codes (PR #535); mutation kill-rate (PR #531); publish-npm hard-fail on absent NPM_TOKEN (PR #563)
- [x] Version bump: 0.2.0 → 0.3.0 (semver minor: new SDKs; PR #568)

**v0.3.0 ceremony status — READY (all registries published, CI ✅):**
- [x] **PyPI**: RESOLVED — founder switched to twine token auth (commit `38c3214`); Release run #79 `conclusion: success` ✅ (2026-06-05T18:00Z). crates.io ✅ npm ✅ PyPI ✅ all published.
- [ ] **Preferred path**: Trigger `finalize` workflow_dispatch on `release.yml` — handles Steps 1–4 automatically.
- [ ] **Manual fallback** (if finalize unavailable): merge #568 → main, tag `v0.3.0`, GH Release, back-merge. Do NOT re-publish registries.

---

## 🔧 Post-v0.3.0 — Unreleased on develop (→ v0.4.0)

> These commits are on develop but NOT in the `release/v0.3.0` branch. They will ship in v0.4.0.

- [x] **fix(sdk): argv-smuggling guard (Node+Py) + Python 64 MiB output cap** — PR #590, squash `61350b59` ✅ MERGED 2026-06-06 (founder).
- [x] **docs(rfc): RFC-0113 stdlib/builtin callee classification design** — PR #575, squash `7c1a675x` ✅ MERGED.
- [x] **feat(core): RFC-0113 Phase 1 — `classify.rs` static callee classifier** — PR #576 ✅ MERGED.
- [x] **feat(core): RFC-0114 Phase 1 — `health.rs` graph-native project health scorer** — PR #577 ✅ MERGED.
- [x] **docs(rfc): RFC-0117/0116/0115 architectural constraint DSL / pre-edit safety / coverage-aware test-gap** — PRs #578–#580 ✅ MERGED.
- [x] **feat(editors): RFC-0112 Phase 1 — VS Code extension MVP** — PR #587 ✅ MERGED.
- [x] **feat(integrations): GitHub Action — code-intelligence summary in CI** — PR #588 ✅ MERGED.
- [x] **feat(core): RFC-0113 Phase 2 — additive `class` field on `get_callees`** — PR #595 ✅ MERGED (squash `4adce0c`). Phase 3 → Issue #598.
- [x] **ci(nightly): fix mutants.out file/directory collision** — PR #597 MERGED (`b36d3ff`).
- [x] **ci(nightly): upload mutants.out/ report directory as artifact** — PR #603 MERGED (squash `5303351`).
- [x] **chore(pm): PM states v84–v88** — various PRs ✅ MERGED.
- [x] **feat(core): RFC-0118 Part A — `NodeKind::Unresolved` + `is_real_symbol()` gate** — PR #616 (squash `8b04acb2`) ✅ MERGED.
- [x] **feat(core): RFC-0118 Parts B+C (core)** — PR #618 (squash `5b09145b`) ✅ MERGED.
- [x] **feat(core): RFC-0113 Phase 3** — PR #620 (squash `12cf4252`) ✅ MERGED. Issue #598 closed.
- [x] **feat(core): RFC-0119 Phase 1+2 (ranking)** — PRs #623/#626 ✅ MERGED.
- [x] **feat(core): RFC-0116 Phase 1 + RFC-0115/0117 pure core** — PR #629 ✅ MERGED.
- [x] **feat(core): RFC-0118 Part B resolution engine + extractor (Rust, F5 fix)** — PRs #633/#635 ✅ MERGED. `get-callers Store>upsert_node` 0→60.
- [x] **RFC-0120 Phase 1a+1b (token bench + BPE)** — PRs #639–#645 ✅ MERGED. `bpe_charter_sla_binding` FAILS (ratio 0.753) — RFC-0120 P0 governance event → founder.
- [x] **feat(packs): RFC-0118 Part B (JS, Java, C#, C++, Go, Ruby)** — PRs #653/#656/#659/#661/#662/#665 ✅ MERGED. ALL 9 LANGUAGE PACKS COMPLETE.
- [x] **feat(core): RFC-0118 Part B rule-b (Rust param-type receiver)** — PR #667 (squash `f7330ae`) ✅ MERGED.
- [x] **docs(skills): Three-Surface INDEX 94/94 + graph-structure Skill** — PR #668 (squash `27cbe5ab`) ✅ MERGED.
- [x] **chore(pm): PM states v115–v116** — PRs #669 ✅ MERGED.
- [x] **chore(pm): PM states v117–v118** — PRs #670 ✅ MERGED (via develop, prior to #671).
- [x] **test(RFC-0118): AC-20 regression tests — rank_symbols excludes Unresolved phantoms** — PR #671, squash `40ffbc6f` ✅ MERGED 2026-06-07. `rank_symbols_excludes_unresolved_phantom` (MCP, `tests.rs:1445`) + `rank_symbols_json_shape_parity_with_mcp` (CLI, `cli_centrality.rs:63`). RFC-0118 AC-20 ✅. Codex P2 → Issue #673 (CLI phantom integration test, P2 defense-in-depth).

---

## Live priorities (ordered)

**P0 — Founder action (both required before release):**
1. **PR #568 finalize**: All registries published (crates.io ✅ npm ✅ PyPI ✅). Trigger `finalize` workflow_dispatch on `release.yml` (preferred) OR manual Steps 1–4: merge #568 → main, tag `v0.3.0`, GH Release, back-merge. **Do NOT re-publish registries.**
2. **RFC-0120 Charter §2 governance event** (PR #649): ratio = **0.753** vs ≤0.30 claim. `bpe_charter_sla_binding` fails. See `crates/mycelium-mcp/tests/corpus/REPORT.md §Decision`. Choose: **A** (retract claim, amend Charter §2 + README), **B** (redesign TextFormatter for ≥70% reduction), or **C** (reframe comparison to Hyphae query syntax). Charter §9 amendment requires BDFL approval.

**P1 — Next autonomous:**
3. **Issue #636** (RFC-0118 Part B Phase 3): Shadowed local bindings — scope-aware receiver inference. Next P1 autonomous now that AC-20 is done.
4. **RFC-0119 AC-12/AC-13** (e2e-runner): Real-corpus context query + dogfood transcript.

**P2 — Deferred:**
5. **Issue #673**: Add CLI integration test for rank-symbols phantom exclusion (AC-20 defense-in-depth). Requires understanding packs/rust extractor `NodeKind::Unresolved` creation path.
6. **MCP god-file split slice 4** — lib.rs ~4,485 lines.
7. **RFC-0104 cold SLA numbers**: Charter §2 amendment (founder, after nightly data collected).
8. **Skills marketplace submission**: metadata sign-off (founder).
9. **VS Code Phase 1.5**: `vsce publish` + marketplace metadata (after v0.3.0 ships; founder sign-off).
10. **GitHub Action live run**: Test on Mycelium repo with a real PR (after v0.3.0 ships).
11. **RFC-0120 Phase 1c real corpus**: If Option B chosen, rebuild TextFormatter; otherwise retract the ≤0.30 claim.

---

## Dispatch state (2026-06-07 v119)

| Agent | Status | Current item |
|---|---|---|
| founder | **P0 action (2 items)** | **(1)** PR #568: v0.3.0 ceremony READY — trigger `finalize` workflow_dispatch on `release.yml`. **(2)** RFC-0120 Charter §2 governance event — REPORT.md §Decision on develop: choose Option A/B/C. |
| PM | **DONE ✅** | v119: PR #671 merged (AC-20 tests `40ffbc6f`); PR #672 closed superseded; Issue #673 opened; decisions.jsonl appended. |
| release | **P0 — READY** | PR #568: Release CI ✅. crates.io ✅ npm ✅ PyPI ✅. Awaiting founder `finalize`. |
| security-reviewer | **P2** | Post-v0.3.0 regression scan (after release ships). |
| architect | **P1** | RFC-0104 cold SLA Charter §2 amendment (after nightly data; founder). |
| rust-implementer | **P1** | Issue #636 (RFC-0118 Part B Phase 3: shadowed local bindings, scope-aware receiver inference). |
| e2e-runner | **P2** | v0.3.0 regression pass. AC-12/AC-13 RFC-0119 dogfood. |
| bench | **P2** | `sla_ancestors_100k` nightly (RFC-0104 cold SLA data). |
| tech-writer | **P2** | Skills marketplace submission (founder sign-off). VS Code Phase 1.5 docs. |

---

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- **Skill marketplace listing metadata sign-off** (P2, pending).
- **RFC-0104 cold SLA measurement**: Charter §2 table amendment requires measured nightly data.
- **RFC-0120 §2 charter amendment (ACTIVE — P0)**: `bpe_charter_sla_binding` FAILS — ratio **0.753** vs ≤0.30 threshold. Per RFC-0120 REPORT.md §Decision: founder must choose **Option A** (retract claim — *recommended*), **Option B** (redesign TextFormatter), or **Option C** (reframe to Hyphae query syntax comparison).
- ~~**RFC-0112 IDE plugin design sign-off**~~: ✅ RESOLVED — PR #587 merged (VS Code MVP Phase 1).
- ~~**RFC-0111 Charter §3 amendment**~~: ✅ RATIFIED — PRs #559/#565 MERGED.
- ~~**RFC-0105 Three-Surface EXCEPTION**~~: ✅ RATIFIED 2026-06-03T12:30Z.

---

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder if available): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/vX.Y.Z branch, publish.

---

## Archive

### 2026-06-07 PM dispatch v119 (this run)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, `_orchestrator.md`, decisions tail-20, anti-patterns (domains: ci/testing/release-governance/git-workflow), PM state v118 (from chore branch), v0.2 PRD.

**Assessment:**
- 3 open PRs: #671 (AC-20 tests, CI 22/22 ✅, 1 Codex P2), #672 (PM v118 chore, CI 22/22 ✅, 1 Codex P2), #568 (release/v0.3.0, founder).
- 0 open P0/P1 issues.
- Develop CI GREEN (develop HEAD = `276807a` PM v116; feature branch `40ffbc6f` after merge).
- RFC-0118 Part B: ALL 9 languages complete + rule-b + AC-20 pending merge.

**Actions taken:**
1. **Analyzed Codex P2** on #671/#672: CLI test uses `prepare_diamond()` (no phantom) — only tests JSON shape, not phantom exclusion. Valid finding. `is_real_symbol` guard is in shared core code path; MCP test covers it directly. Decision: option (b) rejection + option (c) spin-off.
2. **Opened Issue #673**: CLI phantom integration test (AC-20 defense-in-depth, P2). ✅
3. **Replied to Codex** on PR #671 (comment 3370025710): rejection + Issue #673 reference. ✅
4. **Replied to Codex** on PR #672 (comment 3370027902): same justification. ✅
5. **Merged PR #671** (squash `40ffbc6f`): AC-20 tests on develop. ✅
6. **Closed PR #672** as superseded (chore stacked on #671; becomes stale post-squash). ✅
7. **PM state v119** written + decisions.jsonl appended. ✅

**Escalations to founder (carried forward):**
- **(P0-1)** PR #568: v0.3.0 ceremony READY — trigger `finalize` workflow_dispatch on `release.yml`.
- **(P0-2)** RFC-0120 Charter §2 governance event — REPORT.md §Decision: choose Option A/B/C.

### 2026-06-07 PM dispatch v118 (prior run)

CI red on #671+#672 (clippy: doc_markdown × 3, map_unwrap_or × 1); fix commit `724dd27` pushed to feature branch; chore branch rebased; both CIs re-triggered to green.

### 2026-06-07 PM dispatch v117 (prior run)

PR #670 merged (PM v116 chore); AC-20 tests PR #671 opened; RFC-0118 Part B ALL 9 COMPLETE confirmed.

### 2026-06-07 PM dispatch v116 (prior run)

PR #668 (Three-Surface INDEX 94/94 fix, squash `27cbe5ab`) + PR #669 (PM v115, squash `651819a1`) merged. Codex P2 on #668 fixed (JSON shape `dead_code`). Codex P2 on #669 rejected (merge ordering).

### Earlier dispatches (v1–v115)

*(archived in git history)*
