# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-06-04 (PM dispatch v53 — PR #547 merged; post-v0.2.0 security scan CLEAN; v0.2.1 queue defined) |
| Current sprint | **Post-v0.2.0 stabilization — ceremony 3/4 complete (tag v0.2.0 pending founder); v0.2.1 queue open** |
| Active release branch | none — `release/v0.2.0` branch merged and deleted |
| Next release target | **v0.2.1** — MCP god-file split (lib.rs 6,048 lines) + npm E404 tightening (post-scope-registration) |
| Final release target | v0.3.0 (cross-repo indexing, IDE plugins) |
| Last shipped | **v0.2.0 (ceremony 3/4)** — crates.io ✅ + main merge ✅ + back-merge ✅; tag v0.2.0 ⏳ founder action pending. |

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

## ⚠️ v0.2.0 — CEREMONY 3/4 COMPLETE (tag pending founder)

**What shipped in v0.2.0:**
- [x] RFC-0109 all 7 graph-list tools → shared core builders + object shape + budget knob (PRs #501–#513)
- [x] RFC-0102 nested `budget{}` response object + BudgetMode tag + per-call override + cap fixes (PRs #497–#499)
- [x] RFC-0110 npm/bun CLI distribution: prebuilt-binary optionalDependencies model; 5-platform build matrix; release.yml publish-npm job (PRs #517–#520)
- [x] fix(npm): 128+signal exit codes in launcher (PR #535)
- [x] test(mcp): exact-count assertions — mutation kill-rate ≥70% SLA fix (PR #531)
- [x] ci(dco-check): grep full body for `Signed-off-by` — systemic DCO false-fail fix (PR #544)
- [x] ci(release): graceful npm publish for E404 scope-not-found + absent NPM_TOKEN (PR #533)
- [x] All v0.1.19→v0.2.0 content on develop (RFC-0109/102/110 roll-out)

**v0.2.0 ceremony status:**
- [x] **Step 1**: `release/v0.2.0` → `main` — PR #523 MERGED ✅ (2026-06-04)
- [ ] **Step 2**: Tag `v0.2.0` pushed → ⏳ **FOUNDER ACTION REQUIRED**
- [x] **Step 3**: All 5 crates to crates.io ✅ (release.yml, 2026-06-04)
- [x] **Step 4**: Back-merge `release/v0.2.0` → `develop` — PR #537 MERGED ✅ (`4e60400f`)

**v0.2 PRD success metrics status:**
- [x] Capabilities reachable from all 3 surfaces: 93/93 MCP tools + CLI + Skills ✅ (Charter §5.13 enforced)
- [x] Category Skills published: 10+ ✅
- [ ] Skills marketplace presence: ≥1 (Claude Code) — **P2, not yet submitted**
- [x] Open P0 bugs: 0 ✅
- [x] Dogfood pass rate: 8/8 (CI dogfood job passing) ✅
- [x] Charter §2 SLA rows satisfied ✅

---

## 🔧 Post-v0.2.0 — Unreleased on develop (→ v0.2.1)

> Commits on develop HEAD (`640a8dcf`) that will ship in v0.2.1:

- [x] chore(pm): dispatch v29–v53 (PM state + decisions.jsonl maintenance)
- [x] ci(dco-check): systemic DCO false-fail fix (PR #544, `0554ee7`)
- [x] fix(npm): 128+signal exit codes (PR #535, `3f81241`)
- [x] test(mcp): mutation kill-rate exact-count assertions (PR #531, `b696953`)
- [x] ci(release): graceful npm E404 + absent-token handling (PR #533, `fdd3525`)

---

## Live priorities (ordered)

**P0 — Founder actions required (ceremony gate):**
1. **Push tag `v0.2.0`** + create GitHub Release — Charter §5.12 Step 2. Crates.io ✅, main merge ✅, back-merge ✅. Only tag missing.
2. **Register `@aimasteracc` npm scope** on npmjs.com + add `NPM_TOKEN` to repo Settings → Environments → `npm`. Required for v0.2.1 npm publish to succeed (Issue #534 tracks the E404 tightening step).

**P2 — Autonomous (unblock after founder P0s land):**
3. **MCP god-file split residual** (⚠️ Issue #428 partial): `crates/mycelium-mcp/src/lib.rs` at 6,048 lines. Target: extract `tools/context.rs`, `tools/graph.rs`, move `mod tests` → `tests/` submodule. v0.2.1 sprint item.
4. **Issue #534**: Remove E404 graceful degradation from `publish_one()` in `release.yml` — pending founder npm scope registration. Code change is 3-line removal.
5. **RFC-0104 cold SLA numbers**: Measure nightly `sla_ancestors_100k` on redb for Charter §2 cold-open budget. Requires founder Charter §2 amendment once data is collected.
6. **Skills marketplace submission**: Claude Code marketplace metadata (icon, screenshots, examples). Requires founder sign-off on listing metadata.

---

## Dispatch state (2026-06-04 v53)

| Agent | Status | Current item |
|---|---|---|
| founder | **P0 action required** | **(1)** Push tag `v0.2.0` + GitHub Release (Charter §5.12 Step 2). **(2)** Register `@aimasteracc` npm scope + add `NPM_TOKEN` secret. |
| PM | **DONE ✅** | v53 complete: PR #547 merged (640a8dcf); post-v0.2.0 security scan CLEAN; v0.2.1 queue defined. |
| release | **idle** | v0.2.0 ceremony 3/4 ✅. Next: cut `release/v0.2.1` once MCP god-file split + Issue #534 ready. |
| security-reviewer | **DONE ✅** | Post-v0.2.0 scan (release.yml + npm/): CLEAN. |
| architect | **idle** | RFC-0104 cold SLA Charter §2 amendment (needs nightly measurement data first). |
| rust-implementer | **P2** | MCP god-file split: `crates/mycelium-mcp/src/lib.rs` 6,048→tools/ modules (Charter §5.4 quality). |
| e2e-runner | **idle** | Dogfood 8/8 verified by CI dogfood job ✅. Next: v0.2.1 regression pass after god-file split. |
| bench | **P2** | `sla_ancestors_100k` nightly (RFC-0104 cold SLA data collection). |
| tech-writer | **P2** | Skills marketplace submission prep (sign-off from founder needed). |

---

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- **Skill marketplace listing metadata sign-off** (P2, pending).
- **RFC-0104 cold SLA measurement**: Charter §2 table amendment requires measured nightly data.
- ~~**RFC-0105 Three-Surface EXCEPTION**~~: ✅ RATIFIED 2026-06-03T12:30Z.
- ~~**v0.1.17 git ceremony skip**~~: ✅ RESOLVED.
- **Systemic**: `release.yml` finalize merge — ceremony script is workaround; RFC-0110 `finalize` job uses `git push origin main` (not GitHub PR API), so the old v0.1.6–v0.1.18 auto-close bug is RESOLVED for v0.2.0+.

---

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder if available): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/v0.2.x branch, publish.

---

## Archive

### 2026-06-04 PM dispatch v53 (this run)

**Pre-flight:** Read CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl (local clone, entries v1–v45), anti-patterns, PM state (v28 on local main — stale; v51/v52 from PR #547 commit history), v0.2 PRD.

**Assessment:**
- 1 open PR: #547 (PM v51 chore, 20/20 CI ✅, 1 Codex finding with `aimasteracc` reply = Hard Rule satisfied).
- 1 open issue: #534 (P2, npm E404 tightening — founder-gated).
- develop HEAD: `0fe4f99c` (PM v50 squash, from PR #546). CI green.
- v0.2.0 ceremony: Steps 1/3/4 ✅; Step 2 (tag) ⏳ founder pending.
- Queue: entirely founder-gated P0s + P2 autonomous items.

**Actions taken:**
1. **Merged PR #547** (squash `640a8dcf`) — PM v51/v52 wrap-up; Codex P2 replied/fixed by prior session. ✅
2. **Post-v0.2.0 security scan** (release.yml + npm/ code reviewed): CLEAN — no hardcoded secrets; E404 grace is by design (Issue #534); id-token:write is legitimate npm provenance requirement; all tokens properly as `secrets.*`. ✅
3. **Composed PM state v53** — updated header, v0.2.0 ceremony status, v0.2.1 queue, dispatch state. ✅
4. **NOTE**: decisions.jsonl NOT appended this session. MCP `get_file_contents` returns local-main clone content regardless of branch parameter, which would truncate develop's v29–v52 entries if pushed. Anti-pattern recorded.

**Escalations to founder:**
1. **(P0)** Push tag `v0.2.0` + create GitHub Release (Charter §5.12 Step 2 — sole remaining ceremony gate).
2. **(P0)** Register `@aimasteracc` on npmjs.com + add `NPM_TOKEN` to repo Settings → Environments → `npm` (unblocks v0.2.1 npm publish).

### 2026-06-04 PM dispatch v52 (PR #547 branch — Codex P2 fix + MCP split P2 item added)

*(see merged commit `640a8dcf` for full archive)*

### 2026-06-04 PM dispatch v51 (PR #546 merged; 2 stale P2 items cleared; post-v0.2.0 queue tightened)

*(see merged commit `0fe4f99c` for full archive)*

### 2026-06-04 PM dispatch v50 (PRs #544+#545 merged; DCO fix deployed)

*(see commit `0fe4f99c` squash message for full archive)*

### 2026-06-04 PM dispatch v46 (Codex P1+P2 fixes; v0.2.0 ceremony Steps 1+3+4 ✅; security scan CLEAN)

*(see commit `e089b66a` for full archive)*

### 2026-06-04 PM dispatch v36 (v0.2.0 release in progress; PR #522 merged)

*(see commit `b2fe917c` for full archive)*

### 2026-06-04 PM dispatch v29 (PRs #508+#513 merged; RFC-0109 7/7 complete)

*(see commit `e94acb42` for full archive)*

### 2026-06-03 PM dispatch v28 (develop CI fix PR #508; ADR-0010 merged; v0.1.19 boundary corrected)

*(see commit `bf0399a2` for full archive)*

### Earlier dispatches (v1–v27)

*(archived in git history)*
