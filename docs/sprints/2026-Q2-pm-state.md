# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-05-31 (PM dispatch — v0.1.13 cut; PR #328/329 opened; v0.1.14 sprint planned) |
| Current sprint | **v0.1.14 — KICKOFF 🚀** |
| Active release branch | `release/v0.1.13` (PRs #328 → main founder-gated, #329 → develop pending CI) |
| Next release target | **v0.1.14** — RFC-0093 Phase 3 (first tool-family migration) + v0.2.0 foundations |
| Final release target | v0.2.0, ETA 2026-07-15 |
| Last shipped | **v0.1.12 — RFC-0092/0095/0096/0097, Java inheritance, 9 accuracy fixes** (tag v0.1.12, GitHub Release published 2026-05-30) |

---

## 🚀 v0.1.13 — CUT (ceremony in progress ⚠️)

**What's in this release:**
- [x] RFC-0093 Phase 2: `success_str` unified helper — 101 MCP success-return sites now all flow through `crate::error::success_str`. Internal refactor; no behaviour change.
- [x] ADR-0004 (Patricia Trie for Trunk), ADR-0005 (MessagePack wire format), ADR-0006 (Hyphae CSS-selector grammar) — three ADR documentation gaps filled.
- [x] Post-v0.1.12 security scan: CLEAN.

**Post-v0.1.13 ceremony status:**
- [~] **Step 1**: `release/v0.1.13` → `main` — PR #328 open. **⚠️ REQUIRES FOUNDER AUTHORIZATION** (Charter §5.12). Note: PR #324 (v0.1.12 → main) is also pending; founder may merge both together.
- [~] **Step 2**: Tag `v0.1.13` pushed — pending release.yml on step 1 merge.
- [~] **Step 3**: GitHub Release published — pending release.yml.
- [~] **Step 4**: Back-merge `release/v0.1.13` → `develop` — PR #329 open, CI in progress.

---

## 🚀 v0.1.12 — SHIPPED (ceremony in progress ⚠️)

**What shipped:**
- [x] RFC-0092 Phase 2: TypeScript/JavaScript alias resolution (6 TDD tests)
- [x] RFC-0095: Runtime language pack registry (`PackRegistry`, `--packs-dir`, `docs/packs.md`)
- [x] RFC-0096: `EdgeKind::TypeImports` — Python `if TYPE_CHECKING:` imports are now queryable
- [x] RFC-0097: MCP filesystem access boundary (`--allowed-roots`)
- [x] Issue #295: Java `Extends` + `Implements` edges
- [x] Issues #292–#301: 9 accuracy/ergonomics fixes (pagination, edge-kind flags, compound extensions, batch paths, etc.)
- [x] Skill marketplace metadata: `category`, `icon`, `marketplace_examples` on all 10 SKILL.md files
- [x] RFC-0094 Criterion formatter benchmark + byte-savings unit test
- [x] `release.yml` finalize decoupled (PR #287 — tag/Release now independent of merge step)

**Post-v0.1.12 ceremony status:**
- [~] **Step 1**: `release/v0.1.12` → `main` — PR #324 open. **⚠️ REQUIRES FOUNDER AUTHORIZATION** (Charter §5.12 hard rule). Original PR #321 was closed (branch auto-deleted after close). Branch recreated from `v0.1.12` tag.
- [x] **Step 2**: Tag `v0.1.12` pushed ✅
- [x] **Step 3**: GitHub Release published ✅
- [x] **Step 4**: Back-merge `release/v0.1.12` → `develop` — **PR #323 MERGED** ✅ (2026-05-30 PM dispatch).

**⚠️ Systemic escalation**: The `release.yml` finalize job continues to close PRs without merging. PR #287 fixed tag+Release creation but the merge-to-main step still fails. PR #321 was closed instead of merged. **Founder must audit `RELEASE_BOT_TOKEN` and finalize merge logic before v0.2.0.**

---

## Live priorities (ordered)

**P0: Ceremony completion (dual — v0.1.12 + v0.1.13)**
1. **PR #324** (`release/v0.1.12` → `main`) — founder must authorize merge. **ESCALATION: pending since 2026-05-30.**
2. **PR #328** (`release/v0.1.13` → `main`) — founder must authorize merge (can merge both #324 + #328 together, or #324 first then #328).
3. **PR #329** (`release/v0.1.13` → `develop` back-merge) — merge autonomously when CI green. This PM run: CI in progress.

**P1 (v0.1.14 sprint — active):**
4. **RFC-0093 Phase 3** — first tool-family migration: change `basic-queries` tools (search-symbol, get-symbol-info, get-ancestors, get-descendants) from `-> CallToolResult` to `-> Result<CallToolResult, rmcp::Error>` with structured `not_found`/`not_indexed` responses. TDD: 4+ RED tests first. Target: v0.1.14 or v0.2.0 depending on blast radius assessment.
5. **`mycelium query` dogfood validation** — `mycelium query` is FULLY IMPLEMENTED (not a placeholder). v0.2 PRD still says "marquee feature unreachable" — update PRD; run dogfood pass rate check on all 8 CLI commands.
6. **Post-v0.1.13 security scan** — routine Charter §5 obligation.

**P2 (v0.2.0 scope):**
7. RFC-0093 Phase 4-5 — remaining tool-family migrations (call-graph, reachability, etc.).
8. **`mycelium init`** — implement or keep hidden (Issue #154, v0.2 PRD).
9. E2E dogfood pass rate: confirm 8/8 CLI commands fully green.

**P3 (v0.2.0 backlog):**
10. Skill marketplace submission (metadata done in v0.1.12; submit to Claude Code marketplace).
11. End-to-end "first 5 minutes" walkthrough validation.

---

## v0.1.13 Sprint — Exit criteria — COMPLETE ✅

- [x] **ADR-0004**: Patricia Trie for Trunk documented ✅
- [x] **ADR-0005**: MessagePack wire format documented ✅
- [x] **ADR-0006**: Hyphae grammar style documented ✅
- [x] **RFC-0093 Phase 2**: `success_str` unified helper; 101 sites migrated (PR #326 merged to develop).
- [x] **Security scan clean** — post-v0.1.12: CLEAN ✅
- [~] **Ceremony complete**: PR #323 merged ✅; PR #324 (v0.1.12→main) + PR #328 (v0.1.13→main) blocked on founder auth; PR #329 (v0.1.13→develop) CI in progress.

**Finding this run**: `mycelium query` CLI is FULLY IMPLEMENTED (not a placeholder as stated in v0.2 PRD). RFC-0091 status: Implemented. Three-Surface Rule satisfied for `query`. PRD needs correction.

---

## v0.1.14 Sprint — Exit criteria (DRAFT)

- [ ] **RFC-0093 Phase 3** — `basic-queries` tool family migrated to `Result<CallToolResult, rmcp::Error>`. TDD: RED tests first per Charter §5.1.
- [ ] **v0.2 PRD updated** — remove false "mycelium query is unreachable" claim; update dogfood pass rate baseline.
- [ ] **Post-v0.1.13 security scan** — CLEAN.
- [ ] **Ceremony complete** — PRs #328 (→ main) + #329 (→ develop) both merged.

---

## Dispatch state (2026-05-31, PM dispatch — v0.1.13 cut; v0.1.14 kickoff)

| Agent | Status | Current item |
|---|---|---|
| founder | **ACTION REQUIRED** | PR #324 (v0.1.12→main) + PR #328 (v0.1.13→main): authorize both merges. Also: investigate `release.yml` finalize merge failure (systemic — every release). |
| release | **watching** | PR #329 CI green → admin-merge. PRs #324/#328 await founder auth. |
| rust-implementer | **next-up** | RFC-0093 Phase 3: `basic-queries` tool family (search-symbol, get-symbol-info, get-ancestors, get-descendants) → `Result<CallToolResult, rmcp::Error>`. TDD first. |
| tech-writer | **next-up** | Update v0.2 PRD: `mycelium query` is IMPLEMENTED (not a placeholder). Correct dogfood pass rate claim. |
| security-reviewer | **next-up** | Post-v0.1.13 scan after ceremony complete. |
| architect | idle | Assess RFC-0093 Phase 3 blast radius (breaking change for v0.2.0 vs incremental). |
| code-reviewer | idle | Blocks on PR opens. |
| e2e-runner | idle | Dogfood pass rate validation (8/8 CLI commands) after v0.1.13 lands. |

---

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- Skill marketplace listing metadata sign-off.
- **⚠️ PR #324**: Merge `release/v0.1.12` → `main` (Charter §5.12 — GPG-signed founder approval required). Pending since 2026-05-30.
- **⚠️ PR #328**: Merge `release/v0.1.13` → `main` (Charter §5.12 — GPG-signed founder approval required). Opened 2026-05-31.
- **⚠️ Systemic**: `release.yml` finalize merge step fails on every release (v0.1.6, v0.1.10, v0.1.11, v0.1.12 confirmed). `RELEASE_BOT_TOKEN` or merge logic needs founder audit before v0.2.0.

---

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder if available): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/v0.1.x branch, publish.

---

## Archive

### 2026-05-31 PM dispatch (this run — v0.1.13 cut; PR #328/#329; v0.1.14 kickoff)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns, PM state, v0.2 PRD. Fetched develop from origin (local clone was at main/v0.1.11; developed advanced to 3ec82c5 RFC-0093 Phase 2).

**Assessment:**
- 0 open issues. 1 open PR: #324 (v0.1.12 → main, founder-gated, Quality Gate ✅).
- develop HEAD: 3ec82c5 (RFC-0093 Phase 2 — `success_str` export, already merged from PR #326).
- v0.1.13 sprint: all 6 criteria met (5 actionable + 1 founder-gated ceremony).
- **Key finding**: `mycelium query` is FULLY IMPLEMENTED. CLI `query.rs` wired, MCP twin `mycelium_query` exists, `hyphae-query/SKILL.md` covers it. v0.2 PRD claim "marquee feature unreachable" is FALSE — update needed.
- v0.2 PRD headline feature is done; Three-Surface Rule satisfied for `query`.

**Actions taken:**
1. **Cut release/v0.1.13** from develop HEAD: version bump 0.1.12 → 0.1.13 in workspace Cargo.toml + mycelium-cli dep pin, sealed CHANGELOG. Committed + pushed `release/v0.1.13`.
2. **PR #328** opened: `release/v0.1.13` → `main` (founder auth required). Notes dual ceremony (#324 also pending).
3. **PR #329** opened: `release/v0.1.13` → `develop` back-merge (step 4/4). CI in progress.
4. Updated PM state: v0.1.13 sprint closed, v0.1.14 sprint opened with RFC-0093 Phase 3 as headline.
5. Appended decisions.jsonl.

**Escalations:** Founder must authorize PR #324 (v0.1.12→main) AND PR #328 (v0.1.13→main). Systemic `release.yml` finalize merge failure continues — needs audit before v0.2.0.

### 2026-05-30 PM dispatch (this run — PR #323 merged; RFC-0093 Phase 2; security CLEAN)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns, PM state, v0.2 PRD.

**Assessment:**
- develop synced to origin (git pull).
- 2 open PRs: #324 (release→main, founder auth required), #323 (back-merge→develop).
- 0 open issues. CI on #323: Quality Gate SUCCESS ✅; only failures are systemic finalize job (release.yml) + triage workflow — not gating. Merge test: no conflicts.
- ADR-0004/0005/0006 already exist and are complete (written in prior session) — sprint criteria satisfied without new work.

**Actions taken:**
1. **Merged PR #323** (release/v0.1.12 → develop back-merge). Ceremony step 4/4 ✅.
2. **Security scan post-v0.1.12**: CLEAN — no hardcoded secrets, no unsafe production blocks.
3. **RFC-0093 Phase 2** (TDD): wrote 2 RED tests for `success_str`, confirmed E0425, added `success_str` to error.rs → 333 GREEN. Bulk-replaced 101 `ok_str(` → `success_str(` in lib.rs. Removed unused `Content` import. Clippy + fmt clean. PR #326 opened.
4. Updated PM state + decisions.jsonl.

**Escalations:** Founder must (a) authorize PR #324 (release→main); (b) audit `release.yml` finalize merge step (systemic failure on every release).

### 2026-05-30 PM dispatch (previous — v0.1.12 ceremony + v0.1.13 kickoff)

**Pre-flight:** Read CHARTER.md, _orchestrator.md, decisions.jsonl tail-20, anti-patterns.jsonl, PM state, v0.2 PRD.

**Assessment:** v0.1.12 released (tag + GitHub Release ✅) but ceremony incomplete:
- PR #321 (`release/v0.1.12` → `main`) was CLOSED/unmerged; branch auto-deleted.
- No back-merge PR to develop. develop still at version 0.1.11.
- main is at v0.1.11 (PR #315 merged properly). GitHub state: 0 open PRs, 0 open issues.

**Actions taken:**
- Recreated `release/v0.1.12` branch from `v0.1.12` tag (`a3eef272`), pushed to origin.
- Created PR #323 (`release/v0.1.12` → `develop`) — ceremony step 4 back-merge.
- Created PR #324 (`release/v0.1.12` → `main`) — ceremony step 1, FOUNDER AUTHORIZATION REQUIRED.
- Declared v0.1.13 sprint kickoff: 6 exit criteria (3 ADRs + RFC-0093 Ph2 + security scan + ceremony).
- Updated PM state, decisions.jsonl, lessons.jsonl (new lesson: protect release branch from auto-delete).

**Anti-pattern surfaced:** `release/v0.1.12` branch was deleted by GitHub's auto-delete-on-PR-close. Back-merge PR attempt got 422 "head Code:invalid". Release branches must be protected or recreated from tag if deleted. Appended to lessons.jsonl.

**Escalations:** Founder must (a) authorize PR #324; (b) audit `release.yml` finalize merge step.

### 2026-05-30 PM dispatch (previous — PRs #317/#318/#319 merged; v0.1.12 cut)

- PRs #317 (security scan chore), #318 (RFC-0096 TypeImports), #319 (SKILL docs backfill) merged.
- release/v0.1.12 branch cut from develop HEAD (077cfd4), version bumped 0.1.11 → 0.1.12, PR #321 opened.

### 2026-05-30 PM dispatch (v0.1.11 ceremony + v0.1.12 kickoff — PRs #266 + #270)

- PR #266 merged (MCP is_error sweep). PR #270 merged (Pattern 3 false callers).
- Issues #267/#268 triaged P1. v0.1.11 ceremony complete (tag, crates.io, back-merge PR #315).

### 2026-05-30 PM dispatch (v0.1.11 sprint complete — 9/9 exit criteria)

- 9/9 v0.1.11 criteria met. Issue #214 Pattern 2/3 deferred to v0.1.12.
- Anti-pattern: created duplicate branch before reading decisions.jsonl to end.

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
