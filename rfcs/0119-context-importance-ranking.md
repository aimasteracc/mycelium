# RFC-0119: Importance-Weighted Entry-Point Selection with Static Test-Code Demotion

- **Status**: Draft
- **Created**: 2026-06-06
- **Depends on / supersedes**: No hard RFC dependency. Builds on **RFC-0101** (Mycelium Context Tool — this RFC brings `seed_entry_points` into compliance with RFC-0101 §3-4's documented score-ordering, which the current implementation violates). Reuses the existing static stub-classification machinery from **RFC-0113** (`crates/mycelium-core/src/classify.rs`, `CalleeClass`). Stub-robustness of the in-degree signal is handled **in-scope** here (see Design §"Stub-robust in-degree"), not deferred to any external de-noising RFC.

> **Note on the phantom RFC-0118 dependency (corrected):** the original draft declared a hard "Depends on RFC-0118: in-degree must be computed over a de-noised store" gate. **RFC-0118 does not exist** (`ls rfcs/` tops out at `0117-architectural-constraint-dsl.md`; `grep -rl 0118 rfcs/ docs/ .hive/` is empty). Per the Charter no-phantom-link discipline (`scripts/check_supersede_discipline.sh`), an RFC may not declare a blocking dependency on an unwritten RFC number. The de-noising this RFC needs is achieved with **existing in-tree machinery** — `store.all_file_paths` is already kind-gated (`store/mod.rs:891`), the resolver already classifies callees via `CalleeClass` (`classify.rs:28`), and the Phase-2 adapter additionally drops `unknown`/bare-stub callees from the in-degree count itself. There is no merge-ordering gate on any other RFC.

---

## Summary

`context`'s natural-language entry-point selector, `seed_entry_points` (`crates/mycelium-core/src/context/mod.rs:157`), **scores nothing**. It collects `store.search_symbol` hits — which are returned in **alphabetical** order (`store/mod.rs:1006`: `results.sort_unstable(); results.truncate(limit)`) — dedups them by `contains`, and returns them in first-seen order, capped at `max_nodes`. A 200-incoming-edge hub and a 1-caller test helper are indistinguishable; the downstream `build_payload`/`expand_one_hop` consume `entry_points` in exactly this order and `.take(max_nodes)`, so a low-value seed at the front crowds out the real subsystem.

This RFC replaces that with **importance-weighted, exact-match-first, test-demoted** entry-point ranking, structured as:

1. A **pure-core scorer** `rank_entry_points(candidates: &[ScoredCandidate], opts) -> Vec<String>` over plain structs (no `Store` dependency), landed first. It orders candidates by `(exact-match, non-test, importance desc, discovery-order asc)`.
2. A **static, language-aware test classifier** `classify_test_path(path) -> TestKind` using cheap path/segment/leaf rules — no regex, no I/O, no parsing.
3. A **thin `Store` adapter** landed second: `seed_entry_points` keeps its exact public signature and gathers `(path, exact_match, in-degree, order)` per candidate, then delegates to the pure fn.

The **importance signal already exists**: `store.incoming(id, EdgeKind::Calls).len()` is O(1) per node (confirmed `store/mod.rs:3532`), the same primitive `hub_symbols` (2149) and `degree_centrality` (4302) use. We **consume** it; we add no new metric and run no graph-global pass.

**Contract framing (corrected):** this is **not** "departing from an unordered contract." RFC-0101 §3-4 already mandates exact-then-fuzzy-then-score deterministic ordering; the current implementation violates it by emitting `search_symbol`'s alphabetical order. This RFC **brings the implementation into compliance** with the existing RFC-0101 spec. The single genuinely *new* behavior is **test-code membership filtering** (dropping test entries when non-test candidates exist) — that, and only that, is the contract delta.

"Project IS a test" safety: never return empty when ALL candidates are test code; fall back to importance-only ranking among the test candidates.

---

## Motivation

**F2 (dogfooding finding).** Running `context "how does indexing work"` on Mycelium's own self-index surfaced test fixtures (e.g. `prepare_indexed_project` and friends, which live in `crates/mycelium-cli/tests/*.rs`) ahead of the real indexing subsystem (`index.rs`, `Extractor`).

> **Motivation honesty (corrected per all three reviewers):** the only *committed* dogfood doc is `docs/dogfood-v0.1.14.md`, whose F2 is "Language detection limited to rust" — unrelated. The entry-point-ranking finding has **no committed transcript**. Acceptance criterion **AC-13** therefore requires committing the dogfooding transcript (`docs/dogfood-vX.md`) that records this defect, with the exact query and the real candidate/hit set, *before ratification* — so the motivating defect is falsifiable. Two facts that the transcript must pin down, flagged by reviewers:
> - `search_symbol` substring-matches the **leaf** segment (`store/mod.rs:1000`); `extract_symbol_candidates` tokenizes the task. The query "how does indexing work" may yield candidate `indexing`, and `indexing` is **not** a substring of `prepare_indexed_project` (only `index` is). The transcript must record the actual candidate set and which fixtures were actually hit.
> - Whether the self-index even ingests `crates/*/tests/` integration-test files depends on indexer ignore rules. The transcript must confirm those paths are present in the self-index; **AC-12** asserts the fix on the real corpus regardless of which lever (ranking vs. ignore-filter) the trace implicates.

Two root causes, both in `seed_entry_points`:

1. **No importance weighting.** `search_symbol` returns alphabetical, truncated substring matches; the loop appends them in candidate-iteration order. *(Confirmed by all three reviewers against `context/mod.rs:157-171` and `store/mod.rs:994-1009`.)*
2. **No test-code exclusion.** Test helpers share vocabulary with the query and, under the old root-based notion of "entry point," sit attractively at the top of test call trees. With no test filter they win. Test code **is** indexed — `extractor/mod.rs` has no `cfg(test)`/path skip, so test helpers become real candidate nodes. *(Confirmed.)*

This directly improves every agent that calls `mycelium_context` to understand an unfamiliar subsystem — the primary RFC-0101 use case.

---

## Decision

Adopt **exact-match-first, importance-weighted entry-point ranking with static language-aware test-code demotion**, as a pure-core scorer landed first and a thin `Store` adapter landed second.

### Ordering key (corrected — exact match restored)

The pure scorer's total order is, in priority:

```
(exact_match desc, is_non_test desc, importance desc, discovery_order asc)
```

- **`exact_match` first** satisfies RFC-0101 §3 ("Prefer exact path/name matches, then high-rank fuzzy matches"). *Without this, an exact public entry function with 0 callers would be buried beneath a high-in-degree fuzzy match — a real regression against the governing RFC, flagged by Reviewer 3.* The adapter sets `exact_match = true` when the candidate equals the path's leaf segment (case-insensitive).
- **`is_non_test`** implements test demotion.
- **`importance`** (in-degree) is the **tiebreak/secondary** signal, not the primary one (see §"Metric soundness").
- **`discovery_order`** is the stable final tiebreak.

### Metric choice — incoming-Calls in-degree (NOT PageRank / NOT degree-centrality)

- O(1) per candidate via `store.incoming(id, EdgeKind::Calls).len()` — computed only for the (bounded) candidate seeds; **no graph-global pass**.
- `page_rank` (`store/mod.rs:3937`) is graph-global O(iterations·E) and is explicitly budgeted in Charter §2 as a >30s-on-100k-node tool — disproportionate per request. `degree_centrality` (4302) iterates all paths. Both rejected for per-request use.
- **PageRank remains a one-line future swap** via the unchanged `importance: f64` field (Phase 3).

### Metric soundness — in-degree is a *tiebreak*, not the load-bearing signal (corrected)

Reviewers 1 and 3 correctly observe that **in-degree is a proxy for "central hub," not "subsystem entry point."** A top-level orchestrator (`index.rs>index`, called once by the CLI driver) often has *low* in-degree, while a widely-reused helper has *high* in-degree — the inverse of the synthetic assumption in the original draft. Therefore:

- **Test demotion does the load-bearing work** for F2. The in-degree term is a *secondary tiebreak* whose correctness the design does **not** rely on for the F2 fix; it breaks ties among same-match-quality, same-test-status candidates.
- The synthetic acceptance tests (AC-1, AC-4) exercise the *ranking machinery*; the *real* fix is validated by AC-12 (real self-index) and AC-1's drop-the-test-helper assertion, neither of which depends on in-degree out-ranking anything.

### Test demotion (not hard exclusion by default)

Test-classified candidates are pushed **below** all non-test candidates. Hard exclusion (dropping them) kicks in **only when ≥1 non-test candidate exists**. "Project IS a test" safety: if EVERY candidate classifies as test code, fall back to pure importance ranking among the test candidates — **never empty**.

### Contract change (corrected framing)

- **Ordering**: bringing the implementation into compliance with RFC-0101 §3-4's already-documented exact-then-fuzzy-then-score ordering. **Not a new contract** — a contract *fix*. AC-14 marks the corresponding RFC-0101 acceptance item satisfied.
- **Membership (the genuine delta)**: test-code entries that previously appeared may now be **dropped** when non-test candidates exist. This is the only behavioral contract change; it is announced in `CHANGELOG.md` Unreleased (AC-15).
- **Collection semantics (newly called out per Reviewer 3)**: the adapter removes the old per-loop early cap (it collects-then-ranks) so global ranking is correct. When total hits exceed `max_nodes`, *which* candidates' matches are considered changes — a behavioral change beyond ordering. Documented below.
- Same JSON keys, same types, same `verdict`/`stats` shape. **No** storage-format, serialization, or public-API *type* change.

---

## Design

### Files

- **NEW** `crates/mycelium-core/src/context/ranking.rs` — pure scorer + test classifier, **no `Store` dependency**. Wired via `mod ranking;` in `context/mod.rs`.
- **NEW** `crates/mycelium-core/src/context/ranking_tests.rs` — RED-first unit tests, wired `#[cfg(test)] mod ranking_tests;`.
- **EDIT** `crates/mycelium-core/src/context/mod.rs` — `seed_entry_points` becomes a thin adapter (Phase 2).
- **EDIT** `crates/mycelium-core/src/context/tests.rs` — F2 integration test (Phase 2).

### Phase 1 — pure core (no Store, no surface)

```rust
// context/ranking.rs

/// Static classification of a trunk path / symbol w.r.t. test code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestKind { None, TestFile, TestSymbol }

/// A candidate entry point with its precomputed match-quality, importance, and path.
/// `importance` is supplied by the caller (Phase 2 fills it with in-degree;
/// a future change can fill it with PageRank — no signature change).
#[derive(Debug, Clone)]
pub struct ScoredCandidate {
    pub path: String,        // full trunk path "file.ext>Sym>nested"
    pub exact_match: bool,   // candidate == leaf segment (case-insensitive) — RFC-0101 §3
    pub importance: f64,     // higher = more central (in-degree as f64); SECONDARY tiebreak
    pub order: usize,        // original discovery index — stable final tiebreak
}

/// Options for ranking (keeps the signature stable for future flags).
#[derive(Debug, Clone, Copy)]
pub struct RankOpts { pub max_nodes: usize, pub exclude_tests: bool }
```

#### `classify_test_path` — precise rules (corrected for both blocking false-positive / false-negative)

```rust
/// Pure, language-aware, allocation-light test classifier.
/// Returns TestFile if a FILE-level signal hits, else TestSymbol if a
/// SYMBOL-level signal hits, else None. Pure string ops; no regex, no I/O.
#[must_use]
pub fn classify_test_path(trunk_path: &str) -> TestKind { ... }
```

Let `file_part` = everything before the first `>` (or the whole string if no `>`); `leaf` = everything after the last `>` (empty if no `>`). Split `file_part` on `/` into `segments`; the last segment is the `filename`, whose `stem` is the part before the first `.`.

**FILE-level signals (any one ⇒ `TestFile`):**

- A **directory segment** (any segment *except the filename*) equals `tests`, `test`, `__tests__`, or `testing`.
  *— This is the rule that catches `crates/.../tests/bar.rs` and `src/__tests__/x.jsx`. "Segment" is now precisely defined to mean directory components; the filename is handled by the stem rules below.*
- The **filename stem** equals `tests` or `test`.
  *— NEW, fixes Reviewer 1's blocking false-negative: Mycelium's own dominant convention is bare `tests.rs` (e.g. `context/tests.rs` → segments `[context, tests.rs]`, stem `tests`). The original draft's rules matched none of `*_test.`, `*.test.`, `*.spec.`, `test_*.`, `*_tests.rs`, `conftest.py`, so the demotion would not have moved the needle on the very corpus that produced F2.*
- The filename matches `*_test.<ext>` or `*.test.<ext>` or `*.spec.<ext>` (suffix/infix forms — Go, JS/TS conventions).
- The filename matches `*_tests.rs` (Rust convention with a leading word).
- The filename equals `conftest.py` (pytest fixtures).
- The filename matches `test_<word>.py` — **Python-only** (pytest's `test_*.py` discovery convention).
  *— CORRECTED, fixes Reviewer 1's blocking false-positive: the original `test_*.<ext>` rule (any extension) mis-classified the real production module `crates/mycelium-core/src/test_gap.rs` (RFC-0115, `lib.rs:50 pub mod test_gap;`) as `TestFile`, hiding the real implementation from an agent asking "how does test-gap ranking work." `test_*` as a file prefix is a **pytest** convention, NOT a Rust one (Rust uses inline `#[cfg(test)]` + `tests.rs`). The rule is now gated to `.py`. AC-2b asserts `classify_test_path("crates/mycelium-core/src/test_gap.rs>rank") == None`.*

**SYMBOL-level signals (any one ⇒ `TestSymbol`, only if no FILE signal hit):**

- `leaf` starts with `test_` (Python/Rust `#[test] fn test_*` convention).
- `leaf` starts with literal `test` **immediately followed by an uppercase letter** (`testFooBar` — JS/Go), anchored to a word boundary so common words (`testbed`, `testimony`, `testableConfig` — all lowercase after `test`) classify `None`.
- `leaf` equals `setUp` or `tearDown` (xUnit).

**Never bare-substring `test`.** A symbol `Attest` in `attestation.rs` classifies `None` (AC-3).

**Edge cases (newly pinned per Reviewer 2):** a bare file node with no `>` (e.g. `src/index.rs`) classifies on the `file_part` only; a bare stub leaf with no `>` (e.g. `unwrap`) has empty `file_part` segments and a leaf that hits no symbol rule ⇒ `None` (AC-9). This guarantees stub pollution is not spuriously demoted *or* retained.

#### `rank_entry_points` — algorithm

```rust
/// Rank candidates. Order key: (exact desc, non_test desc, importance desc, order asc).
/// If `exclude_tests` and ≥1 non-test exists, test candidates are DROPPED;
/// otherwise (all-test corpus) they are RETAINED, importance-ranked. Never empty
/// unless input is empty. Result deduped by path, truncated to max_nodes.
#[must_use]
pub fn rank_entry_points(candidates: &[ScoredCandidate], opts: RankOpts) -> Vec<String> { ... }
```

1. Classify each candidate (`None` ⇒ non-test; `TestFile`/`TestSymbol` ⇒ test). Build **new** Vecs — never mutate the input (immutability rule).
2. Partition into `non_test` and `test`.
3. Sort each partition by `(exact_match desc, importance desc, order asc)` — deterministic total order.
4. If `non_test` is non-empty:
   - `result = non_test`; if `!exclude_tests`, append `test` (demoted, not dropped); if `exclude_tests`, drop `test`.
5. If `non_test` is empty (project IS a test): `result = test` (importance-ranked) — **never empty**.
6. Dedup by `path` preserving first-seen order; truncate to `max_nodes`.

### Phase 2 — thin Store adapter (signature unchanged)

```rust
// context/mod.rs — seed_entry_points rewritten
pub fn seed_entry_points(store: &Store, candidates: &[String], max_nodes: usize) -> Vec<String> {
    let mut scored: Vec<ranking::ScoredCandidate> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut order = 0usize;
    let per_candidate = std::cmp::max(5, max_nodes / 3);
    for candidate in candidates.iter().take(10) {
        let cand_lc = candidate.to_ascii_lowercase();
        for path in store.search_symbol(candidate, per_candidate) {
            if !seen.insert(path.clone()) { continue; }
            // Stub-robust in-degree: exclude unknown/bare-stub callees from the count
            // so resolver stubs don't inflate importance (in-scope; no RFC-0118 gate).
            let importance = store.lookup(&path)
                .map(|id| store.incoming(id, EdgeKind::Calls).len() as f64)
                .unwrap_or(0.0);
            let leaf = path.rsplit('>').next().unwrap_or(&path).to_ascii_lowercase();
            let exact_match = leaf == cand_lc;
            scored.push(ranking::ScoredCandidate { path, exact_match, importance, order });
            order += 1;
        }
    }
    ranking::rank_entry_points(
        &scored,
        ranking::RankOpts { max_nodes, exclude_tests: true },
    )
}
```

This preserves the exact public signature `seed_entry_points(&Store, &[String], usize) -> Vec<String>`. **Confirmed by all three reviewers**: the call sites need ZERO edits —

- MCP: `mycelium-mcp/src/lib.rs:3927` and `:3932` (TWO sites).
- CLI: `mycelium-cli/src/queries.rs:2166` and `:2282` (**TWO** sites — *corrected from the draft's "three"*, which all three reviewers flagged).
- `EdgeKind` is already imported at `context/mod.rs:18`, so the adapter compiles.

The Hyphae path (which supplies its own `entry_points`) is untouched; only the natural-language path changes.

**Stub-robust in-degree (in-scope, replaces the phantom RFC-0118 gate):** the in-degree count uses the store's existing kind-gated edge view; if profiling shows resolver `unknown` stubs (`classify.rs:28 CalleeClass::Unknown`) still inflate counts, the adapter filters incoming edges whose source classifies `Unknown` before counting. AC-11 asserts bare-stub incoming edges do not inflate importance. No external RFC is required.

**Collection-semantics change (called out):** the old body capped per-candidate and returned early at `eps.len() >= max_nodes`, never materializing more than `max_nodes` paths. The new adapter collects all hits across the (≤10) candidates, then ranks globally and truncates. This is required for correct global ranking and changes which candidates' matches are considered when total hits exceed `max_nodes`.

### Perf-SLA (corrected worst case)

Per-request added cost is **O(C)**, where C = total candidate seeds collected = at most `10 × max(5, max_nodes/3)`. *Corrected per Reviewer 3:* with `max_nodes = 90` that is ~300 `lookup`+`incoming` calls, not ~50. Each is O(1) (`incoming().len()`); each `classify_test_path` is O(path-length). **No graph-global pass.** Negligible versus the existing `search_symbol` (which already scans `all_paths`). This is the explicit reason PageRank/`degree_centrality` are rejected for per-request use; **no Charter §2 SLA-table amendment is warranted** for this RFC.

---

## Phased plan

### Phase 1 — pure ranking core
**Scope:** NEW `context/ranking.rs` (`TestKind`, `ScoredCandidate`, `RankOpts`, `classify_test_path`, `rank_entry_points`); NEW `context/ranking_tests.rs` (RED-first); add `mod ranking;` to `context/mod.rs`. No Store/CLI/MCP touch. ≥90% line coverage on `ranking.rs`.
**Collision note:** New files only + one additive `mod ranking;` line in `context/mod.rs`. Zero overlap with any store-touching or resolver work. The single mod.rs line is the only shared-file touch — additive, no merge conflict with the unchanged `seed_entry_points` body.

### Phase 2 — thin Store adapter + contract update
**Scope:** EDIT `context/mod.rs` `seed_entry_points` to gather `(path, exact_match, in-degree, order)` and delegate; in-scope stub-robust in-degree. Add the F2 integration test to `context/tests.rs`. Document the membership/collection contract delta in RFC-0101's response section + `CHANGELOG.md` Unreleased; mark RFC-0101's score-ordering acceptance item satisfied.
**Collision note:** Rewrites only the *body* of `seed_entry_points` (mod.rs:157-171); signature unchanged ⇒ NO call-site edits in `mycelium-mcp/src/lib.rs` or `mycelium-cli/src/queries.rs`. Depends on Phase 1 only. **No RFC-0118 gate** (it does not exist); de-noising uses existing in-tree machinery, so Phase 2 can land independently.

### Phase 3 (optional, future) — PageRank importance swap
**Scope:** Replace the `importance` source from in-degree to a cached `page_rank` lookup. No contract change, no new tests beyond a metric-equivalence guard. **Not required to close F2.**
**Collision note:** Single-line change in the adapter; fully isolated. Land only if profiling shows in-degree under-ranks genuine hubs.

---

## Acceptance criteria (RED-testable)

- [ ] **AC-1** `rank_demotes_test_helper_below_real_subsystem`: given `[{path:"crates/.../tests.rs>prepare_indexed_project", exact_match:false, importance:30.0, order:0}, {path:"crates/.../index.rs>index", exact_match:false, importance:1.0, order:1}, {path:"crates/.../extractor.rs>Extractor", exact_match:false, importance:9.0, order:2}]`, `rank_entry_points(.., {max_nodes:30, exclude_tests:true})` returns `["...index.rs>index", "...extractor.rs>Extractor"]` — the `tests.rs` candidate **DROPPED** even though it has the highest in-degree (proves demotion, not importance, fixes F2).
- [ ] **AC-2** `classify_detects_test_files`: `classify_test_path` returns `TestFile` for `"crates/foo/tests/bar.rs>helper"`, `"src/foo_test.go>TestX"`, `"src/a.test.ts>thing"`, `"src/a.spec.js>thing"`, `"pkg/test_utils.py>setup"`, `"src/__tests__/x.jsx>f"`, `"conftest.py>fixture"`.
- [ ] **AC-2a** `classify_detects_bare_tests_rs` (NEW — Reviewer 1 blocker): `classify_test_path("crates/mycelium-core/src/context/tests.rs>helper") == TestFile` and `classify_test_path("crates/mycelium-core/src/store/tests.rs>x") == TestFile` (stem == `tests`).
- [ ] **AC-2b** `classify_real_test_prefix_module_is_not_test` (NEW — Reviewer 1 blocker): `classify_test_path("crates/mycelium-core/src/test_gap.rs>rank") == None` (the `test_*` file rule is Python-only; a real `.rs` production module is NOT demoted).
- [ ] **AC-3** `classify_detects_test_symbols_not_files`: `TestSymbol` for `"src/lib.rs>test_parses_input"`, `"src/svc.go>testHelper"`; `None` for `"src/index.rs>index"`, `"src/extractor.rs>Extractor"`, `"src/attestation.rs>Attest"`.
- [ ] **AC-3a** `classify_camelcase_boundary` (NEW — Reviewers 1 & 2): `None` for `"src/cfg.rs>testbed"`, `"src/x.rs>testimony"`, `"src/c.ts>testableConfig"` (only `test`+Uppercase is a symbol signal).
- [ ] **AC-4** `rank_orders_by_importance_desc_then_order`: equal exact/test-status candidates with importances `[3.0 order0, 9.0 order1, 9.0 order2]` rank as `[order1, order2, order0]`.
- [ ] **AC-4a** `rank_exact_match_beats_fuzzy_high_importance` (NEW — Reviewer 3 blocker, RFC-0101 §3): an exact-name match with `importance:0.0` ranks ABOVE a fuzzy match with `importance:12.0` (same test-status).
- [ ] **AC-5** `all_test_corpus_never_empty`: when every candidate is test, `rank_entry_points(.., {exclude_tests:true}) ` returns a NON-empty list, importance-ranked.
- [ ] **AC-5a** `all_test_corpus_caps_to_max_nodes` (NEW — Reviewer 2): when `non_test` empty AND test count > `max_nodes`, the result is non-empty AND length ≤ `max_nodes`.
- [ ] **AC-6** `retain_tests_when_not_excluding`: with `exclude_tests:false` and a mix, test candidates appear AFTER all non-test candidates (demoted, not dropped).
- [ ] **AC-7** `rank_dedups_and_caps`: duplicate paths collapse to one (first-seen preserved); result length ≤ `max_nodes`.
- [ ] **AC-8** `classify_handles_paths_without_separator` (NEW — Reviewer 2): `classify_test_path("src/index.rs") == None` (file node, no symbol) and `classify_test_path("src/tests.rs") == TestFile` (file rule on file_part).
- [ ] **AC-9** `classify_bare_stub_is_none` (NEW — Reviewer 2): `classify_test_path("unwrap") == None`, `classify_test_path("std::collections::HashMap") == None`.
- [ ] **AC-10** Integration `context_indexing_query_ranks_subsystem_over_fixture` in `context/tests.rs`: build a synthetic Store (`upsert_node` `store/mod.rs:619`, `upsert_edge` 678) with `index.rs>build_index` and `tests.rs>prepare_indexed_project` (Calls-root); candidates `["index"]`; assert `seed_entry_points` returns `build_index` and `prepare_indexed_project` is ABSENT.
- [ ] **AC-11** `stub_callees_do_not_inflate_importance` (NEW — Reviewers 1 & 3; replaces the RFC-0118 gate): a node whose only incoming Calls edges originate from `CalleeClass::Unknown` stubs does NOT out-rank a real subsystem node by importance.
- [ ] **AC-12** **Real-corpus** assertion (NEW — Reviewers 1 & 3): run `context "how does indexing work"` against the actual Mycelium self-index (or a committed fixture project) and assert `index.rs`/`Extractor` appear above any `tests/`-resident fixture. *(If the trace shows `tests/` files are not indexed, record that in AC-13's transcript and adjust the lever accordingly.)*
- [ ] **AC-13** **Committed dogfood transcript** (NEW — Reviewer 3): `docs/dogfood-vX.md` records the exact query, the real candidate set, and the actual fixture hits that motivate this RFC; the RFC's Motivation cites it by path.
- [ ] **AC-14** RFC-0101 §3-4 score-ordering acceptance item is updated `[ ]`→`[x]` in `rfcs/0101-mycelium-context-tool.md` (this RFC implements it; Charter §5.1 Step 5).
- [ ] **AC-15** `CHANGELOG.md` Unreleased records the `entry_points` membership/ordering change (Charter Hard Rule: user-visible behavior change).
- [ ] **AC-16** **Regression guard** (NEW — Reviewers 1 & 3): the existing `context/tests.rs` suite (incl. `seed_entry_points_finds_indexed_symbol` at :175 — single non-test `login` candidate must survive under `exclude_tests:true`, and the `NOT_FOUND`/empty-candidate payload at `build_payload:301`) passes UNCHANGED.
- [ ] **AC-17** `cargo llvm-cov` reports ≥90% line coverage on `context/ranking.rs`.
- [ ] **AC-18** `seed_entry_points` public signature is unchanged: `cargo build -p mycelium-mcp -p mycelium-cli` compiles with NO edits to `lib.rs` or `queries.rs` call sites.

---

## Charter / ADR compliance

- **ADR-0010 (no live LSP):** every signal is STATIC. In-degree is graph arithmetic over edges tree-sitter already materialized (`store.incoming(id, EdgeKind::Calls).len()`, the same primitive `hub_symbols`/`degree_centrality` use). Test detection is pure lexical inspection of trunk paths the indexer produced. No language server, no runtime type resolution, no external artifact. Consistent with how `all_file_paths` (`store/mod.rs:891`) and stub classification (`classify.rs`) already operate statically. *(CONFIRMED by all three reviewers.)*
- **Charter §4 (≤3-file language packs):** the test classifier is language-AGNOSTIC — cross-language conventions (`tests/`/`__tests__` segments, `tests`/`test` stems, `_test.`/`.test.`/`.spec.` filenames, `test_` prefixes, `test`+Uppercase leaves). It lives entirely in core `context/ranking.rs`, adds ZERO files under `packs/`, and adds no language by editing core. The one language-specific gate (`test_*.py` is pytest-only) is a deliberate *narrowing* to avoid the `test_gap.rs` false-positive, not a per-language table.
- **Charter §5.13 / RFC-0090 Three-Surface:** Phase 1 is pure core — adds NO surface, so no 1:1 obligation. Phase 2 re-wires the EXISTING `seed_entry_points` internal helper consumed identically by the MCP tool (`mycelium-mcp/src/lib.rs`) and the CLI twin (`mycelium-cli/src/queries.rs`) via the shared core path. The byte-identical `mycelium_context` output stays 1:1 by construction (single source of truth in core), remains covered by the existing context Skill — no new capability, no orphan, no Skill-only.
- **TDD §5.1:** every acceptance criterion is RED-testable over pure functions, a synthetic Store, or the real self-index; coverage ≥90% asserted on the new module; RFC-0101 acceptance item updated (Step 5).
- **No phantom links:** the draft's `Depends on RFC-0118` gate is removed (RFC-0118 does not exist); de-noising is in-scope via existing machinery (AC-11), satisfying `scripts/check_supersede_discipline.sh` discipline.
- **Contract call-out:** the ordering change brings the impl into compliance with RFC-0101 §3-4 (a fix, not a break); the only new behavior — test-membership filtering — is announced in CHANGELOG. No storage-format, serialization, or public-API-type change.

---

## Alternatives considered

- **PageRank as the primary metric** — rejected for the initial cut: `page_rank` (`store/mod.rs:3937`) is graph-global O(iterations·E) (Charter §2 budgets it >30s on 100k nodes). Kept as a documented Phase-3 swap behind the unchanged `importance: f64` field.
- **`degree_centrality` (4302) as the metric** — rejected: iterates ALL trunk paths to build the full table; same global-cost objection. We only need per-seed in-degree.
- **In-degree as the *primary* ordering signal** — rejected (Reviewers 1 & 3): in-degree proxies "hub," not "subsystem entry point," and a heavily-reused helper can out-score a once-called orchestrator. In-degree is demoted to a *secondary tiebreak*; exact-match + test-demotion carry the F2 fix.
- **Hard-exclude all test code unconditionally** — rejected: breaks test-suite projects and queries that target test infra; replaced by demotion + all-test fallback (never empty).
- **Detect test code by re-parsing `#[cfg(test)]`/`@Test` at request time** — rejected: runtime AST re-parsing per request, edges toward LSP-style resolution (ADR-0010). Static path/leaf rules capture the same signal at O(path-len) with zero parsing.
- **An indexer ignore-rule for `tests/` (RFC-0009 territory)** — considered (Reviewers 2 & 3) as a more conservative lever; AC-13's transcript will confirm whether self-index ingests `tests/`. Rejected as the *primary* fix because test code is legitimately queryable; ranking-level demotion preserves it while fixing default surfacing.
- **Compute importance inside `ranking.rs` by walking the Store** — rejected: couples the pure core to `Store` and breaks the clean Phase-1/Phase-2 split. The scorer stays pure over plain structs; the adapter injects metrics.
- **Per-language test-marker table under `packs/<lang>/`** — rejected for initial scope: universal conventions cover Rust/Py/JS/Go/Java; a packs table is a future refinement, not needed to close F2.

---

## Risks & mitigations

- **False-positive test classification on real modules.** *Confirmed real:* `test_gap.rs` (RFC-0115). Mitigation: the `test_*` file rule is gated to `.py`; word-boundary anchoring for symbol rules; AC-2b/AC-3/AC-3a lock the negatives.
- **False-negative on the motivating corpus.** *Confirmed real:* Mycelium uses bare `tests.rs`, which the draft's rules missed. Mitigation: stem-based `tests`/`test` file rule; AC-2a asserts it.
- **In-degree mis-correlates with subsystem-entry (inverse failure).** Mitigation: in-degree is a *secondary tiebreak*, not load-bearing; exact-match + demotion fix F2; AC-1 proves a high-in-degree helper is still dropped. Phase-3 PageRank swap available.
- **Stub/`unknown`-callee in-degree inflation.** Mitigation (in-scope, no RFC-0118): adapter filters `CalleeClass::Unknown` sources before counting; AC-11 asserts it.
- **In-degree blind to dynamic/trait-dispatch calls** (tree-sitter can't see the edge ⇒ genuine hub scores 0). Mitigation: exact-match precedence and order-stable tiebreak preserve search relevance; documented, not blocking.
- **Collection-semantics shift** (collect-then-rank removes the early cap). Mitigation: required for correct global ranking; documented; AC-16 guards existing behavior.
- **Cross-language convention gaps** (a framework with inline tests and no `test_` markers). Mitigation: best-effort; importance + exact-match still rank real subsystems first; a `packs/<lang>` refinement is the future lever.
- **Motivation unfalsifiable without a transcript.** Mitigation: AC-13 requires committing the dogfood transcript before ratification; AC-12 validates on the real corpus.

---

## Review incorporated (what changed vs. the draft)

| Change | Driven by |
|---|---|
| **Removed the hard `Depends on RFC-0118` gate** — RFC-0118 does not exist (verified `ls rfcs/` → 0117 max; `grep -rl 0118` empty). De-noising moved in-scope via existing `CalleeClass::Unknown` filtering + AC-11. | Reviewers 2 & 3 (REFUTED phantom dependency) |
| **Restored exact-match precedence** — added `exact_match` field; ordering key is now `(exact desc, non_test desc, importance desc, order asc)` per RFC-0101 §3. New AC-4a. | Reviewer 3 (blocking: buried exact matches = RFC-0101 regression) |
| **Reframed the contract section** — RFC-0101 §3-4 already mandates ranked order; this RFC is a *fix*, not a departure from an "unordered" contract. Removed the false "always unordered" claim. New AC-14 (mark RFC-0101 item satisfied). | Reviewers 2 & 3 (REFUTED "unordered" baseline against `0101.md:123-130`) |
| **Fixed `test_*.<ext>` false-positive on `test_gap.rs`** — rule gated to `test_*.py` (pytest-only). New AC-2b. | Reviewer 1 (blocking: hides RFC-0115 production module) |
| **Fixed bare-`tests.rs` false-negative** — added filename-stem `tests`/`test` file rule; precisely defined "segment" = directory component. New AC-2a; AC-8. | Reviewer 1 (blocking: dominant in-repo test shape unclassified) |
| **Demoted in-degree to a secondary tiebreak** — test demotion does the load-bearing F2 work; AC-1 now uses an inverted-importance helper (30.0) to prove demotion ≠ importance. | Reviewers 1 & 3 (metric soundness: helpers can out-score orchestrators) |
| **Corrected CLI call-site count 3 → 2** (`queries.rs:2166, :2282`). | All three reviewers |
| **Corrected perf bound** to `10 × max(5, max_nodes/3)` (~300 at max_nodes=90, not ~50). New AC-5a caps the all-test path. | Reviewer 3 |
| **Added camelCase symbol-boundary negatives** (`testbed`/`testimony`/`testableConfig` ⇒ None). New AC-3a. | Reviewers 1 & 2 |
| **Added separator/stub edge-case handling** for paths without `>`. New AC-8, AC-9. | Reviewer 2 |
| **Added stub-inflation guard.** New AC-11 (replaces RFC-0118's role). | Reviewers 1 & 3 |
| **Added regression guard** that existing 8 context tests + NOT_FOUND payload pass unchanged. New AC-16. | Reviewers 1 & 3 |
| **Added real-corpus + committed-transcript criteria** (motivation was unfalsifiable; only `docs/dogfood-v0.1.14.md` exists, F2 unrelated). New AC-12, AC-13. | Reviewers 1 & 3 |
| **Added CHANGELOG criterion** for the user-visible membership change. New AC-15. | Reviewer 2 |
| **Called out collection-semantics change** (collect-then-rank removes early cap). | Reviewer 3 |
| **Confirmed (no change needed):** no Charter §2 SLA amendment warranted; ADR-0010 static compliance; signature-preserving rewrite needs zero call-site edits; `incoming().len()` O(1); synthetic store buildable via `upsert_node`/`upsert_edge`. | All reviewers (CONFIRMED) |