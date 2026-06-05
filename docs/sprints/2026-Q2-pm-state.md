# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-06-05 (PM dispatch v30 — PRs #563+#565 merged; PR #557 escalated to founder; RFC-0111 Node+Python SDKs on develop) |
| Current sprint | **RFC-0111 bindings complete (Node + Python SDK on develop); v0.2.1 pending founder git ceremony** |
| Active release branch | `release/v0.2.1` — PR #557 open; crates.io + npm published ✅; awaiting Step 1–4 git ceremony |
| Next release target | **v0.2.1** — RFC-0094 Phase 4 text output + RFC-0103 Extends stubs + RFC-0111 bindings + CI hardening |
| Final release target | v0.3.0, ETA 2026-08-01 (cross-repo + IDE plugins) |
| Last shipped | **v0.2.0 (2026-06-04)** — main at `5468797`; crates.io + npm published via release.yml |

---

## ✅ v0.1.13 through v0.1.19 — SHIPPED

*(All four ceremony steps complete for each. See archive entries in v28 PM state for details.)*

**v0.1.19** shipped 2026-06-03T15:49Z:
- fix(packs/rust): extractor precision 67% → 99.8% recall
- docs(adr): ADR-0008, ADR-0009
- docs(rules): Codex review Hard Rule added
- RFC-0105 EXCEPTION: WatchEngine Three-Surface exception ratified

---

## ✅ v0.2.0 — SHIPPED (2026-06-04)

