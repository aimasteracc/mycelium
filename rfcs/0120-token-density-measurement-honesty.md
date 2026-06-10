# RFC-0120: Token-Density Measurement Honesty

- **Status**: Implemented (Phase 3B complete — CLI twin + byte-identity harness + EXCEPTION retracted)
- **Created**: 2026-06-06
- **Depends on / supersedes**: Depends on RFC-0094 (token-efficient `TextFormatter` output — this RFC makes RFC-0094's asserted headline number machine-verified and cross-links it). Reconciles RFC-0090 (Three-Surface Rule / CLI↔MCP↔Skill parity) by converting the existing `get_token_stats` `EXCEPTION: MCP-only` into a real CLI twin. Touches Charter §2 (SLA table) as a flagged governance event. Supersedes nothing.

---

## Summary

Mycelium ships two unsubstantiated, machine-unverified token-efficiency marketing claims and one verification tool that measures the wrong axis:

- `README.md:38` — "AI gets 3–4× more meaning per token."
- `README.md:71` — "Emmet-like compact DSL output. ~70% fewer tokens than JSON for the same information."
- `mycelium_get_token_stats` (`crates/mycelium-mcp/src/lib.rs:3975`) — compares JSON **text bytes** vs MessagePack **binary bytes** (a wire-format compression ratio) on a hardcoded 3-string sample, plus a single-char-key `compact_chars` trick. It never invokes a tokenizer and never touches the `TextFormatter` the marketing claim is actually about.

The governing RFC-0094 cites "562/1973 = ~72% token reduction" for a **synthetic** 50-node tree, measured once by hand; the only committed guards (`formatter.rs` `text_format_byte_count_under_80_percent_of_json_for_50_node_tree` and `benches/formatter.rs::bench_byte_ratio`) check **bytes** (<80%), with NOTE comments explicitly admitting the headline ~73% is a *token* number that "can't be linked into the unit test without tiktoken-rs." The token figure is asserted everywhere and verified nowhere.

This RFC adopts a **measure-first, claim-second** discipline. Phase 1 lands a pure-core token-accounting module (`mycelium-mcp::token_bench`) plus a committed corpus of **real** tool-output JSON captured from an indexed fixture repo, measured Text-vs-JSON with a real BPE tokenizer (tiktoken-rs `cl100k_base`, the assumption stated and committed), behind a `measure_corpus(corpus, counter) -> CorpusReport` pure function over plain structs, with a committed snapshot that pins the true measured number. Phase 2 either keeps `README:71` (if the honest number reproduces ~70%) or restates both lines to the measured figure and retracts the overclaim, and reconciles Charter §2's SLA row (a flagged governance event requiring founder sign-off if it differs). Phase 3 rewrites `mycelium_get_token_stats` onto the correct Text-vs-JSON **token** axis over the corpus, relabels the old byte ratio as a separate `wire_format_byte_ratio`, and converts the tool's documented MCP-only exception into a real CLI twin under the Three-Surface Rule. Fully static (no LSP), no new language pack, every acceptance criterion phrased as a RED-testable assertion.

---

## Motivation

F4 (founder dogfooding finding): for an explicitly AI-native, dogfooding-honest project, the first thing a skeptical adopter does is measure the headline number. Three separate honesty defects exist today, all confirmed against the merged tree by the review team:

1. **Unbacked README claims.** `README:38` and `README:71` cite per-token figures with zero committed measurement on real default-path output.
2. **Wrong-axis verification tool.** `mycelium_get_token_stats` measures JSON-vs-MessagePack **bytes** on a hardcoded sample — a wire-format compression ratio that has nothing to do with the `TextFormatter`/per-token claim. (CONFIRMED by all three reviewers against `lib.rs:3975–4015`.)
3. **Unmeasured Charter §2 SLA.** Charter §2 (`CHARTER.md:44`) asserts "AI token efficiency (Hyphae DSL vs JSON) ≤ 30% of JSON token count," resting on the same unmeasured assumption — *and* labels the formatter "Hyphae DSL" when the actual agent-visible serialization is the `TextFormatter` (TOON-inspired), a separate label-honesty defect the reviewers surfaced.

RFC-0094's "~72%" is synthetic (generated `src/module_{i}.rs>Type_{i}>method_{i}` paths), measured once, never machine-verified. This RFC makes the number true *by construction*: measure it reproducibly on real output, then keep the claim because it holds or retract/restate it to the measured figure, and reconcile both the §2 number **and** its formatter label.

---

## Decision

Adopt a measurement-first, claim-second discipline:

1. **Build a pure-core token-accounting module** that, given a corpus of real tool-output JSON values, renders each through the existing `JsonFormatter` (pretty, the agent-visible path) and `TextFormatter` (the MCP stdio default since RFC-0094 Phase 4) and counts **tokens** with a real BPE tokenizer, returning a structured `CorpusReport` with per-fixture and aggregate Text/JSON token ratios.
2. **Commit the corpus** as captured real outputs from an *indexed* fixture repo (a real source tree the capture script actually indexes — **not** the empty/unindexed `tests/contract.rs` server, which produces no real output), plus a snapshot test pinning the measured aggregate so the headline is always backed by green CI.
3. **State the tokenizer assumption explicitly** (tiktoken-rs `cl100k_base`); the claim is "per `cl100k_base` over corpus vN." A claim without a stated tokenizer is meaningless.
4. **Resolve the claim:** if the honestly-measured aggregate token reduction is ≥ ~70%, keep `README:71` and back it with the test + a "measured with cl100k_base over corpus vN" footnote; replace the vaguer `README:38` "3–4× more meaning per token" with the same concrete figure (3–4× meaning-per-token is the reciprocal of a ~70–75% reduction, so it must be *derived from*, not asserted independently of, the measured ratio). If the number does not reproduce, **restate** both lines to the true figure and **retract** the overclaim from README and any SKILL/doc/marketing copy.
5. **Rewrite `mycelium_get_token_stats`** to report the Text-vs-JSON **token** axis over the committed corpus as its primary metric, and **relabel** the old JSON-vs-MessagePack byte ratio as a clearly-named secondary `wire_format_byte_ratio` so the two axes are never conflated again.
6. **Reconcile Charter §2:** if the honest per-corpus token ratio differs from the current "≤ 30% of JSON" SLA, *propose* the §2 amendment to the measured figure and flag it as a governance event requiring founder sign-off (Charter §2 is the constitution); **also** reconcile the §2 row's "Hyphae DSL" label to name the actual `TextFormatter` output being measured.

The whole pipeline is **static** — it consumes already-built JSON tool outputs and runs a tokenizer over strings. No language server, no runtime graph mutation, no Store.

---

## Design

### Static algorithm (no LSP)

Pure string/tokenizer arithmetic over already-materialized JSON. `JsonFormatter::format` and `TextFormatter::format` are confirmed pure `&Value -> String` functions (`formatter.rs:49–52`, `:106–111`, `:121–145`), so the measurement reuses the exact agent-visible code paths with no I/O.

### NEW module `crates/mycelium-mcp/src/token_bench.rs` (pure, no I/O, no Store)

Plain structs + free functions:

```rust
/// One captured tool output.
pub struct FixtureCase { pub name: String, pub value: serde_json::Value }

pub struct FixtureReport {
    pub name: String,
    pub json_tokens: usize, pub text_tokens: usize,
    pub json_bytes:  usize, pub text_bytes:  usize,
}

pub struct CorpusReport {
    pub fixtures: Vec<FixtureReport>,
    pub total_json_tokens: usize, pub total_text_tokens: usize,
    pub total_json_bytes:  usize, pub total_text_bytes:  usize,
}
impl CorpusReport {
    pub fn token_reduction_pct(&self) -> f64;      // 100*(1 - total_text/total_json) over aggregate TOKENS
    pub fn text_to_json_token_ratio(&self) -> f64; // total_text/total_json
    pub fn byte_reduction_pct(&self) -> f64;
}

/// Abstraction so the pure fn is testable with a fake counter AND the real one.
pub trait TokenCounter { fn count(&self, s: &str) -> usize; }

/// Hermetic, dependency-light DEFAULT (split on ASCII whitespace + punctuation runs).
/// Used in core unit tests; NOT the figure-of-record.
pub struct WhitespaceTokenCounter;

/// Figure-of-record. Wraps tiktoken-rs `CoreBPE` (cl100k_base).
/// Gated behind the `tiktoken` cargo feature so core tests stay hermetic.
#[cfg(feature = "tiktoken")]
pub struct BpeTokenCounter { /* CoreBPE */ }

/// Renders JsonFormatter.format(&value) and TextFormatter::default().format(&value)
/// (REUSING formatter.rs — the agent-visible paths), counts tokens via counter.count,
/// records bytes via .len().
pub fn measure_case<C: TokenCounter>(case: &FixtureCase, counter: &C) -> FixtureReport;

/// Single pure entry point Phase 1 lands and tests; folds per-case into aggregates.
pub fn measure_corpus<C: TokenCounter>(corpus: &[FixtureCase], counter: &C) -> CorpusReport;
```

**JSON baseline is pinned (reviewer-2 / reviewer-3 concern):** `measure_case` counts the JSON side over `JsonFormatter::format` — i.e. `serde_json::to_string_pretty` (`formatter.rs:110`), the JSON an agent actually receives — **not** compact `to_string`. This removes the pretty-vs-compact apples-to-oranges escape hatch and is asserted by an acceptance criterion.

### Corpus definition (committed, reproducible)

`crates/mycelium-mcp/tests/corpus/` holds N (≥8) JSON files, each the captured `success`-payload `serde_json::Value` of a representative real tool, spanning the response shapes agents pay for: `mycelium_context` (composite), `mycelium_get_callee_tree` (deep tree), `mycelium_get_caller_tree`, `mycelium_get_subclasses_tree` (hierarchy), `mycelium_search_symbol` (flat list), `mycelium_get_symbol_info` (record), `mycelium_query` (Hyphae result), `mycelium_get_importers_tree`.

**Capture source — CORRECTED (reviewer-1 blocking, REFUTED draft):** the draft cited "the indexed source tree used by `tests/contract.rs`." That is **wrong** — `crates/mycelium-mcp/tests/contract.rs:9–11` explicitly uses an *empty, unindexed* server with catch-all args and indexes nothing, so it produces no real output to capture. The corpus is instead captured from a **real indexable tree**: `scripts/capture_token_corpus.sh` indexes `tests/e2e/fixtures/ripgrep/` (a real in-repo source tree, confirmed present) into a tempdir via `mycelium index <root>` (the exact pattern `crates/mycelium-cli/tests/cli_call_graph.rs:12–29` already uses), then invokes each tool and writes the `success` payload to `tests/corpus/<tool>.json`. The script documents tool + args + fixture path for every file so the corpus is regenerable, not magic.

### Tokenizer assumption (stated everywhere)

Token counts use tiktoken-rs `cl100k_base` (the GPT-4o/Claude-adjacent BPE family RFC-0094 §Numbers implicitly referenced as "gpt-4o tokeniser"). The claim is explicitly "per `cl100k_base` over corpus vN," stated in the module docs, the README footnote, and the Charter §2 note. A different tokenizer yields a different number — acknowledged, not hidden.

### Snapshot / regression test `crates/mycelium-mcp/tests/token_corpus.rs`

Loads the committed corpus, runs `measure_corpus` with `BpeTokenCounter` (under the `tiktoken` feature), and:

- asserts the aggregate `token_reduction_pct()` falls within a committed band `[LOWER, UPPER]` where **`LOWER` equals the exact figure README/Charter cite** and **`UPPER = LOWER + ≤3pp`** (reviewer-3 concern: a loose band like `[40,99]` would pass trivially while README says 70%; the snapshot, not the band, is the real guard);
- regenerates and byte-compares a committed snapshot `tests/corpus/REPORT.md` (per-fixture `json_tokens`/`text_tokens`/ratio + aggregate). **Reviewer-1 concern resolved:** to avoid the band-vs-snapshot conflict (a tiktoken-rs patch bump shifting one count would fail an exact per-fixture snapshot even inside the band), `REPORT.md` records per-fixture counts under an explicitly stated "regenerate-on-tokenizer-bump" discipline — the snapshot is pinned to a pinned tiktoken-rs version (see Risks), and any version bump regenerates `REPORT.md`, the README footnote, and the Charter §2 footnote atomically (a single `scripts/check` asserts the three citations agree).

### Tool rewrite `mycelium_get_token_stats` (`lib.rs:3975`)

Replace the hardcoded 3-string sample + JSON-vs-MessagePack byte ratio with: load the committed corpus (embedded via `include_str!` so the binary is self-contained), run `measure_corpus` with `BpeTokenCounter`, return:

```json
{
  "tokenizer": "cl100k_base",
  "corpus_version": "vN",
  "fixtures": [ /* per-fixture json_tokens, text_tokens, ratio */ ],
  "aggregate_json_tokens": 0,
  "aggregate_text_tokens": 0,
  "text_to_json_token_ratio": 0.0,
  "token_reduction_pct": 0.0,
  "wire_format_byte_ratio": 0.0
}
```

The old JSON-vs-MessagePack byte ratio is **retained but renamed** `wire_format_byte_ratio`, with a description making clear it is a wire-compression metric, NOT the per-token-meaning metric. The tool `description` is rewritten to describe the Text-vs-JSON token axis as primary.

**`render()` funnel — CORRECTED (reviewer-2 + reviewer-3 blocking, REFUTED draft):** the draft claimed output "renders through the existing `render()` funnel (`lib.rs:450`) so format selection stays 1:1." That is false: the current tool is `async fn mycelium_get_token_stats(&self)` — it takes **no args** and calls `success_str(json!{...}.to_string())` directly (`lib.rs:4004`), never touching `render()`. Wiring it through `render()` would require **adding an `output_format` arg to the tool's argument schema** — itself a public-API/contract change. **Decision (option b): keep the tool argument-less and format-fixed**, emitting plain JSON via `success_str`. This avoids an arg-schema change, keeps the CLI twin's surface minimal, and is the smaller honest change. The RFC does **not** claim `render()` reuse.

### Three-Surface reconciliation (Phase 3) — CORRECTED

The draft framed the missing CLI twin as "a pre-existing 1:1 gap … verified absent" and Phase 3 as "RESTORING compliance." **All three reviewers refuted this.** The merged tree records `get_token_stats` as a **documented exception**: `skills/INDEX.md:122` declares it `✅ EXCEPTION: MCP-only — MCP server-state stats, no CLI equivalent`, and `skills/index-management/SKILL.md:332` lists it in the "parity-backfill epic (v0.1.4)." It is *not* an untracked orphan. Moreover, RFC-0090 §MCP-only requires an `EXCEPTION:` line in the **governing RFC** and states "we currently have zero such capabilities" — so the `INDEX.md` exception is **unanchored** (a pre-existing governance contradiction this RFC also resolves).

Phase 3 therefore **converts the documented MCP-only exception into a real CLI twin**, and — per Charter's "never leave a superseded approach actionable" rule — the *same change* MUST:

- (a) DELETE the `EXCEPTION: MCP-only` row for `get_token_stats` at `skills/INDEX.md:122` and reclassify it as a normal CLI↔MCP↔Skill triple;
- (b) UPDATE `skills/index-management/SKILL.md:332` to drop the "parity-backfill epic (v0.1.4)" wording for `get-token-stats`;
- (c) add a `scripts/check` assertion that **no tool simultaneously has a CLI twin and an `EXCEPTION: MCP-only` line**.

The CLI twin `mycelium token-stats` shares the exact core (`measure_corpus` over the same `include_str!` corpus) so its JSON is **structurally identical** to the MCP tool body's output. **Byte-identity harness — CORRECTED (all three reviewers blocking):** the draft cited "a contract test (tests/contract.rs style) guards byte-identity." `tests/contract.rs` is an in-process MCP-only harness (count/description/`is_error` invariants over `tokio::io::duplex`); it never shells out to the CLI and asserts no cross-surface byte-identity. **No such harness exists today.** Phase 3 BUILDS a new one — `crates/mycelium-cli/tests/cli_token_stats.rs` — that spawns the compiled `mycelium token-stats` binary, captures stdout, calls the MCP tool body in-process over the same core, and `assert_eq!`s the two JSON strings. Skill coverage is confirmed under the exposed name `mcp__mycelium__get_token_stats` (reviewer-3 note: coverage is by the MCP-exposed name, not the bare fn name).

### Charter §2 reconciliation

Current row: `| AI token efficiency (Hyphae DSL vs JSON) | ≤ 30% of JSON token count for the same payload |` (`CHARTER.md:44`). After Phase 1 yields the real `text_to_json_token_ratio`, Phase 2 either:

- (a) confirms it is ≤ 0.30 and adds a footnote "measured: ratio = X.XX via cl100k_base over corpus vN (RFC-0120)"; or
- (b) if it is > 0.30, **proposes** amending the SLA target to the measured value and **flags** it as a Charter governance event requiring founder sign-off before the §2 edit merges (kept in its own commit).

Either way, Phase 2 **also reconciles the LABEL** (reviewer-3 blocking concern): the §2 row says "Hyphae DSL" but the corpus measures the `TextFormatter` (TOON) output — the agent-visible serialization. Phase 2 corrects the row to name the formatter actually measured (e.g. "Text formatter (TOON) vs JSON"), so the SLA stops mislabeling its own subject. The same wording reconciliation applies to `README:38/71`'s "compact DSL" copy.

### Files touched

- **NEW** `crates/mycelium-mcp/src/token_bench.rs`
- **NEW** `crates/mycelium-mcp/tests/token_corpus.rs`
- **NEW** `crates/mycelium-mcp/tests/corpus/*.json` + `REPORT.md`
- **NEW** `scripts/capture_token_corpus.sh`
- **NEW** `crates/mycelium-cli/tests/cli_token_stats.rs` (Phase 3 byte-identity harness)
- **NEW** `docs/adr/NNNN-tiktoken-tokenizer-dependency.md` (ADR for the external dependency + tokenizer choice; CLAUDE.md requires an ADR for external deps)
- **EDIT** `crates/mycelium-mcp/src/lib.rs` (`mod token_bench;`; rewrite `mycelium_get_token_stats`)
- **EDIT** `crates/mycelium-mcp/Cargo.toml` + workspace `Cargo.toml` (tiktoken-rs under a `tiktoken` feature)
- **EDIT** `crates/mycelium-cli/src/main.rs` + `queries.rs` (Phase 3 `token-stats` subcommand)
- **EDIT** `README.md:38` & `:71`
- **EDIT** `CHARTER.md` §2 row (Phase 2, governance-flagged, own commit)
- **EDIT** `skills/INDEX.md:122` + `skills/index-management/SKILL.md:332` (retract MCP-only exception)
- **EDIT** `CHANGELOG.md` [Unreleased] (BREAKING output-shape note)
- **UPDATE** `rfcs/0094-token-efficient-output.md` (cross-link the now-real measurement)
- **EDIT** `scripts/check` (or new `scripts/check_token_honesty.sh`) for the consistency guards

---

## Phased plan (pure-core-first)

### Phase 1 — Pure-core token-accounting module + committed real corpus + honest measurement (no surface change)

**Scope:** Add `crates/mycelium-mcp/src/token_bench.rs` (`TokenCounter` trait, `WhitespaceTokenCounter` hermetic default, `BpeTokenCounter` under `tiktoken` feature, the structs, pure `measure_case`/`measure_corpus` reusing `JsonFormatter`+`TextFormatter`). Capture the committed corpus under `crates/mycelium-mcp/tests/corpus/*.json` from `tests/e2e/fixtures/ripgrep/` via `scripts/capture_token_corpus.sh`. Add `tests/token_corpus.rs` running `measure_corpus` with the real BPE counter, asserting the band and pinning `REPORT.md`. Add tiktoken-rs under a `tiktoken` feature; verify `cargo deny`/`cargo audit` green on the feature-on build; land the dependency ADR. No tool/CLI/Skill wiring.

**Collision note:** `token_bench.rs` is a brand-new file; only an additive `mod token_bench;` line in `lib.rs`. New `tests/corpus/` dir and `tests/token_corpus.rs` are new paths. Cargo edits are additive. Does NOT touch the `mycelium_get_token_stats` body or `formatter.rs` internals (only calls their `format`). Zero new MCP/CLI surface. **Genuinely isolated from PR #606** — proceeds in parallel.

### Phase 2 — Resolve the claim against the measured number; reconcile README + Charter §2 (governance-flagged)

**Scope:** Read the aggregate `token_reduction_pct` Phase 1's green test pins. If ≥ ~70%: keep `README:71`, rewrite `README:38` to a concrete sourced figure derived from the ratio, both footnoted "measured via cl100k_base over corpus vN (RFC-0120)." If lower: restate both lines to the true figure and retract the overclaim from README + any SKILL/docs/marketing copy. Update CHANGELOG [Unreleased]. Compare measured `text_to_json_token_ratio` to Charter §2: confirm with a footnote, or propose the amendment as a flagged governance event. **Also reconcile the §2 "Hyphae DSL" label** and the README "compact DSL" wording to name the `TextFormatter` actually measured.

**Collision note:** Touches only prose: `README.md:38/71`, `CHANGELOG.md`, `CHARTER.md` §2 row, SKILL/doc copy. No code. The Charter §2 edit is the one governance-sensitive change — held behind founder sign-off, kept in its own commit so it can be approved/reverted independently of the README copy. No overlap with PR #606.

### Phase 3 — Rewrite `mycelium_get_token_stats` onto the correct axis + convert the MCP-only exception into a CLI twin

**Scope:** Replace the hardcoded-sample byte body of `mycelium_get_token_stats` (`lib.rs:3975`) with a call into `measure_corpus` over the embedded corpus (`include_str!`), returning Text-vs-JSON token metrics as primary and the renamed `wire_format_byte_ratio` as a labeled secondary; rewrite the tool description. **Keep the tool argument-less, emitting via `success_str` (no `render()` arg-schema change).** Add the CLI twin `mycelium token-stats` over the same core; build the new `crates/mycelium-cli/tests/cli_token_stats.rs` byte-identity harness. Retract the `EXCEPTION: MCP-only` line at `skills/INDEX.md:122` and the stale epic note at `skills/index-management/SKILL.md:332`; reclassify as a full triple. Add the `scripts/check` guard. Cross-link RFC-0094.

**Collision note (CORRECTED — reviewer-3 blocking, REFUTED draft):** Phase 3 **collides head-on with in-flight PR #606 (RFC-0114 Phase 2, OPEN)**, which the draft never mentioned. `gh pr view 606` confirms #606 edits the exact surface Phase 3 touches: `crates/mycelium-mcp/src/lib.rs`, `crates/mycelium-mcp/tests/contract.rs`, `crates/mycelium-cli/src/main.rs`, `crates/mycelium-cli/src/queries.rs`, and `skills/*/SKILL.md` — both add a new CLI subcommand to the same `main.rs` dispatch. **Phase 3 is gated to start only after PR #606 merges (or rebases onto it), and MUST re-verify the merged `origin/develop` tree before touching `lib.rs`/`main.rs`/`queries.rs`/`contract.rs`.** Phase 3 changes a tool body/description but not the tool count — confirm `EXPECTED_TOOL_COUNT` (currently 93 in `contract.rs`) is unaffected. Phase 1+2 are isolated and proceed in parallel; Phase 3 is sequenced last.

---

## Acceptance criteria (RED-testable)

- [ ] **RED:** `cargo test -p mycelium-rcig-mcp token_bench::measure_corpus_aggregates` fails before `token_bench.rs` exists; GREEN after — `measure_corpus` over a 2-fixture in-test corpus with a fake `TokenCounter` returns a `CorpusReport` whose `total_text_tokens`/`total_json_tokens` equal the sum of per-fixture counts and whose `token_reduction_pct() == 100*(1 - total_text/total_json)`.
- [ ] **RED-testable:** a unit test asserts `measure_case` renders the SAME bytes as `JsonFormatter.format(&value)` and `TextFormatter::default().format(&value)` for a known fixture (so the measurement provably uses the agent-visible formatter paths, not a re-implementation).
- [ ] **RED-testable (JSON baseline pinned):** a test asserts `measure_case`'s `json_tokens` are counted over `JsonFormatter::format` output (pretty, `to_string_pretty`, `formatter.rs:110`) — NOT compact `to_string` — since pretty-vs-compact materially changes the headline.
- [ ] **RED-testable:** `cargo test -p mycelium-rcig-mcp --features tiktoken --test token_corpus` loads the committed `crates/mycelium-mcp/tests/corpus/*.json` (≥8 real tool outputs), runs `measure_corpus` with `BpeTokenCounter` (cl100k_base), and asserts the aggregate `token_reduction_pct` is within the committed band `[LOWER, UPPER]` where **`LOWER` == the exact figure README/Charter cite** and **`UPPER ≤ LOWER + 3pp`**; the test fails if any corpus file is missing or the ratio drifts outside the band.
- [ ] **RED-testable (snapshot):** the committed `crates/mycelium-mcp/tests/corpus/REPORT.md` (per-fixture `json_tokens`/`text_tokens`/ratio + aggregate) is regenerated and byte-compared by the test; a stale `REPORT.md` fails CI. The snapshot is pinned to the pinned tiktoken-rs version; a documented "regenerate on tokenizer bump" step covers version changes.
- [ ] **RED-testable (corpus reproducibility):** re-running `scripts/capture_token_corpus.sh` against `tests/e2e/fixtures/ripgrep/` reproduces the committed `*.json` byte-for-byte (a CI/`scripts/check` step diffs regenerated-vs-committed), proving the corpus is regenerable, not magic. Requires the tool payloads to be order-deterministic (serde_json ordered maps); the check fails loudly if not.
- [ ] **RED-testable (hermetic core):** `WhitespaceTokenCounter` is deterministic — a test asserts `count("a:  b\nc")` is stable, and the core `measure_corpus` test passes WITHOUT the `tiktoken` feature.
- [ ] **RED-testable (counter direction agreement):** a feature-gated test asserts `WhitespaceTokenCounter` and `BpeTokenCounter` agree on the SIGN of the reduction over the corpus (both yield Text < JSON), so a green hermetic CI run can't mask a counter that disagrees with the figure-of-record.
- [ ] **RED-testable (README↔CI consistency):** a feature-gated test (or `scripts/check_token_honesty.sh`) asserts `README:71`'s cited percentage falls **within the committed band** (not exact-match against the `~` prose — reviewer-1 concern) and equals the band `LOWER`; README and CI can never silently diverge. The check greps for an exact expected string rather than parsing arbitrary prose.
- [ ] **RED-testable (tool axis):** `mycelium_get_token_stats` returns JSON containing `tokenizer: "cl100k_base"`, a `text_to_json_token_ratio` numeric field, a `token_reduction_pct` field, and a SEPARATE `wire_format_byte_ratio` field; an MCP integration test asserts all four keys present and that `wire_format_byte_ratio != text_to_json_token_ratio` (the two axes are distinct).
- [ ] **RED-testable (no hardcoded sample):** a test asserts `mycelium_get_token_stats` no longer emits the hardcoded `src/engine/store.rs>Store...` 3-string sample (the `fixtures` count field equals the committed corpus file count).
- [x] **RED-testable (Phase 3 cross-surface byte identity — NEW harness):** `crates/mycelium-cli/tests/cli_token_stats.rs` spawns the compiled `mycelium token-stats` binary, captures stdout, calls the MCP tool body in-process over the same `measure_corpus` core, and `assert_eq!`s the two JSON strings. (No `tests/contract.rs`-style harness exists for this — it is built here.)
- [x] **RED-testable (Phase 3 exception retraction):** `scripts/check` asserts no tool has both a CLI twin AND an `EXCEPTION: MCP-only` line; specifically that `get_token_stats` no longer carries `EXCEPTION: MCP-only` at `skills/INDEX.md:122`, is reclassified as a full CLI↔MCP↔Skill triple, and the "parity-backfill epic (v0.1.4)" wording is dropped from `skills/index-management/SKILL.md:332`.
- [x] **RED-testable (Skill coverage):** `scripts/check` confirms `mcp__mycelium__get_token_stats` (the exposed name) appears in ≥1 `SKILL.md` `allowed-tools`.
- [x] **RED-testable (tool count unaffected):** `tests/contract.rs`'s `EXPECTED_TOOL_COUNT` (93) is unchanged by the Phase 3 body/description rewrite (no tool added/removed).
- [ ] **Governance gate (Phase 2):** if measured `text_to_json_token_ratio > 0.30`, the PR description records the proposed Charter §2 amendment and the §2 edit is held in a separate commit pending founder sign-off; `scripts/check` verifies `CHARTER.md` §2 carries a measured-footnote citing RFC-0120 and corpus vN.
- [ ] **Label reconciliation (Phase 2):** `scripts/check` (or reviewer checklist) verifies the Charter §2 row and the README copy name the `TextFormatter` (TOON) output actually measured, not an imprecise "Hyphae DSL"/"compact DSL" label.
- [ ] **BREAKING-change gate:** `CHANGELOG.md` [Unreleased] carries a BREAKING note documenting that `mycelium_get_token_stats`'s output shape changes (old `{json_bytes, msgpack_bytes, ratio, compact_chars, token_ratio}` keys replaced; byte ratio renamed `wire_format_byte_ratio`); a `scripts/check` asserts the note exists.
- [ ] **Dependency gate:** `cargo deny check` and `cargo audit` pass with the `tiktoken` feature **enabled** (not just the default feature-off build), and `docs/adr/NNNN-tiktoken-tokenizer-dependency.md` records the dependency + tokenizer choice + license verification (CLAUDE.md: external deps require an ADR).
- [ ] **Citation-drift gate:** a single `scripts/check` asserts the three citations agree — `REPORT.md` aggregate, the README footnote figure, and the Charter §2 footnote — so a corpus/tokenizer-version bump can't leave them out of sync.
- [ ] **Coverage:** `token_bench.rs` reports ≥ 90% line coverage under `cargo llvm-cov` (Charter §2 / coverage rule).
- [ ] **No-LSP / static check:** the corpus is captured offline into committed JSON and the measurement runs with no Store, no indexing, and no network in CI — verified by `token_corpus.rs` having no Store/Tokio-runtime dependency.

---

## Charter / ADR compliance

- **ADR-0010 (no live LSP):** fully satisfied. The entire pipeline is static string/tokenizer arithmetic over already-materialized JSON tool outputs (captured offline into committed fixtures). `measure_corpus` takes plain `serde_json::Value` structs and a `TokenCounter`; nothing queries a language server or runs at edit-time. (CONFIRMED by all three reviewers.)
- **Charter §4 (language pack ≤3 files, 0 core lines):** untouched — this RFC adds no language and edits no `packs/<lang>/` files; it is pure measurement/diagnostics infrastructure.
- **Charter §5.13 / RFC-0090 Three-Surface:** Phase 1 is a pure-core no-op (adds no surface). Phase 3 **converts a documented, *unanchored* `EXCEPTION: MCP-only`** (`skills/INDEX.md:122`, contradicting RFC-0090 §MCP-only's "zero such capabilities") into a real CLI twin and retracts the now-false exception line + stale epic note in the same change — satisfying the Charter "never leave a superseded approach actionable" rule. A new `crates/mycelium-cli/tests/cli_token_stats.rs` harness guards byte-identity (none existed before).
- **§5.1 TDD:** every acceptance criterion is phrased as a RED-then-GREEN assertion with a named test.
- **Charter §2 SLA / GOVERNANCE call-out (required by prompt):** this RFC directly touches the §2 "AI token efficiency ≤ 30% of JSON token count" row — Phase 2 reconciles both the number (proposing a flagged §2 amendment requiring founder sign-off if it differs, kept in its own commit) **and the mislabeled "Hyphae DSL" formatter name**.
- **Serialization / public-API call-out:** `mycelium_get_token_stats`'s JSON OUTPUT SHAPE changes (new token-axis fields; MessagePack byte ratio renamed to `wire_format_byte_ratio`). The tool stays **argument-less** (no `render()`/`output_format` arg added — the draft's `render()` reuse was REFUTED), so the **argument schema is unchanged**. The output change is flagged here, covered by the BREAKING CHANGELOG note and the new CLI↔MCP byte-identity test.
- **External dependency:** tiktoken-rs is gated behind a `tiktoken` feature, recorded in a new ADR, and required to pass `cargo deny`/`cargo audit` on the feature-on build (CLAUDE.md: external deps require an ADR; `deny.toml` `exceptions=[]` today, so the bundled BPE rank-file license must land inside the existing allow-list or the ADR justifies an addition).
- **Storage format:** unchanged. **Perf SLA:** heavy-graph and latency SLAs unaffected; only the token-efficiency row is touched.
- **Coverage:** ≥ 90% on `token_bench.rs`.
- **Memory discipline:** a `decisions.jsonl` entry will record the measured ratio and the keep-or-restate-or-retract outcome.

---

## Alternatives considered

- **Keep the byte-proxy and reword README to "fewer bytes":** rejected — bytes are not what LLMs are billed on; the AI-native positioning is a per-TOKEN claim, so a byte proxy is the same dishonesty in a smaller font.
- **Delete `mycelium_get_token_stats` entirely:** rejected — a tool that lets an adopter verify the headline in one call is exactly the dogfooding-honesty asset the project wants; fix the axis rather than remove the capability (and removal still leaves the README claim unbacked).
- **Use only `WhitespaceTokenCounter` (no tiktoken dependency):** rejected as the HEADLINE counter — whitespace tokenization systematically mis-counts vs real BPE and would be its own dishonest proxy; kept only as the hermetic default for core unit tests, with BPE behind a feature flag as the figure-of-record.
- **Synthetic generated fixtures (RFC-0094's `src/module_{i}` loop):** rejected as the corpus basis — the finding is precisely that synthetic fixtures don't substantiate a claim about REAL default-path output.
- **Capture from `tests/contract.rs`'s server:** rejected — it is empty/unindexed and produces no real output (reviewer-1). The corpus is captured from `tests/e2e/fixtures/ripgrep/`, indexed via `mycelium index`.
- **Wire the tool through `render()` with an `output_format` arg:** rejected — that adds an argument-schema change to the strict CLI↔MCP 1:1 contract for no honesty benefit; the tool stays argument-less and format-fixed.
- **Pretty-printed vs compact JSON ambiguity:** pinned explicitly — the corpus measures Text vs the `JsonFormatter` pretty output agents actually receive (`to_string_pretty`, `formatter.rs:110`), asserted by a dedicated criterion; the footnote states the JSON baseline.
- **`o200k_base` instead of `cl100k_base`:** a reasonable alternative; the RFC commits to one and STATES it, since the deliverable is honesty-about-assumption. Switching is a corpus-version bump with a re-pinned snapshot.
- **Exact-match parse of README's prose percentage:** rejected (reviewer-1 / reviewer-3) — the `~` tilde and table-cell prose make exact parsing brittle; the consistency check asserts the README figure is *within the committed band* / matches an exact expected string instead.

---

## Risks & mitigations

- **Tokenizer dependency weight/licensing:** tiktoken-rs and its bundled BPE rank files add build weight and must pass `cargo deny`/`cargo audit` against `deny.toml`'s current allow-list (`exceptions=[]`). Mitigated by gating it behind a `tiktoken` feature, making feature-on `cargo deny`/`cargo audit` a **hard pre-merge gate** (acceptance criterion), and recording the dependency + license verification in an ADR. **Conservative fallback:** if BPE cannot be vendored/licensed cleanly, the honest move is to **retract** the unbacked README claim — NOT ship `WhitespaceTokenCounter` as the figure-of-record.
- **The honest number may be WORSE than ~70%,** forcing a public retraction and a Charter §2 amendment — socially uncomfortable but exactly the point of F4; mitigated by treating retraction as a first-class accepted outcome.
- **Charter §2 amendment is a governance event:** if the measured ratio exceeds the ≤30% SLA, editing the constitution needs founder sign-off and could block the PR; mitigated by isolating the §2 edit in its own commit so the measurement + tool fix can land while the SLA edit awaits approval.
- **Corpus determinism / key ordering:** `serde_json::Map` iteration order depends on the `preserve_order` feature; if any tool builds responses from a `HashMap`, recaptured JSON key order can differ, breaking the byte snapshot. Mitigated by the reproducibility acceptance criterion (regenerate-and-diff) which fails loudly if payloads aren't order-deterministic, requiring the fix before the snapshot is pinned.
- **Corpus staleness:** if tool output shapes change, fixtures drift and the pinned number goes stale; mitigated by the regenerable capture script, a `corpus_version` field, and the snapshot test that fails on drift.
- **Public-API/serialization change to the tool output** could break consumers of the old `{json_bytes, msgpack_bytes, ratio, compact_chars, token_ratio}` shape; mitigated by a BREAKING CHANGELOG note, retaining the byte ratio under `wire_format_byte_ratio`, and the new byte-identity test.
- **BPE counts vary across tiktoken-rs versions,** risking flaky band assertions; mitigated by pinning the tiktoken-rs version, asserting a tight band (not exact equality) for the ratio, and the documented atomic regenerate-on-bump discipline for `REPORT.md` + README footnote + Charter footnote.
- **Phase 3 / PR #606 collision:** #606 (OPEN) edits the exact `lib.rs`/`main.rs`/`queries.rs`/`contract.rs`/`SKILL.md` surface; mitigated by gating Phase 3 to start only after #606 merges, re-verifying the merged `develop` tree first, and confirming `EXPECTED_TOOL_COUNT` is unaffected.

---

## Review incorporated

What changed versus the draft, and which reviewer drove it:

- **Corpus capture source corrected (reviewer-1 blocking; REFUTED).** Draft cited `tests/contract.rs`'s "indexed source tree"; that server is empty/unindexed (`contract.rs:9–11`) and produces no output. Repointed to `tests/e2e/fixtures/ripgrep/` indexed via `mycelium index` (the `cli_call_graph.rs:12–29` pattern). Verified present in this session.
- **Three-Surface premise corrected (reviewers 1, 2, 3 blocking; REFUTED).** Draft called the missing CLI twin an "untracked 1:1 gap" to "restore." It is a *documented* `EXCEPTION: MCP-only` (`skills/INDEX.md:122`) — and an *unanchored* one per RFC-0090. Phase 3 now "converts the exception into a CLI twin" and explicitly retracts the exception line + stale epic note (`SKILL.md:332`), with a `scripts/check` guard. Verified both lines in this session.
- **`render()` funnel reuse removed (reviewers 2, 3 blocking; REFUTED).** Draft claimed `render()` reuse keeps format selection 1:1; the tool is argument-less and calls `success_str(...)` directly (`lib.rs:4004`). Decision: keep it argument-less/format-fixed; no `output_format` arg added; arg-schema unchanged.
- **Byte-identity harness specified concretely (reviewers 1, 2, 3 blocking).** Draft pointed at a nonexistent "tests/contract.rs-style" comparator. Replaced with a new `crates/mycelium-cli/tests/cli_token_stats.rs` that spawns the CLI binary and `assert_eq!`s against the in-process MCP body.
- **PR #606 collision added (reviewer-3 blocking; REFUTED "no collision").** `gh pr view 606` confirms it edits the exact Phase 3 surface. Phase 3 is now gated to start after #606 merges, with a merged-tree re-verification step and an `EXPECTED_TOOL_COUNT` (93) check.
- **Charter §2 label reconciliation added (reviewer-3 blocking concern).** §2 says "Hyphae DSL" but the corpus measures `TextFormatter` (TOON). Phase 2 now reconciles the label, not just the number.
- **README-consistency criterion relaxed (reviewers 1, 3 concern).** Changed from exact-parse of `~70%` prose to a band-membership / exact-expected-string check, and made it feature-gated (the BPE number lives behind `tiktoken`).
- **Band-vs-snapshot conflict resolved (reviewer-1 concern).** Band `LOWER` now == the cited figure, `UPPER ≤ LOWER+3pp`; `REPORT.md` pinned to a pinned tiktoken-rs version with a documented regenerate-on-bump step.
- **JSON-baseline criterion added (reviewers 2, 3 missing-criterion).** New RED-testable assertion that `json_tokens` are counted over `JsonFormatter::format` (pretty, `to_string_pretty`).
- **Dependency gate + ADR added (reviewers 2, 3 missing-criterion).** `cargo deny`/`cargo audit` must pass feature-on; a new ADR records the tokenizer/dependency choice and license verification; conservative retraction fallback stated if BPE can't land cleanly.
- **Corpus reproducibility + counter-direction-agreement + citation-drift + BREAKING-change + tool-count criteria added** (reviewers 1, 2, 3 missing-criteria).
- **Crate-name prose fixed (reviewer-3 minor):** package is `mycelium-rcig-mcp`; the `-p mycelium-rcig-mcp` test flags were already correct and are retained.
- **Retained from draft (REFUTED-against rejections held):** byte-proxy reword, tool deletion, whitespace-only headline counter, and synthetic fixtures remain rejected alternatives — all three reviewers CONFIRMED the wrong-axis tool, the unbacked README claims, the byte-only guards, and the §2 SLA row, validating the RFC's core thesis unchanged.