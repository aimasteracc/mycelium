# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-06-02 (PM dispatch v11 — PR #445 merged; Issues #426+#428 AC-state verified; Issue #426 AC#1+AC#3 confirmed done; Issue #428 AC#1+AC#3 confirmed done; comments posted on both issues) |
| Current sprint | **v0.1.17 — IN PROGRESS (Issue #426 AC#4 gated on founder RFC-0104 review; Issue #428 AC#2 tool-impl split remaining P2)** |
| Active release branch | none (cut release/v0.1.17 after security scan + founder sign-off on Issue #426) |
| Next release target | **v0.1.17** — RFC-0101/0102 contract + RFC-0100 Phase 3 redb readiness + tech debt god-file split (#428) |
| Final release target | v0.2.0, ETA 2026-07-15 |
| Last ceremony-complete release | **v0.1.14** (all 4 Charter §5.12 steps ✅). v0.1.16 is on main+tag but crates.io PENDING (step 3 incomplete — not considered "shipped" until crates.io confirmed). |

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

## 🚀 v0.1.15 — CONTENT DONE; CEREMONY BROKEN (Issue #375)

**Content shipped to develop (PRs #374–#394):**
- [x] RFC-0100 governance guardrails + release secret config (#376, #377)
- [x] RFC-0100 redb Phase 1: `StorageBackend` trait + `InMemoryBackend` + `RedbBackend` foundation
- [x] RFC-0100 redb Phase 2: property equivalence, crash-safety, adjacency, file-scoped replacement, edge-count cache, RSS instrumentation, MCP watch persistence, SLA benchmarks (PRs #378-#394)
- [x] RFC-0101/0102/0103 RFC drafts (PRs #385, #386, #387)
- [x] MCP agent routing instructions strengthened (PR #384)

**v0.1.15 ceremony status — BROKEN ⚠️:**
- ❌ **Step 1**: `release/v0.1.15` → `main` — PR #361 CLOSED UNMERGED (release workflow created orphan tag)
- ❌ **Step 2**: Tag `v0.1.15` exists but points to orphan commit not on main or develop
- ❌ **Step 3**: GitHub Release published but at wrong commit
- ❌ **Step 4**: `release/v0.1.15` → `develop` — PR #362 CLOSED UNMERGED
- **Root cause**: `release.yml` used `cargo publish ... || true` masking CRATES_IO_TOKEN failures; tag created before main/develop merges. `crates.io` still at `0.1.10`.
- **Decision gate (founder required)**: Repair v0.1.15 OR cut fresh v0.1.16. See Issue #375.

---

## ⚠️ v0.1.16 — SHIPPED (ceremony INCOMPLETE — crates.io PENDING)

**What shipped:**
- [x] RFC-0100 Phase 1+2: `StorageBackend` + redb backend, crash-safety, property equivalence
- [x] Incremental persistence journal (Issue #343)
- [x] Memory budget / bounded store with LRU eviction (Issue #344)
- [x] MCP server routing instructions (Issue #366)
- [x] Release ceremony script (Issue #375)
- [x] Dep bumps: salsa 0.18→0.26, logos 0.14→0.16, redb 2.6→4.1
- [x] fix: measure_rss test platform-aware (Windows CI unblocked — commit b52b8fb)

**v0.1.16 ceremony status — INCOMPLETE ⚠️ (Charter §5.12 step 3: crates.io PENDING):**
- [x] **Step 1**: `release/v0.1.16` → `main` — PR #416 MERGED ✅ (founder authorized)
- [x] **Step 2**: Tag `v0.1.16` pushed ✅
- [ ] **Step 3**: All five crates published to crates.io — **PENDING** ⚠️ (release.yml run 26793340808 failed; hotfix PR #419 now on `main` + `develop` with fixed publish logic — **founder re-trigger required**)
- [x] **Step 4**: Back-merge `release/v0.1.16` → `develop` — MERGED ✅ (PR #423, squash)
- **GitHub Release**: Published ✅ (bonus step, not one of Charter §5.12's four required steps)

**Hotfix (PR #419 → PR #423):** Fixed `release.yml`: replaced `cargo search` with `curl` to crates.io API; `max_version` → version-list membership check (`$VERSION` ∈ `versions[]`); `|| {}` idempotency pattern (no pipefail); macOS SLA 30ms headroom. Both `main` and `develop` have the fix — re-triggering the release workflow against the `v0.1.16` tag will use the corrected logic.

---

## 🚀 v0.1.17 — CONTENT COMPLETE; ceremony pending

**What will ship (all merged to develop, HEAD `ec3266d`):**
- [x] RFC-0101 Phase 2: `mycelium context` CLI twin (PR #414) — Three-Surface compliant ✅
- [x] Governance guardrails: `check_supersede_discipline.sh` CI gate (PR #424)
- [x] fix(storage): `node_kind_tag` panic on unmapped variant + roundtrip test (PR #425)
- [x] docs(adr): renumber ADR-0007 redb → ADR-0008 (PR #429 — Issue #428 AC#1) ✅
- [x] feat(mcp): RFC-0101 `related_files` + `apply_budget` + `edge_kinds` (PR #431)
- [x] feat(context): shared `mycelium_core::context` builder, byte-identical CLI↔MCP (PR #436)
- [x] feat(budget): `OutputBudget` to core, context budgeted on both surfaces (PR #438 — Issue #427 CLOSED) ✅
- [x] feat(core): 100k-node redb SLA gate; remove orphan LRU (PR #440 — Issue #426 AC#1+3) ✅
- [x] refactor(core): redb codec → `redb_codec.rs` (PR #441 — Issue #428 AC#3 slice) ✅
- [x] refactor(mcp): `mod tests` → `src/tests.rs` lib.rs 12191→5627 (PR #442 MERGED — Issue #428 AC#2 slice 1) ✅

**v0.1.17 ceremony status — NOT STARTED ⚠️ (gated on v0.1.16 crates.io step 3 first):**
- [ ] **Step 1**: `release/v0.1.17` → `main` — branch not yet cut
- [ ] **Step 2**: Tag `v0.1.17`
- [ ] **Step 3**: All five crates published to crates.io
- [ ] **Step 4**: Back-merge `release/v0.1.17` → `develop`

---

## Live priorities (ordered)

**P0 — Founder action required: re-trigger v0.1.16 crates.io publish**

Charter §5.12 step 3 is still open. The hotfix is on both `main` and `develop`. Options:
1. Re-push `release/v0.1.16` branch → `release.yml` re-runs automatically, OR
2. Run the ceremony script from PR #375 against the `v0.1.16` tag manually.

**Recent events (2026-06-02 v11 — absorbed v10):**
- PR #445 (PM chore v10) MERGED ✅
- Issue #426: Comments posted confirming AC#1 ✅ (100k nightly gate) + AC#3 ✅ (BoundedStore removed). AC#2 (RSS-cap gate) confirmed blocked on RFC-0104 mechanism decision. AC#5 blocked on AC#2+AC#4.
- Issue #428: Comments posted confirming AC#1 ✅ (ADR-0008 rename) + AC#3 ✅ (codec split PR #441). AC#2 slice 1 ✅ (PR #442); slice 2 (tool-impl split, lib.rs at 5,626 lines) still open.

**v11 absorbed v10 events:**
- PR #416 (v0.1.16 release → main) MERGED ✅ (founder authorized)
- PR #419 (hotfix: release.yml publish fixes) MERGED to main ✅
- PR #423 (back-merge hotfix → develop) MERGED ✅
- PR #424 (governance: supersede-discipline CI gate) MERGED ✅
- PR #425 (fix: NodeKind tag 255 silent corruption) MERGED ✅
- PR #429 (ADR-0008 renumber — Issue #428 AC#1) MERGED ✅
- PR #431 (RFC-0101 contract: `related_files` + `apply_budget` + `edge_kinds`) MERGED ✅
- PR #436 (RFC-0101 shared core context builder) MERGED ✅
- PR #438 (RFC-0102: OutputBudget to core, budget context on both surfaces) MERGED ✅ — Issue #427 CLOSED ✅
- PR #440 (100k-node redb SLA gate + remove orphan LRU — Issue #426 AC#1+3) MERGED ✅
- PR #441 (redb codec extraction to redb_codec.rs — Issue #428 AC#3 slice) MERGED ✅
- PR #442 (Issue #428 AC#2 slice 1: `mod tests` 12191→5627 lines) MERGED ✅
- PR #443 (PM chore v9) MERGED ✅
- PR #444 (RFC-0104 draft: Charter §2 warm/cold SLA split — Issue #426 AC#4) OPENED as draft, **BDFL decision required**

**P0 — Founder action required (two items):**
1. **Re-trigger v0.1.16 crates.io publish** (Charter §5.12 step 3 still PENDING). Hotfix is on main + develop. Re-push `release/v0.1.16` or run ceremony script manually.
2. **Review RFC-0104 draft (PR #444)** — Charter §2 warm/cold SLA split. Three open questions require founder input (cold target values, measurement protocol, warm-up scan scope). This unblocks Issue #426 AC#2 + AC#5 (redb default flip).

**P1 (after P0 resolved):**
3. **Security scan post-v0.1.16** — no scan since v0.1.14 (two releases behind).
4. **Cut release/v0.1.17** — gated on item 3 + founder sign-off on Issue #426.

**P2 (v0.2.0 scope):**
5. Issue #428 AC#2 remaining: split tool implementations into `tools/context.rs` + `tools/graph.rs` (lib.rs at 5,627 lines, above 800-line cap).
6. `release.yml` finalize merge step — `RELEASE_BOT_TOKEN` systemic fix (blocking since v0.1.6).
7. Skill marketplace submission.

---

## Dispatch state (2026-06-02 v11 — Issues #426+#428 AC-state verified; PR #445 merged)

| Agent | Status | Current item |
|---|---|---|
| founder | **action required** | (1) Re-trigger v0.1.16 crates.io publish (hotfix on `main`+`develop`). (2) Review RFC-0104 draft (PR #444) — Charter §2 warm/cold SLA split (unblocks Issue #426 AC#2+AC#5). (3) Authorize v0.1.17 ceremony after crates.io + security scan. (4) Systemic: `RELEASE_BOT_TOKEN` fix. |
| orchestrator/pm | **done** | Dispatch v11: PR #445 merged; AC-state audited on Issues #426+#428; comments posted; PM state updated. |
| security-reviewer | **NEXT** | Post-v0.1.16 scan (no scan since v0.1.14). |
| rust-implementer | **QUEUED** | Issue #428 AC#2 slice 2: split tool impls into `tools/context.rs` + `tools/graph.rs` (lib.rs 5,626 lines → target < 1,000). After founder sign-off on RFC-0104: Issue #426 AC#2 RSS-cap CI gate. |
| architect | idle | Prepare meta-RFC for Issue #426 AC#4 Charter §2 warm/cold split (for founder review). |
| tech-writer | idle | Skill marketplace submission prep (P2). |
| e2e-runner | idle | RSS-curve measurement — benchmark harness exists in `redb_sla.rs`. |


---

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- Skill marketplace listing metadata sign-off.
- **⚠️ R2 / RFC-0098**: incremental persistence implementation touches storage format (Charter §3 deviation from WAL/HAMT/time-travel row). Founder must sign off on ADR + approach before any implementation PR.
- **⚠️ Systemic**: `release.yml` finalize merge step fails on every release (v0.1.6–v0.1.14 all affected). Founder must audit `RELEASE_BOT_TOKEN` or merge logic before v0.2.0.

---

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder if available): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/v0.1.x branch, publish.

---

## Archive

### 2026-06-02 PM dispatch v9 (this run — PR #442 opened: Issue #428 AC#2 slice 1; PRs #436/#438/#440/#441 absorbed; Issue #426 founder-escalated)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns (no domain hits), PM state (v8 from conflicted PR #439), v0.2 PRD.

**Assessment:**
- develop HEAD: `ec3266d` (PR #441 — redb codec extraction). 1 open PR (#439 chore v8, 22/22 CI green but merge conflict with develop). 2 open issues: #428 P2 (god-file split), #426 P1 (RFC-0100 Phase 3 remaining). CI in_progress on develop from #441 merge.
- Issue #426 remaining criteria: AC#2 (RSS-cap CI gate) requires AC#4 (Charter §2 warm/cold split) first — that is a meta-RFC / Charter amendment per Charter §9, requiring BDFL approval. AC#5 (flip default) gated on AC#2+AC#4. All three are founder-gated. ESCALATED.
- Issue #428: AC#1 (ADR renumber, PR #429) ✅; AC#3 slice (redb codec, PR #441) ✅; AC#2 (split lib.rs, 12,191 lines) — main remaining item.
- PR #439 (PM v8 chore): superseded by this v9 run due to merge conflict.

**Actions taken:**
1. PR #439 superseded — not merged (conflict). This v9 chore supersedes it.
2. **Issue #428 AC#2 slice 1 (TDD)**:
   - RED: `mod tests` block (6,564 lines) was inline in lib.rs, violating Charter 800-line cap.
   - GREEN: Extracted to `src/tests.rs`; lib.rs redirects with `#[cfg(test)] mod tests;`. lib.rs: 12,191 → 5,627 lines (−54%).
   - `cargo test --all`: **584 passed, 0 failed**. `fmt --check` clean. `clippy -D warnings` clean.
   - PR #442 opened (refactor/mcp-lib-split-tests → develop).
3. Escalated Issue #426 to founder (AC#4 Charter §2 amendment required).
4. Updated PM state v9 + appended decisions.jsonl.

**Escalations:**
- **Issue #426 AC#4 (founder, meta-RFC)**: Charter §2 warm/cold latency split — must be a `meta` RFC with BDFL approval before AC#2 (RSS-cap CI gate) and AC#5 (default flip) can proceed.
- **v0.1.16 crates.io (founder)**: re-trigger publish with fixed release.yml (`curl` idempotency fix on main+develop).

### 2026-06-02 PM dispatch v6 (this run — PRs #431+#432 merged; PR #433 opened: BoundedStore removal)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20 (latest: v5 dispatch PR #431 opened), anti-patterns (no domain hits), PM state v5, v0.2 PRD.

**Assessment:**
- develop HEAD: `e49d0b7` (v5 PM dispatch). 2 open PRs: #431 (feature, 24/24 CI green), #432 (chore, 24/24 CI green). 3 open issues: #426 (P1 redb Phase 3), #427 (P1 contract), #428 (P2 tech-debt).
- ADR renumber (Issue #428 AC#1): already done via PR #429 in develop. ✅
- PR #431: all 24 CI checks SUCCESS. Merged immediately.
- PR #432: all 24 CI checks SUCCESS. Merged immediately.
- BoundedStore (`memory_budget.rs`): zero production callers confirmed by grep. `MemoryBudget`, `FileAccessTracker`, `GLOBAL_ACCESS_CLOCK`, `tick()` also self-contained. `estimate_store_bytes` and `measure_rss` retained per Issue #426.

**Actions taken:**
1. **Merged PR #431** (feat: RFC-0101 response contract — related_files, edge_kinds, apply_budget, 24/24 CI SUCCESS, squash). Addresses Issue #427 partial (4 of 7 acceptance criteria).
2. **Merged PR #432** (chore: PM dispatch v5, 24/24 CI SUCCESS, squash).
3. **Removed BoundedStore** (Issue #426 criterion 3): deleted `BoundedStore`, `MemoryBudget`, `FileAccessTracker`, `GLOBAL_ACCESS_CLOCK`, `tick()` from `memory_budget.rs` + their 3 unit tests. Kept `estimate_store_bytes` + `measure_rss` + their 2 tests. 572 tests GREEN. fmt ✅ clippy ✅. PR #433 opened.
4. **Updated CHANGELOG Unreleased** with BoundedStore removal entry.
5. **Updated PM state v6** + appended decisions.jsonl.

**Sprint status:** v0.1.17 content in progress. Issue #427: 4/7 AC done (PR #431). Issue #426: 1/5 criteria done (PR #433 pending CI). Security scan + ceremony still pending.

**Escalations:** (1) v0.1.16 crates.io step 3 — founder re-trigger required. (2) Issue #426 remaining (100k SLA gate, RSS-cap CI, Charter §2 split) — founder ADR sign-off required before proceeding.

### 2026-06-01 PM dispatch v3 (this run — PRs #395+#405 merged; v0.1.16 sprint defined)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20 (latest entry: `2026-06-01T21:30:00Z` second PM dispatch merging CI bumps #397-#401, fixing PR #395 test), anti-patterns (no relevant hits), PM state, v0.2 PRD.

**Assessment:**
- develop HEAD: `5f81d9f` (CI bump #401 merged). 5 open PRs: #395 (feature, 22/22 CI green after test fix), #402-#404 (dep bumps, red CI), #405 (PM chore, green CI). 3 open issues: #375 (P0 founder gate), #343 (R2), #344 (R3).
- PR #395: all 22 CI checks SUCCESS (Quality Gate, rustfmt, clippy, DCO, skill coverage I1+I2, unit/integration tests linux/mac/windows/nightly, coverage, security, build, e2e dogfood + real projects). Ready.
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
- PR #395: 1 failing test `server_info_tests::get_info_includes_primary_tool_selection_rules`. Root cause: item-1 label in `MCP_INSTRUCTIONS_BASE` used unquoted `How does X work /` but test asserted `"\"How does X work?\""` (literal quotes + `?`). Also header said "89 tools" vs actual 90.

**Actions taken:**
1. **Fixed PR #395 test failure**: changed `MCP_INSTRUCTIONS_BASE` item-1 label to `\"How does X work?\"` + header 89→90. Confirmed RED→GREEN locally (all 566 tests pass, fmt+clippy clean). Pushed to `feature/mycelium-context-tool`.
2. **Merged PRs #397/#398/#399/#400/#401** (5 CI action bumps, all Quality Gate green): checkout 4→6, upload-artifact 4→7, setup-python 5→6, stale 9→10, action-gh-release 2→3.
3. **Triaged deferred dep bumps** (#402 redb 4.x, #403 logos 0.16, #404 salsa 0.26): all red CI. Salsa 0.26 is the riskiest — 8 major versions ahead, core reactivity layer; needs dedicated RFC before adopting.
4. Updated PM state + appended decisions.jsonl.

**Escalations:** Issue #375 remains P0 (founder: decide repair v0.1.15 or cut v0.1.16). Salsa/redb/logos bumps need RFC/analysis before adoption.

### 2026-06-01 PM dispatch (previous run — PR #395 rebased + architecture-context Skill; Issue #375 P0 escalated)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns, PM state (stale — last updated 2026-05-31), v0.2 PRD.

**Assessment:**
- develop HEAD: `d3cfaca` (PR #394 merged — RFC-0100 redb Phase 2 T06a SLA benchmarks). 20+ PRs merged since last PM state.
- 1 open PR: #395 (`feature/mycelium-context-tool`, `mergeable_state: dirty`) — 3 new features: mycelium_context (90th MCP tool), OutputBudget, import-aware stub resolution. Three-Surface violation: MCP-only, no CLI twin, no Skill. Opened today 2026-06-01.
- 3 open issues: #375 (v0.1.15 ceremony BROKEN — P0 founder gate), #343 (R2 persistence, founder gate), #344 (R3 memory, measurement first).
- v0.1.15 ceremony status: BROKEN — `release.yml` used `|| true` on publish, CRATES_IO_TOKEN failures masked, orphan tag created before main/develop merges. PRs #361/#362 closed unmerged.

**Actions taken:**
1. **Rebased PR #395** onto develop HEAD (d3cfaca). Resolved two conflicts in `crates/mycelium-mcp/src/lib.rs`: (a) MCP_INSTRUCTIONS text (took feature branch's definitive version), (b) watch-loop structure (took develop's refactored version with extension filter via `source_extension()`).
2. **Removed .claude/worktrees/ artifacts** from feature branch (two gitmodule subproject entries accidentally committed by Claude Code worktree feature).
3. **Added `skills/architecture-context/SKILL.md`** covering `mcp__mycelium__context`. Satisfies RFC-0090 I1: parity check now 90/90 PASS (--strict).
4. **Updated `skills/INDEX.md`** with `context` row (EXCEPTION: MCP-only pending BDFL sign-off, CLI tracked as RFC-0101 Phase 2).
5. **Updated CHANGELOG Unreleased** with mycelium_context, OutputBudget, import-aware stubs, architecture-context Skill entries.
6. **Force-pushed** to `feature/mycelium-context-tool` — PR #395 now clean, CI triggered.
7. **Updated PM state** to reflect v0.1.15 content complete but ceremony broken, v0.1.16 scope.

**Escalations:**
- Issue #375 (P0): founder must decide repair v0.1.15 vs cut v0.1.16; audit CRATES_IO_TOKEN + RELEASE_BOT_TOKEN.
- RFC-0101 Phase 2 (Three-Surface): `mycelium context` CLI twin needed; currently EXCEPTION: MCP-only pending BDFL sign-off.

### 2026-05-31 PM dispatch (this run — Issue #366 CLOSED; PR #365 CI re-triggered; PR #369 replaced)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns, PM state (via git show origin/develop), v0.2 PRD.

**Assessment:**
- develop HEAD: `f003f65` (MCP server instructions PR #368 already MERGED). v0.1.14 ceremony: ALL 4 STEPS COMPLETE ✅. v0.1.15 IN PROGRESS.
- 3 open PRs: #365 (RFC-0100 Phase 1, Quality Gate FAILURE — Windows runner CANCELLED, transient), #367 (Phase 2 T01, Quality Gate SUCCESS ✅, stacked on #365), #369 (PM dispatch, dirty/mergeable:false — 20-commit stale branch).
- 3 open issues: #366 (OPEN despite PR #368 MERGED — auto-close failed), #343 (R2 decision-gate), #344 (R3 Phase 0 done).

**Actions taken:**
1. **Closed Issue #366** — resolved by PR #368 on develop HEAD.
2. **Re-triggered CI on PR #365** — pushed empty DCO-signed commit (`6212c65`) to `feature/rfc0100-storage-trait-and-inmemory`. The prior "CI re-triggered" claim in PR #369 didn't materialize a new run (latest still 17:00 UTC with transient Windows cancel).
3. **Replaced PR #369** (dirty, 20-commit stale branch) with this fresh chore branch from develop HEAD `f003f65`.
4. **Updated PM state**: priorities corrected (PR #368 done, Issue #366 closed, PR #365 CI re-triggered, PR #367 awaiting #365).

**Escalations:**
- Founder: (a) `release.yml` RELEASE_BOT_TOKEN systemic fix; (b) Review + merge PR #365 once CI green (RFC-0100 Phase 1 — feature-flagged OFF, zero behavior change).

### 2026-05-31 PM dispatch (this run — PR #356 MERGED; PR #357 PM-chore rebased+merged; RFC-0099 PR #358 escalated)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns (scale/memory domain — no hits), PM state, v0.2 PRD.

**Assessment:**
- develop HEAD: `5d5e43a` (PR #356 merged this run). Prior stale PM state (header said "KICKOFF", sprint state stale).
- 3 open PRs: #356 (R3 measurement, `dirty` — conflict with RFC-0098 commit), #357 (PM dispatch chore, blocked), #358 (RFC-0099 draft, explicitly do-not-auto-merge).
- 2 open issues: #343 (R2), #344 (R3).
- v0.1.14 ceremony: COMPLETE (all 4 steps). v0.1.15 sprint: R1 DONE, R3 Phase 0 in PR #356, R2+R3 design gated.

**Actions taken:**
1. **Rebased PR #356** onto develop (conflict: decisions.jsonl + pm-state.md header — append-only resolution). Tests green (0 FAILED). Force-pushed `feature/r3-memory-curve`. **Merged PR #356** ✅.
2. **Rebased PR #357** onto post-#356 develop (conflict: decisions.jsonl + pm-state.md — append-only + --theirs strategy). **Updated PM state** for this run. Pushing as amended PR #357.
3. **Escalated PR #358** (RFC-0099 do-not-auto-merge) to founder — Phase 1 and Phase 2 implementation blocked on founder sign-off.

**Escalations:**
- Founder: (a) `release.yml` RELEASE_BOT_TOKEN systemic fix before v0.2.0; (b) RFC-0098 R2 decision gate; (c) RFC-0099 PR #358 sign-off (Phase 1 streaming index + Phase 2 LRU approach).

### 2026-05-31 PM dispatch (this run — PR #353 merged; PM state corrected; PR #356 CI pending)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns (scale/parallel/memory domain), PM state, v0.2 PRD.

**Assessment:**
- develop HEAD: `dd266ce` (RFC-0098 draft merged in this run). Prior HEAD: `c052e4a` (v0.1.14 ceremony commit).
- 2 open PRs: #353 (RFC-0098 Draft, Quality Gate 21/21 SUCCESS — pure docs), #356 (R3 measurement, CI pending — state=pending, 0 commit statuses yet).
- 2 open issues: #343 (R2 persistence, decision-gate), #344 (R3 memory, measurement-first).
- v0.1.14 ceremony: COMPLETE (PR #352 merged to main by founder; commit 59521bd; all 4 steps done).
- R1 parallel indexing: DONE — PR #351 merged, Issue #342 closed (`index_path_parallel` via `thread::scope` + `Store::merge`).
- PM state body was stale (header updated by c052e4a but ceremony body still showed PR #352 open; priorities still showed P0 as ceremony, R1 as NEXT).

**Actions taken:**
1. **Merged PR #353** (docs/rfc-0098-incremental-persistence — 1 file, 430 lines RFC-0098 Draft, Quality Gate 21/21 SUCCESS). Marks R2 design process advancing; implementation gated on founder sign-off + ADR.
2. **Corrected PM state**: v0.1.14 ceremony body → ALL FOUR STEPS COMPLETE; R1 DONE; Live priorities updated; dispatch table updated.
3. **Appended decisions.jsonl** (this run's summary).
4. **Chore PR opened** targeting develop.

**Escalations:**
- Founder must (a) audit `release.yml` finalize merge step (systemic, v0.1.6–v0.1.14); (b) sign off on RFC-0098 + ADR before R2 implementation begins.

**Note on PR #356:** CI was in `pending` state (0 commit statuses) at assessment time — neither failed nor queued. Will merge when green. Code is TDD-complete (Store::heap_size_estimate() + 3 CI tests + 3 #[ignore] RSS-curve tests).

### 2026-05-31 PM dispatch (this run — PR #350 merged; release/v0.1.14 conflicts resolved; PR #352 opened; security CLEAN)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns, PM state, v0.2 PRD.

**Assessment:**
- 1 open PR: #350 (chore/pm-dispatch prior session, CI 21/21 green). 2 open issues: #343 (R2 persistence), #344 (R3 memory).
- v0.1.14 shipped (tag + GitHub Release ✅; develop has v0.1.14 via PR #349 back-merge ✅).
- Ceremony step 1 (→ main) blocked: PR #348 was auto-closed by release.yml without merge; branch deleted; PR marked `dirty` due to CHANGELOG + Cargo.toml version conflicts.
- main is at v0.1.13; develop at v0.1.14.

**Actions taken:**
1. **Merged PR #350** (chore pm-dispatch prior session, squash, green CI) ✅
2. **Recreated release/v0.1.14** from tag `v0.1.14` (9690fc60); resolved 3 mechanical conflicts:
   - `CHANGELOG.md`: kept v0.1.14 section (origin/main had empty Unreleased)
   - `Cargo.toml`: kept `version = "0.1.14"` (origin/main had `0.1.13`)
   - `crates/mycelium-cli/Cargo.toml`: kept `mycelium-mcp = "0.1.14"` pin (origin/main had `0.1.13`)
3. **Pushed release/v0.1.14** to origin. **Created PR #352** (`release/v0.1.14` → `main`). FOUNDER AUTHORIZATION REQUIRED.
4. **Security scan post-v0.1.14**: CLEAN — no hardcoded secrets, zero unsafe blocks (compiler-enforced deny), GitHub Actions token refs correct.
5. **Triaged issues #343/#344**: #343 = P1 R2 (medium risk, storage format change, needs ADR + spike); #344 = P1/P2 R3 (medium-high risk, measurement spike first). Both confirmed as v0.1.15 sprint items.
6. Updated PM state + decisions.jsonl.

**Escalations:**
- Founder must (a) authorize PR #352 (release→main) when CI green; (b) audit `release.yml` finalize merge step — same systemic failure on every release since v0.1.6 (`RELEASE_BOT_TOKEN` not configured causes auto-close of release PRs); (c) decision gate for R2 if storage format changes.

### 2026-05-31 PM dispatch (previous — PRs #346+#347 merged; release/v0.1.14 cut; PRs #348+#349 opened; R1 step 2 deferred)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns, PM state, v0.2 PRD.

**Assessment:**
- Open PRs: #346 (RFC-0093 Phase 3 docs, CI ✅), #347 (PM dispatch v0.1.14 DONE, CI ✅). Both fully green.
- Open issues: #342/#343/#344 (scale-gap R1/R2/R3). Latest tag: v0.1.13. v0.1.14 not yet released.
- v0.1.14 sprint declared DONE (6/6 criteria) in PR #347. Release not yet cut.

**Actions taken:**
1. **Merged PR #346** (RFC-0093 Phase 3 docs) — green CI, squash merge. ✅
2. **Merged PR #347** (PM dispatch v0.1.14 DONE) — green CI, squash merge. ✅
3. **Cut release/v0.1.14**: bumped version 0.1.13→0.1.14 in Cargo.toml + cli dep pin; sealed CHANGELOG [Unreleased]→[0.1.14]-2026-05-31; committed + pushed branch.
4. **Opened PR #348** (`release/v0.1.14` → `main`) — FOUNDER AUTHORIZATION REQUIRED (Charter §5.12).
5. **Opened PR #349** (`release/v0.1.14` → `develop`) — back-merge (ceremony step 4), can admin-merge when CI green.
6. **R1 step 2 deferred**: `cargo check` took 65s in this environment. Full TDD cycle (RED→GREEN→clippy→fmt) not feasible within 25-min window. Scheduled for next run (full session).
7. Updated PM state + decisions.jsonl.

**Escalations:** Founder must (a) authorize PR #348 (release→main); (b) audit `release.yml` merge step; (c) decision gate for R2 if storage format changes.

### 2026-05-31 PM dispatch (previous — PRs #340/#341/#345 merged; PR #346 opened; v0.1.14 DONE; scale-gap R1/R2/R3 triaged)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns, PM state, v0.2 PRD.

**Assessment:**
- develop HEAD at `9299ead` (PR #340 — PM dispatch from prior session, 5/6 v0.1.14 criteria).
- 3 open PRs: #340 (PM dispatch, green ✅), #341 (scale-gap docs, green ✅), #345 (Store::merge R1 step 1, green ✅).
- 3 open issues: #342 (R1 parallel), #343 (R2 persistence), #344 (R3 memory) — new scale-gap priorities from external review.
- RFC-0093 Phase 3 = sole remaining v0.1.14 criterion. Discovered: tools already use `-> CallToolResult` + `is_error` helpers; `Result<>` wrapper unnecessary. Phase 3 = CHANGELOG BREAKING + RFC Implemented.

**Actions taken:**
1. Merged PR #341 (scale-gap docs: scale-gap-analysis.md + vision-vs-reality.md) ✅
2. Merged PR #345 (feat(core): Store::merge — R1 parallel-index primitive step 1/2) ✅
3. Merged PR #340 (chore(pm): PM dispatch prior session state update) ✅
4. RFC-0093 Phase 3: created feature/rfc-0093-phase3-changelog; added CHANGELOG BREAKING entry; updated RFC acceptance criteria (all [x]); status → Implemented. PR #346 opened (CI running).
5. Closed Issue #209 (RFC-0093 tracking issue).
6. Updated PM state: v0.1.14 6/6 criteria done, scale-gap R1/R2/R3 as P1.

**Escalations:** (1) Founder must authorize `release.yml` finalize merge fix before v0.2.0. (2) R2 incremental persistence may need founder decision gate if storage format changes.

### 2026-05-31 PM dispatch (this run — PRs #335+#337 merged; PR #336 closed; PR #338 rebased and opened)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns, PM state, v0.2 PRD.

**Assessment:**
- develop HEAD: `88def9f` (after PR #337 merged). 3 open PRs: #335 (CI gate, green), #336 (conflicted), #337 (dogfood, CI in progress).
- 0 open issues. v0.1.14 sprint: Skills gate + dogfood criteria both addressed in open PRs.
- PR #336 `mergeable_state: dirty` — conflicts from PRs #335+#337 merges, CI never ran.
- `docs/vision-model-tiering-clean` branch already existed as a clean rebase; only needed rebase onto post-#335/#337 develop.

**Actions taken:**
1. **Merged PR #335** (ci/skill-parity-quality-gate, Quality Gate SUCCESS) — closes Skills INDEX.md CI gate criterion.
2. **Merged PR #337** (docs/v0.1.14-dogfood-report, Quality Gate SUCCESS) — closes Dogfood 8/8 criterion.
3. **Closed PR #336** (conflicted) — superseded by PR #338.
4. **Rebased** `docs/vision-model-tiering-clean` onto develop (clean, no conflicts). Force-pushed, PR #338 already existed and now has CI running.
5. Updated PM state + decisions.jsonl.

**Sprint status:** 5/6 v0.1.14 exit criteria done. Only RFC-0093 Phase 3 remains.

**Escalations:** Founder must audit `release.yml` finalize merge step (systemic — every release).

### 2026-05-31 PM dispatch (previous — Skills INDEX.md CI gate promoted to required; PR #334 merged)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns, PM state (from PR #334), v0.2 PRD.

**Assessment:**
- 1 open PR: #334 (chore/pm-dispatch-2026-05-31-v2) — CI 20/20 green. 0 open issues.
- v0.1.13 ceremony: COMPLETE. Tags: v0.1.13 is latest.
- PM state (from PR #334): v0.1.14 in progress; Skills CI gate and RFC-0093 Phase 3 pending.
- Confirmed all 89 tools return `CallToolResult` directly (not `String`, not `Result<>`).
- Confirmed `parity.yml` runs in `--strict` mode but is NOT in `ci.yml` Quality Gate — informational only.
- Ran `check_skill_parity.py --strict` locally: I1 PASS (89/89), I2 PASS, 0 violations.

**Actions taken:**
1. **Merged PR #334** (CI 20/20 green, chore/pm-dispatch-2026-05-31-v2).
2. **Promoted skill-parity to required CI** — added `skill-parity` job to `ci.yml` + wired into Quality Gate's `needs`. Fixes Charter §5.13 enforcement gap (parity was informational since v0.1.5).
3. Updated `skills/INDEX.md` Phase 3 status (was stale: "blocked on PR #176", which merged at v0.1.4).
4. Updated `CHANGELOG.md` Unreleased section.
5. Updated PM state: `Skills INDEX.md CI gate` sprint criterion marked ✅.

**Escalations:** `release.yml` finalize merge step still systemic; RFC-0093 Phase 3 (89 tools → Result) deferred to next session.

### 2026-05-31 PM dispatch (v0.1.13 SHIPPED; RFC-0096 Phase 2 TS; PRD corrections; security scan)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns, PM state (stale), v0.2 PRD.

**Assessment:**
- develop HEAD: `c8e7f18` (back-merge v0.1.13). main HEAD: `83806ce` (v0.1.13 release).
- Only 1 open PR: #330 (stale PM state chore from prior session — had conflicts, superseded by this run).
- 0 open issues. v0.1.13 ceremony: ALL 4 STEPS COMPLETE (PRs #324 merged to main via PR #332, tag ✅, GitHub Release ✅, PR #333 back-merge ✅).
- RFC-0096 Phase 2 TypeScript: already done (PR #331 merged to develop, 2026-05-31).
- Security scan post-v0.1.13: CLEAN — no hardcoded secrets, no unsafe blocks.
- PRD v0.2 had two stale claims: (1) "`mycelium query` is a placeholder" — FALSE, fully implemented. (2) "0 Skills" — FALSE, 10 Skills exist since v0.1.12.

**Actions taken:**
1. Attempted merge of PR #330 → conflicts in decisions.jsonl (prior session's entry vs RFC-0096 Phase 2 entry from PR #331). Created new chore branch `chore/pm-dispatch-2026-05-31-v2` from develop HEAD.
2. Appended missing decisions.jsonl entry from PR #330 (preserve memory continuity, append-only).
3. **Corrected PRD v0.2**: updated "marquee feature unreachable" → acknowledged implemented; "0 Skills" → "10 Skills"; success metrics table updated.
4. Updated PM state to reflect v0.1.14 sprint scope and v0.1.13 COMPLETE ceremony.
5. Appended this run's decisions.jsonl entry.
6. Committed + pushed chore PR.

**Escalations:** Founder must audit `release.yml` finalize merge step (systemic — every release since v0.1.6).

### 2026-05-31 PM dispatch (v0.1.13 cut; PR #328/#329; v0.1.14 kickoff)

**Assessment:** PR #324 (v0.1.12 → main) still pending; develop at 3ec82c5 (RFC-0093 Phase 2 merged). 0 open issues.

**Actions taken:**
- v0.1.13 sprint DECLARED COMPLETE (5/6 actionable; 6th = founder ceremony).
- Cut release/v0.1.13 from develop HEAD; bumped 0.1.12 → 0.1.13; sealed CHANGELOG.
- PRs #328 (→ main, founder-gated) and #329 (→ develop) opened.
- KEY FINDING: `mycelium query` CLI is FULLY IMPLEMENTED.

**Escalations:** PR #328 founder auth + release.yml systemic fix.

### 2026-05-30 PM dispatch (PR #323 merged; RFC-0093 Phase 2; security CLEAN)

**Actions taken:**
1. Merged PR #323 (release/v0.1.12 → develop back-merge). Ceremony step 4/4 ✅.
2. Security scan post-v0.1.12: CLEAN.
3. RFC-0093 Phase 2 (TDD): 2 RED tests → GREEN. PR #326 opened.

**Escalations:** PR #324 founder auth + release.yml systemic fix.

### 2026-05-30 PM dispatch (v0.1.12 ceremony + v0.1.13 kickoff)

- PR #321 closed/unmerged; release branch auto-deleted. Recreated from v0.1.12 tag.
- PRs #323 (back-merge → develop) + #324 (→ main, founder-gated) created.
- v0.1.13 sprint declared: 3 ADRs + RFC-0093 Ph2 + security scan + ceremony.

### 2026-05-30 PM dispatch (PRs #317/#318/#319 merged; v0.1.12 cut)

- PRs #317 (security scan chore), #318 (RFC-0096 TypeImports), #319 (SKILL docs backfill) merged.
- release/v0.1.12 branch cut from develop HEAD (077cfd4), version bumped 0.1.11 → 0.1.12, PR #321 opened.

### 2026-05-30 PM dispatch (v0.1.11 ceremony + v0.1.12 kickoff — PRs #266 + #270)

- PR #266 merged (MCP is_error sweep). PR #270 merged (Pattern 3 false callers).
- Issues #267/#268 triaged P1. v0.1.11 ceremony complete (tag, crates.io, back-merge PR #315).

### 2026-05-30 PM dispatch (v0.1.11 sprint complete — 9/9 exit criteria)

- 9/9 v0.1.11 criteria met. Issue #214 Pattern 2/3 deferred to v0.1.12.

### 2026-05-30 PM run (earlier — v0.1.11 kickoff + issue #206 re-triage)

- 0 open PRs; 2 open issues labeled. Issue #206 S1 added to P2 queue.

### 2026-05-30 PM run (post-v0.1.10 — RFC-0094 Phase 1 + back-merge)

- PM state fast-forwarded v0.1.6 → v0.1.10. PRs #240 + #241 merged.
- Escalation: release.yml finalize job failing repeatedly.

### 2026-05-29 PM run (v0.1.4 close)

v0.1.4 sprint declared complete. All 7 exit criteria met.

### 2026-05-30 PM call (v0.1.2 era — superseded)

1. v0.1.0 + v0.1.1 shipped. Three-Surface Rule is law.
2. PRD for v0.2 at [`docs/prd/v0.2-the-three-surface-release.md`](../prd/v0.2-the-three-surface-release.md).

### 2026-06-02 PM dispatch v10 (PRs #442+#443 merged; RFC-0104 draft PR #444 opened)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns (no relevant new hits), PM state (v9 from PR #443), v0.2 PRD.

**Assessment:**
- develop HEAD 153eb8f (PM v9 chore merged). 2 open PRs: #442 (Issue #428 AC#2 slice 1 — 24/24 CI SUCCESS), #443 (PM chore v9 — 22/22 CI SUCCESS). 1 open P1 issue: #426 (RFC-0100 Phase 3; AC#4 founder-gated).
- PR #414 (RFC-0101 Phase 2 CLI twin) already merged per GitHub (not in open PR list).
- Sprint v0.1.17: content largely done (PRs #414/#424/#425/#429/#431/#436/#438/#440/#441 all merged). Issue #426 AC#4 is the sole founder-gated blocker for redb default.
- Anti-patterns checked: no new relevant hits. "auto-loop without checking priorities" anti-pattern noted — confirmed highest-priority is Issue #426 AC#4 unblocking.

**Actions taken:**
1. **Merged PR #442** (Issue #428 AC#2 slice 1 — tests extraction, 24/24 CI green, squash) ✅.
2. **Merged PR #443** (PM chore v9, 22/22 CI green, squash) ✅.
3. **Drafted RFC-0104** (`rfcs/0104-charter-warm-cold-sla-split.md`): formalises ADR-0008 Decision-4; proposes Charter §2 warm/cold column split; defines madvise(MADV_DONTNEED) measurement protocol; surfaces three open questions for BDFL decision.
4. **Updated ADR-0008** Decision-4 with RFC-0104 cross-reference.
5. **Opened PR #444** (draft meta RFC — BDFL decision required).
6. Updated PM state v10 + appended decisions.jsonl.

**Sprint status:** v0.1.17 content complete modulo Issue #426 AC#4 (founder gate). Next action for the Hive is security scan; for the founder: RFC-0104 review + v0.1.16 crates.io re-trigger.

**Escalations:** (1) v0.1.16 crates.io PENDING — re-trigger required. (2) RFC-0104 PR #444 — three BDFL questions need answers before implementation.

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

### 2026-06-02 PM dispatch v5 (Issues #426/#427/#428 triaged; PR #431 RFC-0101 contract)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns, PM state (develop `8dde565` — v0.1.16 shipped, PM v4 done), v0.2 PRD.

**Assessment:**
- develop HEAD: `8dde565` (PM dispatch v4, PR #430). 0 open PRs. 3 new issues from founder (expert-panel review at 02:47 UTC): #426 (RFC-0100 Phase 3 redb SLA), #427 (RFC-0101/0102 contract completion), #428 (tech-debt: ADR + god-files).
- v0.1.16: main ✅, tag ✅, develop ✅; crates.io PENDING (P0 founder action).
- ADR renumbering (#428 AC#1) already done via PR #429 (`75739aa`).

**Actions taken:**
1. Triaged issues: #426 → P1, #427 → P1, #428 → P2.
2. Picked Issue #427 partial as highest-impact autonomous P1 (no founder gate).
3. TDD: 5 RED-first tests (compile fail: `edge_kinds` missing, `related_files` missing) → implemented: `edge_kinds` field on `GetContextRequest`, `related_files` in both NOT_FOUND + success responses, `apply_budget` wired into success path → 5/5 GREEN. fmt ✅ clippy ✅ cargo test --all 572+ ✅.
4. Updated RFC-0101 acceptance criteria (4 new [x]). Updated CHANGELOG Unreleased.
5. PR #431 opened (feature/rfc-0101-context-contract → develop).

**Escalations:** Founder: (1) Re-trigger v0.1.16 crates.io (hotfix on main+develop). (2) Sign off Issue #426 RFC-0100 Phase 3 redb-default decision gate before architect prepares ADR.

### 2026-06-02 PM dispatch v4 (hotfix ceremony complete; governance + NodeKind fix merged; v0.1.17 content complete)

**Context:** PR #421 (v3) was closed by founder as "[SUPERSEDED by v4]" — develop advanced (PRs #423, #424, #425 merged) while v3 waited for CI. This v4 is cut fresh from develop HEAD `2c4cb66`.

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20 (latest: `2026-06-02T12:08:00Z` TDD-gate-skip governance PR #424), anti-patterns, PM state (develop `2c4cb66`), v0.2 PRD.

**Assessment:**
- develop HEAD: `2c4cb66` (PR #425 NodeKind fix). main HEAD: `cd662788` (PR #419 hotfix squash-merged by founder).
- 1 open PR: #429 (ADR-0008 renumber, CI running).
- v0.1.16: main ✅, tag ✅, develop back-merge ✅; crates.io PENDING (step 3 — release.yml fixed, founder re-trigger required).
- v0.1.17: content complete (PRs #414+#424+#425 all merged to develop).

**Actions taken:**
1. Confirmed PR #421 review thread (outdated, already addressed in bc9baf4) — no action needed.
2. Created `chore/pm-dispatch-2026-06-02-v4` from develop HEAD `2c4cb66`.
3. Updated PM state: added v0.1.16 section (INCOMPLETE, step 3 PENDING); added v0.1.17 section (content complete); updated header/priorities/dispatch.
4. Appended decisions.jsonl + anti-patterns.jsonl (v3 session entries that never reached develop).
5. Opened PR v4 targeting develop.

**Key v3 session findings (recorded here since v3 never merged):**
- macOS SLA: `sla_ancestors_100k` at 11.664ms vs 5ms limit on macOS CI runners — fixed with `#[cfg(target_os = "macos")]` → 30ms limit.
- `crate_published()` `max_version` bug: field returns highest published version, not whether `$VERSION` is in the list. Fixed by checking `versions[]` membership.
- pipefail: `if ! cargo publish | tee` always returns 0 (tee's exit code). Fixed with `|| {}` pattern.
- Anti-pattern: do not infer crates.io ceremony success from a develop back-merge commit.

**Escalations:** Founder: (1) v0.1.16 crates.io re-trigger; (2) PR #429 review; (3) v0.1.17 ceremony authorization; (4) RELEASE_BOT_TOKEN systemic fix.
