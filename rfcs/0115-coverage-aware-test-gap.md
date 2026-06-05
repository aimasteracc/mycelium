# RFC-0115: coverage-aware test-gap analysis — rank untested code by graph reach (design)

- **Status**: **Draft** (design — no implementation in this PR)
- **Author(s)**: orchestrator (Hive AI agent)
- **Created**: 2026-06-06 (UTC)
- **Depends on**:
  [ADR-0010](../docs/adr/0010-no-live-lsp-prefer-scip-ingestion.md) (no live
  LSP / no language-server subprocess — **consume external artifacts, never
  execute**; this RFC's "consume `coverage.json`, don't run pytest" is exactly
  that sanctioned static lever),
  RFC-0114 (graph-native scoring over a plain metrics struct — this RFC reuses
  its pure-core-first shape; link resolves once that RFC lands on develop),
  the `mycelium_impact` / blast-radius capability and the public Store metrics
  (`degree_centrality`, callers/callees) used for ranking,
  Charter §5.13 (Three-Surface Rule), Charter §4 (≤3-file language packs)
- **Affected paths** (when implemented):
  `crates/mycelium-core/src/` (new pure `test_gap` scorer/ranker module),
  `crates/mycelium-core/src/store/` (thin adapter that feeds Store metrics into
  the pure core), `crates/mycelium-cli/` + `crates/mycelium-mcp/` (Phase-2
  surface wiring), `skills/<category>/SKILL.md` (Phase-2 Skill coverage),
  fixtures under `crates/mycelium-core/tests/`
