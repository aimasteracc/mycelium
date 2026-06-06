# RFC-0114: graph-native project health grade (A–F)

- **Status**: **Draft** (design + Phase-1 scorer core in the opening PR)
- **Author(s)**: orchestrator (Hive AI agent)
- **Created**: 2026-06-06 (UTC)
- **Depends on**: existing graph metrics — [RFC-0046](0046-node-degree.md)/[RFC-0073](0073-degree-histogram.md)
  (degree), [RFC-0076](0076-clustering-coefficient.md) (clustering), the
  `dead_symbols` / `isolated_symbols` queries; Charter §5.13 (Three-Surface)
- **Affected paths** (when implemented): `crates/mycelium-core/src/health.rs`
  (new), then `crates/mycelium-cli/src/main.rs` + `crates/mycelium-mcp/src/`
  (the CLI+MCP surface) + a category Skill
- **Reuses (concept)**: tree-sitter-analyzer's project-health grading
  (`health_scorer.py`) — the *idea* of a one-call A–F project grade. The
  *metrics* are re-grounded on Mycelium's graph (see below).

## Summary

A one-call **A–F health grade** for an indexed project, computed **purely from
the RCIG graph** Mycelium already builds — dead-code ratio, isolation ratio, and
connectivity — with a weighted 0–100 score and a per-dimension breakdown. No
cyclomatic-complexity parser, no coverage file, **cross-language by
construction**. "Grade my codebase's structural health in one call" is a
marketable, embeddable-context feature no CC-based tool can do across languages.

## Motivation

TSA markets project-health grading as a differentiator ("no other open-source
tool grades your whole project in one call"). Mycelium can do a **stronger,
graph-native** version: its value *is* the cross-language graph, so a grade
derived from graph structure (not per-language CC) is both on-brand and
language-agnostic. It turns metrics Mycelium already computes
(`dead_symbols`, `isolated_symbols`, degree, clustering, node/edge counts) into
a single actionable signal for agents and humans ("is this codebase healthy
enough to build on?").

## Scope — re-ground TSA's dimensions on the graph

TSA's scorer weights size / **complexity (CC)** / deps / **coverage** /
duplication / structure / git-hotspots. Mycelium has **no CC and no coverage**
(ADR-0010: no live LSP; no test runner). So this RFC does **not** port those
dimensions — it uses the **graph-native** metrics Mycelium *does* have:

| Dimension | Source (existing public API) | Intuition |
|---|---|---|
| **Dead code** | `dead_symbols` / total symbols | unreachable code is rot |
| **Isolation** | `isolated_symbols` / total | symbols with no edges = disconnected/unused |
| **Connectivity** | `edge_count` / `node_count` (density) | too sparse = under-linked; healthy code wires together |
| **Structure** *(Phase 2)* | `degree_histogram` skew, `clustering_coefficient` | a few god-nodes vs. even coupling |

Weights and thresholds are documented constants (adapted from TSA's spirit, not
its CC numbers), tunable in one place.

## Decision

A **pure scorer core** + a thin **graph adapter**, mirroring RFC-0113's
`classify` module pattern (build the testable pure core first, defer the
`Store`/CLI/MCP wiring):

1. `health::HealthMetrics` — a plain struct of the raw inputs
   (`total_symbols`, `dead_count`, `isolated_count`, `edge_count`).
2. `health::score(&HealthMetrics) -> HealthReport` — **pure**, returns
   `{ grade: A|B|C|D|F, score: 0..=100, dimensions: [(name, 0..=100)] }`.
   Fully unit-testable with no `Store`. **(Phase 1 — this PR.)**
3. `Store::health(&self) -> HealthReport` — a thin adapter that fills
   `HealthMetrics` from the public API (`node_count`, `dead_symbols`,
   `isolated_symbols`, `edge_count`) and calls `score`. **(Phase 2.)**
4. `mycelium project-health` CLI + `mycelium_project_health` MCP (byte-identical)
   + a category Skill — the Three-Surface trio. **(Phase 2.)**

Building the pure core first (no `Store`, no CLI/MCP) keeps Phase 1 **zero-risk
and zero-collision** while the design is reviewed.

## Scoring model (Phase 1, documented + tunable)

Each dimension yields a 0–100 sub-score; the grade is the weighted mean:

- `dead = 100 * (1 - dead_count / total)`
- `isolation = 100 * (1 - isolated_count / total)`
- `connectivity = clamp(density / TARGET_DENSITY) * 100`, `density = edges/nodes`

`score = round(0.45*dead + 0.35*isolation + 0.20*connectivity)`; grade bands
`A ≥ 90, B ≥ 80, C ≥ 70, D ≥ 60, F < 60`. Empty project (`total == 0`) →
`grade = F, score = 0` (nothing to grade, fail-closed). All constants live in
one block for tuning; Phase 2 may add structure (degree skew / clustering).

## Three-Surface Rule (Charter §5.13)

This **is** a new capability, so Phase 2 MUST ship `project-health` as a strict
CLI↔MCP 1:1 pair **and** cover it in a category Skill. Phase 1 (the pure scorer
core) adds no surface — it is internal scaffolding, exactly like RFC-0113's
`classify.rs`.

## Acceptance criteria

**Phase 1 — scorer core (this PR):**
- [x] `health.rs`: `HealthGrade`, `HealthMetrics`, `HealthReport`, pure `score()`.
- [x] TDD: healthy project → A/B; high dead-code → drops a grade; all-isolated →
      low; empty project → F/0; band boundaries (89→B, 90→A). fmt + clippy clean.
- [x] No edits to `store/mod.rs` / `synapse/` (zero overlap with PR #572).

**Phase 2 — surface (follow-up, after #572 + this design's review):**
- [ ] `Store::health()` adapter over the public metric API.
- [ ] `project-health` CLI + `mycelium_project_health` MCP (byte-identical) + Skill.
- [ ] README + CHANGELOG; optional structure dimension (degree/clustering).

## Alternatives considered

- **Port TSA's CC/coverage scorer verbatim.** Rejected: Mycelium computes
  neither CC (no per-function complexity) nor coverage (no test runner), and
  ADR-0010 rules out the live LSP that would supply CC. Graph-native metrics are
  what Mycelium has — and are cross-language, which CC is not.
- **Surface raw metrics, let callers grade.** Rejected: the *one-call grade* is
  the product value; raw metrics already exist via the degree/dead-code tools.

## Conflicts with binding constraints

- **ADR-0010:** ✅ no LSP, no subprocess — pure graph arithmetic.
- **Charter §5.13:** Phase 2 adds the CLI↔MCP pair + Skill; Phase 1 adds no surface.
- **PR #572 collision:** Phase 1 is a new module + `lib.rs` decl only; it does
  not touch the resolver files #572 edits.
