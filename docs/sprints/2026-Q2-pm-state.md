# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-06-08 (PM dispatch v127 — PR #689 (MCP agent-experience) rebased post-#688 + merged first (22/22 ✅, squash `c2dddcc`); PR #687 (PM v126 chore) Codex P2 rejected + rebased twice (post-#688, post-#689) + merged; PR #688 (Hyphae docs) added to unreleased) |
| Current sprint | **v0.3.0 ceremony READY** (P0 — founder action) + **RFC-0120 Charter §2 governance event** (P0 — ratio 0.753 vs ≤0.30 claim). RFC-0118: ALL PARTS COMPLETE. Three-Surface **94/94** ✅. MCP agent UX: entry-points paginates ✅, path-not-found teaches format ✅, reachability tools disambiguated ✅. |
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
- [x] **docs(rfc): RFC-0113 stdlib/builtin callee classifier design** — PR #575, squash `7c1a675x` ✅ MERGED.
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
- [x] **chore(pm): PM state v116** — PR #670 ✅ MERGED (squash `276807a`). PM dispatches v117–v118 had no separate develop chore commits (v117: dispatch-only; v118: feature-branch CI fix).
- [x] **test(RFC-0118): AC-20 regression tests — rank_symbols excludes Unresolved phantoms** — PR #671, squash `40ffbc6f` ✅ MERGED 2026-06-07. `rank_symbols_excludes_unresolved_phantom` (MCP, `tests.rs:1445`) + `rank_symbols_json_shape_parity_with_mcp` (CLI, `cli_centrality.rs:63`). RFC-0118 AC-20 ✅. Codex P2 → Issue #673 (CLI phantom integration test, P2 defense-in-depth).
- [x] **chore(pm): PM state v120** — PR #674, squash `bc13809` ✅ MERGED 2026-06-07. Codex P2 (line 94 v117-v118/PR#670 attribution) fixed.
- [x] **fix(resolver): RFC-0118 Part B Phase 3 — shadowed local bindings decline to bind** — PR #675, squash `95b75e4` ✅ MERGED 2026-06-08. Closes Issue #636. New test `infer_shadowed_local_returns_none` (RED→GREEN). `cargo test --all` + clippy -D warnings + fmt ✅.
- [x] **feat(core): RFC-0118 Part A.2 — gate graph-theory queries on real-symbol induced subgraph** — PR #677, squash `2b3654d` ✅ MERGED 2026-06-08. New `Store::symbol_universe()` single source of truth; 19 graph-theory queries gated (phantoms excluded as nodes + edge endpoints); centrality normalization over `|real symbols|`; ~22 new twin-oracle tests; ADR-0012. AC-23 ✅ (19 listed queries). `degree_centrality` deferred → Issue #678 (P2).
- [x] **fix(resolver): RFC-0118 Part A — narrow TypeAlias/Go named-type guard in call-resolution** — PR #682, squash `b474618` ✅ MERGED 2026-06-08. Go `type Status int` named types are callable as type conversions — blanket TypeAlias guard was wrong. Narrowed `is_uncallable_target_for_call_stub` + `resolve_call_site_contexts` to spare `.go>` defs. RED-first test `store_resolve_go_named_type_call_still_resolves`. Issue #678 CLOSED (degree_centrality verified via PR #682 `degree_centrality_excludes_phantoms_and_induces_degree`, `store/tests.rs:1510`). RFC-0118 AC-23 fully satisfied.
- [x] **fix(watch): ignore-aware WatchEngine NonRecursive directory watches** — PR #686, squash `bf2d246` ✅ MERGED 2026-06-08. WatchEngine::attach now registers per-directory NonRecursive watches honouring `.gitignore`/`.myceliumignore`/`target/`/`.mycelium/` exclusions. Fixes serve --mcp EMFILE crash on large repos. Follow-up commit: drive() pre-pass rescans newly created dirs + atomic dir expansion.
- [x] **test(cli): AC-20 defense-in-depth — rank_symbols_excludes_unresolved_phantom with positive control** — PR #684 ✅ MERGED 2026-06-08. Fixture extended with `entry()→caller()` giving `caller` a real incoming edge; positive control (`src/lib.rs>caller` present) + negative control (`unknown_extern_fn` absent). Closes Issue #673. Codex P2 fixed (`be73ba9`).
- [x] **fix(docs): Hyphae kind-selector examples corrected + parse-verified query examples added** — PR #688 ✅ MERGED 2026-06-08. All documented Hyphae examples (README, Node/Python SDK snippets, architecture-context skill) used dot-less `function:calls(#X)` — invalid Hyphae (kind selectors require leading `.`). Corrected to `.function:calls(#AuthService)` / `#Foo>.method`. Both `mycelium_query` MCP description and CLI `query` help now carry copy-pasteable parse-verified examples (`#Foo`, `*:calls(#Foo)`, `.function:calls(#Foo)`, `.class:has(.method)`, `#Foo>.method`) in lockstep. Regression test `documented_examples_parse.rs` added. Codex P2 (CLI↔MCP drift) fixed. Refs: RFC-0003, RFC-0090.
- [x] **fix(mcp): MCP agent-experience — entry-points pagination + actionable path-not-found + reachability disambiguation** — PR #689 ✅ MERGED 2026-06-08. (A) `mycelium_get_entry_points` now accepts `limit`/`offset`/`budget` + returns `{entry_points,count,total_count}` via shared core builder; CLI `get-entry-points` gains byte-identical flags (was bare JSON dump ~37K tokens). (B) shared `not_found()` MCP helper + CLI `path_not_found()` now emit `path not found: {p} — symbol paths are file>Type>member; run mycelium_search_symbol`. (C) `get_cross_refs`/`get_reachable_to`/`get_reaches_into`/`batch_reachable_to`/`get_caller_tree` descriptions carry verified "When to use vs alternatives" table. Codex P2 (`entry_points` key missing from `apply_budget`) fixed in commit 2 (`68b9051`). 1635 tests pass.

