# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-05-30 (PM dispatch — v0.1.8 latest; #229 attr-assignment alias fix in PR #231 CI; #227 TYPE_CHECKING fix pushed as `fix/227-type-checking-import-guard`) |
| Current sprint | **v0.1.9 planning** |
| Active release branch | none — v0.1.8 shipped |
| Next release target | **v0.1.9** — Python accuracy (PRs #231 + #227 guard) + MCP error model (#209) |
| Final release target | v0.2.0, ETA 2026-07-15 |
| Last shipped | **v0.1.8 — self/cls method resolution** (tag v0.1.8, crates.io published 2026-05-30) |

---

## 🚀 v0.1.8 — SHIPPED ✅

**What shipped:**
- [x] `self.method()` / `cls.method()` inside a class now resolve to sibling methods (#220, from #214 reliability report). Fixes dominant pattern behind 533 false positives in `get-isolated-symbols`.
- [x] `get-dependency-depth` expected to improve for complex Python methods (issue #221 — verify post-v0.1.9).

---

## Live priorities (ordered)

**P0: none** — no blocking issues.

**P1 (action items):**
1. **PR #231 (fix/229-python-attribute-assignment-alias)** — CI in_progress as of last check
   (rustfmt/clippy/DCO/unit tests ✅, integration/matrix/coverage still running).
   Closes #229 (attribute-assignment alias `local = _h.fn; local()` pattern).
   **Merge when Quality Gate passes.**
2. **PR fix/227-type-checking-import-guard** — just pushed. Closes #227 (TYPE_CHECKING
   false-positive cycles — 7 spurious cycle nodes in tree-sitter-analyzer).
   **Create PR on GitHub then merge when CI green.**
   ⚠️ GitHub MCP token expired this session — founder must create PR or re-auth token.
3. **Issue #214** — Comprehensive Python reliability: intra-file calls not tracked (533 false
   positives root cause, partially fixed by v0.1.8 self/cls), destructured imports file-level
   under-count, transitive alias over-count. Needs RFC-0092 Phase 2 scoping.

**P2 (v0.1.9 scope):**
4. **Issue #209** — MCP error model: use `is_error: Some(true)` for application errors.
   Low-effort sweep of ~89 tools. No RFC needed. rust-implementer task.
5. **Issue #221** — `get-dependency-depth` returns 0 for complex methods. Re-verify after
   v0.1.8 self/cls fix — may auto-close.
6. **Security scan** — routine post-v0.1.8 window. security-reviewer task.
7. **Charter §2 SLA** — 100K-node heavy-graph benchmark row. architect task.

**P3 (v0.2.0 backlog):**
8. **Issue #211** — Cross-tool response contract tests.
9. **Issue #206** — Comprehensive enhancement suggestions (token-efficient output, runtime packs).
10. Skill marketplace submission metadata.
11. End-to-end "first 5 minutes" walkthrough / asciinema recording.

---

## Dispatch state (2026-05-30, PM run post-v0.1.8)

| Agent | Status | Current item |
|---|---|---|
| release | **idle** | v0.1.8 shipped. Next: v0.1.9 after PRs #231 + #227-guard merge. |
| pack-author | **in-flight** | PR #231 (Python attr-assignment alias) — CI running, merge when gate passes. |
| rust-implementer | **done-this-run** | Issue #227 fix pushed as `fix/227-type-checking-import-guard`. PR creation blocked by token expiry — needs founder re-auth or manual PR. |
| architect | idle | RFC-0092 Phase 2 scoping (issue #214 patterns). |
| tech-writer | idle | Marketplace metadata + asciinema. |
| code-reviewer | idle | Blocks on PR opens. |
| security-reviewer | **next-up** | Routine post-v0.1.8 scan. |
| e2e-runner | idle | Python fixture tests after PRs #231 + #227-guard merge. |

---

## v0.1.9 Sprint — Draft exit criteria

- [ ] PR #231 merged: Python attribute-assignment alias closes #229.
- [ ] PR fix/227-type-checking-import-guard merged: TYPE_CHECKING false positive cycles closes #227.
- [ ] Issue #209: MCP `is_error` sweep (all ~89 tool error paths).
- [ ] Re-verify issue #221 (`get-dependency-depth` 0 for complex methods) after v0.1.8 self/cls fix.
- [ ] Security scan clean.

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
- **⚠️ GitHub MCP token expired during this session.** Re-auth required to create/merge PRs via automation.
  Branch `fix/227-type-checking-import-guard` is pushed; PR at:
  `https://github.com/aimasteracc/mycelium/pull/new/fix/227-type-checking-import-guard`

---

## Archive

### 2026-05-30 PM run (v0.1.8 era — #229 attr-alias + #227 TYPE_CHECKING)

- Discovered latest release is v0.1.8 (self/cls method resolution), not v0.1.7 as PM state showed. State refreshed.
- PR #231 (fix/229 attribute-assignment alias pattern) in CI — rustfmt/clippy/DCO/unit ✅.
- Issue #227 (TYPE_CHECKING false positive cycles): implemented fix this run.
  - TDD: 2 RED→GREEN tests in extractor/tests.rs
  - Rust helper `is_inside_type_checking_block()` in extractor/mod.rs
  - CHANGELOG Unreleased entry (merged with #229 entry, no conflict)
  - Pushed branch `fix/227-type-checking-import-guard`
  - PR creation blocked by GitHub MCP token expiry — escalated to founder.
- Open issues after this run: #229 (in PR), #227 (in PR, needs create), #214 (scoping needed), #209 (P2), #221 (verify), #211 (P3), #206 (P3).

### 2026-05-30 PM run (post-v0.1.6 — RFC-0092 + alias fix kickoff)

- v0.1.6 confirmed fully shipped: tag v0.1.6 exists, crates/npm/PyPI published.
- Discovered stale `decisions.jsonl` merge conflict (lines 143-149); fixed in PR #218.
- Closed stale PR #215 (superseded by #213, develop already had v0.1.6 content).
- Merged PR #216 (RFC-0092 cross-language alias resolution draft).
- Found PR #217 (Python alias dispatch #205) already in flight with full TDD implementation, CI running.
- Triaged new issues: #214 (P1 Python reliability), #209 (P2 MCP error), #210 (P2 token output), #211 (P3 contract tests), #212 (P3 runtime packs).
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
