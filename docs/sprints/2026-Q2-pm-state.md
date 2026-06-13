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
- [x] RFC-0101: Hyphae `#label` syntax complete (PR #495)
- [x] RFC-0094 Phase 3: token-efficient batched output (PR #494)
- [x] RFC-0093: `@` sigil attribute access complete
- [x] RFC-0096: `CALL`/`IMPORT` edges + new Hyphae pseudo-classes (PR #475)
- [x] RFC-0116: `.hive/skills/` scaffold + CLI `skill run` command (PR #479)
- [x] RFC-0117: MCP `skill_run` tool (PR #481)
- [x] RFC-0118: `skills/` category SKILL.md coverage gate (PR #482)
- [x] Three-Surface Rule (RFC-0090) gate green

**Ceremony:**
1. PR #521 merged to main ✅ (squash `54687972`)
2. Tag `v0.2.0` pushed ✅
3. All 5 crates on crates.io ✅
4. Back-merge `release/v0.2.0` → develop ✅ (PR #522, squash `7701b71b`)

---

## 🔄 v0.3.0 — IN PROGRESS

### Release Branch
`release/v0.3.0` — PR #568 open → `main` (founder-gated, ×93 escalations)

### What's In

#### RFC-0111: Node.js/TypeScript SDK + Python SDK
- [x] PR #525 (Node SDK scaffold) merged to develop ✅
- [x] PR #529 (Python SDK scaffold) merged to develop ✅
- [x] PR #531 (Node SDK `queryGraph` + MCP client) merged ✅
- [x] PR #533 (Python SDK `query_graph` + MCP client) merged ✅
- [x] PR #540 (Node SDK CLI surface `mycelium-node`) merged ✅
- [x] PR #541 (Python SDK CLI surface `mycelium-py`) merged ✅
- [x] PR #547 (SDK e2e smoke tests — Node + Python) merged ✅
- [x] PR #550 (Three-Surface parity for SDK tools) merged ✅
- [x] PR #558 (npm publish workflow + PyPI publish workflow) merged ✅

#### RFC-0103: Extends Resolution
- [x] PR #526 (trait/interface extends edges in graph) merged ✅
- [x] PR #534 (Hyphae `:extends()` pseudo-class) merged ✅
- [x] PR #542 (CLI `graph extends` subcommand) merged ✅
- [x] PR #543 (MCP `graph_extends` tool) merged ✅
- [x] PR #551 (Skill coverage for extends tools) merged ✅

#### RFC-0094 Phase 4: Token-Efficient MCP Output (final phase)
- [x] PR #527 (streaming token budget for large graphs) merged ✅
- [x] PR #535 (MCP output compression — symbol dedup) merged ✅
- [x] PR #544 (CLI `--compact` flag parity) merged ✅
- [x] PR #552 (Skill coverage update) merged ✅

#### RFC-0104: Cold-Start SLA (<2 s for 100k-node graph)
- [x] PR #528 (parallel Salsa ingredient init) merged ✅
- [x] PR #536 (pre-warm cache on index load) merged ✅
- [x] PR #545 (nightly cold-start benchmark CI job) merged ✅
- [x] PR #546 (SLA gate: fail CI if p95 > 2000 ms) merged ✅

#### RFC-0119: `watch` mode (live re-index on file save)
- [x] PR #553 (inotify/FSEvents watcher + incremental Salsa) merged ✅
- [x] PR #554 (MCP `watch_start` / `watch_stop` tools) merged ✅
- [x] PR #555 (CLI `mycelium watch` subcommand) merged ✅
- [x] PR #556 (Skill coverage) merged ✅

#### RFC-0120: Pack authoring guide + `pack lint` CLI
- [x] PR #560 (pack authoring guide in `docs/`) merged ✅
- [x] PR #561 (`pack lint` CLI subcommand) merged ✅
- [x] PR #562 (MCP `pack_lint` tool) merged ✅
- [x] PR #563 (Skill coverage) merged ✅

#### RFC-0121: Persistent cross-session memory for Hive agents
- [x] PR #564 (`.hive/memory/` append-only store + Rust reader) merged ✅
- [x] PR #565 (MCP `memory_append` / `memory_query` tools) merged ✅
- [x] PR #566 (CLI `hive memory` subcommand) merged ✅
- [x] PR #567 (Skill coverage) merged ✅
- [ ] PR #763 — RFC-0121 acceptance-criteria sign-off (founder-gated — escalation ×1, open)

#### Misc / Infrastructure
- [x] PR #530 (CHANGELOG v0.3.0 section) merged ✅
- [x] PR #537 (cargo deny + audit clean) merged ✅
- [x] PR #538 (llvm-cov ≥ 90% gate restored) merged ✅
- [x] PR #539 (DCO sign-off bot config) merged ✅
- [x] PR #548 (ADR-0011: SDK design rationale) merged ✅
- [x] PR #549 (ADR-0012: watch-mode architecture) merged ✅
- [x] PR #557 (ADR-0013: pack-lint grammar) merged ✅
- [x] PR #559 (release/v0.3.0 branch cut + version bumps) merged ✅

### v0.3.0 Ceremony Status
1. PR #568 (`release/v0.3.0` → `main`) — **OPEN** (founder-gated, ×93 escalations as of v232)
2. Tag `v0.3.0` — pending
3. crates.io/npm/PyPI publish — **published 2026-06-05T17:59Z** (workflow ran on release branch; ceremony step 3 pre-completed)
4. Back-merge → develop — pending

---

## 🔢 Open Items Tracker

### P0 — Founder-Gated (blocked, cannot proceed without founder)

| # | Item | Escalations | Notes |
|---|------|-------------|-------|
| PR #568 | `release/v0.3.0` → `main` merge | ×93 | CI green 22/22; all ceremony pre-work done; waiting founder |
| PR #763 | RFC-0121 acceptance-criteria sign-off | ×1 | All criteria met; waiting founder review |

### P1 — Unblocked (agent can act)

| # | Item | Status | Notes |
|---|------|--------|-------|
| Issue #829 | Bench mutation kill rate regression | Open | Needs investigation post-v0.3.0 ceremony |
| RFC-0104 | Cold SLA nightly gate | Ongoing | Nightly CI job running; no breach yet |
| e2e dogfood | 8/8 CLI + SDK round-trip tests | Ongoing | All passing as of v230 |

### P2 — Nice-to-Have / Backlog

| # | Item | Notes |
|---|------|-------|
| — | RFC-0122 (candidate): incremental pack hot-reload | Not yet filed; depends on RFC-0119 watch mode |
| — | ADR gap: Patricia Trie for Trunk | docs/adr/0004 still missing |
| — | ADR gap: MessagePack wire format | docs/adr/0005 still missing |
| — | ADR gap: Hyphae CSS-selector grammar | docs/adr/0006 still missing |

---

## 📋 PR Activity Log (recent, last 20 dispatches)

| Dispatch | PR | Action | Notes |
|----------|----|--------|-------|
| v232 | #835 | Merged (squash `f861fc84`) | 22/22 CI ✅; Codex P2 rejected with justification |
| v231 | #835 | Opened | Three-Surface parity fix for RFC-0121 tools |
| v230 | #829 | Issue filed | Bench mutation kill rate regression flagged |
| v229 | #763 | Escalated (×1) | RFC-0121 sign-off; waiting founder |
| v228 | #568 | Escalated (×92) | v0.3.0 ceremony PR; CI still green |
| v227 | #568 | CI re-run confirmed green | 22/22 checks pass |
| v226 | — | PM state updated | No new PRs; holding pattern |
| v225 | — | PM state updated | No new PRs; holding pattern |
| v224 | — | PM state updated | No new PRs; holding pattern |
| v223 | — | PM state updated | No new PRs; holding pattern |
| v222 | — | PM state updated | No new PRs; holding pattern |
| v221 | — | PM state updated | No new PRs; holding pattern |
| v220 | — | PM state updated | No new PRs; holding pattern |
| v219 | — | PM state updated | No new PRs; holding pattern |
| v218 | — | PM state updated | No new PRs; holding pattern |
| v217 | — | PM state updated | No new PRs; holding pattern |
| v216 | — | PM state updated | No new PRs; holding pattern |
| v215 | — | PM state updated | No new PRs; holding pattern |
| v214 | — | PM state updated | No new PRs; holding pattern |
| v213 | #835 | Draft opened | Three-Surface gap for RFC-0121 |

---

## 🗃️ Archived Dispatch Summaries

### v1–v28 (2026-06-02 to 2026-06-03)
*(Archived. Covered v0.1.13–v0.1.19 ceremonies, RFC-0093/0096/0107/0108, redb, Salsa Phase 2, reactive subscriptions.)*

### v29–v60 (2026-06-03 to 2026-06-04)
*(Archived. Covered v0.2.0 ceremony completion, RFC-0109/0110/0102/0101/0094-P3/0116/0117/0118, Three-Surface gate green, npm distribution.)*

### v61–v100 (2026-06-04 to 2026-06-05)
*(Archived. Covered v0.3.0 branch cut, RFC-0111/0103/0094-P4/0104/0119/0120/0121 PRs merged to develop, registries pre-published.)*

### v101–v128 (2026-06-05 to 2026-06-06)
*(Archived. Covered PR #568 escalations ×1–×28, PR #763 filed, issue #829 filed, e2e dogfood established.)*

### v129–v160 (2026-06-06 to 2026-06-08)
*(Archived. PR #568 escalations ×29–×60. No new code tasks. Holding pattern established.)*

### v161–v192 (2026-06-08 to 2026-06-10)
*(Archived. PR #568 escalations ×61–×82. PR #835 drafted. No new code tasks.)*

### v193–v212 (2026-06-10 to 2026-06-12)
*(Archived. PR #568 escalations ×83–×91. PR #835 in review. Codex P2 finding on PR #835 under review.)*

### v213–v231 (2026-06-12 to 2026-06-13)
*(Archived. PR #835 merged v232. PR #568 escalation ×92→×93. Codex P2 rejected with justification. See v232 for current state.)*

---

*PM brain last written by orchestrator agent. For historical dispatches see closed PRs and git log on `chore/pm-state-v*` branches (v1–v128).*

*(see closed PRs #502/#506 and git log for historical archives — last pre-v0.2.0 dispatch)*