---

## Live priorities (ordered)

**P0 — Founder action (both required before release):**
1. **PR #568 finalize**: All registries published (crates.io ✅ npm ✅ PyPI ✅). Trigger `finalize` workflow_dispatch on `release.yml` (preferred) OR manual Steps 1–4: merge #568 → main, tag `v0.3.0`, GH Release, back-merge. **Do NOT re-publish registries.**
2. **RFC-0120 Charter §2 governance event** (PR #649): ratio = **0.753** vs ≤0.30 claim. `bpe_charter_sla_binding` fails. See `crates/mycelium-mcp/tests/corpus/REPORT.md §Decision`. Choose: **A** (retract claim, amend Charter §2 + README), **B** (redesign TextFormatter for ≥70% reduction), or **C** (reframe comparison to Hyphae query syntax). Charter §9 amendment requires BDFL approval.

**P1 — Next autonomous:**
3. **RFC-0119 AC-12/AC-13** (e2e-runner): Real-corpus context query + dogfood transcript.
4. **RFC-0118 Part B dogfood #2+**: Additional dogfood QA passes on the release CLI (get-callers precision, phantom-free graph queries, multi-language edge cases) using the indexed Mycelium codebase.

**P2 — Deferred:**
5. **Issue #657**: Method/function definition spans use enclosing type extent (jump-to-definition precision, P2 enhancement).
7. **Issue #612**: RFC-0118 Phase 1 implementation notes (Phase 2b prerequisite: cross-file extraction ordering).
8. **MCP god-file split slice 4** — lib.rs ~4,485 lines.
9. **RFC-0104 cold SLA numbers**: Charter §2 amendment (founder, after nightly data collected).
10. **Skills marketplace submission**: metadata sign-off (founder).
11. **VS Code Phase 1.5**: `vsce publish` + marketplace metadata (after v0.3.0 ships; founder sign-off).
12. **GitHub Action live run**: Test on Mycelium repo with a real PR (after v0.3.0 ships).
13. **RFC-0120 Phase 1c real corpus**: If Option B chosen, rebuild TextFormatter; otherwise retract the ≤0.30 claim.

---

## Dispatch state (2026-06-08 v127)

| Agent | Status | Current item |
|---|---|---|
| founder | **P0 action (2 items)** | **(1)** PR #568: v0.3.0 ceremony READY — trigger `finalize` workflow_dispatch on `release.yml`. **(2)** RFC-0120 Charter §2 governance event — REPORT.md §Decision on develop: choose Option A/B/C. |
| PM | **DONE ✅** | v127: PR #689 rebased (conflict with #688) + merged FIRST (22/22 ✅, `c2dddcc`); PR #687 Codex P2 rejected + rebased twice + merged (20/20 ✅, `664843b`); PR #688 + #689 noted in unreleased; PM state v127 chore PR #690 pushed. |
| release | **P0 — READY** | PR #568: Release CI ✅. crates.io ✅ npm ✅ PyPI ✅. Awaiting founder `finalize`. |
| security-reviewer | **P2** | Post-v0.3.0 regression scan (after release ships). |
| architect | **P1** | RFC-0104 cold SLA Charter §2 amendment (after nightly data; founder). |
| rust-implementer | **P1 (next)** | RFC-0119 AC-12/AC-13 real-corpus dogfood (after v0.3.0 ceremony); RFC-0118 Part B dogfood #2. |
| e2e-runner | **P1** | AC-12/AC-13 RFC-0119 real-corpus dogfood. v0.4.0 regression pass (after v0.3.0 ceremony). |
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

### 2026-06-08 PM dispatch v127 (this run)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, `_orchestrator.md`, decisions.jsonl tail-20 (160 lines; last entries = PR #686 follow-ups + PR #688 Hyphae docs + PM v126 dispatch), anti-patterns (domains: ci/release-governance/storage/async/git-workflow/tdd), PM state v126 (from chore/pm-state-v126 branch — develop v124, PRs #682+#684+#686 merged post-v124), v0.2 PRD.

**Assessment:**
- 3 open PRs: #689 (fix/mcp-agent-experience, `dirty` — conflict with PR #688 in CHANGELOG + decisions.jsonl; Codex P2 already fixed in commit `68b9051`/`a88c8bc`), #687 (PM v126, `dirty` — conflict with PR #688; 1 Codex P2 — outdated tree-ancestry check), #568 (release/v0.3.0, founder-gated).
- 0 open P0/P1 issues.
- Develop GREEN (HEAD `f5835e3` — PR #688 Hyphae docs fix, squash-merged since v126 dispatch).

**Actions taken:**
1. **Rebased PR #689** (`fix/mcp-agent-experience`) onto `origin/develop` (post-#688): resolved CHANGELOG + decisions.jsonl conflicts (both append-only, kept all bullets/entries). Force-pushed as `68b9051`. CI ran 22/22 ✅ (incl. Windows). ✅
2. **Rejected Codex P2 on PR #687**: finding used PR branch tree ancestry (not develop's); `278f0f7f` (PR #684 squash) IS develop's base — Codex was checking the docs-only branch's own tree, not develop. Replied with option (b) justification (comment `3371134196`). ✅
3. **Rebased PR #687** (`chore/pm-state-v126`) onto `origin/develop` (post-#688): resolved decisions.jsonl conflict (Hyphae docs entry `00:00` + PM v126 dispatch `05:20`); both kept. Force-pushed as `b1d7be8`. ✅
4. **Merged PR #689** once all 22/22 ✅ — squash `c2dddcc`. Develop HEAD now `c2dddcc`. ✅
5. **Re-rebased PR #687** onto `origin/develop` (post-#689): resolved decisions.jsonl conflict again (new `07:30` MCP-fixes entry + `05:20` PM-v126 entry); all kept. Force-pushed as `a2cae03`. CI running (19 checks). ✅
6. **Merged PR #687** once CI 20/20 ✅ — squash `664843b`. PM state v126 lands on develop. ✅
7. **Added PR #688 + PR #689 to post-v0.3.0 unreleased section**; updated PM state to v127; created `chore/pm-state-v127` + pushed chore PR (#690). ✅

**Escalations to founder (carried forward):**
- **(P0-1)** PR #568: v0.3.0 ceremony READY — trigger `finalize` workflow_dispatch on `release.yml`.
- **(P0-2)** RFC-0120 Charter §2 governance event — choose Option A/B/C.

### 2026-06-08 PM dispatch v126 (this run)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, `_orchestrator.md`, decisions.jsonl tail-20 (158 lines; last entry = PR #686 follow-ups `06:00`), anti-patterns (domains: ci/release-governance/storage/async/git-workflow/tdd), PM state develop (v124 — PRs #682+#686 merged post-v124 dispatch), v0.2 PRD.

**Assessment:**
- 3 open PRs: #685 (PM v125 chore — dirty/conflict, superseded by v126; decisions.jsonl conflict caused by #682+#686 merging after branch creation from `afaf634`), #684 (test/Issue #673 CLI phantom test, 1 Codex P2 unresolved, CI 22/22 ✅ on original commit then CI in-progress after fix), #568 (release/v0.3.0, founder-gated).
- 0 open P0/P1 issues (Issue #678 confirmed CLOSED by PM v125: degree_centrality verified in PR #682; Issue #673 resolved by PR #684).
- Develop GREEN (HEAD `bf2d246` — PR #686 ignore-aware watch fix).

**Actions taken:**
1. **Diagnosed Codex P2 on PR #684** (thread `3370804817`): negative-only assertion — `prepare_with_unresolved_call()` had only `caller()` with 0 callers; an empty result would still pass. Fix: add `entry()` calling `caller()` → `caller` gets 1 incoming edge → appears in `rank-symbols` output. ✅
2. **Pushed fix commit `be73ba9`** to branch `fix/issue-673-rank-symbols-phantom-cli-test`. CI re-triggered. ✅
3. **Replied to Codex P2 thread** on PR #684 (comment `3370954353`): explains positive + negative controls. ✅
4. **Closed PR #685** (PM v125 chore, superseded — decisions.jsonl conflict after #682+#686 merged post-branch creation). v126 supersedes it. ✅
5. **Merged PR #684** (test/Issue #673, CI 22/22 ✅, Codex P2 fixed `be73ba9`) once Windows CI green → Issue #673 CLOSED. ✅
6. **PM state v126 written + decisions.jsonl appended** (this entry). ✅

**Escalations to founder (carried forward):**
- **(P0-1)** PR #568: v0.3.0 ceremony READY — trigger `finalize` workflow_dispatch on `release.yml`.
- **(P0-2)** RFC-0120 Charter §2 governance event — choose Option A/B/C.

### 2026-06-08 PM dispatch v125 (superseded — PR #685 closed due to decisions.jsonl conflict)

PM v125: Merged PR #683 (PM v124 chore, squash `afaf634`). Closed Issue #678 (degree_centrality confirmed via PR #682 twin-oracle test). Opened PR #684 (test/Issue #673 CLI phantom test). PM v125 chore PR #685 had decisions.jsonl conflict (develop advanced via #682+#686 merging after branch creation from `afaf634`). Superseded by v126.

### 2026-06-08 PM dispatch v124 (this run)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, `_orchestrator.md`, decisions.jsonl tail-20 (153 lines; last entry = v123 dispatch), anti-patterns (domains: ci/release-governance/storage/async/git-workflow/tdd), PM state v123 (develop HEAD = `b329eca`... + v123 PM merge `ca90d98`), v0.2 PRD.

**Assessment:**
- 3 open PRs: #682 (RFC-0118 Part A call-resolution, CI in-progress — 1 Codex P2 unresolved), #681 (PM state v123, CI 20/20 ✅, Codex P2 resolved), #568 (release/v0.3.0, founder finalize).
- 0 open P0/P1 issues (Issue #678 is P2 waiting on PR #682 to land).
- Develop CI: GREEN (HEAD `ca90d98` after PM v123 merge).

**Actions taken:**
1. Admin-merged PR #681 (chore/pm-state-v123, CI 20/20 ✅, Codex P2 `is_resolved:true`) → squash `ca90d98`. ✅
2. Diagnosed Codex P2 on PR #682: Go named types (`type Status int`) stored as `NodeKind::TypeAlias` but ARE callable as type conversions (`Status(1)`) — blanket TypeAlias guard incorrectly blocked them. Fix: narrow `is_uncallable_target_for_call_stub` + inline guard in `resolve_call_site_contexts` to spare `.go>` definitions. ✅
3. Added RED-first test `store_resolve_go_named_type_call_still_resolves` → confirmed FAIL before fix. ✅
4. Applied fix (commit `d55129f`), ran 836 core tests (all green), fmt + clippy -D warnings clean. ✅
5. Pushed fix to PR #682 branch. CI triggered. ✅
6. Posted Codex P2 reply on PR #682 (option a — fixed, `d55129f`). ✅
7. PM state v124 written + decisions.jsonl appended (this entry). ✅

**Escalations to founder:**
- PR #568: v0.3.0 ceremony READY — trigger `finalize` workflow_dispatch.
- RFC-0120: Charter §2 ratio claim — Option A/B/C decision.

### 2026-06-08 PM dispatch v123 (this run)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, `_orchestrator.md`, decisions tail-20, anti-patterns (domains: ci/testing/release-governance/storage/async/git-workflow), PM state v122 (develop HEAD `e6966194` = PM state v122 chore squash), v0.2 PRD.

**Assessment:**
- 4 open PRs: #680 (RFC-0118 Part B dogfood #1, CI all green on original SHAs), #679 (PM state v122 chore, CI green), #676 (PM state v121, merge conflict, superseded), #568 (release/v0.3.0, founder).
- 0 P0 issues, 0 P1 issues.
- Develop CI GREEN (HEAD `e6966194` = PM v122 squash merge).
- **Codex P1 on PR #679**: decisions.jsonl rewritten (148→46 lines) — Hard Rule violation (append-only memory). Branch was created from `main` (46 lines) instead of `develop` (148 lines).
- **Codex P2 on PR #680**: `enclosing_self_type` fired for ALL type-container languages including TypeScript/JavaScript (which share `class_declaration` with Java/C# in tree-sitter). False caller edges possible in TS/JS class bodies with duplicate method names.

**Actions taken:**
1. **Closed PR #676** (stale, merge conflict, superseded by PR #679 v122). ✅
2. **Fixed PR #679 Codex P1**: Restored `.hive/memory/decisions.jsonl` from develop (148 lines) + v122 entry (line 149). Commit `57357bf` pushed to `chore/pm-state-v122`. Replied to Codex thread. ✅
3. **Fixed PR #680 Codex P2**: Parallel agent had already pushed `IMPLICIT_SELF_SCOPES` structural `(method-kind, container-kind)` pair allow-list fix (commit `8d39725`) — superior to a file-extension check (no extension-list maintenance). Confirmed fix is correct and complete. Replied to Codex thread with justification (commit `8d39725`, 3 new exclusion tests). ✅
4. **Merged PR #679** (chore/pm-state-v122): squash `e6966194`. CI 20/20 ✅, Codex P1 fixed. ✅
5. **Rebased PR #680** onto new develop (decisions.jsonl conflict after #679 merge): rebased + force-pushed `a4b2c5c`, CI re-triggered (green trajectory). ✅
6. **Merged PR #680** (fix/RFC-0118-bare-self-method-disambiguation): once CI 20/20 ✅. RFC-0118 Part B dogfood #1 on develop. ✅
7. **Updated PM state to v123** (this file). ✅

**Escalations to founder (carried forward):**
- **(P0-1)** PR #568: v0.3.0 ceremony READY — trigger `finalize` workflow_dispatch on `release.yml`.
- **(P0-2)** RFC-0120 Charter §2 governance event — choose Option A/B/C.

### 2026-06-08 PM dispatch v122 (prior run)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, `_orchestrator.md`, decisions tail-20, anti-patterns (domains: ci/testing/release-governance/storage/async/git-workflow), PM state v120 (develop HEAD `2b3654d` = RFC-0118 Part A.2 squash), v0.2 PRD.

**Assessment:**
- 3 open PRs: #675 (fix #636 shadowed bindings, CI 22/22 ✅, 0 Codex), #676 (PM v121 chore, CI 22/22 ✅, 1 Codex P2), #677 (RFC-0118 Part A.2, CI in_progress → 20/20 ✅, 1 Codex P2).
- 4 open issues: #673 (P2), #657 (P2), #636 (P1), #612 (P2).
- Release PR #568 (v0.3.0, all registries published, awaiting founder finalize — P0).
- Develop CI GREEN (HEAD `2b3654d` after #677 squash merge).

**Actions taken:**
1. **Merged PR #675** (fix/rfc-0118-part-b-phase3-shadowed-bindings, CI 22/22 ✅, 0 Codex): squash `95b75e4`. Closes Issue #636. ✅
2. **Opened Issue #678** (degree_centrality gating, RFC-0118 Part A.2 follow-up, P2) — Codex P2 on PR #677, option (c) spin-off per Charter Codex Hard Rule. ✅
3. **Replied to Codex** on PR #676 (general comment + thread reply `3370408703`): PR #675 being merged this dispatch; v122 corrects PM state inconsistency. ✅
4. **Replied to Codex** on PR #677 (thread reply `3370408796`): degree_centrality spun off as Issue #678 option (c). ✅
5. **Merged PR #677** (feat/RFC-0118-partA2-graph-gating, CI 20/20 ✅, Codex thread addressed): squash `2b3654d`. RFC-0118 Part A.2 + ADR-0012 on develop. AC-23 ✅. ✅
6. **PR #676 merge FAILED** (conflict — develop moved after #675+#677): closed #676 as superseded. This v122 chore supersedes it. ✅
7. **Updated PM state to v122** (this file, branch chore/pm-state-v122). ✅
8. **Appended decisions.jsonl** (this dispatch). ✅

**Escalations to founder (carried forward):**
- **(P0-1)** PR #568: v0.3.0 ceremony READY — trigger `finalize` workflow_dispatch on `release.yml`.
- **(P0-2)** RFC-0120 Charter §2 governance event — choose Option A/B/C.

### 2026-06-08 PM dispatch v121 (chore PR #676 closed, superseded by v122)

PM v121: opened PR #675 (fix #636 shadowed local bindings, RFC-0118 Part B Phase 3). PM state chore PR #676 opened but merged with conflict (develop moved after v122 merged #675+#677). Content folded into v122 dispatch. decisions.jsonl for v121 combined into v122 entry.

### 2026-06-07 PM dispatch v120 (prior run)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, `_orchestrator.md`, decisions tail-20, anti-patterns, PM state v119 (PR #674 branch), v0.2 PRD.

**Assessment:**
- 2 open PRs: #674 (PM v119 chore, CI 22/22 ✅, 1 Codex P2), #568 (release/v0.3.0, founder).
- 4 open issues: #673 (P2), #657 (P2), #636 (P1), #612 (P2).
- Develop CI GREEN (all 3 workflows on develop HEAD `40ffbc6f` success).
- PR #568: all registries published; awaiting founder `finalize`.

**Actions taken:**
1. **Analyzed Codex P2** on PR #674 (thread `PRRT_kwDOSq56sc6HsT9w`): line 94 incorrectly attributed "PM states v117–v118" to PR #670, but archive confirms PR #670 = PM v116 chore (squash `276807a`); v117/v118 had no separate develop chore commits. Decision: option (a) fix.
2. **Fixed line 94**: corrected to "PM state v116 — PR #670 ✅ MERGED (squash `276807a`); v117–v118 had no separate develop chore commits." ✅
3. **Updated PM state to v120** (header, dispatch state, archive). ✅
4. **Appended decisions.jsonl**. ✅
5. **Pushed fix to PR #674 branch** + replied to Codex thread. ✅
6. **Merged PR #674** once CI green. ✅

**Escalations to founder (carried forward):**
- **(P0-1)** PR #568: v0.3.0 ceremony READY — trigger `finalize` workflow_dispatch on `release.yml`.
- **(P0-2)** RFC-0120 Charter §2 governance event — choose Option A/B/C.

### 2026-06-07 PM dispatch v119 (prior run)

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
