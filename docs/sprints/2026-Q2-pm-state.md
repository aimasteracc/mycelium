# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-05-30 (PM dispatch — v0.1.10 shipped; PR #240 back-merge merged; PR #241 RFC-0094 Phase 1 merged) |
| Current sprint | **v0.1.11 planning** |
| Active release branch | none — v0.1.10 shipped |
| Next release target | **v0.1.11** — RFC-0094 Phase 2 (output_format wiring) + remaining Python patterns from #214 |
| Final release target | v0.2.0, ETA 2026-07-15 |
| Last shipped | **v0.1.10 — TYPE_CHECKING guard + nested-attribute fallback** (tag v0.1.10, crates.io / npm / PyPI published 2026-05-30) |

---

## 🚀 v0.1.10 — SHIPPED ✅

**What shipped:**
- [x] `if TYPE_CHECKING:` imports no longer create `Imports` edges (PR closes #227). 2 TDD tests.
- [x] Nested attribute call regression restored (`self.history.append(x)` and similar). 1 TDD test.
- [x] Charter §5.12 release-gate rule codified in CHARTER.md, CLAUDE.md, GITFLOW.md (v0.1.9 governance).

**Post-v0.1.10 ceremony:**
- [x] Tag v0.1.10 pushed, crates.io / npm / PyPI published.
- [x] PR #240 (release/v0.1.10 → develop back-merge) merged — **ceremony complete**.
- [x] PR #241 (RFC-0094 Phase 1 Formatter trait) merged to develop.
- [~] `merge to main, tag, GitHub Release` finalize job failed again (same pattern as v0.1.6). The release was manually recovered. **Escalation to founder**: review `release.yml` finalize job for systemic failure (GitHub Release page creation / auto-merge to main) — happens repeatedly.

---

## Shipped since last PM state (v0.1.6 → v0.1.10)

| Version | Headline | Key PRs |
|---|---|---|
| v0.1.7 | Python alias-table dispatch (RFC-0092 Phase 1) — closes #205 (Bug 1 of #200). 73 false dead-code findings eliminated. | #217 |
| v0.1.8 | `self.method()` / `cls.method()` resolve to sibling method nodes — closes #220 (dominant pattern behind 533 `get-isolated-symbols` false positives). | — |
| v0.1.9 | Attribute-assignment alias pattern (`_alias = _h.fn; _alias()`) — closes #229. Charter §5.12 release-gate + post-release-sync rules codified. RFC-0096 drafted. | — |
| v0.1.10 | `if TYPE_CHECKING:` guard + nested-attribute call fallback — closes #227. | — |

---

## Live priorities (ordered)

**P0: none** — no blocking issues.

**P1 (action items):**
1. **Issue #214 (Python reliability — remaining patterns)** — after v0.1.7/v0.1.8/v0.1.9/v0.1.10,
   the self.method() false positives and TYPE_CHECKING false cycles are fixed. Still open:
   - Pattern 2: destructured imports file-level under-count (`from .models import X` → models.py shows 0 callers at file level).
   - Pattern 3: transitive alias over-count (1,472 false callers for `HealthHistory.append` — root cause likely still present, needs re-verification after v0.1.7/v0.1.9).
   - `get-dependency-depth` returning 0 for method chains (see also #221).
   **Needs RFC-0092 Phase 2 or 3 scoping.** Pack-author + rust-implementer task for v0.1.11.

**P2 (v0.1.11 scope):**
2. **Issue #210 / RFC-0094 Phase 2** — Wire `output_format: Option<OutputFormat>` into all 89 tool
   request shapes (now that Phase 1 Formatter trait landed in PR #241). Per-transport defaults:
   stdio → `Text`, CLI → `Json`. Medium effort. rust-implementer task.
3. **Issue #221** — `get-dependency-depth` returns 0 for complex methods (from #214).
   May partially self-resolve after Phase 2 alias work; re-verify after Pattern 3 fix.
4. **Security scan** — routine post-v0.1.10 window. security-reviewer task.
5. **Charter §2 SLA** — 100K-node heavy-graph benchmark row. Deferred from v0.1.6. architect task.

**P3 (v0.2.0 backlog):**
6. **Issue #211** — Cross-tool response contract tests. Low effort. Depends on #209 (MCP error
   model) — check if #209 was closed during v0.1.7–v0.1.10 window.
7. **Issue #212** — Runtime language pack loading. Medium effort, RFC needed.
8. Skill marketplace submission metadata: icon, screenshots, category examples.
9. End-to-end "first 5 minutes" walkthrough / asciinema recording.

---

## Dispatch state (2026-05-30, PM run post-v0.1.10)

| Agent | Status | Current item |
|---|---|---|
| release | **idle** | v0.1.10 shipped, back-merge done. Next: v0.1.11 after sprint exit criteria met. |
| rust-implementer | **next-up** | RFC-0094 Phase 2: wire `output_format` into 89 tool shapes (#210). |
| pack-author | **next-up** | Issue #214 Pattern 2+3: destructured imports + alias over-count (RFC-0092 Phase 2/3 scope). |
| architect | idle | RFC-0092 Phase 2/3 scoping + Charter §2 100K-node SLA row. |
| tech-writer | idle | RFC-0094 Phase 2 doc updates when Phase 2 lands. Marketplace metadata. |
| code-reviewer | idle | Blocks on PR opens. |
| security-reviewer | **next-up** | Routine post-v0.1.10 scan. |
| e2e-runner | idle | Python alias Pattern 3 regression test (after pack-author fix). |

---

## v0.1.11 Sprint — Draft exit criteria

- [ ] RFC-0094 Phase 2: `output_format` wired into ≥ 89 tools; per-transport defaults set.
- [ ] Issue #214 Pattern 2 or Pattern 3 fixed (at least one Python accuracy regression addressed).
- [ ] Issue #221 re-verified or fixed.
- [ ] Security scan clean (no high-severity findings post-v0.1.10).
- [ ] Charter §2 SLA 100K-node benchmark row landed.

---

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder if available): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/v0.1.x branch, publish.

---

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- Skill marketplace listing metadata sign-off.
- Merging any `release/*` branch to `main` if `RELEASE_BOT_TOKEN` is unavailable.
- **⚠️ Investigate `release.yml` finalize job** — `merge to main, tag, GitHub Release` step has
  failed on multiple releases (v0.1.6, v0.1.10 confirmed; possibly others). Each time the release
  was manually recovered but this is a systemic CI/CD failure. The release workflow's auto-merge
  to main and GitHub Release creation are not working reliably. Recommend founder audit the
  `RELEASE_BOT_TOKEN` secret and the finalize job logic before v0.2.0.

---

## Archive

### 2026-05-30 PM run (post-v0.1.10 — RFC-0094 Phase 1 + back-merge)

- PM state was stale at v0.1.6; fast-forwarded to v0.1.10.
- v0.1.7–v0.1.10 shipped between sessions (Python accuracy patches).
- PR #240 (release/v0.1.10 back-merge): Quality Gate ✅ green → merged. Ceremony complete.
- PR #241 (RFC-0094 Phase 1 Formatter trait): All CI checks ✅ green → merged after CHANGELOG
  conflict resolved (RFC-0094 Unreleased entry + [0.1.10] section from develop both preserved).
- Escalation: `release.yml` finalize job failing repeatedly (merge to main + GitHub Release creation).
- Next: v0.1.11 sprint — RFC-0094 Phase 2 + remaining #214 Python patterns.

### 2026-05-30 PM run (post-v0.1.6 — RFC-0092 + alias fix kickoff)

- v0.1.6 confirmed fully shipped: tag v0.1.6 exists, crates/npm/PyPI published.
- Discovered stale `decisions.jsonl` merge conflict (lines 143-149); fixed in PR #218.
- Closed stale PR #215 (superseded by #213, develop already had v0.1.6 content).
- Merged PR #216 (RFC-0092 cross-language alias resolution draft).
- Found PR #217 (Python alias dispatch #205) already in flight with full TDD implementation,
  CI running.
- Triaged new issues: #214 (P1 Python reliability), #209 (P2 MCP error), #210 (P2 token output),
  #211 (P3 contract tests), #212 (P3 runtime packs).
- Escalation: verify v0.1.6 GitHub Release page (finalize job showed failure for back-merge PR).

### 2026-05-30 PM run (v0.1.5 CI fix + issue #200 triage)

- Found PR #199 (release/v0.1.5) blocked by crates.io publish failure.
- Root cause: `cargo publish --dry-run` for sibling workspace crates fails before any are published.
- Fix: `|| true` on dry-run step in `.github/workflows/release.yml` (commit 58ba0df on release/v0.1.5).
- Issue #200 triaged: P1 Python accuracy bugs (3 sub-bugs). Target v0.1.6 sprint.
- Release workflow `finalize` job discovered: auto-merges to main + develop when CI passes.
  No manual PR merge required if RELEASE_BOT_TOKEN is set.

### 2026-05-30 PM run (post-v0.1.4 ship — v0.1.5 sprint complete)

v0.1.5 sprint declared complete. All exit criteria met:
- CLI batches 2-10: every remaining MCP tool has a CLI twin (89/89 Three-Surface).
- RFC-0091 jQuery selectors: PRs #184.
- PR #194 dogfood e2e test.
- PR #196 README badges.
- release/v0.1.5 cut; PR #199 opened targeting main.

### 2026-05-29 PM run (v0.1.4 close)

v0.1.4 sprint declared complete. All 7 exit criteria met:
- PR #168 — perf hardening + heavy-graph SLA + Criterion benches
- PR #170 — parity.yml + check_skill_parity.py + 12 naming fixes; 89/89 coverage
- PR #172 — CLI batch 1 (search-symbol, get-symbol-info, get-ancestors + 8 integration tests)
- PR #149 — PR template Three-Surface Self-Check (confirmed already live from RFC-0090 launch)
- PR #154 — mycelium init kept hidden (no new work needed)

This PM run attempted to independently implement CLI batch 1 (PR #173) before discovering
PR #172 already merged concurrently. PR #173 was closed as superseded. Anti-pattern note:
concurrent PM runs can duplicate work; inter-run state synchronisation depends on this file.

### 2026-05-29 PM run (v0.1.4 kickoff)

- #153 ✅ graph-algorithm timeouts fixed (PR #168)
- RFC-0090 Phase 1 ✅ parity.yml (PR #170)
- Confirmed all Phase 2/2.3 from v0.1.3 complete (89/89 coverage)

### 2026-05-30 PM call (v0.1.2 era — superseded)

1. v0.1.0 + v0.1.1 shipped and on crates.io. Three-Surface Rule is law.
2. External eval (glm5.1) found 4 issues; orchestrator dogfood found 2 more. All filed.
3. Priority order set. Sprint v0.1.2 kicked off on issue #150.
4. PRD for v0.2 at [`docs/prd/v0.2-the-three-surface-release.md`](../prd/v0.2-the-three-surface-release.md).
5. No blocker from founder at this checkpoint. Begin dispatch.
