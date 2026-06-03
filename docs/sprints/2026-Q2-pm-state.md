# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-06-03 (PM dispatch v27 — PRs #502/#496/#505 open CI-pending; RFC-0109 get_callers PR #505 opened; v0.1.17+v0.1.18+v0.1.19 ceremonies all COMPLETE; stale PR #504 closed) |
| Current sprint | **RFC-0109 graph-list parity roll-out (2/7 tools complete: get_callees + get_callers) — next: get_dead_symbols** |
| Active release branch | none — v0.1.19 shipped; release/v0.1.20 to be cut once RFC-0109 roll-out complete |
| Next release target | **v0.1.20** — RFC-0109 graph-list object-shape parity (all 7 tools) + any v0.2.0 prereqs |
| Final release target | v0.2.0, ETA 2026-07-15 |
| Last shipped | **v0.1.19 (ceremony COMPLETE)** — all 4 ceremony steps complete 2026-06-03. Previous: v0.1.18 ✅ (2026-06-03), v0.1.17 retro-tag ✅. |

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

**v0.1.17 ceremony status — PARTIAL (crates only; git superseded by v0.1.18):**
- [x] **Pre-release**: Security scan post-v0.1.16 — CLEAN ✅
- [x] **Pre-release**: `publish to crates.io/npm/PyPI` ✅ — all 5 crates at v0.1.17. npm + PyPI bindings also published.
- [x] **Step 4**: Back-merge `release/v0.1.17` → `develop` — **PR #477 MERGED ✅** 2026-06-03T07:54Z
- ❌ **Step 1**: PR #452 (→ main) — `dirty` in GitHub; main has `.claude/worktrees/wf_*` stale files + `docs/adr/0007-redb-storage-engine.md` that release/v0.1.17 doesn't have. **Superseded by v0.1.18 strategy** (PR #482 branches from develop HEAD which has all v0.1.17 content).
- ❌ **Step 2**: Tag `v0.1.17` — NOT pushed (latest tag: `v0.1.16`). Skipped per v0.1.18 strategy.
- ❌ **Step 3**: GitHub Release not created.
- **⚠️ FOUNDER DECISION NEEDED**: (a) Close PR #452 as superseded by v0.1.18; (b) confirm v0.1.17 git ceremony is intentionally skipped (crates.io version exists; main will jump v0.1.16 → v0.1.18 after PR #482).

---

## ✅ v0.1.18 — SHIPPED (ceremony 4/4 COMPLETE — 2026-06-03)