**What shipped in v0.2.0:**
- [x] RFC-0109 7/7 graph-list parity: all tools via shared core builders + CLI object-shape + budget knob (PRs #508–#513)
- [x] RFC-0102: per-call budget override knob + nested `budget{}` response object
- [x] RFC-0110: npm/bun CLI distribution via prebuilt-binary optionalDependencies (5 platforms: darwin-arm64/x64, linux-x64/arm64, win32-x64)
- [x] ADR-0010: no live LSP; prefer static SCIP/LSIF
- [x] RFC-0104: Charter §2 warm/cold SLA split; macOS SLA guard 30ms → 100ms
- [x] CI hardening: npm-token preflight, graceful E404 scope handling, darwin-x64 cross-compile fix

**v0.2.0 ceremony status:**
- [x] **Step 1**: `release/v0.2.0` → `main` — main HEAD at `5468797 chore(release): v0.2.0` ✅
- [ ] **Step 2**: Tag `v0.2.0` — unverified (needs founder confirmation)
- [ ] **Step 3**: GitHub Release v0.2.0 — unverified
- [ ] **Step 4**: Back-merge `release/v0.2.0` → `develop` — unverified

> ⚠️ Steps 2–4 need founder verification. crates.io + npm were published by release.yml (confirmed by `5468797` parent commits showing CI fixes on release branch). If ceremony is incomplete, repair needed before v0.2.1 ceremony.

---

## 🔖 v0.2.1 — CRATES + NPM PUBLISHED; GIT CEREMONY PENDING FOUNDER

**What ships in v0.2.1 (PR #557, `release/v0.2.1` → `main`):**
- [x] **RFC-0103**: import-aware `Extends`-stub resolution — cross-file inheritance accuracy (PR #554)
- [x] **RFC-0094 Phase 4**: stdio MCP default output → `text` — ~72% fewer tokens for tree-shaped responses; 77 format sites unified (PR #552)
- [x] **Issue #428 slice 3**: god-file-split — 93 request schema types → `requests.rs`; `lib.rs` 6,048 → 4,694 lines −22% (PR #550)
- [x] **fix**: npm launcher signal exit codes: `128 + signal_number` (POSIX/shell) (PR #535)
- [x] **test**: Mutation testing kill-rate: exact-count assertions on 6 weak MCP tests (PR #531)

**v0.2.1 ceremony status:**
- [ ] **Step 1**: CI green → admin-merge PR #557 → `main` *(founder action)*
- [ ] **Step 2**: Push tag `v0.2.1`
- [ ] **Step 3**: GitHub Release (release.yml triggers on tag push)
- [ ] **Step 4**: Back-merge `release/v0.2.1` → `develop`

**PR #557 CI**: 30/30 checks ✅ (all success or skipped). Codex P1 finding spun off as Issue #560 (fixed on develop via PR #563). **Ready for Step 1.**

---

## 🔧 Post-v0.2.1 — Unreleased on develop (→ v0.2.2)

- [x] **RFC-0111 Phase 1**: Node.js SDK — thin typed client wrapping mycelium binary, npm `mycelium-rcig` package (merged before this session)
- [x] **RFC-0111 Phase 2**: Python SDK — `mycelium-rcig` PyPI package, 32 tests (PR #565, merged this session `64e865f2`)
- [x] **fix(ci)**: NPM_TOKEN check-npm-token hard preflight; publish-crates gated on token presence (PR #563, merged this session `cd9ff0e4`)

---

## Live priorities (ordered)

**P0 (none — develop CI green)**

**P1 (next sprint items):**
1. **Founder**: v0.2.1 git ceremony — admin-merge PR #557 → main, tag `v0.2.1`, GH Release, back-merge PR to develop.
2. **Founder**: Verify v0.2.0 ceremony Steps 2–4 (tag, GH Release, back-merge) are complete.
3. **rust-implementer**: Issue #555 — RFC-0103 per-edge Extends rewrite. Needs `Synapse::remove_edge(kind, src, dst)` + `AdjacencyList::remove(src, dst)`. Once primitive exists, rewrite each subclass Extends edge independently, remove stub when degree reaches 0.
4. **e2e-runner**: Dogfood re-run with v0.2.1 content (RFC-0094 text output + RFC-0103 Extends). 8/8 CLI commands.
5. **bench**: RFC-0104 cold SLA measurement — nightly `sla_ancestors_100k` data for Charter §2 table amendment.

**P2 (v0.3.0 scope):**
6. Issue #428 god-file-split remaining slices.
7. Skill marketplace submission.
8. `release.yml` finalize merge step systemic fix (ceremony script is current workaround).
9. Cross-repo / multi-root indexing (v0.3.0 scope).

---

## Dispatch state (2026-06-05 v30)

| Agent | Status | Current item |
|---|---|---|
| founder | **action requested (P1)** | **(1)** PR #557: CI ✅, crates+npm published — admin-merge → main, tag `v0.2.1`, GH Release, back-merge. **(2)** Verify v0.2.0 ceremony Steps 2–4. |
| PM | **DONE ✅** | v30 complete: PRs #563+#565 merged; PM state updated; decisions.jsonl appended. |
| release | **waiting** | After v0.2.1 ceremony: cut `release/v0.2.2` once v0.2.2 scope defined. |
| security-reviewer | **P1** | Post-#563+#565 security scan (CI clean; bindings new attack surface). |
| architect | idle | RFC-0103 per-edge Extends primitive design (Issue #555). |
| e2e-runner | **P1** | Dogfood re-run with v0.2.1 content (8/8 CLI). |
| bench | **P1** | RFC-0104 cold SLA nightly data. |
| tech-writer | idle | RFC-0111 SDK docs (README examples, usage guide). |
| rust-implementer | **P1** | Issue #555: Synapse::remove_edge + RFC-0103 per-edge Extends. |

---

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- Skill marketplace listing metadata sign-off.
- **RFC-0104 cold SLA measurement**: Charter §2 table amendment (warm/cold split) requires measured nightly data.
- **v0.2.0 ceremony Steps 2–4**: tag, GH Release, back-merge — need founder verification/completion.
- **v0.2.1 ceremony Step 1**: admin-merge PR #557 → main (CI ✅, ready now).

---

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder if available): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/v0.2.x branch, publish.

---

## Archive

### 2026-06-05 PM dispatch v30 (this run)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl (develop tail-45, ends at v0.2.0 release prep 2026-06-04T05:26:18Z), anti-patterns (domain hits: ci/release-governance/merge-discipline), PM state v28 (stale on develop), v0.2 PRD.

**Assessment:**
- 3 open PRs: #557 (release/v0.2.1 → main, 30/30 CI ✅, Codex P1 spun off to #560), #563 (NPM_TOKEN fix, 20/20 CI ✅, Codex P1 fixed c5690b9), #565 (Python SDK RFC-0111 Phase 2, 20/20 CI ✅, Codex P1+P2 fixed af9a575).
- 1 open issue: #555 (RFC-0103 per-edge Extends, P1 enhancement).
- develop CI: ✅ (last run 2026-06-05T10:12Z, SHA 19fb6f1f).
- Main at `5468797 chore(release): v0.2.0` — v0.2.0 crates+npm published.
- PM state v28 is 2 days stale; v29 entries exist only on main's decisions.jsonl (not back-merged to develop).

**Actions taken:**
1. **Verified** Codex findings on #563 (P1 fixed in c5690b9 — check-npm-token preflight added) and #565 (P1+P2 both fixed in af9a575 — publish-pypi gated on publish-npm; OSError broadened to catch PermissionError). ✅
2. **Merged PR #563** (fix/issue-560-publish-npm-token-exit-code) → squash `cd9ff0e4`. ✅
3. **Merged PR #565** (feat/RFC-0111-python-sdk) → squash `64e865f2`. ✅
4. **Updated PM state** v28 → v30. ✅
5. **Appended decisions.jsonl** (v30 entry). ✅

**Escalations to founder:**
- **(1) PR #557**: release/v0.2.1 — 30/30 CI ✅, crates.io + npm published. Ready for Step 1 admin-merge → main.
- **(2) v0.2.0 ceremony**: Verify Steps 2–4 (tag, GH Release, back-merge) complete.

### 2026-06-03/04 PM dispatches v28–v29 (summary)

- v28 (2026-06-03): macOS SLA flake fix (PR #508); ADR-0010 merged; PM state corrected (v0.1.19 boundary).
- v29 (2026-06-04): RFC-0109 7/7 complete; PRs #508+#513 merged; PM chore rebased.
- Post-v29: RFC-0110 npm distribution (increments 1–3); v0.2.0 release branch prepared and pushed.

### 2026-06-03 PM dispatch v27 and earlier

*(see decisions.jsonl entries v13–v27 for full detail)*
