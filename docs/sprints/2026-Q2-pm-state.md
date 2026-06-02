# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-06-02 (PM dispatch v12 — security scan CLEAN; release/v0.1.17 CUT; PR #452 → main (founder-gated) + PR #453 back-merge opened) |
| Current sprint | **v0.1.17 — RELEASE BRANCH CUT** (`release/v0.1.17`, PR #452 → main pending founder auth) |
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
- [x] **Step 1**: `release/v0.1.13` → `main` — PR #332 MERGED ✅ (founder authorized 2026-05-31 "按照我们的流程发布")
- [x] **Step 2**: Tag `v0.1.13` pushed ✅
- [x] **Step 3**: GitHub Release published ✅
- [x] **Step 4**: Back-merge `release/v0.1.13` → `develop` — PR #333 MERGED ✅

**Note on release.yml systemic failure**: The `merge to main, tag, GitHub Release` job still fails on every release. Tag and GitHub Release are published correctly; only the auto-merge step fails. This remains an open escalation to founder before v0.2.0.

---

## ✅ v0.1.14 — SHIPPED (ceremony 4/4 COMPLETE — PR #352 merged to main, tag + Release + back-merge done)

**What shipped:**
- [x] RFC-0096 Phase 2 TypeScript: `import type` → TypeImports edges + TS resolver bug fix
- [x] RFC-0093 Phase 3 (BREAKING): all 89 MCP tools → `is_error: Some(true)` per MCP spec
- [x] Skills INDEX.md CI gate: `skill-parity` promoted to required Quality Gate
- [x] Store::merge R1 parallel-index primitive (step 1/2)
- [x] Dogfood pass rate 8/8: all 8 core CLI commands green

**v0.1.14 ceremony status — ALL FOUR STEPS COMPLETE ✅:**
- [x] **Step 1**: `release/v0.1.14` → `main` — PR #352 MERGED ✅ (founder authorized 2026-05-31; one-time --admin due to squash-trailer DCO artifact on cecb11f — all real quality gates green)
- [x] **Step 2**: Tag `v0.1.14` pushed ✅
- [x] **Step 3**: GitHub Release published ✅
- [x] **Step 4**: Back-merge `release/v0.1.14` → `develop` — PR #349 MERGED ✅

**Note also shipped in v0.1.14:**
- [x] R1 parallel index step 2: `index_path_parallel` via `thread::scope` + `Store::merge` reduce — Issue #342 CLOSED (PR #351 merged)

**⚠️ Systemic escalation (recurring):** `release.yml` finalize auto-closes the release-to-main PR on every release (v0.1.6, v0.1.10–v0.1.14 affected). Root cause: `RELEASE_BOT_TOKEN` not configured → merge step skipped → PR auto-closed and branch deleted. **Founder must fix before v0.2.0.**

---

## ✅ v0.1.15 — CONTENT DONE; CEREMONY BROKEN (superseded by v0.1.16)

**Content shipped to develop (PRs #374–#394):**
- [x] RFC-0100 governance guardrails + release secret config (#376, #377)
- [x] RFC-0100 redb Phase 1: `StorageBackend` trait + `InMemoryBackend` + `RedbBackend` foundation
- [x] RFC-0100 redb Phase 2: property equivalence, crash-safety, adjacency, file-scoped replacement, edge-count cache, RSS instrumentation, MCP watch persistence, SLA benchmarks (PRs #378-#394)
- [x] RFC-0101/0102/0103 RFC drafts (PRs #385, #386, #387)
- [x] MCP agent routing instructions strengthened (PR #384)

**v0.1.15 ceremony status — BROKEN ⚠️ (orphan tag; content absorbed into v0.1.16):**
- ❌ **Step 1**: `release/v0.1.15` → `main` — PR #361 CLOSED UNMERGED (release workflow created orphan tag)
- ❌ **Step 2**: Tag `v0.1.15` exists but points to orphan commit not on main or develop
- ❌ **Step 3**: GitHub Release published but at wrong commit
- ❌ **Step 4**: `release/v0.1.15` → `develop` — PR #362 CLOSED UNMERGED
- **Root cause**: `release.yml` used `cargo publish ... || true` masking CRATES_IO_TOKEN failures; tag created before main/develop merges. `crates.io` still at `0.1.10` for those crates at time of failure.
- **Resolution**: v0.1.15 content absorbed into v0.1.16 release (skipped version number in crates.io; orphan tag left as historical artifact).

---

## ✅ v0.1.16 — SHIPPED (ceremony 4/4 COMPLETE — 2026-06-02)

**What shipped:**
- [x] RFC-0100 Phase 1+2: redb `StorageBackend` trait + `InMemoryBackend` + `RedbBackend` (feature-flagged)
- [x] RFC-0101 draft, RFC-0102 draft, RFC-0103 draft (context tool, output budgets, cross-file refs)
- [x] MCP server routing instructions + primary tool-selection decision tree (Issue #366)
- [x] Incremental persistence journal (Issue #343, append-only JSONL, 5 tests)
- [x] Memory budget / bounded store (Issue #344, `memory-bound` feature flag)
- [x] Release ceremony script `scripts/release-ceremony.sh` (Issue #375)
- [x] Dep bumps: redb 2.6.3→4.1, logos 0.14→0.16, salsa 0.18→0.26
- [x] Platform-aware `measure_rss` test + crates.io publish idempotency fix in `release.yml`
- [x] mycelium_context (90th MCP tool) + OutputBudget + import-aware stub resolution (#395)

**v0.1.16 ceremony status — ALL FOUR STEPS COMPLETE ✅:**
- [x] **Step 1**: `release/v0.1.16` → `main` — commit `0d27c5a` 2026-06-02T01:27Z ✅
- [x] **Step 2**: Tag `v0.1.16` pushed ✅
- [x] **Step 3**: GitHub Release published 2026-06-02T01:27:33Z ✅
- [x] **Step 4**: Back-merge `release/v0.1.16` → `develop` — commit `cb31814` 2026-06-02T01:28Z ✅

**Note**: crates.io publish idempotency systemic fix landed in post-release hotfix commits (`4d2cf88`, `e0651e8`, `cd66278`) on main. `release.yml` now uses `curl` to crates.io REST API for idempotency check + tolerates "already exists" errors.

---

## 🚀 v0.1.17 — IN PROGRESS (10 commits on develop since v0.1.16, all CI green)

**Content already on develop (post-v0.1.16):**
- [x] RFC-0101 Phase 2: `mycelium context` CLI twin — Three-Surface Rule fully satisfied (PR #414, 2026-06-02T01:50Z)
- [x] RFC-0102 Implemented: OutputBudget moved to `mycelium-core`; `mycelium context` budgeted on MCP+CLI byte-identically (PR #438)
- [x] RFC-0100 Phase 3: **redb is now the default storage backend** — mmap ACID, bounded RAM, per-file incremental writes; legacy snapshot read retained for soft migration (PR #448)
- [x] RFC-0104: Charter §2 warm/cold SLA split — founder-approved 2026-06-02 (PR #444); cold numbers pending nightly benchmark before Charter table is amended
- [x] Issue #428 god-file-split slice 1: redb value codecs → `store::redb_codec` (PR #441, `redb_backend.rs` 1585→1440 lines)
- [x] Issue #428 god-file-split slice 2: `mod tests` → `src/tests.rs` (PR #442, `lib.rs` 12191→5627 lines, −54%)
- [x] 100k-node redb SLA gate + env-guarded nightly benchmark (PR #440)
- [x] Orphan `BoundedStore`/`MemoryBudget`/`FileAccessTracker` LRU removed (PR #440; RFC-0099 measurement tooling retained)
- [x] Repo hygiene: orphan `.claude/worktrees/` gitlinks removed + `.gitignore` updated (PR #449)
- [x] Vision scorecard updated to v0.1.16+ reality (PR #450)

**v0.1.17 ceremony status — NOT STARTED:**
- [ ] **Pre-release**: Security scan post-v0.1.16 (P1 gate)
- [ ] **Pre-release**: CHANGELOG Unreleased section verified
- [ ] **Step 1**: `release/v0.1.17` → `main` (founder authorization required)
- [ ] **Step 2**: Tag `v0.1.17` pushed
- [ ] **Step 3**: GitHub Release published
- [ ] **Step 4**: Back-merge `release/v0.1.17` → `develop`

---

## Live priorities (ordered)

**P0:** none.

**P1 (v0.1.17 release gates — SECURITY SCAN DONE ✅):**
1. ~~**Security scan post-v0.1.16**~~ — **COMPLETE 2026-06-02**: CLEAN (no secrets, no env files, 1 legitimate unsafe macOS RSS block). ✅
2. ~~**Cut `release/v0.1.17`**~~ — **COMPLETE 2026-06-02**: `release/v0.1.17` branch pushed; PR #452 (→ main) + PR #453 (→ develop) opened. ✅
3. **Founder authorization** — merge `release/v0.1.17` → `main` (Charter §5.12). CI must be green on PR #452. (`founder` action — NEXT STEP)

**P1 (post-v0.1.17):**
4. **RFC-0104 cold SLA numbers** — nightly `sla_ancestors_100k` benchmark results needed before Charter §2 table is amended from placeholder to measured values. (`bench` agent: set `MYCELIUM_BENCH_LARGE=1` on nightly runner)
5. **ADR-0008** for redb as default backend — architect decision record, required before v0.2.0. (`architect` task, now unblocked by Phase 3 flip)
6. **Dogfood re-run with redb-as-default** — validate 8/8 CLI commands green under new storage default. (`e2e-runner` task)

**P2 (v0.2.0 scope):**
7. Issue #428 god-file-split remaining slices — `redb_backend.rs` still ~1440 lines; `mycelium-cli/src/queries.rs` may need splitting.
8. Skill marketplace submission to Claude Code marketplace.
9. "First 5 minutes" walkthrough validation (README + docs site).
10. `release.yml` finalize merge step — systemic since v0.1.6; `RELEASE_BOT_TOKEN` audit needed for v0.2.0 automation vs. `scripts/release-ceremony.sh` workaround.

---

## Dispatch state (2026-06-02 v12 — security CLEAN; release/v0.1.17 CUT; PRs #452+#453 open)

| Agent | Status | Current item |
|---|---|---|
| founder | **action requested** | (1) Review PR #452 (`release/v0.1.17` → `main`) once CI green — authorize merge (Charter §5.12 Step 1). (2) Run `scripts/release-ceremony.sh` → pushes tag (Step 2) + publishes all 5 crates (Step 3) + GitHub Release. (3) **Only after Steps 1+2+3 complete**: admin-merge PR #453 (back-merge → develop, Step 4). ⚠️ Do NOT merge #453 before crates.io publish — would leave develop with sealed 0.1.17 unreachable by users (v0.1.15 drift pattern). (4) Systemic: `release.yml` finalize merge fix before v0.2.0. (5) Schedule `sla_ancestors_100k` nightly benchmark run for RFC-0104 cold numbers. |
| security-reviewer | **DONE ✅** | Post-v0.1.16 scan completed this run: CLEAN. |
| release | **DONE ✅** | `release/v0.1.17` branch cut; PR #452 (→ main) + PR #453 (→ develop) opened. Awaiting founder auth for Step 1. |
| bench | **P1** | Run `sla_ancestors_100k` nightly to produce RFC-0104 cold SLA numbers. |
| e2e-runner | **P1** | Dogfood re-run with redb-as-default (validate 8/8 CLI commands green under new storage). |
| rust-implementer | idle | Issue #428 remaining god-file-split slices: `redb_backend.rs` modularization. |
| architect | idle | ADR-0008: redb as default backend (required before v0.2.0). |
| tech-writer | idle | Skill marketplace submission prep (P2). |

---

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- Skill marketplace listing metadata sign-off.
- **RFC-0104 cold SLA measurement**: Charter §2 table amendment (warm/cold split) requires measured nightly data — currently uses placeholder numbers.
- **Systemic**: `release.yml` finalize merge step fails on every release (v0.1.6–v0.1.16 all affected). Founder must audit `RELEASE_BOT_TOKEN` or approve ceremony-script-only approach for v0.2.0.

---

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder if available): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/v0.1.x branch, publish.

---

## Archive

### 2026-06-02 PM dispatch v12 (this run — security scan CLEAN; release/v0.1.17 CUT; PRs #452+#453 opened)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl (stub — gitignored placeholder after git checkout), anti-patterns (no domain hits for release/security), PM state (v11 — on PR #451 branch), v0.2 PRD.

**Assessment:**
- develop HEAD: `6fdb6e97` (PR #451 merged this run — PM dispatch v11 chore). 0 open PRs after merge, 0 open issues.
- CI: last 5 runs all SUCCESS (Quality Gate, E2E, Triage).
- v0.1.17 sprint: 10 commits on develop, all CI green. Security scan was the only gate before release cut.
- PR #451 had 22/22 CI SUCCESS (pure docs/memory chore) — merged at session start.

**Actions taken:**
1. **Merged PR #451** (chore PM dispatch v11 — 22/22 CI SUCCESS, squash merge). ✅
2. **Security scan post-v0.1.16** (acting as security-reviewer): CLEAN — no hardcoded secrets, no committed .env/private-key files, 1 legitimate unsafe block (macOS RSS via `libc::task_info`, `MaybeUninit` pattern, platform-gated), GitHub Actions token refs all expected. ✅
3. **Verified CHANGELOG Unreleased section**: complete for all 10 v0.1.17 content commits (redb default, mycelium context RFC-0101/RFC-0102, Charter §2 SLA RFC-0104, god-file-split slices 1+2, 100k SLA gate, governance supersede enforcement, orphan LRU removal, vision scorecard). ✅
4. **Cut release/v0.1.17**: bumped workspace version 0.1.16→0.1.17 in `Cargo.toml`, CLI mcp-dep pin 0.1.14→0.1.17 in `crates/mycelium-cli/Cargo.toml`, sealed `CHANGELOG.md` `[Unreleased]`→`[0.1.17] - 2026-06-02`. Committed (DCO-signed) + pushed `release/v0.1.17`. ✅
5. **Opened PR #452** (`release/v0.1.17` → `main`, founder-gated per Charter §5.12). ✅
6. **Opened PR #453** (`release/v0.1.17` → `develop`, back-merge ceremony step 4 — admin-merge after Step 1). ✅
7. **Updated PM state v12** + appended decisions.jsonl.

**Sprint status:** v0.1.17 content complete; release branch cut; awaiting founder authorization for PR #452.

**Escalations:** Founder must (1) authorize PR #452 once CI green; (2) run ceremony script for tag+crates.io+GitHub Release; (3) admin-merge PR #453 after Step 1.

### 2026-06-02 PM dispatch v11 (this run — v0.1.16 SHIPPED confirmed; v0.1.17 sprint defined)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl (246 lines, last=2026-06-02T00:00Z), anti-patterns (no domain hits), PM state (stale — still showed PR #414 pending), v0.2 PRD.

**Assessment:**
- develop HEAD: `09290a5` (docs/vision-vs-reality refresh, 2026-06-02T15:16Z). 0 open PRs, 0 open issues. CI green on develop and main.
- v0.1.16 ceremony: ALL 4 STEPS COMPLETE ✅ — Step1: 0d27c5a→main; Step2: tag v0.1.16; Step3: GitHub Release 2026-06-02T01:27Z; Step4: back-merge cb31814→develop 2026-06-02T01:28Z.
- 10 commits on develop post-v0.1.16 (RFC-0101 CLI twin, OutputBudget-core, 100k SLA gate, redb codec extracted, lib.rs mod-split, RFC-0104 Charter §2, redb default flip, orphan LRU removal, repo hygiene, vision docs).
- PM state was stale (still said v0.1.16 in-progress, PR #414 pending — 3+ hours of post-release develop work not reflected).
- Key finding: decisions.jsonl PM v9/v10 commits claimed "decisions.jsonl appended" but file ends at 246 lines (2026-06-02T00:00Z) — those dispatches only updated the PM state file. Anti-pattern: commit message claims not matching actual file updates.

**Actions taken:**
1. **Updated PM state to v11**: v0.1.16 SHIPPED (4/4 ceremony), v0.1.17 sprint defined (10 content commits ready), priorities updated, dispatch table refreshed.
2. **Appended decisions.jsonl** with this run's summary (247th entry).
3. **Opened PR** `chore/pm-dispatch-2026-06-02-v11` → `develop` (this chore).

**No code tasks this run** — 0 open PRs/issues; all v0.1.17 content is already on develop and CI-green. PM state accuracy was the highest-value action.

**Escalations:**
- Founder: (1) Authorize v0.1.17 release after security scan. (2) RFC-0104 cold SLA nightly benchmark scheduling. (3) `release.yml` finalize merge systemic fix (v0.2.0 blocker).

### 2026-06-02 PM dispatch (RFC-0101 Phase 2 CLI twin; PR #414 opened)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns, PM state (stale — develop at bd25cc8 vs local at 59521bd), v0.2 PRD.

**Assessment:**
- 0 open PRs, 0 open issues. CI green (E2E SUCCESS on bd25cc8 redb bump).
- develop HEAD bd25cc8: dep bumps merged (salsa 0.26, logos 0.16, redb 4.1 — all green CI). R2 persistence (#407) + R3 memory-bound (#344) merged. Release ceremony script (PR #375) merged.
- PM state stale (last updated 2026-06-01). Cargo version: 0.1.14 (needs bump to 0.1.16 at release time).
- P0 (Issue #375 ceremony) resolved: 0 open issues confirms founder closed it.
- **Three-Surface violation**: `mycelium_context` MCP tool (90th tool) existed; CLI twin `mycelium context` was missing. INDEX.md had `⚠️ EXCEPTION: MCP-only`.

**Actions taken:**
1. RFC-0101 Phase 2: TDD (3 RED tests → compile fail) → implemented run_context(), extract_symbol_candidates(), build_context_payload(), context_json(), context_path_leaf() in queries.rs; added Context variant + dispatch in main.rs. 3/3 GREEN tests. fmt ✅ clippy ✅ cargo test --all ✅.
2. Updated skills/architecture-context/SKILL.md: CLI tool added to allowed-tools + CLI reference section.
3. Updated skills/INDEX.md: context row ⚠️ → ✅ Three-Surface v0.1.16.
4. Updated rfcs/0101-mycelium-context-tool.md: Status → Implemented; acceptance criteria ticked.
5. Updated CHANGELOG.md Unreleased.
6. Opened PR #414 (feature/rfc-0101-phase2-cli-twin → develop).
7. Updated PM state + decisions.jsonl.

**Sprint status:** v0.1.16 content complete pending PR #414 merge + CI green.

**Escalations:** Founder review needed to authorize v0.1.16 ceremony after PR #414 lands.

### 2026-06-01 PM dispatch v3 (this run — PRs #395+#405 merged; v0.1.16 sprint defined)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20 (latest entry: `2026-06-01T21:30:00Z` second PM dispatch merging CI bumps #397-#401, fixing PR #395 test), anti-patterns (no relevant hits), PM state, v0.2 PRD.

**Assessment:**
- develop HEAD: `5f81d9f` (CI bump #401 merged). 5 open PRs: #395 (feature, 22/22 CI green after test fix), #402-#404 (dep bumps, red CI), #405 (PM chore, green CI). 3 open issues: #375 (P0 founder gate), #343 (R2), #344 (R3).
- PR #395: all 22 CI checks SUCCESS. Ready.
- PR #405: green CI. PM state chore from prior run. Mergeable.
- Issue #375: prerequisites done (PRs #374/#376 merged, credentials configured). Blocked on founder GPG decision.
- Dep bumps #402-#404: still deferred — all red CI (breaking API changes).

**Actions taken:**
1. **Merged PR #395** (feat: mycelium_context + OutputBudget + import-aware stubs, 22/22 CI green, squash). 90th MCP tool. Three-Surface Rule: I1/I2 pass (architecture-context Skill covers it). CLI twin deferred per RFC-0101 Phase 2.
2. **Merged PR #405** (chore: PM dispatch state — v2, squash). PM memory updated.
3. **Updated PM state v3**: sprint renamed v0.1.16, priorities and dispatch table updated.
4. **Appended decisions.jsonl** for this run.

**Escalations:** Issue #375 P0 — founder GPG-signed decision required (repair v0.1.15 or cut v0.1.16). All technical prerequisites are complete.

### 2026-06-01 PM dispatch (this run — PR #395 test fixed; PRs #397-#401 merged; dep bumps triaged)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns, PM state (from PR #396 — v0.1.15 ceremony broken), v0.2 PRD.

**Assessment:**
- develop HEAD: `dc3e521` (PR #396 — PM dispatch chore from previous session today).
- 9 open PRs: #395 (feature, CI failing), #397-#404 (7 dependabot bumps). 3 open issues: #375 (P0), #343/#344 (P1).
- CI action bumps #397-#401: all Quality Gate SUCCESS — safe to merge.
- Dep bumps #402-#404 (redb/logos/salsa): red CI — API breaking changes, need RFC.
- PR #395: 1 failing test `server_info_tests::get_info_includes_primary_tool_selection_rules`.

**Actions taken:**
1. **Fixed PR #395 test failure**: changed `MCP_INSTRUCTIONS_BASE` item-1 label + header 89→90. RED→GREEN confirmed locally. Pushed to `feature/mycelium-context-tool`.
2. **Merged PRs #397/#398/#399/#400/#401** (5 CI action bumps, all Quality Gate green).
3. **Triaged deferred dep bumps** (#402 redb 4.x, #403 logos 0.16, #404 salsa 0.26): all red CI. Deferred.
4. Updated PM state + appended decisions.jsonl.

**Escalations:** Issue #375 remains P0 (founder: decide repair v0.1.15 or cut v0.1.16). Salsa/redb/logos bumps need RFC/analysis before adoption.

### 2026-06-01 PM dispatch (previous run — PR #395 rebased + architecture-context Skill; Issue #375 P0 escalated)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns, PM state (stale — last updated 2026-05-31), v0.2 PRD.

**Actions taken:**
1. Rebased PR #395 onto develop HEAD (d3cfaca). Resolved two conflicts.
2. Removed .claude/worktrees/ artifacts from feature branch.
3. Added `skills/architecture-context/SKILL.md` covering `mcp__mycelium__context`. I1 parity: 90/90 PASS.
4. Updated `skills/INDEX.md` with `context` row.
5. Updated CHANGELOG Unreleased.
6. Force-pushed to `feature/mycelium-context-tool` — PR #395 now clean.
7. Updated PM state.

**Escalations:** Issue #375 (P0): founder must decide repair v0.1.15 vs cut v0.1.16.

### 2026-05-31 PM dispatch (this run — Issue #366 CLOSED; PR #365 CI re-triggered; PR #369 replaced)

- Closed Issue #366 — resolved by PR #368.
- Re-triggered CI on PR #365 — pushed empty DCO-signed commit.
- Replaced PR #369 (dirty, 20-commit stale branch) with fresh chore branch.
- Updated PM state.

### 2026-05-31 PM dispatch (PRs #335+#337 merged; PR #336 closed; PR #338 rebased)

- Merged PR #335 (ci/skill-parity-quality-gate) — closes Skills INDEX.md CI gate criterion.
- Merged PR #337 (docs/v0.1.14-dogfood-report) — closes Dogfood 8/8 criterion.
- Closed PR #336 (conflicted) — superseded by PR #338.
- Rebased `docs/vision-model-tiering-clean` onto develop.
- Sprint status: 5/6 v0.1.14 exit criteria done.

### 2026-05-31 PM dispatch (previous — Skills INDEX.md CI gate promoted to required; PR #334 merged)

- Merged PR #334. Promoted skill-parity to required CI. Updated INDEX.md + CHANGELOG.

### 2026-05-31 PM dispatch (v0.1.13 SHIPPED; RFC-0096 Phase 2 TS; PRD corrections; security scan)

- v0.1.13 ceremony: ALL 4 STEPS COMPLETE. Security scan: CLEAN. PRD v0.2 corrections.

### 2026-05-31 PM dispatch (v0.1.13 cut; PR #328/#329; v0.1.14 kickoff)

- v0.1.13 sprint DECLARED COMPLETE. Cut release/v0.1.13. PRs #328/#329 opened.

### 2026-05-30 PM dispatch (PR #323 merged; RFC-0093 Phase 2; security CLEAN)

- Merged PR #323 (back-merge v0.1.12). Ceremony step 4/4 ✅. Security scan: CLEAN.

### 2026-05-30 PM dispatch (v0.1.12 ceremony + v0.1.13 kickoff)

- PRs #323 (back-merge → develop) + #324 (→ main, founder-gated) created.

### 2026-05-30 PM dispatch (PRs #317/#318/#319 merged; v0.1.12 cut)

- PRs #317/#318/#319 merged. release/v0.1.12 branch cut. PR #321 opened.

### 2026-05-30 PM dispatch (v0.1.11 ceremony + v0.1.12 kickoff — PRs #266 + #270)

- PR #266 merged. v0.1.11 ceremony complete (tag, crates.io, back-merge PR #315).

### 2026-05-30 PM dispatch (v0.1.11 sprint complete — 9/9 exit criteria)

- 9/9 v0.1.11 criteria met.

### 2026-05-30 PM run (post-v0.1.10 — RFC-0094 Phase 1 + back-merge)

- PM state fast-forwarded v0.1.6 → v0.1.10. PRs #240 + #241 merged.

### 2026-05-29 PM run (v0.1.4 close)

v0.1.4 sprint declared complete. All 7 exit criteria met.

### 2026-05-30 PM call (v0.1.2 era — superseded)

1. v0.1.0 + v0.1.1 shipped. Three-Surface Rule is law.
2. PRD for v0.2 at [`docs/prd/v0.2-the-three-surface-release.md`](../prd/v0.2-the-three-surface-release.md).
