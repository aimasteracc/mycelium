# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain. Update on every cadence checkpoint. Older states roll into the dated archive at the bottom.

| Field | Value |
|---|---|
| PM | orchestrator (Hive AI agent) |
| Last updated | 2026-05-29 (PM run — #153 perf PR opened) |
| Current sprint | v0.1.4 (performance hardening + CLI parity backfill) |
| Active release branch | none (between releases) |
| Next release target | v0.1.4, ETA 2026-06-20 |
| Final release target | v0.2.0, ETA 2026-07-15 |
| Last shipped | **v0.1.3 — Hyphae lands + Skill umbrella sprint 1** (backmerge: develop commit 36d250d) |

## Live priorities (ordered)

P1:
1. [#153](https://github.com/aimasteracc/mycelium/issues/153) — **PR #168 OPEN** (pending CI + review). Root cause: cold-start/deserialization overhead, not algorithmic. Delivered: `symbol_nodes()`, parent-map BFS, 8 regression tests, Criterion benchmark, 2 new Charter §2 SLA rows.

P2 (governance backfill):
2. RFC-0090 Phase 1 — `parity.yml` CI workflow. **v0.1.4.** (Was Sprint v0.1.3 item 3; slipped to v0.1.4.)
3. `mycelium init` — still hidden/unimplemented per #154. Either implement or remove. **v0.1.4.**

Sprint v0.1.4 backlog:
4. Charter §2 SLA `mycelium index` benchmarks against 1K/10K/100K node repos — new SLA row. *(heavy-graph SLA rows done in PR #168)*
5. CLI parity backfill — ~87 CLI subcommands still pending (🟡 rows in INDEX.md); systematic batch landing.

Completed in v0.1.3 (2026-05-29):
- [#151](https://github.com/aimasteracc/mycelium/issues/151) ✅ — `mycelium query` Hyphae CLI wired (PR #159)
- RFC-0090 Phase 2 ✅ — 9 category Skills: hyphae-query, basic-queries, call-graph, import-graph, reachability, centrality, inheritance, graph-structure, batch-ops (73/88 caps in PRs #159–#162)
- RFC-0090 Phase 2.3 ✅ — 16 remaining capabilities triaged; index-management Skill created; 89/89 coverage (PR #166)

## Dispatch state (2026-05-29)

| Agent | Status | Current item |
|---|---|---|
| rust-implementer | **waiting-review** | PR #168 (#153) — awaiting CI + founder review |
| architect | idle | next: `parity.yml` CI workflow design (heavy-graph SLA rows done) |
| tech-writer | idle | next: doc-sync pass on Skills for recently added tools (brief descriptions for `find_call_path`, `get_leaf_symbols`, etc.) |
| code-reviewer | idle | blocks on PR opens |
| security-reviewer | idle | next: routine post-sprint scan |
| e2e-runner | idle | next: add perf regression tests for #153 tools after fix |

## Cadence

- **Hourly (autonomous)**: each agent picks the top item from its queue.
- **Daily PM check** (orchestrator): scan issue queue for new P0/P1; rebalance.
- **Weekly Sprint review** (orchestrator + founder if available): mark sprint exit criteria; cut next sprint.
- **Bi-weekly release** (orchestrator): if sprint exit criteria met, cut release/v0.1.x branch, publish.

## Sprint v0.1.4 exit criteria

- [ ] All 6 timeout tools complete on 1K-node graph in < 2 s wall-clock.
- [ ] New SLA rows in Charter §2 for heavy-graph algorithms.
- [ ] Benchmarks in `benches/` for each of the 6 tools.
- [ ] `parity.yml` CI workflow live (informational → required).
- [ ] `mycelium init` resolved (implement or remove).

## Decision gates (require founder)

- Any name change to a public crate or CLI subcommand.
- Charter §5.X amendment or new commitment.
- Re-licensing (forbidden — see Charter §5.8).
- Storage-format break.
- Skill marketplace listing metadata sign-off.

## Archive

### 2026-05-30 PM call (v0.1.2 era — superseded)

1. v0.1.0 + v0.1.1 are shipped and on crates.io. Three-Surface Rule is law.
2. External eval (glm5.1) found 4 issues; orchestrator dogfood found 2 more. All filed.
3. Priority order is set. Sprint v0.1.2 kicks off the moment rust-implementer picks up [#150](https://github.com/aimasteracc/mycelium/issues/150).
4. PRD for v0.2 is at [`docs/prd/v0.2-the-three-surface-release.md`](../prd/v0.2-the-three-surface-release.md). 5 sprints to v0.2.0.
5. No blocker from the founder needed at this checkpoint. Begin dispatch.
