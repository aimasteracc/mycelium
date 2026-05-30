# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-05-30 (PM dispatch — v0.1.10 shipped; PM state re-synced) |
| Current sprint | **v0.1.11 planning** |
| Active release branch | none — v0.1.10 shipped |
| Next release target | **v0.1.11** — remaining Python reliability (#214) + MCP error model (#206) |
| Final release target | v0.2.0, ETA 2026-07-15 |
| Last shipped | **v0.1.10 — Python TYPE_CHECKING + nested attribute calls** (tag v0.1.10, crates.io published 2026-05-30) |

---

## 🚀 v0.1.7 — v0.1.10 SHIPPED ✅

**What shipped (v0.1.7 through v0.1.10):**
- [x] Python module-alias dispatch (`from . import mod as alias; alias.fn()`) — PR #217 → v0.1.7.
- [x] `self.method()` / `cls.method()` inside a class resolves to sibling method — v0.1.8. Closes the dominant pattern behind 533 `get-isolated-symbols` false positives (#214).
- [x] Python attribute-assignment alias pattern (`_alias = _h.fn; _alias()`) — v0.1.9.
- [x] RFC-0095 runtime language pack loading (spec only) — v0.1.8.
- [x] Charter §5.12 release-gate rule codified — v0.1.9.
- [x] RFC-0096 type-only import edge kind drafted — v0.1.9.
- [x] Python `if TYPE_CHECKING:` imports skipped — v0.1.10 (#227).
- [x] Nested attribute call regression fixed (post-RFC-0092) — v0.1.10 (#238).
- [x] decisions.jsonl merge conflict fixed — PR #218.
- [x] RFC-0092 cross-language alias resolution merged — PR #216.

**Post-v0.1.6 gap items still open:**
- [ ] Issue #206 Suggestion 1: MCP `is_error: Some(true)` sweep (~89 tools). No RFC needed.
- [ ] Issue #214 Pattern 2: destructured imports file-level under-count (`from .models import X` → `models.py` shows 0 callers). Needs incoming-edge test + extractor fix if still present post-v0.1.6.
- [ ] Issue #214 Pattern 3: transitive alias over-count (1,472 false callers for `HealthHistory.append`). Complex, needs RFC-0092 Phase 2.
- [ ] Issue #214: `get-dependency-depth` returns 0 for Python method chains. Needs RFC-0092 Phase 2.
- [ ] Security scan — routine post-v0.1.10 window.
- [ ] Charter §2 SLA 100K-node benchmark row.

---

## Live priorities (ordered)

**P0: none** — no blocking issues.

**P1 (action items):**
1. **Issue #214** — Python reliability: remaining open patterns from the tree-sitter-analyzer dogfood. Pattern 2 (destructured imports file-level) may already be fixed by v0.1.6; needs verification test. Pattern 3 (transitive alias over-count) and `get-dependency-depth` require RFC-0092 Phase 2 scoping. **Pack-author + rust-implementer task for v0.1.11.**
2. **Issue #206 Suggestion 1** — MCP `is_error` model: set `is_error: Some(true)` on all ~89 application-error returns. Requires either (a) changing tool return type from `String` to `CallToolResult` using rmcp 1.7's `IntoCallToolResult` trait, or (b) post-processing in a `call_tool` override. No RFC needed. **Rust-implementer task. Medium effort (architecture choice first).**

**P2 (v0.1.11 scope):**
3. **Issue #206 Suggestion 3** — Contract tests: verify all tools return non-empty content and use canonical key names. Low effort, high regression-prevention value.
4. **Issue #206 Suggestion 2** — Token-efficient text output format. Medium effort, RFC needed (RFC-0097).
5. **Security scan** — routine post-v0.1.10 window. security-reviewer task.
6. **Charter §2 SLA** — 100K-node heavy-graph benchmark row. architect task.

**P3 (v0.2.0 backlog):**
7. **Issue #206 Suggestion 4** — Runtime language pack loading implementation (RFC-0095 spec exists, implementation deferred).
8. Skill marketplace submission metadata: icon, screenshots, category examples.
9. End-to-end "first 5 minutes" walkthrough / asciinema recording.

---

## Dispatch state (2026-05-30, PM run post-v0.1.10 re-sync)

| Agent | Status | Current item |
|---|---|---|
| release | **idle** | v0.1.10 shipped. Next: v0.1.11 after sprint exit criteria met. |
| pack-author | **next-up** | Issue #214 Pattern 2 verification + fix (if still present). |
| rust-implementer | **next-up** | Issue #206 Suggestion 1: MCP `is_error` sweep. Architecture decision (String→CallToolResult or call_tool override) required first. rmcp 1.7 supports `CallToolResult::structured_error(value)` via `IntoCallToolResult`. |
| architect | idle | RFC-0092 Phase 2 scoping (issue #214 Patterns 3 + dependency-depth) + Charter §2 100K-node row. |
| tech-writer | idle | Marketplace metadata + asciinema. RFC-0092 Phase 2 doc updates. |
| code-reviewer | idle | Blocks on PR opens. |
| security-reviewer | **next-up** | Routine post-v0.1.10 scan. |
| e2e-runner | idle | Incoming-edge test for Pattern 2 verification (after pack-author verifies). |

---

## v0.1.11 Sprint — Draft exit criteria

- [ ] Issue #214 Pattern 2: extractor test verifying `models.py` has incoming Imports edges when N files do `from .models import X`. Green = already fixed; Red = fix needed.
- [ ] Issue #206 Suggestion 1: MCP `is_error: Some(true)` implemented for all error paths.
- [ ] Issue #206 Suggestion 3: Contract test suite: every tool with invalid path returns JSON with `"error"` key.
- [ ] Security scan clean (no high-severity findings post-v0.1.10).
- [ ] Charter §2 SLA 100K-node benchmark row.

---

## Architecture note: MCP `is_error` implementation path

rmcp 1.7 supports:
- `CallToolResult::structured_error(serde_json::Value)` — sets `is_error: Some(true)` automatically
- `IntoCallToolResult` trait — tools can return `CallToolResult` directly (not just `String`)
- `impl<T: IntoContents> IntoCallToolResult for T` — current `String` path

**Recommended approach**: Add helper `fn mcp_err(msg: impl fmt::Display) -> CallToolResult` returning `CallToolResult::structured_error(json!({"error": msg.to_string()}))`. Change each tool method's error-path return from `String` to `CallToolResult`. The macro handles mixed return types via `IntoCallToolResult`. This requires changing the method return type annotation in each of ~89 tools but is semantically correct and fully compatible with rmcp 1.7.

**Alternative**: Override `call_tool` in `ServerHandler` to post-process — but `#[tool_handler]` macro generates `call_tool`, making override uncertain without inspecting macro internals.

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
- **⚠️ Verify GitHub Release for v0.1.6–v0.1.10** (tag exists; release notes pages may be incomplete — finalize job failure was observed for v0.1.6).

---

## Archive

### 2026-05-30 PM run (v0.1.10 re-sync)

- Discovered PM state file was stale at v0.1.6 while develop was at v0.1.10.
- Confirmed 0 open PRs, 2 open issues (#214 P1, #206 P2) without labels.
- v0.1.7: Python module-alias dispatch (PR #217, closes #205/Bug1/#200).
- v0.1.8: `self.method()` / `cls.method()` resolution (closes dominant 533 false-positives from #214).
- v0.1.9: attribute-assignment alias fix + charter release-gate rule + RFC-0096.
- v0.1.10: TYPE_CHECKING import skip (#227) + nested attribute call regression fix (#238).
- Assessed remaining open items: issue #214 (Patterns 2+3, dependency-depth) and issue #206 (all 4 suggestions still open).
- Architecture note added: rmcp 1.7 `IntoCallToolResult` enables clean `is_error` fix.
- Drafted v0.1.11 sprint exit criteria. PM state re-synced and committed.

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
  No manual PR merge needed if RELEASE_BOT_TOKEN is set.

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

### 2026-05-30 PM call (v0.1.2 era — superseded)

1. v0.1.0 + v0.1.1 shipped and on crates.io. Three-Surface Rule is law.
2. External eval (glm5.1) found 4 issues; orchestrator dogfood found 2 more. All filed.
3. Priority order set. Sprint v0.1.2 kicked off on issue #150.
4. PRD for v0.2 at [`docs/prd/v0.2-the-three-surface-release.md`](../prd/v0.2-the-three-surface-release.md).
5. No blocker from founder at this checkpoint. Begin dispatch.