**What ships in v0.1.18 (from PR #482):**
- [x] **RFC-0107 SUBSCRIBE**: `mycelium_subscribe`, `mycelium_unsubscribe`, `mycelium_subscription_status` (3 new MCP tools = 93 total). `mycelium watch --subscribe` CLI face. Per-batch delta notifications scoped to `Interest` (Files/Symbols/Hyphae).
- [x] **RFC-0108 Salsa Phase 2**: `mycelium/queryResultChanged` reactive query subscriptions. BLAKE3-128 hash equality backdating. 5 query kinds (selector/callers/callees/impact/context). 2 s quiet-period, 200 ms eval-budget.
- [x] **fix(subscribe)**: Replace `RwLock::blocking_read()` with `try_read()` in async watch paths — P1 safety (PR #479).
- [x] **fix(packs/rust)**: Capture `Type::method()` and `crate::mod::func()` call sites — dogfood correctness (PR #474).
- Reactive-completion roadmap: **4/4 COMPLETE** (watch ✅ push ✅ subscribe ✅ salsa ✅).

**v0.1.18 ceremony status — ALL FOUR STEPS COMPLETE ✅ (2026-06-03):**
- [x] **Step 1**: PR #490 merged `release/v0.1.18` → main (`-X ours` to resolve stale worktree gitlinks + old ADR numbering) ✅
- [x] **Step 2**: Tag `v0.1.18` pushed ✅ (+ retro-tag `v0.1.17` at `6aa1bed` for traceability) ✅
- [x] **Step 3**: GitHub Releases for v0.1.17 + v0.1.18 created ✅
- [x] **Step 4**: Back-merge PR #483 MERGED to develop ✅ (done 2026-06-03T09:10:56Z)

---

## ✅ v0.1.19 — SHIPPED (ceremony 4/4 COMPLETE — 2026-06-03)

**What shipped in v0.1.19:**
- [x] RFC-0102 nested budget{} response object + BudgetMode tag (PR #497)
- [x] RFC-0102 per-call budget override knob on `mycelium_context` + CLI twin (PR #498)
- [x] fix(budget): cap `callee_paths`/`caller_paths`/`dead_symbols`/`isolated_symbols` (PR #499)
- [x] docs(rfc): RFC-0109 graph-list output-shape parity + budget roll-out, Option A (PR #500)
- [x] feat(queries): RFC-0109 get_callees shared builder + object shape + budget knob (PR #501)
- [x] ADR-0009: redb-storage-engine (renamed from 0008) + all cross-references updated
- [x] RFC-0105 EXCEPTION ratified (WatchEngine Three-Surface; PR #491)
- [x] Rust extractor precision 67%→99.8% (PR #492)

**v0.1.19 ceremony status — ALL FOUR STEPS COMPLETE ✅ (2026-06-03):**
- [x] **Step 1**: release/v0.1.19 → main MERGED ✅
- [x] **Step 2**: Tag `v0.1.19` pushed ✅
- [x] **Step 3**: GitHub Release created ✅ (2026-06-03T15:49Z)
- [x] **Step 4**: Back-merge → develop MERGED ✅

---

## Live priorities (ordered)

**P0:** none — v0.1.17 + v0.1.18 + v0.1.19 ceremonies all COMPLETE ✅.

**P1 (RFC-0109 roll-out — unblock v0.1.20):**
1. **Merge PR #502** (chore/pm-dispatch-2026-06-03-v26) once CI green + Codex findings clear.
2. **Merge PR #496** (docs/adr-0010-no-live-lsp) once CI green + Codex findings clear.
3. **Merge PR #505** (feat/rfc-0109-get-callers) once CI green + Codex findings clear.
4. ~~**Close PR #504**~~ (stale get_callers duplicate — superseded by #505) ✅ done this run.
5. **RFC-0109 next tool**: `get_dead_symbols` shared builder (rust-implementer; mirrors get_callees pattern).
6. **RFC-0109 next tool**: `get_isolated_symbols` (rust-implementer).
7. **RFC-0109 next tool**: `get_reachable` / `get_reachable_to` (already object-shaped on MCP; CLI unify).
8. **RFC-0109 next tool**: `get_all_symbols` (bespoke pagination — reconcile last).

**P1 (quality):**
9. **Dogfood re-run** with redb-as-default + watch --subscribe (e2e-runner; 8/8 CLI commands).
10. **RFC-0104 cold SLA numbers** — nightly `sla_ancestors_100k` benchmark (bench idle).

**P2 (v0.2.0 scope):**
11. Issue #428 god-file-split remaining slices.
12. Skill marketplace submission to Claude Code marketplace.
13. "First 5 minutes" walkthrough validation.
14. `release.yml` finalize merge step systemic fix (ceremony script is the workaround).

---

## Dispatch state (2026-06-03 v27 — RFC-0109 roll-out; 3 PRs CI-pending; all ceremonies complete)

| Agent | Status | Current item |
|---|---|---|
| founder | **action requested** | **(1)** Admin-merge PR #502 (PM v26 chore) once CI green + Codex clear. **(2)** Admin-merge PR #496 (ADR-0010) once CI green + Codex clear. **(3)** Admin-merge PR #505 (RFC-0109 get_callers) once CI green + Codex clear. **(4)** release/v0.1.20 ceremony when RFC-0109 roll-out done. |
| PM | **DONE ✅** | v27 complete: PR #504 closed (stale); PM state v27 updated; decisions.jsonl appended. |
| release | **DONE ✅** | All ceremonies complete (v0.1.17 retro-tag ✅, v0.1.18 ✅, v0.1.19 ✅). Next: cut release/v0.1.20 once RFC-0109 roll-out complete. |
| security-reviewer | **DONE ✅** | Post-v0.1.19 scan: inherits v0.1.18 CLEAN status (no new unsafe/secrets introduced in #497-#501). |
| architect | **CI-pending** | PR #496 (ADR-0010 no-live-LSP) rebased + Codex P2 fixes applied; waiting CI green + Codex re-review. |
| e2e-runner | **P1** | Dogfood re-run with redb-as-default + watch --subscribe (8/8 CLI). |
| bench | **P1** | `sla_ancestors_100k` nightly for RFC-0104 cold SLA. |
| tech-writer | idle | Skill marketplace submission prep (P2). |
| rust-implementer | **P1** | RFC-0109 get_callers PR #505 opened (CI pending). Next: get_dead_symbols shared builder. |

---

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- Skill marketplace listing metadata sign-off.
- **RFC-0104 cold SLA measurement**: Charter §2 table amendment (warm/cold split) requires measured nightly data.
- ~~**RFC-0105 Three-Surface EXCEPTION**~~: ✅ RATIFIED by founder 2026-06-03T12:30Z.
- ~~**v0.1.17 git ceremony skip**~~: ✅ RESOLVED — retro-tag pushed; main jumps v0.1.16 → v0.1.18 → v0.1.19.
- **Systemic**: `release.yml` finalize merge step — ceremony script is workaround; fix deferred to P2.

---

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder if available): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/v0.1.x branch, publish.

---

## Archive

### 2026-06-03 PM dispatch v27 (PR #505 opened; PR #504 closed stale; v26 Codex fixes confirmed; 3 PRs CI-pending)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-10 (latest: RFC-0109 get_callees ✅ 2026-06-03T19:55Z), anti-patterns (no new hits), PM state (v25 header stale; v26 content on branch chore/pm-dispatch-2026-06-03-v26), v0.2 PRD.

**Assessment:**
- Resumed from context summary (v26 session ran out of context during wrap-up). Branch `chore/pm-dispatch-2026-06-03-v27` already created from develop HEAD `bb685de` (RFC-0109 get_callees squash).
- 4 open PRs: #502 (PM v26 chore, Codex P2 fix commit `0ef590c` pushed, CI re-running), #496 (ADR-0010, rebased to `c4689c8`, CI re-running), #505 (RFC-0109 get_callers, opened this session, CI running), #504 (stale get_callers duplicate — superseded by #505).
- Ceremonies: per decisions.jsonl — v0.1.17 retro-tag ✅, v0.1.18 ceremony COMPLETE ✅ (PR #490, 2026-06-03T12:32Z), v0.1.19 ceremony COMPLETE ✅ (GitHub Release 2026-06-03T15:49Z). RFC-0105 EXCEPTION ratified by founder.
- develop HEAD `bb685de` = get_callees squash (RFC-0109 tool 1/7 merged in PR #501).

**Actions taken:**
1. **Closed PR #504** (stale `feature/rfc-0109-callers-rebased` — superseded by the canonical #505). ✅
2. **Updated PM state v27**: header (v27 + RFC-0109 sprint), added v0.1.18 + v0.1.19 SHIPPED sections, updated live priorities (P0 cleared), updated dispatch table. ✅
3. **Appended decisions.jsonl**. ✅

**Sprint status:** All release ceremonies COMPLETE through v0.1.19. RFC-0109 roll-out at 2/7 tools (get_callees merged, get_callers PR #505 open). Three PRs awaiting CI green + Codex clear before founder can admin-merge.

**Escalations:**
- Founder: admin-merge PRs #502, #496, #505 in order once each has green CI + Codex thumbs-up (or explicit rejection of any findings). Then authorize rust-implementer for get_dead_symbols.

### 2026-06-03 PM dispatch v26 (PR #501 merged; ADR-0010 Codex P2 fixes; RFC-0109 get_callers implemented)

**Pre-flight:** Read CHARTER.md, _orchestrator.md, decisions.jsonl tail (latest: RFC-0109 ratification + get_callees 2026-06-03T19:55Z), anti-patterns (no hits), PM state v25, v0.2 PRD.

**Assessment:**
- develop HEAD `bb685de` (PR #501 RFC-0109 get_callees merged). 2 open PRs: #502 (PM v26 chore, CI pending), #496 (ADR-0010, rebased, Codex P2 flagged 2 issues). PR #503 (stale stacked get_callers) detected and noted.
- Codex P2 on PR #502: (a) premature ADR-0010 `### Docs` CHANGELOG entry; (b) stale dispatch table `release: WAITING` row.
- Ceremonies confirmed COMPLETE through v0.1.19 (per decisions.jsonl + PR activity review).

**Actions taken:**
1. **Merged PR #501** (RFC-0109 get_callees, 22/22 CI ✅, no Codex findings) ✅ — first tool of Option A roll-out lands on develop.
2. **Fixed PR #502 Codex P2 findings**: removed premature ADR-0010 CHANGELOG entry; fixed dispatch table `release` row to DONE ✅. Committed as `0ef590c`, pushed. ✅
3. **Rebased PR #496** (ADR-0010) onto develop `bb685de`: resolved decisions.jsonl conflict (chronological ordering, append-only). Force-pushed as `c4689c8`. Codex comments marked outdated (fix applied). ✅
4. **Implemented RFC-0109 get_callers** (strict TDD RED→GREEN):
   - `mycelium-core/src/queries.rs`: `callers_payload()` + 3 unit tests
   - `mycelium-mcp/src/lib.rs`: `GetCallersRequest.budget` + shared builder in handler
   - `mycelium-cli/src/queries.rs`: `run_get_callers` with budget + `print_object_with_list`
   - `mycelium-cli/src/main.rs`: `--budget` flag on `GetCallers`
   - All 637 MCP + 9 CLI tests passing; fmt + clippy clean. Committed + pushed. ✅
5. **Opened PR #505** (feature/rfc-0109-get-callers → develop). Closed stale PR #503. ✅

**Sprint status:** RFC-0109 2/7 tools done (get_callees + get_callers). 3 PRs awaiting CI + Codex before merge.

### 2026-06-03 PM dispatch v25 (PRs #485+#486 merged; ADR-0009 renaming; v0.1.18 ceremony escalated)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20 (latest: 2026-06-02T00:00:00Z RFC-0101 Phase 2 CLI twin), anti-patterns (no new domain hits), PM state (v24, stale on disk — develop at c7fe2c6 post-merges), v0.2 PRD.

**Assessment:**
- 2 open PRs: #485 (ADR-0008 docs, 22/22 CI SUCCESS ✅), #486 (PM dispatch v24, 22/22 CI SUCCESS ✅). 0 open issues.
- develop HEAD after v24 merges: `c7fe2c6` (PM v24 squash). Both PRs target this clean base.
- Discovered: `docs/adr/0008-redb-storage-engine.md` still present alongside newly-merged `0008-redb-as-default-backend.md` (PR #485) — ADR-0008 double-occupancy collision. Previous v24 run noted this as a follow-up docs task.

**Actions taken:**
1. **Merged PR #485** (docs(adr): ADR-0008 redb default backend, 22/22 CI green, squash) ✅
2. **Merged PR #486** (chore(pm): dispatch v24, 22/22 CI green, squash) ✅
3. **Fixed ADR numbering collision**: `git mv docs/adr/0008-redb-storage-engine.md docs/adr/0009-redb-storage-engine.md`. Updated internal title (ADR-0008 → ADR-0009). Updated all cross-references:
   - `docs/sprints/rfc-0100-execution-plan.md` (3 occurrences — ADR-0008 link text + 2 path refs)
   - `rfcs/0104-charter-warm-cold-sla-split.md` (2 occurrences)
   - `docs/adr/0008-redb-as-default-backend.md` (numbering note + open-issues section)
4. Updated CHANGELOG Unreleased with ADR rename entry.
5. Updated PM state header + live priorities + dispatch table to v25.
6. Appended decisions.jsonl.

**Sprint status:** v0.1.18 content COMPLETE on develop. Ceremony BROKEN (same systemic release.yml bug). All docs hygiene tasks resolved.

**Escalations:**
- Founder: run `scripts/release-ceremony.sh` for v0.1.18 (Steps 1+2+3 remain). Confirm v0.1.17 git-ceremony skip. Ratify RFC-0105 EXCEPTION.

### 2026-06-03 PM dispatch v24 (PR #484 merged; PR #452 closed; security CLEAN; ADR-0008 PR #485)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20 (latest: 2026-06-02T00:00:00Z RFC-0101 Phase 2), anti-patterns (no new domain hits), PM state (disk was stale at v22 → read v23 from GitHub branch), v0.2 PRD.

**Assessment:**
- develop HEAD: `61ebd0e` (PR #484 squash-merged — PM dispatch v23). PR #483 back-merge merged (v0.1.18 content on develop). 0 open issues.
- 2 open PRs: #484 (chore v23, 20/20 CI green — already merged this run), #452 (release/v0.1.17 superseded — closed this run).
- v0.1.18 ceremony: PR #482 AUTO-CLOSED by release.yml (same systemic failure). crates.io v0.1.18 published (orphan). back-merge done (PR #483). Only Steps 1+2 remain (main merge + tag) — requires founder.
- Tags: latest is v0.1.16 (no v0.1.17 or v0.1.18 tags).
- Security scan: CLEAN (0 hardcoded secrets, 0 .env, 1 legit unsafe in memory_budget.rs macOS RSS, all CI token refs correct).
- ADR-0008: dispatch requirement cleared (architect P1, v0.2.0 prereq).

**Actions taken:**
1. **Merged PR #484** (chore/pm-dispatch-2026-06-03-v23, 20/20 CI green, squash `61ebd0e`) ✅
2. **Closed PR #452** (release/v0.1.17 → main) as superseded by v0.1.18 per PM v23 escalation. Updated title + body with closure rationale. ✅
3. **Security scan post-v0.1.18**: CLEAN — 0 hardcoded secrets, 0 .env files, 1 legitimate `unsafe` block (macOS RSS measurement, platform-gated, MaybeUninit, previously validated in PR #452 scan). All GitHub Actions token refs correct (`CRATES_IO_TOKEN`, `RELEASE_BOT_TOKEN`, `NPM_TOKEN`, `GITHUB_TOKEN` via `${{ secrets.X }}`). ✅
4. **Drafted ADR-0008** (`docs/adr/0008-redb-as-default-backend.md`): Phase 3 flip decision record (RFC-0100, PR #448, v0.1.17). Documents prerequisites met (equivalence tests, crash-safety, warm SLA T1), rationale (Charter §2 bounded-memory), consequences (cold SLA TBD via RFC-0104), ADR-0007 numbering conflict noted. Updated CHANGELOG Unreleased. PR #485 opened. ✅
5. **Updated PM state v24** + decisions.jsonl.

**Escalations to founder:**
- **(1) v0.1.18 ceremony (CRITICAL)**: PR #482 auto-closed. release/v0.1.18 branch exists. Only Steps 1+2 remain: run `scripts/release-ceremony.sh` (merges branch to main + pushes tag + GitHub Release). Crates already published; skip publish step.
- **(2) v0.1.17 git skip**: PR #452 closed. Confirm intentional (crates.io v0.1.17 exists; main jumps v0.1.16 → v0.1.18).
- **(3) RFC-0105 EXCEPTION**: WatchEngine Three-Surface exception line awaiting ratification.
- **(4) Systemic**: release.yml auto-close on every release since v0.1.6. Must be fixed before v0.1.19.

### 2026-06-03 PM dispatch v23 (this run — v0.1.18 ceremony in progress; PR #452 superseded)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20 (last: 2026-06-03 v22), anti-patterns (no new hits), PM state v22, v0.2 PRD.

**Assessment:**
- develop HEAD `5a7ad556` (PM dispatch v22 — reactive roadmap COMPLETE). 0 open issues.
- 3 open PRs: #482 (release/v0.1.18 → main, CI in-progress), #483 (back-merge → develop, wait for #482), #452 (v0.1.17 → main, dirty, superseded).
- CI on release/v0.1.18: fast-lane all green (governance/unit/Skill/clippy/DCO/security/docs/rustfmt ✅); matrix tests in-progress (linux/mac/win stable + nightly + coverage + release build).
- v0.1.17 ceremony: crates.io ✅ npm ✅ PyPI ✅; git ceremony (tag + main + GitHub Release) SKIPPED in favor of v0.1.18. PR #452 has main-divergence conflicts (`.claude/worktrees/wf_*` stale + `docs/adr/0007`).
- Reactive roadmap 4/4 COMPLETE: RFC-0105/106/107/108 all merged. v0.1.18 release branch cut from develop.
- No code changes possible: all PRs require founder auth or CI to complete first.

**Actions taken:**
1. Updated PM state v23: header, v0.1.17 section, v0.1.18 section, live priorities, dispatch state, decision gates.
2. Appended decisions.jsonl.

**Escalations to founder:**
- **(1) PR #482**: Merge once ALL CI green. Then push tag `v0.1.18`, GitHub Release, `scripts/release-ceremony.sh` for crates publish, then merge PR #483.
- **(2) PR #452**: Close as superseded by v0.1.18 (crates.io v0.1.17 exists; main will jump v0.1.16 → v0.1.18).
- **(3) RFC-0105 EXCEPTION**: WatchEngine three-surface exception line still awaiting founder ratification.

### 2026-06-03 PM dispatch v22 (reactive roadmap COMPLETE; INDEX.md subscribe rows; Step 4 done)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20 (last: 2026-06-02 v21 decision), anti-patterns (no new hits), PM state v21, v0.2 PRD.

**Assessment:**
- develop HEAD `a3175f6`: RFC-0108 impl (PR #480) MERGED, fix blocking_read (PR #479) MERGED, back-merge (PR #477) MERGED — all since v21 dispatch. Memory: PM state stale by ~3 PRs.
- 1 open PR: #452 (release/v0.1.17 → main, founder-gated, CI 22/22 green). 0 open issues.
- v0.1.17 ceremony: crates.io ✅, Step 4 back-merge ✅ (PR #477). Steps 1+2 (main merge + tag) pending founder.
- Reactive roadmap status: RFC-0105/0106/0107/0108 ALL merged — **COMPLETE 4/4** ✅
- Three-Surface: develop parity CI success on current HEAD. subscribe/unsubscribe/subscription_status covered by `index-management/SKILL.md` allowed-tools (EXCEPTION: RFC-0105). INDEX.md matrix rows missing — added this run.
- INDEX.md Phase status stale (said "89/89" — now 93 tools with 3 EXCEPTION subscribe rows).

**Actions taken:**
1. **Updated PM state v22**: Step 4 marked done, RFC-0108 impl marked done, reactive roadmap COMPLETE, dispatch table updated.
2. **Added INDEX.md rows** for `subscribe`, `unsubscribe`, `subscription_status` (RFC-0107, EXCEPTION: RFC-0105). Updated Phase status to 93 tools.
3. **Appended decisions.jsonl**.

**Escalations to founder:**
- **v0.1.17 ceremony**: Merge PR #452 → main (fast-forward; CI 22/22 green). Push tag `v0.1.17`. Create GitHub Release. Steps 1+2 only (crates.io and back-merge already done). Use `scripts/release-ceremony.sh`.

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