- **Reuses**: tree-sitter-analyzer (TSA) — same founder, MIT — specifically
  `tree_sitter_analyzer/test_gap_analyzer.py` (the
  `analyze_coverage_gaps` → `_load_coverage_json` →
  `_build_executed_lines_index` → `_symbol_covered_by_coverage` pipeline, and
  the static-graph enrichment in `_enrich_blast_radius` /
  `_enrich_who_should_test`) and `rfcs/0003-coverage-aware-test-gap.md` (the
  consume-don't-trace decision + the Codex-P1 declaration-line subtlety). TSA is
  Python; Mycelium is Rust — this is a **port-concept**, re-grounded on
  Mycelium's graph metrics, not a source dependency.

## Summary

Add a **coverage-aware test-gap** capability that consumes an **external
coverage artifact** (coverage.py `coverage.json`, or lcov) as ground truth for
"is this symbol exercised?", then **enriches** each untested symbol with
Mycelium's static graph — who-calls, blast-radius, degree — to answer the
question coverage tools can't: *"what should I test next, ordered by reach?"*
Coverage tells you **what is untested**; the graph tells you **which untested
thing matters most**. TSA proved the consume-don't-trace approach
(RFC-0003, implemented); Mycelium re-grounds it on a real call graph instead of
naming heuristics, and ships the ranker as a **pure testable core** first.

## Motivation

`coverage.py` is precise because it does **runtime tracing** — a purely static
tool cannot match it (indirect dispatch, reflection, dynamic calls are only
knowable at run time). That is a paradigm boundary, not a gap to close. The
correct move (TSA `rfcs/0003` §Motivation) is to **stand on coverage.py's
shoulders** and add what it lacks: the call/ownership graph around each gap.

Raw `coverage.json` gives a flat list of uncovered lines. It has **no notion of
importance**: a 0%-covered leaf helper nobody calls and a 0%-covered function on
the critical path of forty callers look identical. An agent told "improve
coverage" then wastes effort on trivial gaps. Mycelium already computes exactly
the missing signal — `mycelium_impact` (blast-radius), callers/callees,
`degree_centrality`. Joining coverage (the *untested* set) with the graph (the
*reach* of each symbol) yields a **ranked worklist**: untested **and**
high-impact first. This is novel relative to coverage.py (no graph) and to
codegraph/Sourcegraph (no coverage notion) — the same differentiator TSA
recorded.

This stays **100% static**: Mycelium *reads* an artifact the user/CI already
produced. It never runs the target's test suite, consistent with ADR-0010.

## Decision: pure scorer/ranker core + thin adapter (phased)

Follow the RFC-0114 shape — build the **testable pure core over a plain metrics
struct** first, defer all Store/CLI/MCP wiring to Phase 2.

### Phase 1 — pure core (no I/O, no Store, no surface)

A pure module `test_gap` operating on plain inputs, fully unit-testable with
hand-built fixtures and **no graph, no filesystem, no coverage parsing**:

```
struct SymbolSpan {            // one production symbol, from the graph
    name: String,
    file: String,
    decl_line: u32,            // the def/class/fn signature line
    body_start: u32,           // first BODY line (decl_line + signature/decorator span)
    end_line: u32,
}

struct CoverageFacts {         // parsed from the external artifact, normalised
    executed_lines: BTreeMap<String /*file*/, BTreeSet<u32>>,
}

struct GraphReach {            // pulled from the Store in Phase 2
    blast_radius: u32,         // mycelium_impact / transitive callers
    in_degree: u32,            // direct callers
    degree_centrality: f64,
}

struct TestGap {
    symbol: SymbolSpan,
    is_tested: bool,
    rank_score: f64,
    why: GapReason,            // why-untested + why-it-matters, for agent output
}
```

Two pure functions, mirroring TSA's `_symbol_covered_by_coverage` and its
priority scoring:

1. **`is_covered(span, facts) -> bool`** — a symbol is tested iff **≥1 body
   line** in `[body_start, end_line]` is in `executed_lines` for its file.
   **Declaration/decorator lines are excluded** (see below).
2. **`rank(gaps, reach) -> Vec<TestGap>`** — untested symbols ranked by graph
   reach. Score = a monotonic combination of `blast_radius`, `in_degree`, and
   `degree_centrality` (e.g. `blast_radius` dominant, centrality as tiebreak).
   Tested symbols are dropped from the worklist. Deterministic, total ordering.

> **Why exclude declaration lines (TSA Codex-P1 on #284).** coverage.py records
> the `def`/`class`/decorator **statement** line as executed on mere **import**.
> If we counted *any* executed line in `[decl_line, end_line]`, a function that
> is imported but whose body is never called would read as **covered** — hiding
> exactly the untested code this feature exists to surface. So coverage is
> measured **only on body lines**: `body_start = decl_line + signature/decorator
> span`, derivable from the tree-sitter node Mycelium already has for the symbol.
> A symbol whose **only** executed line is its `def` is a GAP, not covered.
> (TSA `test_gap_analyzer.py` — `_symbol_covered_by_coverage` starts the
> body at `sym.line + 1`; Mycelium generalises this to a real signature span so
> multi-line signatures and decorators are handled.)

This core has zero dependencies on the rest of the engine and is where the TDD
RED tests live (see acceptance criteria). It is the entire reusable algorithm.

### Phase 2 — thin adapter + Three-Surface wiring

A thin Store adapter:
- enumerates `SymbolSpan`s from the graph (decl/body/end lines already in the
  attribute store),
- fills `GraphReach` from existing public metrics (`mycelium_impact` for
  blast-radius, callers for `in_degree`, `degree_centrality`),
- and feeds the pure core. **No new graph metric is invented** — this composes
  what RFC-0114 / impact already expose.

## Coverage artifact handling (consume, never execute)

- **Inputs**: `coverage.json` (coverage.py schema: `files.<path>.executed_lines`
  — TSA `_build_executed_lines_index`) and **lcov** (`DA:<line>,<hits>` records →
  executed = hits>0). Both parse to the same normalised `CoverageFacts`. Format
  is detected by extension/content; the parser is a small per-format adapter,
  language-agnostic.
- **Path normalisation**: coverage paths are stored relative to the run's
  working dir; normalise both sides to project-root-relative, with a basename
  fallback for short-form lookups (TSA `_build_executed_lines_index`).
- **Discovery**: explicit `--coverage` path → `coverage.json` at project root →
  error asking the user to generate one. No magic execution.
- **Error handling (never hard-fail destructively, but never fake a verdict)**:
  malformed/missing artifact → a clear, actionable error (Mycelium has no
  naming-heuristic fallback worth shipping — unlike TSA, our ground truth is the
  artifact, so absence is a user error, not a silent downgrade). Stale artifact
  (mtime older than newest source) → **warn and proceed**, surfacing the
  artifact path + mtime so the caller can judge. Entries for files no longer in
  the graph → skipped. We **never** read the binary `.coverage` SQLite DB
  (SQLite is forbidden in this codebase per Tool Preferences; JSON/lcov only).

## Three-Surface compliance (Charter §5.13) — this IS a new capability

Unlike RFC-0113 (additive field on existing output), this adds a **new
user-facing capability**, so Phase 2 MUST land all three surfaces:

- **CLI** ↔ **MCP** strict **1:1**: one command/tool, byte-identical name,
  description, args, and JSON output. Working shape:
  `test-gap` / `mycelium_test_gap` with
  `{ coverage: <path>, mode: "gaps"|"summary", language?, max_gaps?, top? }`,
  returning the ranked `TestGap[]` plus a summary (counts, coverage_source,
  artifact path). Output defaults follow RFC-0094 (MCP stdio → text; CLI →
  JSON), identical fields on both surfaces.
- **Skill (N:1 covered)**: the `(CLI, MCP)` pair MUST appear in ≥1
  `skills/<category>/SKILL.md` `allowed-tools` — co-located with the existing
  health/quality category Skill. No orphan, no Skill-only.

No surface ships in Phase 1 (pure core only), matching RFC-0113/0114's deferral.

## Acceptance criteria

**Phase 1 — pure core (TDD, RED first):**
- [ ] `is_covered`: body line executed ⇒ tested; **only the `def`/decorator line
      executed ⇒ GAP** (Codex-P1 declaration-line immunity); no line executed ⇒
      GAP; multi-line signature + decorators handled via `body_start`.
- [ ] `is_covered` indirect-dispatch immunity: a symbol with **no** matching
      test name but a body line executed in the artifact ⇒ tested (proves we
      beat naming heuristics; TSA's fake-test/indirect-dispatch fixtures ported).
- [ ] `rank`: untested high-blast-radius symbol ranks above untested leaf;
      tested symbols excluded; ordering deterministic and total (centrality
      tiebreak stable).
- [ ] Pure core has **no** Store/FS/coverage-parse dependency; ≥90% line
      coverage on the module (Charter quality gate).

**Phase 2 — adapter + Three-Surface:**
- [ ] `coverage.json` and lcov parse to identical `CoverageFacts`; path
      normalisation + basename fallback; stale-artifact warning; malformed →
      actionable error (no faked verdict).
- [ ] Store adapter fills `GraphReach` from `mycelium_impact` / callers /
      `degree_centrality` — no new metric invented.
- [ ] CLI `test-gap` ↔ MCP `mycelium_test_gap` byte-identical (name, desc, args,
      JSON); parity test green.
- [ ] `(test-gap, mycelium_test_gap)` covered by a category `SKILL.md`
      `allowed-tools`; no orphan.
- [ ] Dogfood: run on Mycelium itself with a real `coverage.json`; confirm the
      top-ranked gaps are genuinely high-reach untested symbols (report the list
      in the PR).

## Alternatives considered

- **Run the test suite / live tracing inside Mycelium.** Rejected: duplicates
  coverage.py, requires executing the target's code, and abandons the static
  identity. **ADR-0010** forbids language-server subprocesses and the
  consume-don't-execute principle extends here — we read artifacts, we don't run
  the world. (TSA reached the same conclusion, `rfcs/0003` §Alternatives.)
- **Live LSP for precise symbol/coverage mapping.** Rejected outright by
  **ADR-0010** (no live LSP). The tree-sitter symbol spans Mycelium already has
  are sufficient for body-line ranges.
- **Naming-convention heuristic** (`test_foo` ⇒ `foo` tested), TSA's pre-0003
  fallback. Rejected as primary: two-way errors (a `test_workflow` covering
  `foo` reads as a gap; an assert-nothing `test_foo` hides a real gap). The
  external artifact is line-precise; naming is not worth shipping as a fallback
  in a graph-native engine.
- **Flat coverage report with no graph join.** Rejected — that's just
  coverage.py. The whole value is ranking by reach; without the graph there is
  no reason for this to live in Mycelium.
- **Branch-coverage gaps.** Deferred (line coverage only for v1, as TSA).

## Conflicts with binding constraints

- **ADR-0010 (no live LSP / consume external artifacts):** ✅ fully compliant —
  reads a static `coverage.json`/lcov the user already produced; no server, no
  subprocess, never executes the target. This is the precise sanctioned lever.
- **Charter §4 (≤3-file packs):** ✅ no per-language code. Body-line spans come
  from the pack's existing tree-sitter queries; coverage parsing is
  language-agnostic. New languages get test-gap "for free" via their existing
  symbol extraction + a `coverage.json` — **data, not core edits**.
- **Charter §5.13 (Three-Surface):** addressed by construction — new capability
  ⇒ Phase 2 ships CLI ↔ MCP 1:1 + a covering Skill; no orphan, no Skill-only.
- **Tool Preferences (SQLite forbidden):** ✅ we parse JSON/lcov text only and
  never open coverage.py's binary `.coverage` SQLite DB.
- **TDD (Charter §5.1):** ✅ the pure core is RED-testable with hand-built
  fixtures before any wiring exists — the reason the core comes first.
