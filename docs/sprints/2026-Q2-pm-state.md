# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-06-13 (PM dispatch v232 — PR #835 merged (squash `f861fc84`, 22/22 CI ✅; Codex P2 rejected with justification); 1 open issue #829 P1; escalation ×92→×93) |
| Current sprint | **Holding pattern — 0 open code tasks (v232).** P0 ×2 founder-gated (PR #568 v0.3.0 ceremony ×93 escalations; PR #763 RFC-0121). **P1 unblocked**: e2e-runner dogfood 8/8 CLI + SDK round-trip; bench mutation kill rate (issue #829); RFC-0104 cold SLA nightly. |
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

> **Post-v0.3.0 unreleased on develop (→ v0.3.1):** PRs below merged after the release/v0.3.0 branch was cut; will ship in v0.3.1:
- [x] fix(core): entry-points count reflects returned array after budget truncation (PR #746, squash `2037b27`)
- [x] docs(governance): RFC-0123 MCP facade consolidation spec — 95 tools → 11 action facades (PR #747, squash `9cd34d4`)
- [x] fix(core): callee/caller trees collapse unresolved leaves into a count (PR #748, squash `72086df`)
- [x] fix(hyphae): validate kind selectors + human-readable lexer/parser errors (PR #749, squash `c39fd6c`)
- [x] fix(core): Rust symbol spans anchored on item node, not file/impl container (PR #750, squash `aebd6a8`)
- [x] fix(mcp): thread RFC-0102 OutputBudget through query/cross-refs/tree tools (PR #752, squash `bab615a`)
- [x] feat(test-gap): RFC-0115 Phase 2 — `mycelium test-gap` CLI + `mycelium_test_gap` MCP + `graph-structure` Skill; 96/96 Three-Surface ✅ (PR #743, squash `d984370`)
- [x] fix(core): item-level symbol spans for Python/TS/JS/Go/C++ (PR #750 parity, squash `4520459`)
- [x] feat(hyphae): attribute filters after pseudo-classes — RFC-0124 (PR #754, squash `56bc4b7`)
- [x] chore(pm): PM dispatch v171 — PRs #743–#754 documented; RFC-0117/0119 status corrected (PR #756, squash `4d7e681`)
- [x] feat(constraints): RFC-0117 Phase 2 — `check-architecture` CLI + `mycelium_check_architecture` MCP + graph-structure Skill + YAML loader + Store adapter; 97/97 Three-Surface ✅ (PR #757, squash `98636e0`)
- [x] chore(pm): PM state v205 chore (PR #797, squash `763fe66`)
- [x] feat(classify): RFC-0113 Phase 4 — Rust stdlib callee classification; `classify_rust` + `classify_rust_import_gated` + `classify_rust_qualified`; 21 TDD tests; Codex P2 spun off as issue #800 (PR #798, squash `28ee0dc`)

**v0.3.0 ceremony status — REGISTRIES ✅, GIT ⏳ PENDING:**
- [x] **Registries published** 2026-06-05T17:59Z — crates.io ✅, npm ✅, PyPI ✅
- [ ] **Step 1**: `release/v0.3.0` → `main` — **⏳ awaiting `finalize` workflow_dispatch (founder action on PR #568)**
- [ ] **Step 2**: Tag `v0.3.0` — awaiting finalize
- [ ] **Step 3**: GitHub Release v0.3.0 — awaiting finalize
- [ ] **Step 4**: Back-merge `release/v0.3.0` → develop — awaiting finalize

---

## Live priorities (ordered)

> ⚠️ **Two P0 items require founder action.** Develop HEAD `bdad01d` (PM v208 chore, PR #803). RFC-0121 Option A staged as DRAFT PR #763 — **unblocked**. Skills: **97/97 Three-Surface compliant**. **Codex status**: active.
> **v203 update (2026-06-12):** PR #792 (PM v202 chore) merged `22da0e3`; RFC-0113 Phase 3 Go implemented and PR #793 opened (CI pending at end of v203). Escalation ×67→×68.
> **v204 update (2026-06-12):** PR #793 **MERGED** `3b46ba2` (22/22 CI ✅; Codex P1 spun off to issue #795 Phase 3b). Escalation ×68→×69.
> **v205 update (2026-06-12):** PR #796 **MERGED** `b052bcc` (RFC-0113 Phase 3b Go qualified-call fix; Codex P2 fixed in-PR). Issue #795 closed. Escalation ×69→×70.
> **v207 update (2026-06-12):** PR #797 **MERGED** `763fe66`. PR #798 **MERGED** `28ee0dc` (RFC-0113 Phase 4 Rust — `classify_rust` + `classify_rust_import_gated` + `classify_rust_qualified`; 21 TDD tests; Codex P2 spun off to issue #800). Issue #800 opened. Escalation ×70→×72.
> **v208 update (2026-06-12):** PR #801 **MERGED** `016aed9` (PM v207 chore; Codex P2 fixed: banner SHA b052bcc→28ee0dc). **PR #802 OPENED** (`fix/rfc-0113-phase5-rust-qualified`): RFC-0113 Phase 5 — single-segment Rust scoped calls now emit `scope>name` stubs (3 new TDD tests, 957/957 pass). Closes issue #800. Escalation ×72→×73.
> **v209 update (2026-06-12):** PR #803 **MERGED** `bdad01d` (PM v208 chore; Codex P2 replied — issue #800 correctly OPEN, PR #802 pending). **PR #802 parity fix**: diagnosed `Pack query parity` CI failure (MCP+CLI embedded copies not synced); pushed fix `4d93d565`; Pack query parity ✅ on new run; Quality Gate 22/22 ✅; 0 Codex findings. **PR #802 MERGED** `8b14ecd` (RFC-0113 Phase 5; issue #800 CLOSED). Anti-pattern (syncing only core) already in anti-patterns.jsonl `07:40Z` — pre-flight grep missed. Escalation ×73→×74.

**P0 (founder action required):**
1. **PR #568** [×93 consecutive runs] (`release/v0.3.0`, open) — **🚨 IMMEDIATELY ACTIONABLE**: All 50 CI checks are SUCCESS or SKIPPED. Registries published (crates.io ✅, npm ✅, PyPI ✅). Charter §5.12 gate **MET** — trigger `finalize` workflow_dispatch on PR #568 to complete git ceremony (Steps 1–4: merge main + tag v0.3.0 + GitHub Release + back-merge to develop).
2. **RFC-0121** — DRAFT PR #763 staged (22/22 CI ✅). ✅ **UNBLOCKED**: `bpe_charter_sla_binding` asserts per-class thresholds (tree ≤35%, list ≤70%, scalar ≤90%). Founder can un-draft + merge PR #763 directly.

**ℹ️ Codex — active. PR #763 (DRAFT): 0 comments. PR #568: 1 finding (outdated, replied + issue #560 tracked). PR #809 MERGED `2f47f503`. PR #810 MERGED `7600b9db`.**
> **v210 update (2026-06-12):** PR #804 **MERGED** `2961bd3` (PM v209 chore; 20/20 CI ✅). Issue #800 **EXPLICITLY CLOSED** (GitHub does not auto-close on non-default branch merge; closed via API in v210). 0 open issues. Escalation ×74→×75.
> **v211 update (2026-06-12):** PR #805 **MERGED** `a20f64e` (PM v210 chore; 20/20 CI ✅). RFC-0113 Phase 5 docs updated. Escalation ×75→×76.
> **v212 update (2026-06-12):** Codex P2 on PR #806 RFC-0113 line 162 FIXED (commit `9581552`). CI deferred (18/22 at session-end). Escalation ×76→×77.
> **v213 update (2026-06-12):** PR #806 **MERGED** `d2b2a12` (PM v211 + RFC-0113 Phase 5 docs; 20/20 CI ✅; Codex P2 fixed). PR #807 closed (superseded). Escalation ×77→×78.
> **v214 update (2026-06-12):** PR #808 **MERGED** `35bfe2d` (PM v213 chore; Codex P2 rejected — pre-flight SHA). RFC-0113 corpus measurement completed: 1,026 callee edges; 66.4% classified (project 45%, stdlib 6%, builtin 5%), 33.6% unknown tail. PR #809 opened (docs/rfc-0113-corpus-measurement, CI pending). Escalation ×78→×79.
> **v215 update (2026-06-12):** PR #809 **MERGED** `2f47f503` (RFC-0113 corpus docs; 3/3 CI ✅; Codex P1+P2 addressed). PR #810 **MERGED** `7600b9db` (PM v214 chore; 3/3 CI ✅; Codex P2 self-resolved). RFC-0113 **fully IMPLEMENTED + DOCUMENTED** on develop. Escalation ×79→×80.
> **v216 update (2026-06-12):** RFC-0125 planned (JavaScript callee classification). No PRs merged this run. Escalation ×80→×81.
> **v217 update (2026-06-12):** PR #812 **MERGED** `9979b960` (PM v216 chore). PR #813 **MERGED** `ea51977f` (RFC-0125 draft). RFC-0125 Phase 1 implemented (CJS require() extractor); PR #814 opened (CI running). Escalation ×81→×82.
> **v218 update (2026-06-12):** PR #814 **MERGED** `7e711f4f` (RFC-0125 Phase 1 CJS extractor). PR #815 **MERGED** `0cc5bcfd` (PM v217 chore). RFC-0125 Phase 2 implemented (browser-global classifier); PR #817 opened. All 965 tests pass. Escalation ×82→×83.
> **v219 update (2026-06-12):** PR #817 **MERGED** `a6c83af3` (RFC-0125 Phase 2 browser-global). Issue #819 opened (Phase 3 member-call receiver). PM state v219 updated. Escalation ×83→×84.
> **v223 update (2026-06-13):** PR #823 **MERGED** `c2fbc34` (RFC-0126 Phase 3; 977 tests). PRs #822+#824 closed (superseded). Issue #819 closed. Escalation ×84→×86.
> **v224 update (2026-06-13):** PR #826 opened (chore/pm-state-v224; CI pending). Issue #827 filed (pm-state live section inertia). Escalation ×86.
> **v225 update (2026-06-13):** PR #828 **MERGED** `95be1b6` (chore/pm-state-v225 audit entry). Issue #827 open. Escalation ×86.
> **v226 update (2026-06-13):** NEW FINDING: Nightly CI on main FAILED — mutation kill rate <70%. Issue #829 opened (P1). Issue #827 CLOSED (live header fixed). Escalation ×87.
> **v227 update (2026-06-13):** PR #830 **MERGED** `fb3f3cc6` (PM v226 chore; Codex P1 rejected — CI DCO ✅; Codex P2 fixed — open-issue count updated). Escalation ×87→×88.
> **v228 update (2026-06-13):** PR #831 **MERGED** `69463051` (PM v227 chore; Codex P2 rejected — two-snapshot SHA pair valid). Escalation ×88→×89.
> **v229 update (2026-06-13):** PR #832 **MERGED** `eccb51d` (PM v228 chore; Codex P2 rejected — scoped audit note vs live priorities). Escalation ×89→×90.
> **v230 update (2026-06-13):** PR #833 **MERGED** `6160f40` (PM v229 chore; Codex P2 rejected — archive entries are cumulative summaries). Escalation ×90→×91.
> **v231 update (2026-06-13):** PR #834 **MERGED** `de1e016` (PM v230 chore; 22/22 CI ✅; 0 Codex findings). Escalation ×91→×92.
> **v232 update (2026-06-13):** PR #835 **MERGED** `f861fc84` (PM v231 chore; 22/22 CI ✅; Codex P2 rejected — archive entries are post-session summaries, not pre-flight snapshots). Escalation ×92→×93. 1 open issue: #829 P1 (nightly mutation kill rate <70% on main).

**P1 (unblocked — next items):**
- e2e-runner: dogfood 8/8 CLI commands + SDK round-trip (Node/Python SDKs at v0.3.0 in registries)
- bench: RFC-0104 cold SLA nightly benchmark
- bench: verify issue #829 mutation kill rate resolved after v0.3.0 ceremony lands on main

---

## Decision gates (for future sessions)

| Gate | Status | Notes |
|---|---|---|
| RFC-0121 Option A/B/C | ✅ **OPTION A STAGED** — DRAFT PR #763 | Founder: un-draft + merge PR #763 |
| RFC-0123 MCP facade | 📋 **SPEC MERGED** (PR #747) | Founder ratification required before implementation |
| RFC-0104 cold SLA | ⏳ Unblocked | bench agent: run nightly benchmark |
| Mutation kill rate (issue #829) | ⏳ Pending PR #568 ceremony | Likely resolved when main advances to v0.3.0 |

---

## Dispatch state (v232)

| Item | Owner | Status |
|---|---|---|
| PR #568 `release/v0.3.0` → main | founder | ⏳ **IMMEDIATELY ACTIONABLE** ×93 |
| PR #763 RFC-0121 Charter §2 amendment | founder (BDFL) | ⏳ DRAFT — ready to un-draft + merge |
| Issue #829 mutation kill rate | bench / founder | ⏳ awaiting #568 ceremony |
| e2e-runner dogfood (8/8 CLI + SDK) | e2e-runner | ⏳ unblocked |
| RFC-0104 cold SLA benchmark | bench | ⏳ unblocked |
| RFC-0123 MCP facade impl | rust-implementer | 🔒 blocked on founder ratification |
| RFC-0119 AC-12/AC-13 dogfood | e2e-runner | ⏳ unblocked (post-#568 back-merge ideal) |

---

## Inline history (v210–v232)

### v232 (2026-06-13)
- Merged PR #835 (chore/pm-state-v231, squash `f861fc84`, 22/22 CI ✅).
- Codex P2 on PR #835: rejected with justification — archive entries document post-session outcomes, not pre-flight snapshots; decisions.jsonl is the authoritative timestamped log.
- Escalation ×92→×93 for PR #568 (v0.3.0 ceremony).
- 1 open issue: #829 P1 (nightly mutation kill rate <70% on main, awaiting PR #568 ceremony).
- No autonomous code work available.

### v231 (2026-06-13)
- Merged PR #834 (PM v230 chore, squash `de1e016`, 22/22 CI ✅, 0 Codex findings).
- Escalation ×91→×92 for PR #568.
- P0s unchanged; P1 unblocked queue held.

### v230 (2026-06-13)
- Merged PR #833 (PM v229 chore, squash `6160f40`).
- Codex P2 rejected: archive entries are cumulative session summaries; decisions.jsonl is the authoritative event log.
- Escalation ×90→×91.

### v229 (2026-06-13)
- Merged PR #832 (PM v228 chore, squash `eccb51d`).
- Codex P2 rejected: scoped v228 audit note vs live priorities section — 'P1 (unblocked)' section is authoritative for next run.
- Escalation ×89→×90. P1 unblocked items explicitly surfaced.

### v228 (2026-06-13)
- Merged PR #831 (PM v227 chore, squash `69463051`).
- Codex P2 rejected: two-snapshot SHA pair (pre-merge + post-merge) is valid sequential state.
- Escalation ×88→×89.

### v227 (2026-06-13)
- Merged PR #830 (PM v226 chore, squash `fb3f3cc6`).
- Codex P1 rejected: CI DCO gate passed (job 81171862185 ✅); merge commit legitimately exempt.
- Codex P2 fixed: open-issue count 0→1 (#829 P1 nightly mutation kill rate).
- Escalation ×87→×88.

### v226 (2026-06-13)
- NEW FINDING: Nightly CI on main FAILED — mutation testing kill-rate gate <70% (SHA 54687972=v0.2.0).
- Issue #829 opened (P1). Issue #827 CLOSED (live header fixed).
- Decisions.jsonl backfilled v224+v225. Escalation ×87.

### v225 (2026-06-13)
- PR #828 merged `95be1b6` (audit entry only). Issue #827 still open — live header fix deferred to v226.

### v224 (2026-06-13)
- PR #826 opened (chore/pm-state-v224). Issue #827 filed (pm-state live section inertia — live header not updated).

### v223 (2026-06-13)
- PR #823 merged `c2fbc34` (RFC-0126 Phase 3; 977 tests). PRs #822+#824 closed (superseded).
- Issue #819 closed. Escalation ×84→×86.

### v219 (2026-06-12)
- PR #817 merged `a6c83af3` (RFC-0125 Phase 2 browser-global classifier).
- Issue #819 opened (Phase 3 member-call receiver). PM state v219. Escalation ×83→×84.

### v218 (2026-06-12)
- PR #814 merged `7e711f4f` (RFC-0125 Phase 1 CJS extractor). PR #815 merged `0cc5bcfd` (PM v217 chore).
- RFC-0125 Phase 2 implemented (965 tests). PR #817 opened. Escalation ×82→×83.

### v217 (2026-06-12)
- PR #812 merged `9979b960` (PM v216 chore). PR #813 merged `ea51977f` (RFC-0125 draft).
- RFC-0125 Phase 1 (CJS require() extractor) implemented. PR #814 opened. Escalation ×81→×82.

### v216 (2026-06-12)
- RFC-0125 JavaScript callee classification planned as next P1. No PRs merged.
- Escalation ×80→×81.

### v215 (2026-06-12)
- PR #809 merged `2f47f503` (RFC-0113 corpus docs). PR #810 merged `7600b9db` (PM v214 chore).
- RFC-0113 **FULLY IMPLEMENTED + DOCUMENTED** on develop. Escalation ×79→×80.

### v214 (2026-06-12)
- PR #808 merged `35bfe2d` (PM v213 chore). RFC-0113 corpus measurement: 1,026 callee edges, 66.4% classified. PR #809 opened.
- Escalation ×78→×79.

### v213 (2026-06-12)
- PR #806 merged `d2b2a12` (PM v211 + RFC-0113 Phase 5 docs). PR #807 closed (superseded).
- Escalation ×77→×78.

### v212 (2026-06-12)
- Codex P2 on PR #806 RFC-0113 line 162 FIXED (commit `9581552`). CI 18/22 at session-end.
- Escalation ×76→×77.

### v211 (2026-06-12)
- PR #805 merged `a20f64e` (PM v210 chore). RFC-0113 Phase 5 docs updated (Phase 5 AC section added).
- Escalation ×75→×76.

### v210 (2026-06-12)
- PR #804 merged `2961bd3` (PM v209 chore). Issue #800 explicitly closed (GitHub non-default branch auto-close gap).
- 0 open issues. Escalation ×74→×75.

---

## Archive — PM dispatches v130–v209

### v209 (2026-06-12)
- Merged PR #803 `bdad01d` (PM v208 chore; Codex P2 replied — issue #800 correctly OPEN, PR #802 pending).
- Diagnosed PR #802 `Pack query parity` CI failure: MCP+CLI embedded copies not synced; pushed fix `4d93d565`; Quality Gate 22/22 ✅; 0 Codex findings.
- PR #802 MERGED `8b14ecd` (RFC-0113 Phase 5 — single-segment Rust scoped calls emit `scope>name` stubs; parity fix included).
- Issue #800 CLOSED (auto-close by PR #802 on develop; GitHub auto-close fires only on default-branch merge — explicit close via API).

### v208 (2026-06-12)
- PR #801 merged `016aed9` (PM v207 chore; Codex P2 fixed: banner SHA corrected b052bcc→28ee0dc).
- RFC-0113 Phase 5 implemented: new `reference.scoped_call` query captures `@call.scope` + `@name`; extractor builds `scope>name` stub; 3 TDD tests; 957/957 pass. PR #802 opened.
- Escalation ×72→×73.

### v207 (2026-06-12)
- PR #798 merged `28ee0dc` (RFC-0113 Phase 4 Rust stdlib; 21 TDD tests). PR #799 closed (superseded).
- Issue #800 opened (Rust extractor qualified-path enhancement — Codex P2 spin-off).
- Escalation ×70→×72.

### v205/v206 (2026-06-12)
- PR #796 merged `b052bcc` (RFC-0113 Phase 3b — Go qualified-call fix; Pass 1b-go alias table; 4 TDD tests).
- Issue #795 closed. PR #797 merged `763fe66` (PM v205 chore). Escalation ×69→×70.

### v204 (2026-06-12)
- PR #793 merged `3b46ba2` (RFC-0113 Phase 3 Go — classify_go + GO_BUILTINS 17 entries + GO_STDLIB_PKG_NAMES 80+ entries; 11 TDD tests).
- Codex P1 spun off: issue #795 (Phase 3b Go qualified calls). Escalation ×68→×69.

### v203 (2026-06-12)
- CRITICAL ASSESSMENT CORRECTION: RFC-0113 Phase 3 Go was always unblocked (not blocked on #568 ceremony).
- PR #792 merged `22da0e3` (PM v202 chore). PR #793 opened (RFC-0113 Phase 3 Go). Escalation ×67→×68.

### v202 (2026-06-12)
- No autonomous engineering tasks. Both P0s founder-gated. Escalation ×66→×67.

### v201 (2026-06-12)
- PR #790 merged `6b68fa77` (PM v200 chore; Codex P2 fixed: pre-flight SHA label corrected). Escalation ×65→×66.

### v200 (2026-06-12)
- PR #789 merged `8a2c5e2a` (PM v199 chore; Codex P2 fixed + outdated). PM state v200 written. Escalation ×64→×65.

### v199 (2026-06-12)
- PR #788 merged `574ab2b7` (PM v198 chore; Codex P2 addressed — stale SHA, fix commit 093fe0f). Escalation ×63→×64.

### v198 (2026-06-12)
- PR #787 merged `3586948` (PM v197 chore; 0 Codex findings). Escalation ×62→×63.

### v197 (2026-06-12)
- PR #786 merged `1052fc8a` (PM v196 chore; 0 Codex findings). Escalation ×61→×62.

### v196 (2026-06-11)
- PR #785 merged `8d04aae1` (PM v195 chore; Codex P1 rejected — stale SHA DCO). Escalation ×60→×61.

### v195 (2026-06-11)
- PR #784 merged `46aedccd` (PM v194 chore; 0 Codex findings). Escalation ×59→×60.

### v194 (2026-06-11)
- PR #783 merged `ce2a341c` (PM v193 chore; Codex P1 rejected — stale SHA DCO). Escalation ×58→×59.

### v193 (2026-06-11)
- PR #782 merged `9a601c1` (PM v192 chore; Codex P1 rejected — stale SHA DCO). Escalation ×57→×58.

### v192 (2026-06-11)
- PR #781 merged `af889a1` (PM v191 chore; Codex P1+P2 rejected). PR #780 closed (CI anomaly, superseded). Escalation ×56→×57.

### v191 (2026-06-11)
- PR #780 had CI anomaly (only Triage ran; CI/E2E not triggered). PR #781 opened on separate branch. Escalation ×55→×56.

### v190 (2026-06-11)
- PR #779 opened (chore/pm-state-v189). Escalation ×54→×55.

### v189 (2026-06-11)
- PR #778 merged `f948cef` (PM v188 chore; Codex P1 rejected — stale SHA DCO). Escalation ×53→×54.

### v188 (2026-06-11)
- RFC-0113 Phase 2 TS classifier wired into callees_payload (Codex P1 bug fix: TS/JS callers were using Python classifier). `isInteger` removed from TS_GLOBAL_BUILTINS. 4 TDD tests. PR #776 fix commit `9de6484`.
- PR #777 closed (superseded by v188). Escalation ×52→×53.

### v187 (2026-06-11)
- RFC-0113 Phase 2 TS implemented: `classify_typescript` + `TS_BROWSER_APIS` + `TS_GLOBAL_BUILTINS` (fetch/console/etc.) tables. PR #776 opened.
- Escalation ×51→×52.

### v186 (2026-06-11)
- PR #774 merged `c3b03603` (PM v185 chore; 22/22 CI ✅). Escalation ×50→×51.

### v185 (2026-06-11)
- PR #773 merged `84c72d55` (PM v184 chore; 22/22 CI ✅). Escalation ×49→×50.

### v184 (2026-06-11)
- PR #772 merged `ee8332d` (PM v183 chore; 22/22 CI ✅). Codex P0 #3 resolved (confirmed active). Escalation ×48→×49.

### v183 (2026-06-11)
- PR #771 merged `ea20c12` (PM v182 chore; Codex P2 rejected). Codex posted live P2 (billing restored signal). Escalation ×47→×48.

### v182 (2026-06-11)
- PR #770 merged `d9aa509` (PM v181 chore; 20/20 CI ✅). 97/97 Three-Surface confirmed on develop. Escalation ×46→×47.

### v181 (2026-06-11)
- PR #769 merged `ee29ef4` (PM v180 chore; 22/22 CI ✅). Escalation ×44→×45.

### v180 (2026-06-11)
- Issue #766 manually closed (GitHub auto-close failed after squash-merge to non-default branch).
- Escalation ×43→×44.

### v179 (2026-06-11)
- PR #765 merged `c9836688` (PM v178 chore; 20/20 CI ✅). PR #767 opened (fix: per-class SLA thresholds). Escalation ×43.

### v178 (2026-06-11)
- ERROR CORRECTION: `bpe_charter_sla_binding` IS a real test (not phantom). Codex P2 VALID. Issue #766 opened. Anti-pattern recorded (stale-local-search).

### v177 (2026-06-11)
- PR #762 merged `7b062c8` (PM v176 chore; Codex P2 rejected — archive append-only). DRAFT PR #763 opened (RFC-0121 Option A Charter §2 amendment). Escalation ×41→×42.

### v176 (2026-06-11)
- No autonomous code tasks. All paths founder-gated. Escalation ×40→×41.

### v175 (2026-06-11)
- PR #760 merged `45fd3c6` (PM v174 chore; 20/20 CI ✅). PR #568 CI: 50/50 SUCCESS/SKIPPED — Charter §5.12 gate fully MET. Escalation ×39→×40.

### v174 (2026-06-11)
- Fixed PR #568 dirty state: git merge -X ours origin/main + removed duplicate `build-cli-binaries` CI job (merge artifact). Commits `4d03f3b` + `351e4b5` pushed. Escalation ×38→×39.

### v173 (2026-06-11)
- PR #757 merged (RFC-0117 Phase 2 check-architecture; merge conflict resolved; commit `a7000d1`). 97/97 Three-Surface ✅. Develop HEAD `98636e0`. Escalation ×37→×38.

### v172 (2026-06-10)
- RFC-0117 Phase 2 implemented: YAML loader + Store edge adapter + CLI `check-architecture` + `mycelium_check_architecture` MCP + graph-structure Skill. EXPECTED_TOOL_COUNT 96→97. PR #757 opened. Escalation ×36→×37.

### v171 (2026-06-10)
- PR #755 closed (conflict). RFC-0117 + RFC-0119 status corrected Draft→Partially Implemented. RFC-0117 Phase 2 queued as next P1. Escalation ×35→×36.

### v170 (2026-06-10)
- RFC-0124 (Hyphae selector ordering relaxed) implemented: PR #754 merged `56bc4b7`. RFC-0113 Phase 2 Python implemented. Pack spans fixed for 5 more language packs (PR #751). Budget threaded through query/cross-refs/tree tools (PR #752). Escalation ×34→×35.

### v169 (2026-06-10)
- ADR-0013: callee tree collapses unresolved leaves into `unresolved_callees` count. PR #748 merged. Escalation ×33→×34.

### v168 (2026-06-10)
- RFC-0115 Phase 2 test-gap surface implemented. PR #743 opened. RFC-0116 Phase 2 confirmed on develop. Escalation ×32→×33.

### v167 (2026-06-10)
- RFC-0115 Phase 2 implemented (test-gap CLI + MCP + Skill; 96/96 Three-Surface). PR #743 opened. Escalation ×31→×32.

### v166 (2026-06-10)
- PR #741 merged `231a819` (PM v165 chore). RFC-0116 Phase 2 confirmed on develop (`500a2a1`). RFC-0115 Phase 2 next P1. PR #742 opened. Escalation ×30→×31.

### v165 (2026-06-10)
- RFC-0116 Phase 2 implemented: `safe_to_edit_payload()` + CLI `safe-to-edit` + `mycelium_safe_to_edit` MCP + skills/reachability + INDEX.md Phase 3.3 (95/95). PR #740 opened. Escalation ×29→×30.

### v164 (2026-06-10)
- RFC-0115/0116 status corrected Draft→Partially Implemented. RFC-0116 Phase 2 unblocked. PR #738 merged. Escalation ×28→×29.

### v163 (2026-06-10)
- PR #737 merged `7da70b5` (PM v162 chore). Escalation ×27→×28.

### v162 (2026-06-10)
- PR #736 merged `8d0fc17` (PM v161 chore). RFC audit: RFC-0113 PartiallyImplemented, RFC-0114 Implemented, RFC-0115/0116/0117/0119 Draft. P1 backlog: RFC-0113 Phase 2 added. Escalation ×26→×27.

### v161 (2026-06-10)
- PR #735 merged `3b6d192` (PM v160 chore). RFC-0121 Option A confirmed recommendation. Escalation ×25→×26.

### v160 (2026-06-10)
- PR #734 merged `acaddf5` (PM v159 chore). P2 item 9 resolved (release.yml finalize already workflow_dispatch-gated). Escalation ×24→×25.

### v159 (2026-06-10)
- Corrected stale dispatch: RFC-0122 Phase 2b IS Implemented (PR #725). Appended deferred v158 decisions.jsonl entry. Escalation ×23→×24.

### v158 (2026-06-10)
- PR #731 + #732 merged. RFC-0120 COMPLETE: get-token-stats CLI subcommand + EXCEPTION:MCP-only retracted. Three-Surface fully satisfied for get_token_stats. Escalation ×22→×23.

### v157 (2026-06-10)
- RFC-0120 Phase 3B implemented: `token_stats_payload()` shared fn + CLI `Cmd::GetTokenStats` + 4-test byte-identity harness. PR #731 opened. Escalation ×21→×22.

### v156 (2026-06-09)
- PR #728 + #730 merged. RFC-0120 COMPLETE (Phase 3A: MCP `mycelium_get_token_stats` tool wired). All 10 language packs at 94/94 Three-Surface. Escalation ×20→×21.

### v155 (2026-06-09)
- RFC-0114 Phase 2 implemented (project_health CLI + MCP + Skill; 94/94). RFC-0120 Phase 3A planned. PR #726 + #727 opened. Escalation ×19→×20.

### v154 (2026-06-09)
- RFC-0122 Phase 2b implemented (caller_context payload; fix: entry_point field). PR #724 merged. RFC-0114 Phase 2 next P1. Escalation ×18→×19.

### v153 (2026-06-09)
- RFC-0122 Phase 2a implemented (caller_context core + mcp_request). PR #722 + #723 opened. Escalation ×17→×18.

### v152 (2026-06-09)
- RFC-0112 Phase 2 implemented (get_source_span CLI + MCP + Skill; 93/93 Three-Surface). RFC-0122 Phase 2a next P1. Escalation ×16→×17.

### v151 (2026-06-09)
- RFC-0112 Phase 1 implemented (Store::source_span method + 3 TDD tests). RFC-0112 Phase 2 next. Escalation ×15→×16.

### v150 (2026-06-09)
- RFC-0118 Part B implemented (qualified calls `pkg>Func` via @reference.call selector_expression). PR #715 merged. RFC-0112 Phase 1 next P1. Escalation ×14→×15.

### v149 (2026-06-09)
- RFC-0118 Part A implemented (anonymous function calls — @reference.call). PR #713 merged. RFC-0118 Part B next. Escalation ×13→×14.

### v148 (2026-06-09)
- RFC-0118 drafted. RFC-0109 Phase 2b merged (count semantics fix). P1 queue: RFC-0118 Part A (anonymous fn calls). Escalation ×12→×13.

### v147 (2026-06-09)
- RFC-0109 Phase 2a: `get_callee_tree`/`get_caller_tree` budget added (BFS cap). PR #708 merged. Escalation ×11→×12.

### v146 (2026-06-09)
- PR #706 merged (fix/hyphae-kind-validation-and-error-ux). Kind selector validation + human-readable errors. Escalation ×10→×11.

### v145 (2026-06-09)
- PR #704 + #705 merged (RFC-0113 Phase 1 extractor + RFC-0109 Phase 1 query/cross-refs budget). Escalation ×9→×10.

### v144 (2026-06-09)
- RFC-0113 Phase 1 implemented (callee extractor: @reference.call patterns for Python/TS/JS/Ruby). PR #703 opened.
- PR #700 + #701 merged (pack span fixes + tests). Escalation ×8→×9.

### v143 (2026-06-09)
- PR #698 merged (PM v140 chore). RFC-0113 drafted. Escalation ×7→×8.

### v142 (2026-06-08)
- PR #694 + #695 merged (RFC-0109 callee_tree Phase 0 + RFC-0094 Phase 4 token-efficiency). 91/91 Three-Surface. Escalation ×6→×7.

### v141 (2026-06-08)
- PR #691 merged (RFC-0109 Phase 0 callee tree baseline). Three-Surface 91/91 pending #691. Escalation ×5→×6.

### v140 (2026-06-08)
- ADR-0012: real-symbol filter in entry-points/rank (excludes phantom/unresolved stubs). PR #687 + #688 + #689 merged. Escalation ×4→×5.

### v139 (2026-06-08)
- RFC-0116 Phase 1 implemented (SafetyVerdict evaluator: SAFE/CAUTION/REVIEW/UNSAFE). PR #682 + #683 merged. Three-Surface 91/91. Escalation ×3→×4.

### v138 (2026-06-07)
- RFC-0115 Phase 1 implemented (test_gap.rs pure core). RFC-0116 Phase 1 dispatched. PR #679 merged. Escalation ×2→×3.

### v137 (2026-06-07)
- RFC-0120 Phase 1+2 implemented (token stats core + MCP shell). PR #674 + #675 + #676 merged. RFC-0115 next P1. Escalation ×1→×2.

### v136 (2026-06-07)
- PR #568 first appeared as P0 founder-gated (release/v0.3.0 registries published). Develop CI GREEN. Escalation ×1.

---

## Archive — PM dispatches v129–v130 (2026-06-08 to 2026-06-09)

### 2026-06-09 PM dispatch v130 (reconciled from GitHub API)
1. Verified develop HEAD = `8aaef3a6` (PR #672 `redb`→`mycelium-store` rename, squash).
2. Merged PR #697 (squash `d0b3d5f`). ✅
3. Rewrote PM state v130 from scratch (reconciled from GitHub API). ✅

### 2026-06-08 PM dispatch v129 (PRs #690+#693+#696 merged; Codex P1 on #696 rejected)

*(see PR #697 squash commit `d0b3d5f` for full archive)*

### 2026-06-03 PM dispatch v28 and earlier (v1–v128)

*(see closed PRs #502/#506 and git log for historical archives — last pre-v0.2.0 dispatch)*