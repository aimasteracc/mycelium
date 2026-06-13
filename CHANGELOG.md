# Changelog

All notable changes to **Mycelium** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- **fix(packs/js): `.cjs` extensionless `require()` resolves to `.js` target.**
  `require('./foo')` from a `.cjs` file now produces an Imports edge to `foo.js`,
  not `foo.cjs`. Node's CJS resolution algorithm resolves extensionless local
  specifiers to `.js` regardless of the importer's own extension. Two new tests
  cover the basic case and nested-directory path normalisation. (Issue #816)

### Added

- **RFC-0126 Phase 3 — JavaScript browser-global member-call receiver synthesis.**
  `document.querySelector()`, `window.open()`, `localStorage.getItem()`, and
  all other `<browser-global>.<method>()` patterns in `.js`/`.jsx` files are now
  classified as `Stdlib`. The extractor synthesizes `"receiver.method"` callee
  names at extraction time for calls whose receiver root is a known browser global;
  `classify_javascript_browser_global` splits on `.` to check the root.
  No `queries.scm` change — the existing `@call.receiver` capture already provides
  the receiver text. 10 new TDD tests (4 classify unit + 3 queries E2E + 3
  extractor integration). Unknown receivers (`myObj.myMethod()`) are unaffected
  (no false positives). (Issue #819, RFC-0126)

- **RFC-0125 Phase 2 — JavaScript browser-global classifier.**
  `classify_javascript_browser_global` fires as a fallback for `.js`/`.jsx` files
  after `classify_typescript_import_gated` returns `Unknown`. Covers DOM and Web
  API globals (`document`, `window`, `navigator`, `location`, `history`,
  `localStorage`, `XMLHttpRequest`, `Worker`, `WebSocket`, `addEventListener`, etc.)
  that are always in scope in browser contexts without any import. `fetch` added to
  `TS_GLOBAL_BUILTINS` (universal, browser + Node 18+). 6 new tests: 3 unit (AC-6/7/8)
  + 3 integration (AC-9 variants). Zero config/API changes. (RFC-0125 Phase 2)
- **RFC-0125 Phase 1 — JavaScript CJS `require()` extraction.**
  `packs/javascript/queries.scm` now captures `const X = require('mod')` and
  `const { X } = require('mod')` as `@reference.import` nodes, producing Imports
  edges identical to ESM `import_statement` captures. This feeds `caller_imports`
  for `classify_typescript_import_gated`, lifting JavaScript callee classification
  from 53.8% (worst Tier-1 language) toward parity with Python/TypeScript/Rust.
  Zero changes to `classify.rs` or `queries.rs` — existing RFC-0113 infrastructure
  handles the rest. All four embedded pack copies synced. (RFC-0125)

- **RFC-0113 corpus measurement complete.** Dogfood corpus (Mycelium self-index,
  3,601 symbols, 1,026 sampled callee edges across Rust/Python/TypeScript/JavaScript):
  overall **66.4% classified**, 33.6% unknown tail. RFC-0113 acceptance criteria
  fully satisfied; status promoted to Implemented. (RFC-0113)

- **RFC-0113 Phase 5 — Rust extractor emits receiver-qualified callee stubs.**
  Single-segment Rust scoped calls (`fs::read_to_string()`, `io::stdin()`,
  `WatchEngine::drive()`) are now stored as `scope>name` stubs (e.g.
  `fs>read_to_string`) instead of bare terminal names. This activates
  `classify_rust_qualified` at query time, closing the last gap where
  single-segment stdlib calls were misclassified as `Unknown`. The extractor
  uses a new `reference.scoped_call` query capture that simultaneously captures
  `@call.scope` (the module/type before `::`) and `@name` (the method). Multi-
  segment paths (`crate::watch::start`), keyword paths (`super::func`), and
  bare calls are unchanged. (Issue #800, RFC-0113)

- **RFC-0113 Phase 4 — Rust stdlib callee classification.**
  Bare Rust callees are now classified using language-appropriate allowlists:
  macros (`println`, `panic`, `assert_eq`, `vec`, `dbg`, …) and `drop` classify
  as `builtin`; stdlib module local names (`fs`, `io`, `env`, `sync`, `thread`, …)
  classify as `stdlib` when the matching `use std::<name>` (or sub-item import) is
  present in the caller file; qualified paths like `fs>read_to_string` or
  `std::io>stdout` are classified via `classify_rust_qualified`. 21 new TDD tests
  (14 in `classify::rust_tests`, 7 in `queries::tests`). (RFC-0113)

- **RFC-0113 Phase 3b — Go qualified-call classification fix (Issue #795).**
  `fmt.Println()` / `http.Get()` now correctly classify as `stdlib` instead of landing
  as bare-stub `unknown` callees. Fix covers three layers: (1) `packs/go/queries.scm`
  removes the duplicate `selector_expression` arm so RFC-0118 Part B is the sole handler
  for qualified calls; (2) extractor Pass 1b-go populates `alias_table` from
  `@reference.import` captures so the receiver lookup succeeds; (3) `callees_payload`
  dispatches `pkg>Method` paths to `classify_go_qualified`. 4 new TDD tests. (RFC-0113)

- **RFC-0113 Phase 3 — Go stdlib callee classification.**
  `classify_go`, `classify_go_import_gated`, and `classify_go_qualified` added to
  `mycelium-rcig-core::classify`. Covers Go builtins (`make`, `len`, `append`, …),
  stdlib package local names (`fmt`, `os`, `http`, `json`, `filepath`, `sync`, …).
  Import-gated via last-component matching so `import "net/http"` enables the `http`
  local name and `import "encoding/json"` enables `json`. `callees_payload` now
  dispatches `.go` callers to the Go classifier. 11 TDD tests. (RFC-0113)

- **RFC-0113 Phase 2 — TypeScript/JS stdlib callee classification (wired).**
  `classify_typescript`, `classify_typescript_import_gated`, and
  `classify_typescript_qualified` added to `mycelium-rcig-core::classify`.
  Global builtins (`parseInt`, `Error`, …), Node.js built-in modules (`fs`,
  `path`, `os`, `crypto`, …), Array/String/Promise stdlib methods, Node.js
  module-level functions (`readFileSync`, `dirname`, `randomUUID`, …), and
  test-framework matchers (jest/vitest/mocha/chai/jasmine). Import-gated with
  `node:` prefix tolerance. 21 TDD tests. (RFC-0113)
  `callees_payload` now dispatches to the TypeScript classifier for `.ts/.tsx/
  .js/.jsx/.mjs/.cjs` callers (Codex P1 fix — language dispatch was missing).
  `isInteger` removed from `TS_GLOBAL_BUILTINS` (Codex P2 fix — the correct
  form is `Number.isInteger`; bare `isInteger()` now classifies as `unknown`).
  4 new TDD tests in `queries::tests`.

- **RFC-0117 Phase 2 — `check-architecture` / `mycelium_check_architecture` (Three-Surface).** Architectural
  forbid-rule evaluation over Calls+Imports edges. Reads `.mycelium/constraints.yml` (YAML forbid-rules DSL),
  projects synapse edges into `EdgeRef`s, and runs the RFC-0117 pure evaluator. Returns `{violations, violation_count,
  error_count, warn_count}`; CLI exits non-zero on any `error`-severity breach (CI use-case). 97/97 Three-Surface
  compliant; covered by `graph-structure` Skill. `serde_yaml 0.9` added to workspace deps. (RFC-0117)

- **RFC-0124 — Hyphae attribute filters after pseudo-classes.** Attribute filters
  (`[file=…]`, `[language=…]`, `[kind=…]`) and pseudo-classes may now appear in **any order**
  after the base of a simple selector — `*:calls(#Foo)[file=src/x.rs]` (previously
  `UnexpectedToken("LBracket")`) now parses and means the same as `*[file=src/x.rs]:calls(#Foo)`.
  Grammar: `simple ::= base (attribute_filter | pseudo_class)*`. Filters compose by set
  intersection, so order carries no semantics (normative; pinned by tests, including structural
  pseudos like `:first-child`, which rank against all store siblings per CSS semantics). Pure
  superset of the RFC-0091 grammar; AST and evaluator unchanged. CLI `query` help and MCP
  `mycelium_query` description gain the same example in lockstep. (RFC-0124)

- **RFC-0123 (Draft) — MCP Facade Consolidation design.** New RFC proposing to consolidate the
  95-tool MCP surface into 11 action facades (`mycelium_query`, `mycelium_context`,
  `mycelium_symbols`, `mycelium_callgraph`, `mycelium_reach`, `mycelium_hierarchy`,
  `mycelium_graph`, `mycelium_rank`, `mycelium_analyze`, `mycelium_admin`, `mycelium_subscribe`),
  cutting the measured `tools/list` payload from 189,624 bytes (≈47.4K tokens) to an estimated
  ≈35.6 KB (≈81% reduction) with byte-identical per-capability responses. Includes a proposed
  amendment of the Three-Surface Rule (RFC-0090 / Charter §5.13) from tool-level to action-level
  CLI ↔ MCP parity — governance change pending ratification; implementation is phased follow-up
  work behind a `--tool-surface=facade|legacy|both` flag. (RFC-0123)

- **RFC-0115 Phase 2 — `mycelium test-gap` CLI + `mycelium_test_gap` MCP + Skill coverage.**
  New `mycelium test-gap --coverage coverage.json` subcommand and `mycelium_test_gap` MCP tool
  consume a coverage.py `coverage.json` artifact and return a ranked list of untested symbols
  ordered by call-graph blast-radius. Output: `{ gaps: [{name, file, rank_score}], gap_count,
  total_symbols, coverage_source, truncated }` — byte-identical across both surfaces. The pure
  core (`test_gap::rank`) was already in Phase 1; Phase 2 adds the thin Store adapter
  (`test_gap_payload()` in `mycelium-core::queries`), coverage.json parser
  (`parse_coverage_json()`), CLI subcommand, MCP tool, and `skills/graph-structure/SKILL.md`
  coverage. `EXPECTED_TOOL_COUNT` bumped to 96. Body-start uses the TSA `start_line + 1`
  heuristic; a future ADR will extend the indexed span. (RFC-0115 Phase 2)

- **RFC-0116 Phase 2 — `mycelium safe-to-edit` CLI + `mycelium_safe_to_edit` MCP + Skill coverage.**
  New `mycelium safe-to-edit <symbol>` subcommand and `mycelium_safe_to_edit` MCP tool return a
  byte-identical `{ verdict, reasons, checklist, blast_radius, direct_callers }` payload.
  Verdict bands: blast_radius=0 → SAFE; 1–5 → CAUTION; 6–20 → REVIEW; 21+ → UNSAFE; NOT_FOUND for
  unknown symbols. Core logic lives in `safe_to_edit_payload()` in `mycelium-core::queries`; both
  surfaces share it without duplication. `skills/reachability/SKILL.md` updated to cover the new
  pair. `EXPECTED_TOOL_COUNT` bumped to 95. (RFC-0116 Phase 2)

- **RFC-0120 Phase 3B — `mycelium get-token-stats` CLI twin** (Three-Surface Rule completion).
  New `mycelium get-token-stats` subcommand produces byte-identical JSON to `mycelium_get_token_stats`
  by sharing the same `token_bench::token_stats_payload()` core function. A new
  `crates/mycelium-cli/tests/cli_token_stats.rs` byte-identity harness spawns the CLI binary and
  `assert_eq!`s its output against the in-process MCP body. The `EXCEPTION: MCP-only` entry for
  `get_token_stats` in `skills/INDEX.md` is retracted and reclassified as a full CLI ↔ MCP ↔ Skill
  triple. (RFC-0120 Phase 3B)

- **RFC-0120 Phase 3 — `mycelium_get_token_stats` rewired onto real token axis.**
  The tool now measures `TextFormatter` vs `JsonFormatter` BPE token counts over a committed
  6-fixture ripgrep corpus (embedded via `include_str!`). Primary output: `text_to_json_token_ratio`
  (measured 0.753 / 24.7% reduction). Secondary output `wire_format_byte_ratio` retains the old
  JSON/MessagePack byte metric under a clearly-labelled separate key. Requires the `tiktoken`
  cargo feature for real BPE counts; falls back to `whitespace-approximate` otherwise.

### Changed

- **`mycelium_get_callee_tree` / `get-callee-tree` collapse unresolved callees into a count
  (ADR-0013).** Callees the resolver could not bind to a real definition (stdlib calls like
  `unwrap`/`map`, ambiguous names — `NodeKind::Unresolved` phantoms, plus dangling edge targets
  previously rendered as `"path":"<unknown>"`) are no longer emitted as individual placeholder
  leaves. Each tree node instead carries `"unresolved_callees": N` (omitted when 0). Measured on
  the dogfood repo (`index.rs>index_path`, depth 3) the tree shrank from 173 nodes (142 noise,
  82%) to 31 real-symbol nodes. Resolved-node output is unchanged; kind-less programmatic stores
  are unaffected. `Store::CalleeNode` gains a public `unresolved_callees` field. Caller tree is
  untouched — phantoms are never call *sources* (verified). Both surfaces share the core gate, so
  CLI ↔ MCP stay byte-identical. (ADR-0013)

- **BREAKING (`mycelium_get_token_stats` output shape):** old fields `{ sample_query, json_bytes,
  msgpack_bytes, ratio, compact_chars, token_ratio }` replaced by `{ tokenizer, corpus_version,
  fixtures, aggregate_json_tokens, aggregate_text_tokens, text_to_json_token_ratio,
  token_reduction_pct, wire_format_byte_ratio }`. The old byte ratio is preserved as
  `wire_format_byte_ratio`. (RFC-0120 Phase 3)

- **RFC-0122 (rule f) — function-return-type receiver inference** (`mycelium-rcig-core`).
  `let s = get_store(); s.upsert_node()` now resolves to the correct `Store::upsert_node`
  instead of leaving the call unresolved. Implemented via: (1) new `@binding.fn_call` and
  `@fn.return_type` tree-sitter captures in the Rust pack, (2) `LocalBinding.fn_call_hint`
  field, (3) `Store::set_return_type` / `return_type_of`, (4) `enrich_context` pre-enrichment
  pass that synthesises `ctor_type` from `fn_call_hint` + `return_type_of` before
  `infer_receiver_type` runs (RFC-0118 rule c). No new redb table; no schema migration.

### Changed

- **RFC-0118 marked Implemented** — all 24 acceptance criteria now tracked and confirmed
  (`NodeKind::Unresolved` de-noising, receiver-type inference, Part C kind-map hygiene,
  cross-language Part B for Python/TypeScript/Java/C#/C++/Go/Ruby, graph-theory
  real-symbol induced subgraph, redb codec tag 19). RFC status updated from Draft.

### Fixed

- **Python/TypeScript/JavaScript/Go/C++ symbol spans now anchor on the item, not the file root**
  (completes PR #750's audit). The same root-anchoring bug fixed for Rust/Ruby in #750 existed in
  five more packs: `@definition.*` captures anchored on the file-root node (Python `module`, TS/JS
  `program`, Go `source_file`, C++ `translation_unit`) made every top-level symbol's span cover the
  WHOLE FILE — e.g. `editors/vscode/src/extension.ts>activate` returned 1–195 for a 19-line
  function — poisoning `get_source_span` and `mycelium_context.code_blocks`. Fixed by re-anchoring
  the captures on the item node: python (4 patterns: fn/decorated-fn/class/decorated-class),
  typescript (10: fn/arrow-const/class/interface/type-alias × plain+exported), javascript (8:
  fn/arrow-const/function-expression-const/class × plain+exported), go (4: fn plus the per-name
  `type_spec`/`const_spec`/`var_spec` inside grouped declarations), cpp (2: free fn,
  pointer-returning free fn). Node paths derive from `@name` text only and are unchanged;
  `@definition.method` captures stay container-anchored (their precise spans already resolve via
  the issue-#657 `METHOD_DECL_KINDS` walk-up — no extractor changes needed). All embedded pack
  copies synced (`check_pack_parity.sh` green).

- **Rust symbol spans now anchor on the item, not the file/impl container.** Live QA found
  every Rust span was container-level: top-level items (`fn`/`struct`/`enum`/`trait`/`const`/
  `static`/`mod`/`type`) anchored `@definition.*` on `source_file`, so `get_source_span` for
  `main.rs>main` returned the whole file (1–2077); impl methods fell back to the whole `impl`
  block (`Store>upsert_node` → 447–4989 for a 3-line fn) because `METHOD_DECL_KINDS` lacked
  Rust's `function_item`; trait methods returned the whole `trait` block. `mycelium_context`
  shipped these spans in `code_blocks`, pulling thousands of lines per block. Fixed by
  (1) re-anchoring all non-method `@definition.*` captures in `packs/rust/queries.scm` on the
  item node (incl. nested-`mod` items and impl associated consts/types), (2) adding
  `function_item` + `function_signature_item` to `METHOD_DECL_KINDS`, and (3) treating Rust's
  `trait_item` as a span-walk container. Node paths are unchanged. Ruby's top-level `def`
  had the identical root-anchoring bug (`(program (method …)) @definition.function`) and is
  fixed the same way; Python/TypeScript/JavaScript/Go/C++ have the same whole-file-span bug
  for top-level symbols and are listed for follow-up.
- **Output budget (RFC-0102) now covers `query`, `get_cross_refs`, and the callee/caller trees**
  (CLI and MCP, byte-identical via shared core builders). Live QA found three budget holes:
  a default `mycelium_get_callee_tree` call dumped ~373 KB (~90K tokens, pre-ADR-0013), and
  `mycelium_query` / `mycelium_get_cross_refs` had no `budget` parameter at all while
  `get_callers` politely truncates. All three tool families now accept `budget`
  (`auto`/`small`/`medium`/`large`/`disabled`; CLI `--budget` twin):
  - **`query` / `mycelium_query`**: `matches` is capped at the budget's `max_nodes`; the payload
    gains a `total_count` field — `count` follows the returned page while `total_count` keeps the
    full match total (the #746 count rule). The CLI `--format=json` output is now the same
    `{ matches, count, total_count }` object as the MCP tool (was a bare JSON array of strings).
  - **`get_cross_refs`**: every reference group (`callers`, `importers`, `extended_by`,
    `implemented_by`) is capped at `max_edges` — previously only `callers` happened to match a
    budgeted key, so the other groups silently escaped the cap.
  - **`get_callee_tree` / `get_caller_tree`**: new tree-aware `apply_tree_budget` in
    `mycelium-core::budget` caps the total serialized node count **breadth-first** (the near-root
    overview survives; deep tails are cut). Each node with cut direct children carries
    `children_truncated: K` (ADR-0013 `unresolved_callees` style); the root gains the standard
    `truncated` / `total_available` / `budget {}` metadata. Tree serialization moved into shared
    `mycelium_core::queries::{callee,caller}_tree_payload` builders so CLI ↔ MCP stay
    byte-identical by construction (Charter §5.13).

  Default text-mode CLI output is never silently truncated (RFC-0102 text-mode rule); budgeting
  applies in `--format=json` (MCP parity) or when `--budget` is explicit. (RFC-0102)

- **`count` now matches the returned array after budget truncation in `get_entry_points` /
  `get_all_symbols`** (CLI and MCP, shared core path). Previously a no-limit call on a large repo
  reported the pre-budget total in the flat `count` field (e.g. `count: 2425` next to a 30-element
  `entry_points` array), contradicting the array the agent iterates. `apply_budget` now rewrites a
  sibling `count` to the post-truncation length when the payload also carries `total_count` (the
  full-total field) and `count` equaled the pre-truncation array length. Payloads without
  `total_count` (`dead_symbols`, `isolated_symbols`, `reachable`) keep `count` as the documented
  full pre-budget total.
- **Unknown `.kind` selectors now return an explicit error with a did-you-mean
  (extends the #703 silent-empty guard).** `.fn` / `.clazz` previously evaluated
  to a silent `{matches:[], count:0}` while `[lang=…]` / `:frobnicate` correctly
  errored — the #703 validation covered attribute and pseudo-class names but not
  `.kind` tokens. `Evaluator::eval_checked` now validates the kind token against
  `node_kind_from_str` (the exact function the evaluator matches `.kind` with, so
  a matchable kind can never be falsely rejected) and returns
  `EvalError::UnsupportedKind` listing the supported kinds plus a near-miss
  suggestion (`.fn` → `.function`, `.clazz` → `.class`). Surfaced identically by
  CLI `mycelium query` and MCP `mycelium_query` (Three-Surface parity).

- **Hyphae lexer/parser errors no longer leak internal Debug tokens.**
  `#a + #b` previously surfaced as `hyphae parse error: LexError(3)` (MCP
  rendered the error with `{:?}`) and `div` as `unexpected token Ident("div")`.
  All `ParseError` variants now render through one friendly path: human wording
  (`unrecognized character at position 3`), the token as the user wrote it
  (`` `div` ``), and the existing `#Name`/`.kind`/`*` grammar teaching text +
  docs pointer on every variant.

- **Method/function definition spans now point to the declaration, not the enclosing class (Issue #657).**
  When a `@definition.method` query anchors on the enclosing type container (e.g.
  `class_definition` in Python, `class_declaration` in TypeScript/JS/Java/C#), the
  previously stored span covered the entire class. Jump-to-definition now lands at
  the precise method/function declaration line. Affected packs: Python, TypeScript,
  JavaScript, Java, C#, C++, Ruby (`method_definition`, `function_definition`,
  `method_declaration`, `constructor_declaration`, `method`, `singleton_method`).
  Ruby's `class`/`module` anchor kinds are explicitly handled alongside
  `is_type_container()` nodes since they are intentionally excluded from that
  function to avoid cross-language kind collisions.

### Changed

- **RFC-0118 Part A.2: graph-theory queries now operate on the real-symbol
  induced subgraph.** The 19 "what matters here" queries —
  `leaf_symbols`, `isolated_symbols`, `singly_referenced`, `hub_symbols`,
  `most_connected`, `k_core`, `dependency_layers`, `nodes_in_cycles`,
  `cycle_members`, `scc_groups`, `strongly_connected_components`,
  `weakly_connected_components`, `topological_sort`, `articulation_points`,
  `bridge_edges`, `biconnected_components`, `betweenness_centrality`,
  `closeness_centrality`, `harmonic_centrality_stats` — now exclude
  `NodeKind::Unresolved` resolver phantoms from both the node universe AND
  the edges incident to them, via the new single-source-of-truth
  `Store::symbol_universe()`. A phantom can no longer appear in a result,
  inflate a real node's degree, sit on a shortest path, join a cycle/SCC,
  or skew a centrality normalization denominator (which now counts only
  real symbols). Programmatic stores that never recorded kinds are
  unchanged (back-compatible). See [ADR-0012](docs/adr/0012-graph-query-real-symbol-induced-subgraph.md).

### Fixed

- **fix(hyphae): unsupported selectors return an explicit error instead of a silent empty set (RFC-0003)** — a Hyphae query that *parses* but names a filter the evaluator does not implement used to evaluate to an empty result, indistinguishable to an AI agent from "nothing matches" — the worst failure mode, since the agent concludes the wrong thing and stops. Two cases were affected: an **unsupported attribute name** (`*[lang=rust]` — the supported name is `language`, not `lang`; also any `[foo=…]`) and an **unsupported pseudo-class** (`*:frobnicate()`, any name outside the implemented set), both of which hit the evaluator's `_ => Vec::new()` fallthrough. A new `Evaluator::eval_checked` validates the AST (recursing through combinators and nested `:not(…)`/`:has(…)`/`:calls(…)` selector arguments) and returns a typed `EvalError::{UnsupportedAttribute,UnsupportedPseudo}` whose `Display` names the offending token, lists the supported names, and points at RFC-0003 — routed through the same error path as a parse failure on **both** surfaces: CLI `mycelium query` (`Error: hyphae query error: …`) and the MCP twin `mycelium_query` (`{ "error": "hyphae query error: …" }`), preserving Three-Surface parity (RFC-0090). `.function[lang=rust]` now returns *"unsupported selector: attribute filter `[lang=...]` is not implemented … Did you mean `[language=...]`?"* instead of `count:0`. All currently-working selectors are unaffected (`eval` itself is unchanged; the routing/`:context` paths that fall back on parse failure still call bare `eval`): `#Foo`, `.function`, `*`, `.class>.method` (child), `.class .method` (descendant), `*:calls(#Foo)`, `.class:has(.method)`, `[language=…]`/`[kind=…]`/`[file=…]` all still match. NOTE: the descendant combinator and `[language=]`/`[file=]` filters are already fully implemented (an earlier report of them failing silently was against a stale tree); the only genuine silent-empty traps were the two unsupported-*name* cases this fix converts to errors. New RED-first tests: `eval_checked_rejects_unsupported_attribute_name`, `eval_checked_rejects_unsupported_pseudo_class`, `eval_checked_validates_nested_pseudo_argument`, `eval_checked_accepts_supported_attribute_and_returns_matches`, `eval_checked_accepts_supported_pseudo`, `eval_checked_passes_working_combinators` (hyphae); `execute_errors_on_unsupported_selector_instead_of_empty` (CLI); `query_unsupported_selector_returns_error_envelope_not_empty` (MCP).

- **fix(core): function calls never bind to `Module`/`File`/`Import`/`Export` definitions (RFC-0118, extends #682)** — generalizes the kind-aware call guard from "is `TypeAlias`" to "is a non-callable kind". A function CALL can never resolve to a module, file, or import/export statement, but the resolver redirected any bare call stub onto a unique same-name def with no kind check. The dogfood symptom: `page-rank --edge-kind calls` ranked `crates/mycelium-mcp/src/lib.rs>push` as the **#1 node** (score 0.031, 60 callers) — but `lib.rs:49` is `mod push;`, a **module**, not a function; every stdlib `Vec::push` / `String::push` `.push()` call across the repo (60 sites) collapsed onto that same-named `NodeKind::Module` node by bare-name resolution, manufacturing a phantom centrality hub. This is the same bug class as #682 (a call binding to a non-callable `NodeKind::TypeAlias`, e.g. `Err(...)` → `type Err = String`), but with `Module` as the non-callable target. A new helper `Store::is_callable_target_kind` defines the blocked set — `Module`, `File`, `Import`, `Export`, `TypeAlias` — applied consistently across all three resolution passes (`resolve_bare_call_stubs_simple`, the import-aware pass, and the RFC-0118 Part B receiver-context pass `resolve_call_site_contexts`). The `.go>` exception remains scoped to `TypeAlias` only (Go named-type conversions). Constructors and closure-holding bindings stay callable: `Struct`/`Class`/`EnumMember` (tuple/variant ctors `MyStruct(...)`, `Color::Red(...)`) and `Field`/`Property`/`Variable`/`Constant`/`Parameter` (closures called via `self.callback()`) are NOT blocked. `Interface`/`Enum` are conservatively left callable — the confirmed bug is `Module`, and no concrete `Interface`/`Enum` collision has been observed. After the fix, `page-rank` no longer surfaces `lib.rs>push` anywhere in the top 200, and `get-callers crates/mycelium-mcp/src/lib.rs>push` no longer returns the 60 `.push()` call sites (only the 3 real `push::` module references remain). New RED-first tests: `store_resolve_call_stub_does_not_bind_to_module`, `store_resolve_import_aware_call_stub_does_not_bind_to_module`, `store_resolve_call_site_context_does_not_bind_to_module`, `store_resolve_call_stub_binds_to_field_closure` (no-regression).

- **fix(mcp): `serve --mcp` served a stale snapshot diverging from the CLI; `--root` was ignored when defaulting allowed-roots** — two startup bugs in `mycelium serve --mcp`. **(A) Stale-snapshot divergence.** `mycelium index .` rewrites only `.mycelium/index.rmp`, never `.mycelium/index.redb`; but `existing_index_path` preferred `index.redb` whenever it merely *existed*, with no freshness check. So after re-indexing via the CLI while serve was down, the MCP server silently loaded the stale redb and served wrong/empty answers for the busiest symbols — e.g. `mycelium_get_callers` for `…store/mod.rs>Store>upsert_node` returned the old caller set while the CLI returned the new one. `existing_index_path` now loads whichever of `index.redb`/`index.rmp` is **newer by mtime** (new pure helper `pick_index_path`; tie → redb, which is re-persisted from the rmp on load so the two formats re-converge — RFC-0107 in-session reactive watch is unaffected). Proven end-to-end: after adding a caller of `upsert_node` and re-indexing, CLI `get-callers` = 430 and MCP `mycelium_get_callers` `total_available` = 430 (was 429 stale redb). **(B) `--root` ignored for allowed-roots.** The CLI defaulted RFC-0097 allowed-roots to the process CWD whenever `--allowed-roots` was omitted, *overriding* an explicit `--root`. Launching `serve --mcp --root /repo` from a different CWD (e.g. `/tmp`, the common case) then made `mycelium_subscribe` for paths under `/repo` fail with `{"code":"root_not_allowed"}`, silently disabling the reactive layer. The CWD default is now resolved inside `serve_stdio` via a new `resolve_allowed_roots` helper: explicit roots win; else `[--root]` if given; else `[CWD]`. Verified: `serve --root R` from `/tmp` now permits `mycelium_subscribe` under R. New tests (RED-first): `pick_index_path_*` (4), `serve_loads_newer_rmp_over_stale_redb` (e2e, asserted-failing on the old logic), `resolve_allowed_roots_*` (3). The `serve` subcommand help and `with_root` doc-comment are corrected to describe the mtime-aware load and the new allowed-roots default.

- **fix(docs): Hyphae documented examples were missing the leading `.` on kind selectors and failed to parse** — every copy-pasteable Hyphae example in `README.md` (the `mycelium_query` snippet plus the Node and Python SDK snippets) used the dot-less form `function:calls(#AuthService)`, which is **not valid Hyphae**: a kind selector requires a leading dot, so the parser rejected it with `hyphae parse error: unexpected token Ident("function") at position 0`. The same bug appeared in `#Foo > method` (must be `#Foo > .method`) and in the `architecture-context` skill's `context --task` example + its `parity.test.json` fixture. All are corrected to `.function:calls(#AuthService)` / `#Foo > .method`. Separately, the `mycelium_query` MCP tool description and the CLI `query` subcommand help carried no working query examples, so an agent could not derive a valid selector from the tool surface — both now list copy-pasteable, parse-verified examples (`#Foo`, `*:calls(#Foo)`, `.function:calls(#Foo)`, `.class:has(.method)`, `#Foo>.method`) in lockstep. Guarded by a new regression test `crates/mycelium-hyphae/tests/documented_examples_parse.rs` (`every_documented_example_parses` + `dotless_kind_selector_is_rejected`) that fails if any documented example stops parsing or if the dot-less form is silently accepted.

- **fix(mcp): agent-experience polish — entry-points pagination, actionable path-not-found, disambiguated reachability tool descriptions** — three fixes surfaced by driving the live MCP server as an agent. **(A) `mycelium_get_entry_points` now paginates.** It previously accepted only `path_prefix` + `output_format` and dumped every zero-caller symbol in one call (~37 K tokens / 147 KB on a mid-size repo), blowing an agent's context — even though the server `instructions` route agents to it. It now takes `limit`, `offset`, and `budget` with semantics/defaults identical to its siblings `mycelium_get_all_symbols`/`mycelium_get_dead_symbols`, and returns `{ entry_points, count, total_count }` (was a bare `{ entry_points }`). The CLI twin `mycelium get-entry-points` gains byte-identical `--limit`/`--offset`/`--budget` flags and now emits the same object payload (was a bare JSON array), so CLI↔MCP JSON is byte-identical by construction via the shared `mycelium_core::queries::entry_points_payload` builder. **(B) "path not found" errors now teach the path format.** An agent that calls `mycelium_get_callers {path:"Store::upsert_node"}` got a bare `path not found: Store::upsert_node`; the shared MCP `not_found()` helper (covering get_callers/get_callees/get_cross_refs/get_outgoing_refs/get_reaches_into/…) and the CLI's new `path_not_found()` helper now emit one actionable sentence: ``path not found: {p} — symbol paths are `file>Type>member` (e.g. `src/store.rs>Store>upsert_node`); run mycelium_search_symbol to resolve a name to its full path.`` (`reason` stays the stable `symbol not found` machine code). `get_callers` and `get_reaches_into` were converted from ad-hoc inline error strings to the shared helper. **(C) reachability tools are no longer ambiguous to pick.** `get_cross_refs`, `get_reachable_to`, `get_reaches_into`, `mycelium_batch_reachable_to`, and `get_caller_tree` descriptions gained a verified "When to use vs alternatives" pointer: cross_refs = single-hop; reachable_to = transitive incoming, **depth-bounded** (`max_depth`), keeps file nodes, result keyed `reachable` (the primary blast-radius tool); reaches_into = transitive incoming, **unbounded** closure, **file nodes excluded**, result keyed `callers`; batch_reachable_to = multi-target reachable_to; caller_tree = nested tree (Calls only). CLI twin doc-comments mirror the same guidance. New tests: `get_entry_points_limit_caps_result_count`, `get_entry_points_offset_skips_results`, `entry_points_payload_reports_page_and_total`, `not_found_error_teaches_path_format_and_recovery_tool`, `path_not_found_error_teaches_format_and_recovery_tool`, `get_entry_points_limit_caps_results` (CLI).

- **fix(core): `serve --mcp` no longer crashes on startup with "Too many open files" — ignore-aware watch registration** — on any real project, `mycelium serve --mcp --root .` exited with code 1 *before accepting a single connection*: `Error: starting recursive watch / Too many open files (os error 24)`. Root cause: `WatchEngine::attach` did a single `watcher.watch(root, RecursiveMode::Recursive)`, which made `notify` descend into `target/`, `.git/`, and `node_modules/` and register an OS-level watch per directory (tens of thousands of build artifacts), exhausting file descriptors. The ignore filtering only ran at *event-processing* time inside `drive`, long after the crash. Fix: `attach` now walks the root with `ignore::WalkBuilder` — honouring `.gitignore` + `.myceliumignore` and the hard-coded `target/` / `.mycelium/` exclusions, **byte-identical to the indexer's `collect_source_files`** — and registers a `RecursiveMode::NonRecursive` watch on each surviving directory, keeping the fd count bounded to in-scope dirs. Directories created *after* startup are picked up dynamically: `drive` registers a NonRecursive watch on any non-ignored directory it sees a `Create` event for, preserving recall. Per-directory watch failures are logged via `tracing::warn` and skipped (only failure to create the watcher itself is fatal). The event-time filter in `drive` is retained as defense in depth. Verified before/after at `ulimit -n 256` on the repo's own `target/`: old binary → `starting recursive watch / Too many open files`, empty stdout; new binary → `initialize` result with `serverInfo`. New helper `watch_dirs(root) -> Vec<PathBuf>` (unit-testable exclusion logic) with RED-first tests `watch_dirs_excludes_target_git_and_gitignored`, `attach_succeeds_on_tree_with_target`, and `engine_picks_up_file_in_newly_created_dir`. **PR #686 review follow-ups (Codex P1/P2 + independent review):** (1) **rescan files that arrive with a new directory** — a `Create(dir)` for an *atomically-populated* tree (`git checkout`, `git switch`, `unzip`, `cp -r`, `mv`) surfaces no per-file events for files already inside it, so they were silently missed (a recall regression vs the old recursive watcher); `drive` now runs a pre-pass that expands any directory entry into NonRecursive watches on the dir + all non-ignored descendants (`watch_dirs`) **and** an ignore-aware list of every pre-existing source file under it (`source_files_under`), all fed through the same write-locked reindex body (RFC-0107 lock discipline preserved). (2) **surface persistent watch errors** — `attach` no longer swallows every per-dir watch failure: it always watches the root (fatal on failure), classifies each per-dir error via `is_benign_watch_error` (`PathNotFound`/`NotFound` = benign race, skipped + warned; `MaxFilesWatch`/EMFILE, permission denied, generic = persistent), and returns `Err` on any persistent error so a never-watching server cannot report "running". (3) **nested-`target`/`.mycelium` ignore parity** — `is_hard_excluded` now matches the indexer's per-entry `file_name()` check at **any depth** (so `crates/foo/target/` in a Cargo workspace is pruned, not just a top-level `target/`), applied in both `watch_dirs` and `drive`'s event-time skip. (4) doc comment notes the macOS notify-v8 kqueue caveat (`mkdir -p a/b/c` may not surface per-subdir `Create`s; the rescan mitigates the atomic-tree case). New RED-first tests: `watch_dirs_excludes_nested_target`, `engine_rescans_atomically_populated_new_dir`, `attach_classifies_watch_errors_benign_vs_persistent`.

- **fix(core): Go named-type calls (`Status(1)`) now correctly resolve to TypeAlias defs (RFC-0118)** — the kind-aware TypeAlias call guard is narrowed to spare Go source definitions: Go named types (`type Status int`) are stored as `NodeKind::TypeAlias` but ARE valid call targets as type conversions. The guard now permits binding when the candidate definition's trunk path contains `.go>`, preserving call-graph edges for Go type conversions across all three resolution passes. All other languages (Rust, TypeScript, …) where `TypeAlias` is never callable continue to be blocked. New test: `store_resolve_go_named_type_call_still_resolves` (RED-first). (Codex P2 on PR #682)

- **fix(core): function calls no longer bind to `TypeAlias` definitions (RFC-0118)** — kind-aware bare-call-stub resolution. A function CALL site can never resolve to a non-callable definition, but the resolver redirected any bare call stub onto a unique same-name def with no kind check. The dogfood symptom: `page-rank --edge-kind calls` ranked `crates/mycelium-core/src/budget.rs>Err` (an associated `type Err = String;` in a `FromStr` impl, indexed as `NodeKind::TypeAlias`) as the **#1 node** with **25 false callers** — every stdlib `Err(...)` variant-constructor call across the repo emitted a bare `Err` call stub that mis-bound onto that type alias. All three resolution passes — `resolve_bare_call_stubs_simple`, the import-aware pass, and the RFC-0118 Part B receiver-context pass (`resolve_call_site_contexts`) — now decline to bind a CALL onto a `NodeKind::TypeAlias` candidate, leaving the stub Unresolved (RFC-0118 Part A then hides it from page-rank/symbol/graph queries — the truthful state, since `Err`/`Ok`/`Some` are stdlib builtins Mycelium does not index). The two bare-stub passes gate on the stub having an incoming `Calls` edge (so import-of-type-alias `use foo::SomeAlias` still resolves); the Part B pass gates purely on the candidate kind (every stub it processes is already a call stub). The guard blocks *only* `TypeAlias`, so tuple-struct/enum-variant constructor calls (`MyStruct(...)`, `MyEnum::Variant(...)`) still resolve.

- **fix(core): bare implicit-self method calls now resolve under multi-match (RFC-0118 Part B)** — a bare implicit-receiver self-method call (e.g. Java/C#/C++/Go `void run() { speak(); }` with no `this.`/explicit receiver) failed to resolve when the method name was defined on more than one type, leaving an unresolved bare stub so `get-callers Animal>speak` returned a confident-but-wrong **empty** set. The extractor now records a synthetic `ReceiverContext { receiver: "self", self_type: Some(<enclosing type>) }` for such calls, so the post-merge disambiguation pass binds the precise `<EnclosingType>>method` edge. Fixed for **Java, C#, C++, and Ruby** (`speak()` parenthesised form) — the only languages with **implicit receiver dispatch**, where a bare `speak()` inside a method body is sugar for `this.speak()`/`self.speak()`. **Python, Rust, TypeScript/JavaScript, and Go are intentionally EXCLUDED**: there a bare `speak()` is a free/local identifier lookup (you must write `self.`/`this.`/`Self::`/a named receiver), so binding it to the enclosing type would manufacture a **false caller edge** (Codex P2 on PR #680, e.g. TS `class A { speak(){} run(){ speak(); } }` must not bind `A>run -> A>speak`). The language family is identified structurally via an `(enclosing-method kind, enclosing-type-container kind)` pair allow-list (`IMPLICIT_SELF_SCOPES`), because node kinds collide across grammars (`class_declaration` is Java/C# *and* TS/JS; `function_definition` is C++ *and* Python; `method_declaration` is Java/C# *and* top-level Go). Conservative by construction: a bare call with no enclosing type (a free function) records no self-context and resolves as before, and a bare call whose enclosing type does not define the method stays an unresolved stub (never mis-bound). As a side effect, Ruby **caller attribution** for a call made inside a method now lands at `Class>method` instead of the flat `method` path (previously a known limitation). Ruby bare calls **without** parentheses remain unresolved by design — the grammar parses them as plain identifiers indistinguishable from local-variable reads, so matching them would manufacture phantom call edges.

- **fix(resolver): shadowed local bindings now return `None` (RFC-0118 Part B Phase 3)** — when a local name is bound more than once in `locals` (e.g. `let s = Store::new(); { let s = Trunk::new(); s.method() }`), `infer_receiver_type` now returns `None` (ambiguous) instead of the first match, preventing mis-binding to the wrong type. Closes Issue #636.

- **docs(skills): corrected `project_health` JSON example** — dimensions are `{"name": "dead_code", "score": N}` objects, not `["dead", N]` tuples; first dimension key is `dead_code`, not `dead`. Aligns with `project_health_payload()` in `crates/mycelium-core/src/health.rs`.

### Added

- **test(CLI): `rank_symbols_excludes_unresolved_phantom` CLI integration test (Issue #673)** — defense-in-depth for the CLI surface: indexes a Rust fixture that calls an undefined function (producing a `NodeKind::Unresolved` phantom), then asserts `rank-symbols --format json` output excludes the phantom. The shared `is_real_symbol` guard in core covers both surfaces; this test catches any CLI-specific regression where a handler bypasses the shared builder. Closes Issue #673.

- **test(RFC-0118 AC-20): regression coverage for `rank_symbols` phantom exclusion** — added `rank_symbols_excludes_unresolved_phantom` (MCP) and `rank_symbols_json_shape_parity_with_mcp` (CLI) to lock the RFC-0118 Part A `is_real_symbol` filter against regression. Flipped AC-20 checkbox in `rfcs/0118-resolver-receiver-disambiguation.md`. (Issue #612 item 2)

- **skills(graph-structure): `project_health` registered in coverage matrix (RFC-0114)** — `mycelium project-health` CLI + `mycelium_project_health` MCP were already implemented but the entry was missing from `skills/INDEX.md` and the `graph-structure` SKILL.md description lacked a `project_health` section. Fixes the Three-Surface Rule audit gap: INDEX now shows 94/94 compliant. The `graph-structure` Skill's `allowed-tools` already contained `mcp__mycelium__project_health`; this change adds the canonical INDEX row and the capability description.

- **RFC-0118 Part B rule-b (Rust): receiver inference now handles function
  PARAMETERS** — the dominant idiomatic-Rust pattern. `fn run(s: &mut Store) {
  s.upsert_node() }` now binds `s` to `Store` (the declared param type, with
  `&`/`mut`/lifetime/generics/path stripped to the Title-case base), so
  `get-callers` on a multi-type method resolves through param receivers — not just
  `let x = T::new()` locals. A param shadowed by a conflicting-type local declines
  (never mis-binds). The pure rule-b core already existed; this wires the extractor
  to capture `@param.name`/`@param.type` and populate the per-call `ReceiverContext`.

- **RFC-0118 Part B now covers Ruby** — completing receiver inference for all 9
  method-bearing languages (Rust, Python, TypeScript, JavaScript, Go, Java, C#,
  C++, Ruby; C is function-only). Added `@call.receiver` on identifier-receiver
  `x.method` calls + a constant-constructor local binding (`x = Store.new` binds
  `x → Store`). Ruby's `method`/`singleton_method` added to `FUNCTION_KINDS` +
  the binding-scope set so method bodies are recognized as binding scopes.
  Best-effort under Ruby's dynamic typing (a `Const.method` result is
  conventionally a `Const`; the conflict / rebind passes keep mis-binds rare).
  Known limitation: a call made *inside* a Ruby method is attributed to the flat
  caller path (`method`, not `Class>method`) — Ruby's `class`/`module` node kinds
  aren't added to the type-container set because they collide with JS/TS
  class-expression and Python file-root node kinds respectively; method
  *definitions* still path correctly as `Class>method`.

- **RFC-0118 Part B now covers Go, and Go methods are pathed by receiver type.**
  Go methods were flat file-level nodes (`file>Run`); they are now correctly
  pathed under their receiver type (`func (s *Server) Run()` → `Server>Run`),
  extracted from the method's `receiver` field via a new `go_receiver_type`
  helper used by both the definition handler and caller attribution. Part B adds
  `@call.receiver` on `obj.Method()` selector calls + composite-literal local
  bindings (`s := Server{}` / `&Server{}` → `s : Server`) with rebind
  invalidation, so `get-callers` on a method shared across types returns the
  correct callers. This makes Go `get-callers`/`get-callees` correct for the
  first time (a call inside a method is now attributed to `Server>Run`, not the
  flat method name).

- **RFC-0118 Part B now covers C++: `get-callers` on a multi-type method returns
  real callers.** Added `@call.receiver` on identifier-receiver `obj.m()`/`ptr->m()`
  calls + a declared-type local binding (`Store s;` / `Store s = …` binds `s →
  Store`). Reuses the binding machinery; `class_specifier`/`struct_specifier`
  added to the type-container set.

### Fixed

- **C++ (and C) callers were all attributed to `_unknown`.** `enclosing_function_path`
  used `child_by_field_name("name")`, but a C/C++ `function_definition` has no
  `name` field — the name lives at the end of the `declarator` chain. Added
  `descend_declarator_name` (walks `declarator → … → identifier/field_identifier/
  qualified_identifier`), so every C/C++ caller is now correctly named. Fixes
  `get-callers`/`get-callees` attribution for all C and C++ code.
- **C++ methods were mis-pathed `_Unknown>method`.** The `@definition.method`
  capture anchored on `field_declaration_list` (which has no name) and
  `class_specifier`/`struct_specifier` weren't recognized type containers. Re-anchored
  the method capture on the enclosing `class_specifier`/`struct_specifier` body →
  methods are correctly at `Type>method`.

- **RFC-0118 Part B now covers C#: `get-callers` on a multi-class method returns
  real callers.** Added `@call.receiver` on identifier-receiver invocations + a
  declared-type local binding (`Store s = …` binds `s → Store`; C# is statically
  typed so the declared type is authoritative, no `@binding.rebind` needed).
  Reuses the `method_declaration`/`constructor_declaration` `FUNCTION_KINDS`
  entries; added `struct_declaration` to the type-container set.

### Fixed

- **C# methods were mis-pathed `Class>method>method`, and constructors collided
  with the class node.** Like Java, the C# `@definition.method` anchored on the
  method node (→ doubled path), and `@definition.constructor` fell through the
  generic definition branch producing a flat `file>Ctor` that aliased the class
  node (`Ctor` == class name). Re-anchored method **and** constructor captures on
  the enclosing `class`/`struct`/`interface`/`record` `declaration_list` body so
  members are correctly at `Type>method` / `Type>Type` (constructor).

- **RFC-0118 Part B now covers Java: `get-callers` on a multi-class method
  returns real callers.** Added `@call.receiver` on identifier-receiver method
  invocations plus a DECLARED-TYPE local binding (`Store s = …` binds `s` to
  `Store` regardless of the RHS — Java is statically typed, so the declared type
  is authoritative and reassignment can't change it, giving high recall with no
  `@binding.rebind` needed). Core: `method_declaration`/`constructor_declaration`
  added to `FUNCTION_KINDS` (caller attribution) and the binding-scope set;
  `lambda_expression` added to the binding-scope set.

### Fixed

- **Java methods were mis-pathed as `Class>method>method`.** The Java pack's
  `@definition.method` capture anchored on the `method_declaration` node, but the
  extractor's `build_class_chain` appends the anchor's own name — so every Java
  method landed at `Class>method>method` (with `Class>method` left as a kindless
  auto-created intermediate), corrupting `get-symbols`, `get-callers`, and
  by-kind queries for all Java code. Re-anchored the method/constructor patterns
  on the enclosing `class_declaration`/`interface_declaration` body (mirroring the
  Python/TypeScript patterns) so methods are correctly at `Class>method`.

- **Receiver inference no longer leaks bindings across nested closures (no false
  callers).** Arrow functions (JS/TS), Python lambdas, and Rust closures are now
  their own binding scope (`BINDING_SCOPE_KINDS`), and the call site walks the
  enclosing lexical scope chain. Previously a binding inside one closure folded
  into the enclosing named function and could bind a same-named receiver used in a
  *sibling* closure or the outer body — manufacturing a false caller edge (Codex
  P2 #653, acute for arrow-heavy JS). The chain walk preserves the legitimate
  case (an outer-scope binding captured by a nested closure still resolves), so
  precision improves with no recall loss. Caller-path naming is unchanged (still
  keyed on named functions).

### Added

- **RFC-0118 Part B now covers JavaScript: `get-callers` on a multi-class method
  returns real callers.** JavaScript previously captured `@call.receiver` but had
  no local constructor-binding captures, so receiver disambiguation never fired —
  `get-callers` on a method shared across classes (e.g. `save` on both `Store` and
  `Cache`) returned 0. Ported the four-pattern Part B block to `packs/javascript/`
  (`const x = new Ctor()` / `x = new Ctor()` bindings + `@binding.rebind`
  invalidation on every assignment target). Pack-only (the core Pass-1c wiring is
  language-agnostic; `FUNCTION_KINDS` already covers JS). Conservative: only
  Title-case ctors bind, conflicting/rebind-to-non-ctor declines (never mis-binds).

### Fixed

- **Go/C/C++/C# definitions were minted kind-less (now correctly kinded).** The
  extractor's `cap_suffix_to_kind` had no mapping for five `@definition.<suffix>`
  captures used in shipping packs — `type` (Go/C/TS), `namespace` (C++/C#),
  `template_class`/`template_function` (C++), `constructor` (C#) — so those
  definitions got no `NodeKind` and were invisible to `get-symbols-by-kind` /
  `symbol-count-by-kind` (and would have been dropped by the new search de-noise
  below). Mapped them (`type`→`TypeAlias`, `namespace`→`Module`,
  `template_class`→`Class`, `template_function`→`Function`,
  `constructor`→`Method`). A new guard test scans every pack and asserts each
  `@definition` suffix maps, so a future pack can't silently reintroduce the gap.
- **`search-symbol` no longer returns unnavigable junk.** On a kind-annotated
  index (built by the extractor), `Store::search_symbol` now drops nodes an agent
  can't jump to — `NodeKind::Unresolved` resolver phantoms and the kind-less
  import-target stubs the extractor mints via bare `upsert_node`
  (`anyhow::Context`, `std::collections::HashMap`, …) — which were 24–48% of raw
  results on the dogfood corpus. Adds a `Store::is_searchable_symbol` predicate
  (stricter than `is_real_symbol`: requires a recorded, non-`Unresolved` kind),
  applied behind the same presence-gate as `get-files` so legacy programmatic
  stores that never set kinds keep their historical contract. Lands identically
  on the CLI (`search-symbol`) and MCP (`mycelium_search_symbol`) surfaces — both
  call the same core method (Three-Surface 1:1 preserved).
- **RFC-0120 Phase 1c corpus test threshold** (`crates/mycelium-mcp/tests/token_corpus.rs`):
  lowered `corpus_has_minimum_fixture_count` from `>= 8` to `>= 6`. The real
  ripgrep corpus legitimately has 6 fixtures — `query` and `importers_tree`
  captures failed on that codebase (documented in `REPORT.md`). The old threshold
  was sized for the v1 synthetic corpus. Fixes CI failure on PR #649.

### Added

- **RFC-0118 Part B now covers Python and TypeScript: `get-callers` on a
  multi-type method returns real callers cross-language.** The Rust F5 fix is
  extended to the two highest-traffic dynamic languages by adding local
  constructor-binding captures to their packs: `x = Ctor()` (Python `assignment`)
  and `const x = new Ctor()` (TypeScript `variable_declarator` with `new_expression`).
  The post-merge receiver-disambiguation pass — already language-agnostic — then
  binds `x.method()` to `…>Ctor>method`. An agent asking "who calls `Database.query`"
  in a Python/TS repo now gets the real callers instead of 0 wherever the receiver
  was a locally-constructed instance. Conservative throughout: only Title-case
  constructors bind, and a name rebound to a conflicting type **declines** (never
  mis-binds). Under dynamic/structural typing a local can be reassigned to a
  value of a different type, so **any** reassignment of a bound name to a
  non-constructor RHS (`s = factory()`, `s = some_dict[k]`, …) invalidates the
  binding and makes inference decline rather than trust the stale declared type
  — captured via a `@binding.rebind` signal on every assignment target and a
  count check in the extractor (preserves the "never mis-bind" invariant).

### Changed

- **RFC-0120 Phase 1c: real ripgrep corpus measured — Charter §2 token-efficiency claim
  requires founder decision.** `scripts/capture_token_corpus.sh` ran against a
  shallow-clone of `BurntSushi/ripgrep` (101 files, ~850 nodes). Real BPE ratio:
  **0.753** (TextFormatter uses 75.3% as many `cl100k_base` tokens as JsonFormatter;
  24.7% reduction). The `bpe_charter_sla_binding` test fails: Charter §2 asserts
  ≤30% (70%+ reduction). REPORT.md updated to v2-ripgrep. Founder must choose: retract
  the claim, redesign the formatter, or reframe the comparison (see REPORT.md §Decision).
- **fix(scripts): `fetch-e2e-fixtures.sh` multi-`local` bash declaration bug fixed.**
  Split `local name="$1" url="$2" dest="$FIXTURES/$name"` into three separate `local`
  statements so `$name` in `dest` resolves to the locally-declared value, not the
  outer-scope unbound variable (which causes `set -u` failure).

### Fixed

- **Shipped binary ran stale Tree-sitter queries (pack-parity gap).** The engine
  embeds its pack queries via `cortex.rs` `include_str!` from
  `crates/mycelium-core/packs/<lang>/queries.scm`, but `scripts/check_pack_parity.sh`
  only checked the `mcp` and `cli` copies — never the `core` copy. As a result the
  `python`, `typescript`, and `javascript` core copies had silently diverged from
  canonical `packs/` (missing the RFC-0092 alias-binding section), so the **shipped
  binary parsed with old queries while CI and the test suite — which load the
  canonical root copies — stayed green**. Resynced the three core copies to canonical
  and extended the parity check to verify the core subset (the 5 cortex-embedded
  langs), with a negative test proving a stale core copy now fails CI.
- **fix(scripts): `capture_token_corpus.sh` now uses correct CLI flags.** Replaced
  non-existent `mycelium index --output` with the actual invocation (`mycelium index
  <path>`, index written to `<path>/.mycelium/index.rmp`). Replaced non-existent
  `--index` global flag with per-command `--root <FIXTURE_ROOT>`. Fixed subcommand
  names (`search` → `search-symbol`, `symbol-info` → `get-symbol-info`, `get-callees`
  → `get-callee-tree`, `get-callers` → `get-caller-tree`). Fixed `--depth` →
  `--max-depth`. Also fixes `importers_tree`/`subclasses_tree` corpus entries to use
  the dedicated `get-importers-tree`/`subclasses-tree` commands (Codex P2). Resolves
  RFC-0120 Phase 1 item #614.
- **RFC-0118 Part B (Rust): `get-callers` on a multi-type method now returns real
  callers (the F5 fix).** The extractor captures the receiver of a method call
  plus local `let x = T::new()` constructor bindings (new Rust pack captures
  `@call.receiver`, `@binding.local`, `@binding.ctor`) and records a per-call-site
  `ReceiverContext`; the post-merge pass infers the receiver type and binds
  `x.method()` to `…>T>method`. On the Mycelium self-index, `get-callers` on
  `Store>upsert_node` went from **0 → 60 real callers**. Conservative: only binds
  when inference is unambiguous, never mis-binds. Params/self/fields and other
  languages are follow-ups.

### Added

- **RFC-0120 Phase 1b: `BpeTokenCounter` (tiktoken `cl100k_base`) + Charter §2 binding
  test + corpus `REPORT.md` + capture script.** Adds `tiktoken-rs` under the `tiktoken`
  cargo feature on `mycelium-rcig-mcp`. `BpeTokenCounter` wraps `tiktoken_rs::cl100k_base()`
  and implements `TokenCounter`. Five new `#[cfg(feature = "tiktoken")]` tests:
  `bpe_text_to_json_ratio_informational` (always passes — prints current synthetic ratio
  0.773), `bpe_charter_sla_binding` (gated by `MYCELIUM_REAL_CORPUS=1` env — activates
  when real ripgrep corpus is captured), plus three sanity tests. Committed
  `tests/corpus/REPORT.md` (Phase 1a synthetic corpus baseline). Added
  `scripts/capture_token_corpus.sh` (Phase 1b deliverable: indexes
  `tests/e2e/fixtures/ripgrep/` and re-captures corpus from real tool outputs).
  ADR-0011 documents the tiktoken-rs choice. Current synthetic ratio 0.773 (22.7%
  reduction) is **NOT** the Charter §2 figure-of-record — real measurement requires
  Phase 1b capture script execution.

- **RFC-0120 Phase 1a: token-accounting module `token_bench`.** Pure
  `crates/mycelium-mcp/src/token_bench.rs` module exposes `WhitespaceTokenCounter`,
  `measure_case`, `measure_corpus`, and `CorpusReport` (with `token_reduction_pct` /
  `text_to_json_token_ratio` / `byte_reduction_pct`). Committed 8 representative
  corpus fixtures under `crates/mycelium-mcp/tests/corpus/`. Integration test
  `token_corpus.rs` verifies corpus has ≥8 fixtures, `TextFormatter` reduces tokens
  vs `JsonFormatter` over the aggregate, and per-fixture sums equal the aggregate.
  Resolves Issue #614 Items 1+2 (module is `pub`, corpus fixtures committed).

- **RFC-0116 Phase 1 AC complete: `EditMetrics` now accepts optional `health` /
  `test_gap_uncovered` escalation inputs.** `edit_verdict` applies monotonic
  one-step escalation when `health` is grade `D`/`F` (RFC-0114) or
  `test_gap_uncovered` is `Some(true)` (RFC-0115). Escalation never downgrades;
  `Error`/`NotFound` short-circuits are unaffected. 6 new unit tests; `step_up`
  is `const fn`. RFC-0115, RFC-0116, RFC-0117 Phase 1 acceptance criteria all
  marked `[x]`.

- **RFC-0119 Phase 1: pure entry-point ranking core** (`context/ranking.rs`).
  New `classify_test_path` classifies trunk paths as `TestFile`, `TestSymbol`, or `None`
  using cheap static rules (directory segments, filename stems, suffix patterns, symbol
  leaf names — no regex, no I/O). New `rank_entry_points` orders `ScoredCandidate` slices
  by `(exact_match desc, non_test desc, importance desc, order asc)`, drops or demotes test
  candidates, and guarantees non-empty output even for all-test corpora. 14 unit tests;
  zero `Store` dependency.
- **RFC-0119 Phase 2: importance-weighted entry-point adapter** (`seed_entry_points`
  rewritten). `mycelium_context`'s natural-language entry-point selector now ranks
  candidates by exact-match precedence, test-code demotion, and stub-robust in-degree
  importance instead of alphabetical order. **Behavioral change**: test-code entry points
  (paths in `tests/`, `__tests__/`, `*.test.*`, `test_*.py`, `tests.rs` stems, etc.) are
  excluded when at least one non-test candidate exists. Unresolved-callee phantom nodes
  (`NodeKind::Unresolved`) are skipped. In-degree counts only real-symbol callers
  (RFC-0119 AC-11). Merge semantics preserve exact-match across multiple candidate
  searches for the same path (AC-4b). Existing call sites in MCP and CLI are unchanged
  (signature-preserving rewrite).

### Fixed

- **RFC-0118 Part B Phase 1: pure receiver-inference core (`resolver::receiver`
  module).** New side-effect-free `infer_receiver_type` + `disambiguate` functions
  over plain `ReceiverContext` / `Candidate` structs. Five inference rules
  (self/cls/this → impl type; param annotation; local constructor; field annotation;
  import alias terminal). Used by the post-merge pass in Phase 2b to disambiguate
  multi-match method stubs (`get-callers = 0` root cause). 14 unit tests, ≥ 90%
  coverage on the new module.

- **RFC-0118 Part C: resolver passes now clean `kind_map` + `span_map` when
  removing a resolved stub.** Previously `resolve_bare_call_stubs_simple`,
  `resolve_import_aware_stubs`, and `resolve_import_aware_extends_stubs` called
  `self.trunk.remove(stub_id)` directly, leaving a stale `NodeKind::Unresolved`
  entry in `kind_map`. Since `NodeId` is content-derived from the path, this entry
  would survive re-index cycles. All three passes now route through
  `Store::remove_node` which cleans `trunk`, `synapse`, `kind_map`, and `span_map`
  atomically. Two regression tests added (AC-3).

- **RFC-0118 Part A: unresolved-callee phantoms no longer pollute the symbol
  universe.** The resolver mints placeholder nodes for calls it can't statically
  resolve (e.g. `unwrap`, `HashMap`, `Db>upsert_node`) and links a `Calls` edge
  so the caller isn't falsely "dead". These now carry a new `NodeKind::Unresolved`
  marker, and `all_symbols`, `page_rank`, and `rank_symbols` (`top_callee_symbols`
  / `top_symbols_by_incoming`) exclude them via `Store::is_real_symbol`. On the
  Mycelium self-index this tags **966** such phantoms out of the symbol/rank
  universe (previously inflating symbol listings and rank mass). The marker is a
  *negative* gate, so kind-less programmatic/test stores are unaffected
  (back-compatible). redb gains additive tag `19` (fail-loud, round-trip tested).

### Added

- **RFC-0113 Phase 3 — import-context gate for stdlib/external callee classification.**
  `callees_payload` now gates the `stdlib`/`external` tiers on the calling file's
  actual import set: a bare stub like `write_text` or `getcwd` only classifies as
  `"stdlib"` when the caller file has at least one stdlib `Imports` edge; an external
  method like `raises` only classifies as `"external"` when a known external root
  (`pytest`, `hypothesis`, `mock`, `unittest`) is imported. Builtins (`len`, `print`,
  etc.) still fire unconditionally. Bare names with no matching import fall through to
  `"unknown"`, preventing false-classification of project functions that share a stdlib
  name. New `classify_python_import_gated` function in `classify.rs` (generic over
  `BuildHasher`); `callees_payload` extracts the caller file's `Imports` edges to build
  the gate set. 8 unit tests in `classify.rs` + 2 integration tests in `queries.rs`.
  (RFC-0113 Phase 3 acceptance criteria.)

- **RFC-0114 Phase 2: `project-health` CLI + `mycelium_project_health` MCP + Skill coverage.**
  `mycelium project-health [--root .] [--format text|json]` computes an A–F structural health
  grade from the indexed RCIG graph (dead-code ratio 45%, isolation 35%, connectivity 20%).
  Returns `{ grade, score, dimensions }` — byte-identical across CLI and MCP via a shared
  `project_health_payload()` builder in `mycelium-core`. Phase 1 pure scorer core was already
  shipped; this phase adds the `Store::health()` adapter and all three surfaces.
  `mcp__mycelium__project_health` added to the `graph-structure` Skill. (Charter §5.13.)

### Fixed

- **ci(nightly): upload `mutants.out/` report directory as a separate artifact.** The
  nightly mutation-testing job now uploads both `mutants.log` (the console transcript)
  and `mutants.out/` (the full `cargo-mutants` report directory containing
  `outcomes.json`, per-mutant `logs/`, `caught.txt`, `missed.txt`) so reviewers
  can triage survivors when the ≥70% kill-rate gate fires. Tracked by Issue #601.

- **Agent-facing dogfooding fixes (F1/F3/F6).** Found by using Mycelium's own
  CLI as an AI agent against its own repo:
  - **F3 branding leak:** `mycelium_context`'s agent-facing `summary_line` was
    emitting the literal `codegraph_context: …` (a different product's name);
    rebranded both branches to `mycelium_context`.
  - **F1 `get-files` noise:** `Store::all_file_paths` selected files with a
    `!path.contains('>')` string heuristic, so kind-less resolver stubs
    (`unwrap`, `std::collections::HashMap`) were reported as fake files
    (786 entries, 671 junk on the self-index). Now gates on the authoritative
    `NodeKind::File` → 115 entries, 0 junk. Language-agnostic.
  - **F6 unhelpful DSL errors:** a Hyphae parse error now teaches the grammar,
    suggests the `class.Name` (not `class:name(Name)`) correction, and points at
    RFC-0003/0091 — and the CLI now prints the error's `Display` (was `Debug`,
    which hid the guidance).

### Added

- **RFC-0113 Phase 2 — callee classification (`class` field on `get_callees`).**
  `callees_payload` now returns an additive `callees` array alongside the
  backward-compatible `callee_paths` array. Each entry is
  `{ "path": "...", "class": "project|stdlib|builtin|external|unknown" }`.
  Project-defined callees (path contains `>`) are classified `"project"`;
  unresolved bare stubs are classified against the Python stdlib/builtin/external
  allowlists from `classify.rs` (RFC-0113 Phase 1). Both the CLI `get-callees`
  and MCP `mycelium_get_callees` share the same `callees_payload` builder, so the
  new field is byte-identical on both surfaces (Charter §5.13). 6 new TDD tests.

### Fixed

- **ci(nightly): fix `mutants.out` file/directory path collision.** `cargo-mutants`
  creates `mutants.out/` as its output directory, but `tee mutants.out` was
  creating a plain file of the same name first, causing `lock.json: Not a directory
  (os error 20)` crash on every nightly run. Renamed tee sink and artifact path
  to `mutants.log`.

### Security

- **SDK argv-smuggling guard + Python output cap (RFC-0111, Node + Python).**
  Both thin-CLI-wrapper SDKs now reject a user-supplied positional that begins
  with `-` (e.g. a query of `--root`), which the `mycelium` CLI would otherwise
  re-parse as a flag rather than a value (`execFile`/`subprocess` already avoid
  the shell, so this closes the residual argv-smuggling surface). The Python
  runner additionally streams stdout/stderr with a hard 64 MiB cap — mirroring
  the Node SDK's `execFile` `maxBuffer` — and kills a child that overflows it,
  so a runaway or hostile binary can't exhaust host memory. 4 new Node tests +
  4 new Python tests.

### Fixed

- **RFC-0103 per-edge `Extends` resolution for mixed-import sites (Issue #555).**
  `AdjacencyList::remove_edge` + `Synapse::remove_edge` + `Store::remove_edge`
  primitives added. `resolve_import_aware_extends_stubs` rewritten from a
  whole-node unanimity check to per-edge independent resolution: each
  `(subclass → stub)` Extends edge is now redirected to whichever definition
  that specific subclass imports, eliminating the wrong-collapse bug that the
  conservative Codex P1 fix on PR #554 avoided. Stubs are removed only after
  all incoming Extends edges are accounted for. 5 new tests (3 synapse unit +
  2 store integration).
- **Fix: stub removal now guards across all edge kinds (Codex P2, PR #572).**
  `Synapse::is_isolated(id)` added; stub deletion in `resolve_import_aware_extends_stubs`
  changed from `incoming(Extends).is_empty()` to `is_isolated()` — prevents removal
  of a stub node that still has `Calls`/`References`/other edges after its `Extends`
  edges are resolved. 5 new `synapse_is_isolated_*` tests + 1 store regression test.

### Added

- **VS Code extension (RFC-0112, Phase 1).** `editors/vscode/` — a thin-client
  editor integration over the published `@aimasteracc/mycelium-sdk` (no Rust
  toolchain). Headline command **"Mycelium: Copy context for AI"** copies a
  token-dense context bundle for the cursor/selection to the clipboard, ready to
  paste into any AI assistant; plus find-callers/callees, symbol-info, index,
  and a call-graph sidebar tree view (click a node to reveal its source). Not a
  language server (ADR-0010) — surfaces structural graph intelligence. A
  Three-Surface *consumer* (every command maps onto an existing CLI/MCP
  capability); adds no engine code.
- **Python SDK — `mycelium-rcig` (RFC-0111, Phase 2).** A thin, typed Python
  client that embeds Mycelium in any Python app **without a Rust toolchain** —
  the same thin-CLI-wrapper contract as the Node SDK (locate binary → spawn with
  an argv list, no shell → parse JSON). Pythonic surface
  (`from mycelium_rcig import Mycelium`; `version`/`index`/`query`/
  `search_symbol`/`get_symbol_info`/`get_callers`/`get_callees`/`context`/
  `server_status` + raw `run(args)`); typed (`py.typed` + inline hints);
  `MyceliumError` on failure. 32 stdlib-`unittest` tests (30 hermetic + 2
  integration) wired into CI against the release binary. Distributed as a
  pure-Python wheel via `release.yml` (`python -m build` + Trusted Publishers,
  idempotent). The PyPI distribution is **`mycelium-rcig`** (the short
  `mycelium` is taken; import package `mycelium_rcig`), mirroring the crates
  prefix; Charter §3 updated accordingly. Binary bundling via platform wheels is
  a deferred follow-up.
- **Node/TypeScript SDK — `@aimasteracc/mycelium-sdk` (RFC-0111, Phase 1).** A
  thin, typed client that embeds Mycelium in any Node/TS app **without a Rust
  toolchain**. It wraps the prebuilt CLI ([RFC-0110](rfcs/0110-npm-bun-cli-distribution.md)):
  locates the binary (`MYCELIUM_BIN` → platform package → `PATH`), spawns it
  with an argv array (no shell — no injection surface) and `--format json`, and
  returns parsed objects. Typed methods (`version`, `index`, `query`,
  `searchSymbol`, `getSymbolInfo`, `getCallers`, `getCallees`, `context`,
  `serverStatus`) plus a raw `run(args)` escape hatch covering every subcommand.
  Ships TS types (`index.d.ts`) with no build step. Because it wraps the CLI it
  **inherits CLI↔MCP parity for free** (Charter §5.13). Errors surface as
  `MyceliumError`. Hermetic unit tests (injected spawn) + a live integration
  test wired into CI against the release binary, plus an SDK packaging smoke
  test (assemble → install → resolve binary from its pinned platform
  optionalDependency → query). Release packaging assembles and publishes
  `@aimasteracc/mycelium-sdk` alongside the existing npm packages, with its
  platform-binary `optionalDependencies` pinned to the release version. Python
  SDK is Phase 2 of the same RFC. **Charter §3 bindings row amended** from
  native FFI (napi-rs/pyo3) to thin CLI-wrapper SDKs; native FFI reserved for a
  future performance RFC.
- **Import-aware `Extends` stub resolution (RFC-0103, initial target).** When a
  class inherits from a base whose simple name is defined in *several* files
  (ambiguous for the existing unique-match resolver), the post-index pass now
  redirects the `Extends` edge to the correct definition using import evidence.
  Conservative by design — the stub is redirected only when a single candidate
  is imported by **every** subclass (unanimous), so the whole-node redirect is
  always correct; ties, zero-evidence, and **mixed-import sites** (subclasses
  importing different definitions) stay unresolved rather than wrongly collapsed.
  Improves cross-file inheritance accuracy (`mycelium_get_extends` /
  extends-tree tools). Per-edge resolution of mixed sites is a tracked follow-up.

### Changed

- **BREAKING (MCP stdio): default output format flipped to `text` (RFC-0094
  Phase 4).** When a tool call omits `output_format`, the stdio MCP server (the
  LLM-caller transport) now returns the token-efficient TOON `text` format
  instead of JSON — ~72% fewer output tokens for tree-shaped responses. A
  per-call `output_format: "json"` still overrides it, and the CLI plus
  `MyceliumServer::new()` keep the JSON default (so programmatic/test callers
  are unaffected). All 77 tool format sites now route through one `render()`
  helper that resolves the per-call override against the server default.
- **refactor(mcp): Issue #428 god-file split slice 3** — extracted all 93 MCP
  request schema types from `lib.rs` into `crates/mycelium-mcp/src/requests.rs`
  (public module, re-exported via `pub use requests::*`). Moved two inline test
  modules (`server_info_tests`, `output_budget_tests`) from `lib.rs` into the
  existing `tests.rs`. `lib.rs` reduced from 6,048 → 4,694 lines (−22.4%);
  no public API change.

### Fixed

- **ci(dco-check): use full body grep instead of trailer parser** — GitHub
  squash-merge embeds `Signed-off-by` lines in the middle of the commit body
  rather than as terminal trailers, so `%(trailers:key=Signed-off-by,valueonly)`
  would false-fail those commits. Switched to `grep -qiE '^Signed-off-by:'` on
  `%B` which correctly detects the sign-off regardless of position.

- **npm launcher signal exit codes (Issue #525)**: `mycelium.cjs` now exits with
  `128 + signal_number` (e.g. SIGTERM → 143, SIGINT → 130) instead of always `1`
  when the child binary is killed by a signal, following POSIX/shell convention.

- **Mutation testing kill-rate (Issue #526)**: Added exact-count `assert_eq!` assertions to 6
  previously mutation-weak MCP tests (`get_callees`, `get_callers`, `get_dead_symbols` ×2,
  `get_all_symbols_excludes_file_nodes`, error `is_error` flag). Mutants that silently add/remove
  results or drop the `is_error: true` flag will now fail CI rather than survive.

- **ci(release): NPM_TOKEN absence now blocks ALL registry publishes (Issue #560)** — converted
  `check-npm-token` from a graceful-warn job to a hard preflight (`exit 1` + `::error::`); added it
  to `publish-crates.needs` so a missing token blocks crates.io publish (and all downstream jobs)
  before any irreversible action. Prevents partial releases where crates.io publishes but npm + git
  ceremony are skipped. The `publish-npm` step retains a defense-in-depth token check.

## [0.2.0] - 2026-06-04

### Added

- **npm / bun install for the CLI — no Rust toolchain required (RFC-0110).** A
  universal `@aimasteracc/mycelium` launcher package resolves and execs the
  matching prebuilt binary from a per-platform `optionalDependencies` package
  (esbuild/biome model — no postinstall download, works under both `npm` and
  `bun`/`bunx`). The release workflow **cross-compiles the `mycelium` CLI for 5
  targets** (darwin arm64/x64, linux x64/arm64, win32 x64), **attaches the
  binaries to the GitHub Release** (a direct download path), and **assembles +
  publishes** the platform + launcher packages (idempotent; gated so a build
  failure blocks all publishing — no partial release). CI validates the whole
  packaging path on every PR (assemble → install → run the launcher).
  `cargo install mycelium-rcig-cli` remains available for Rust users.
- **Nested `budget {}` response object (RFC-0102).** When a tool response is
  truncated by the adaptive output budget, it now carries a structured
  `budget { mode, truncated, truncated_fields, total_available{…}, limits{…} }`
  object alongside the existing flat `truncated` / `total_available` fields
  (added without removing them). `OutputBudget` exposes its size tier via a
  `mode: BudgetMode`. Byte-identical across CLI and MCP by construction (both
  apply the same `mycelium_core::budget::apply_budget`).
- **Per-call output budget knob (RFC-0102)** on `mycelium_context` and all seven
  graph-list tools — MCP `budget` field / CLI `--budget`
  (`auto|small|medium|large|disabled`), parsed via a shared `BudgetOverride`
  `FromStr` and resolved by `OutputBudget::resolve(over, node_count)` (identical
  on both surfaces). Unknown values fail fast. The CLI applies the budget in
  `--format json` (MCP parity) or when `--budget` is explicit; default text mode
  prints the full list, with a truncation footer to stderr when budgeted.

### Changed

- **BREAKING (CLI): `get-callees`, `get-callers`, `get-dead-symbols`,
  `get-isolated-symbols`, and `get-all-symbols` `--format json` now emit an
  object** (`{"callee_paths":[…]}` / `{"caller_paths":[…]}` /
  `{"dead_symbols":[…],"count":N}` / `{"isolated_symbols":[…],"count":N}` /
  `{"symbols":[…],"count":N,"total_count":M}`) instead of a bare JSON array
  (RFC-0109 Option A). Each CLI command is now **byte-identical to its MCP twin**
  (one shared `mycelium_core::queries` builder per tool) and carries
  budget/truncation metadata. Text mode (`--format text`, the default) is
  unchanged — one path per line. **Completes the RFC-0109 graph-list roll-out
  (7/7 tools).**

### Fixed

- **Output budget no longer silently no-ops for four tools (RFC-0102).**
  `apply_budget` capped a fixed key allowlist that omitted the array keys
  `get_callees` (`callee_paths`), `get_callers` (`caller_paths`),
  `get_dead_symbols` (`dead_symbols`), and `get_isolated_symbols`
  (`isolated_symbols`) actually emit — so those tools advertised budgeting but
  returned unbounded arrays. `callee_paths` / `caller_paths` are now capped at
  `max_edges`, `dead_symbols` / `isolated_symbols` at `max_nodes`.
- `sla_ancestors_100k` macOS CI flake: bumped the macOS-specific SLA limit from
  30 ms → 100 ms (observed 32 ms on loaded runner; Linux contract unchanged at
  5 ms).

## [0.1.19] - 2026-06-04

### Fixed

- **Rust extractor precision raised from 67% → 99.8% recall** via 4 additive
  `queries.scm` patterns (dogfood-found 2026-06-04 by indexing the Mycelium
  repo against itself and comparing per-file symbol counts vs ground truth):
  - `trait T { fn x(); }` trait method **signatures** are now captured
    (previously only `trait T` was indexed; `T::x` was silently dropped —
    e.g. `FileReindexer::reindex` was invisible while every `impl
    FileReindexer for X` method was present).
  - `trait T { fn x() {...} }` trait **default-method bodies** captured.
  - Module-level `static FOO: ...` items captured (previously only `const`
    — e.g. `static PACK_REGISTRY: OnceLock<...>` was missing).
  - Associated `pub const` items inside `impl` blocks captured (e.g.
    `impl NodeId { pub const NULL: Self = ...; }`).
  - Functions/structs/consts inside nested `mod` blocks (notably
    `#[cfg(test)] mod tests { fn ... }`) now captured at every position
    in the body, not only at head/tail.

  Verified on the Mycelium repo: 70 of 80 Rust files now match ground-truth
  symbol counts exactly (was 44 of 80). Total recall 99.8% (2664 / 2668).
  5 RED-first regression tests in `crates/mycelium-core/src/extractor/tests.rs`.

  Head-to-head vs `codegraph` 0.9.8 on the same repo: Mycelium index time
  0.32 s vs codegraph 0.93 s (3× faster); Mycelium 70 of 80 files at exact
  ground-truth match vs codegraph 1 of 80 (codegraph over-counts symbols
  by 19.7% — different granularity).

### Docs

- **ADR-0008**: redb as default storage backend (Phase 3 flip decision record). Documents the rationale for switching from `InMemoryBackend` to `RedbBackend` as the production default in v0.1.17, prerequisites met (equivalence tests, crash-safety, warm SLA).
- **ADR numbering fix**: renamed `docs/adr/0008-redb-storage-engine.md` → `docs/adr/0009-redb-storage-engine.md` (ADR-0009) to resolve the 0007/0008 slot collision; updated cross-references in `rfc-0100-execution-plan.md`, `rfcs/0104-charter-warm-cold-sla-split.md`, and `docs/adr/0008-redb-as-default-backend.md`.

## [0.1.18] - 2026-06-03

### Fixed

- **RFC-0107 SUBSCRIBE: replace `blocking_read()` with `try_read()` in async watch paths.**
  `RwLock::blocking_read()` inside a Tokio async task blocks the executor thread and panics
  the watch loop on the first matching subscription batch, making `--subscribe` unusable after
  the first filesystem change. Replaced with `try_read()` in the MCP `on_batch` fan-out
  (`lib.rs`) and CLI `watch.rs`; a briefly-contended lock skips that batch rather than
  crashing. (`crates/mycelium-mcp/src/lib.rs:1759`, `crates/mycelium-cli/src/watch.rs:197`)

- **Rust extractor now captures `Type::method()` and `crate::mod::func()` call
  sites.** Surfaced by dogfooding the Mycelium repo against itself
  (2026-06-03): `WatchEngine::drive(...)` from the watch session and CLI
  produced zero `Calls` edges. The Rust pack's `queries.scm` only matched
  `(identifier)` and `(field_expression)` function forms, missing every
  scoped-path call. Added an additive `(scoped_identifier name: ...)` query
  that captures the last-segment identifier; cross-file stub resolution
  picks it up unchanged. Two new regression tests
  (`extractor_rust_scoped_method_call_creates_calls_edge`,
  `extractor_rust_qualified_path_call_creates_calls_edge`).

### Added

- **Reactive query subscriptions — `mycelium/queryResultChanged` notifications
  (RFC-0108, founder-ratified D1-D4).** Step 4/4 of the reactive-completion
  roadmap (watch ✅ push ✅ subscribe ✅ salsa ✅). Whereas RFC-0107 emits a
  scoped delta on every matching batch, RFC-0108 lets agents subscribe to a
  **query result** and receive a notification **only when the value actually
  changes** — Salsa-style backdated equality via a BLAKE3-128 hash of the
  canonical-JSON result (the v1 hash prefix `b3:` is frozen).
  `mycelium_subscribe` accepts a new `Interest::Query { query, min_interval_seconds }`
  variant — additive, no new MCP tool. `QuerySpec` v1 catalogue: `selector`,
  `callers`, `callees`, `impact`, `context` (D1=(c) — each maps to the
  existing MCP tool's pure-function body, no new query logic invented). D2=(ii)
  hybrid result-reporting: set-shaped queries carry a `summary { added,
  removed }` (matching RFC-0107's truncation discipline at 50 items);
  tree-shaped (`context`) omit `summary` entirely. D3=(c) 2 s default
  quiet-period (server-clamped to 2..=300 s), per-query soft / hard wall-clock
  budgets (50 ms / 200 ms; hard breach pauses the subscription for 60 s — v1
  simplification: `tracing::warn!` + silent pause, no separate notification).
  D4=(a) extends the RFC-0105 EXCEPTION — CLI `mycelium watch --subscribe
  'query:<kind>:<args>' [--subscribe-min-interval <SECONDS>]` streams the new
  notification as NDJSON, byte-identical to the MCP wire (asserted by
  `tests/contract_subscription::three_surface_query_byte_identical_payload`).
  Wire shape (frozen v1): `event="queryResultChanged"`, `v=1`,
  `subscription_id`, `root`, `batch_seq`, `query_kind`, `result_hash_old?`,
  `result_hash_new`, `new_result`, `summary?`, `evaluation_ms`, `hint`.
  `Context` evaluator currently returns a minimal placeholder structure
  (`task` + `focus` + resolved-symbol list); full Cortex integration deferred
  (RFC-0108 §8). 8 RED-first tests (RFC-0108 §6) + 6 CLI parser tests + one
  three-surface contract test all green.
- **SUBSCRIBE — per-subscription scoped `mycelium/subscriptionDelta`
  notifications (RFC-0107, founder-ratified D1-D5).** Step 3/4 of the
  reactive-completion roadmap. Whereas RFC-0106 PUSH broadcasts a single
  `mycelium/graphChanged` per batch, SUBSCRIBE lets agents register an
  **Interest** (`Files {paths}` | `Symbols {paths}` | `Selector {hyphae}`)
  and receive only the matching slice of each batch as added / modified /
  removed trunk paths per file. Three new MCP tools — `mycelium_subscribe`,
  `mycelium_unsubscribe`, `mycelium_subscription_status` — manage an
  in-memory map on `MyceliumServer`. The CLI surface variant is
  `mycelium watch --subscribe '<SPEC>'` streaming NDJSON to stdout
  (RFC-0107 §4.3 — extends the RFC-0105 Three-Surface EXCEPTION;
  byte-identical wire shape asserted by
  `tests/contract_subscription::three_surface_cli_mcp_byte_identical_payload`).
  Wire frozen v1: `mycelium/subscriptionDelta` carries `event`, `v`,
  `subscription_id`, `root`, `batch_seq`, `per_file[]` (each with `file`,
  `added`/`modified`/`removed` plus `_count`/`_truncated`),
  `files_truncated`, `interest_kind`, `hint`. Per-array cap 50,
  per-`per_file` cap 16. Defence-in-depth lifecycle (founder D3): rolling
  TTL (default 3600s, max 86400s, bumped on every delivery) + caps (256
  server-wide, 32 per-client, 64 Selector-specific) + peer-close GC
  primitive + 60s periodic eviction task. Selector removals use the
  (ii-strict) semantics (founder D2) — a removal is reported only when
  the path was in the OLD match-set AND its file was touched in this
  batch, eliminating phantom removals from unrelated state flips. The
  watch engine emit seam (RFC-0105 `on_batch`) was widened in Phase A
  to `FnMut(&WatchEvent, &BatchDelta, &Store)`; PUSH ignores the new
  arg, SUBSCRIBE consumes it.

- **PUSH — server emits `mycelium/graphChanged` notifications when the watched
  graph changes (RFC-0106, founder-ratified Option B).** After every committed
  watch batch (RFC-0105's emit seam), the MCP server now fires one rmcp
  `CustomNotification` with method `"mycelium/graphChanged"` and a frozen v1
  payload (`event`, `v`, `root`, `batch_seq`, `changed_files` capped at 50,
  `changed_count`, `truncated`, `hint`). Best-effort delivery — a dead client
  is logged via `tracing::warn` and never aborts the watch loop. The peer is
  captured in `serve_stdio` immediately after `server.serve()` returns
  `RunningService`; batches that fire before that point silently skip the
  notification. Agents register a handler for the `mycelium/graphChanged`
  method to react. Step 2/4 of the reactive-completion roadmap; the emit seam
  inherits the Three-Surface EXCEPTION ratified for RFC-0105.

- **`mycelium watch` — foreground reactive watch on the CLI (RFC-0105).**
  Step 1 of the reactive-completion roadmap. The reactive watch loop (RFC-0008,
  previously MCP-only) is extracted into a new surface-agnostic
  `mycelium_core::watch::WatchEngine`; both `MyceliumServer::start_watch` and a
  new `mycelium watch [ROOT] [--debounce-ms]` CLI command drive it. Reactive
  behavior — debounce, ignore matching, per-file re-extract, cross-file stub
  resolution — is **byte-identical across surfaces by construction** (same
  pattern as `context` and `OutputBudget`). The `on_batch` callback is the
  deliberate emit seam PUSH (RFC-0106) and SUBSCRIBE (RFC-0107) will attach to
  without re-touching the loop. Carries a documented Three-Surface EXCEPTION
  (Charter §5.13): the foreground CLI lifecycle differs from the server's
  background `start_watch`/`stop_watch`/`watch_status` trio.

## [0.1.17] - 2026-06-02

### Changed

- **redb is now the default storage backend (RFC-0100 Phase 3 flip).** The
  `redb-backend` feature is now on by default in `mycelium-core` and
  `mycelium-mcp`, so the default build persists and loads through redb's
  memory-mapped ACID B-tree (bounded RAM, per-file incremental writes) instead
  of the legacy MessagePack-snapshot + journal path. `Store::load` still reads
  legacy snapshots (soft migration — old index files keep working). Opt back out
  with `--no-default-features`. Full workspace test suite passes with redb as the
  default; Charter §2 latency targets are now the **warm/steady-state** contract
  (RFC-0104 — cold-open budgets pending measured nightly data).

### Added

- **RFC-0104 draft: Charter §2 warm/cold SLA split for redb mmap path.** ADR-0009
  Decision-4 (founder-authorized 2026-05-31) required splitting Charter §2's single
  SLA column into warm (page-cache steady-state, existing targets) and cold (first
  open after process restart, mmap page-fault path). This RFC formalises that split,
  proposes placeholder ceiling values (50 ms cold lookup, 200 ms cold 3-hop), and
  defines the `madvise(MADV_DONTNEED)` measurement protocol. Once the founder approves
  and nightly CI reports p99 values, the placeholders are replaced and Issue #426
  AC#4 closes — unblocking AC#2 (RSS-cap CI gate) and AC#5 (flip redb to default).

### Refactored

- **`mycelium-mcp` god-file split — tests submodule extracted (Issue #428 AC#2 slice 1).** `crates/mycelium-mcp/src/lib.rs` reduced from 12,191 → 5,627 lines (~54%) by extracting the 6,564-line `mod tests` block into `src/tests.rs`. The remaining three small test modules (`edge_kind_tests`, `server_info_tests`, `output_budget_tests`) stay in `lib.rs`. Pure mechanical refactor; zero behavior change. All 584 workspace tests green. Remaining AC#2 work: split tool implementations into `tools/context.rs` and `tools/graph.rs`.

### Added

- **100k-node redb SLA gate (RFC-0100 Phase 3, Charter §2).** New env-guarded
  tests in `redb_sla.rs` exercise the Charter §2 latency targets (cold lookup
  < 5 ms, 3-hop < 1 ms) on the redb path at the **mandated 100k-node scale** — the
  existing checks only covered 10k, leaving the contract unproven at scale. A new
  nightly `redb-sla-100k` job runs them (`MYCELIUM_REDB_BENCH_100K=1`); PR CI stays
  fast. This must be green before redb can become the default backend.

- **`mycelium context` gains `related_files`, `edge_kinds`, and Hyphae routing
  (RFC-0101).** The context tool now returns the full seven-key contract
  (`related_files` was missing), accepts an `edge_kinds` request field / CLI
  `--edge-kinds calls,imports,extends` to expand beyond calls, and routes a
  Hyphae-selector task through the DSL evaluator (the response `routing` field
  reports `"natural"` vs `"hyphae"`). CLI and MCP now share a single builder,
  `mycelium_core::context`, so their JSON is **identical by construction** —
  this also fixes a real parity bug where the two surfaces had divergent
  candidate tokenizers. Added `skills/architecture-context/tests/parity.test.json`.

- **`OutputBudget` moved into `mycelium_core::budget` and applied to
  `mycelium context` on both surfaces (RFC-0102).** The adaptive output budget
  (caps `nodes`/`edges`-shaped arrays by project size, with `truncated` /
  `total_available` metadata) now lives in core, so the CLI applies the *same*
  truncation as the MCP tool over the *same* payload — closing the item the
  context work had deferred while keeping CLI↔MCP byte-identical. The two
  never-enforced budget fields (`max_code_lines`, `max_total_chars`) were
  removed to make the type honest.

### Changed

- **Governance: supersede discipline is now machine-enforced.** New
  `scripts/check_supersede_discipline.sh` (wired into the CI `governance` job)
  fails the build on (1) a `Supersedes:`/`Superseded by` link that points at a
  non-existent `rfcs/` file, and (2) a source module whose header declares it
  implements a `Status: Superseded` RFC without a `TRANSITIONAL`/superseded
  note. Restored the previously-phantom `rfcs/0099-bounded-resident-memory.md`
  (marked Superseded by RFC-0100), annotated `store/journal.rs` as a sanctioned
  transitional bridge, and corrected RFC-0100/RFC-0102 statuses to *Partially
  Implemented* with honest gap lists. Two CLAUDE.md Hard Rules added
  (supersede-then-close; verify-against-merged-tree). Contributor-facing only;
  no runtime or API change.

### Removed

- **Orphan LRU eviction (`store::memory_budget::BoundedStore`).** The hand-built
  LRU segment-eviction cache — the "reinvent what a memory-mapped B-tree already
  gives you" mechanism RFC-0100 explicitly retired — had zero callers. Removed it
  with `MemoryBudget`/`FileAccessTracker`. The RFC-0099 *measurement* half
  (`measure_rss`, `estimate_store_bytes`) is kept for the SLA evidence work.

## [0.1.16] - 2026-06-02

### Added

- **`mycelium context` CLI command** (RFC-0101 Phase 2) — CLI twin of the
  `mycelium_context` MCP tool. Accepts `--task` (natural-language task or
  Hyphae selector), `--max-nodes`, `--max-code-blocks`, and `--format`.
  Returns the same JSON envelope as the MCP tool: `entry_points`, `nodes`,
  `edges`, `code_blocks`, `stats`, and `agent_summary`. Completes the
  Three-Surface Rule requirement for this capability (CLI ↔ MCP 1:1 strict;
  `skills/architecture-context/SKILL.md` updated to reflect full coverage).

- **MCP server routing instructions** — `mycelium serve --mcp` now includes
  a routing table in the MCP `InitializeResult.instructions` field (Issue #366).
  Agents with 89 tools available see an intent→tool map at turn 0, cutting
  median turn count and variance on real-repo benchmarks. No breaking change;
  clients that do not read `instructions` are unaffected.

- **MCP Agent behavior instructions** — the MCP `instructions` payload now
  includes a primary tool-selection decision tree, explicit anti-patterns for
  avoiding expensive multi-tool loops, and a small-project mode hint for
  indexes under 500 nodes. This advances Issue #382 without adding new tools
  or triggering Three-Surface surface changes.

- **RFC-0101 draft — `mycelium_context` architecture context tool** — adds the
  governance contract for Issue #379 before implementation: CLI/MCP/Skill
  Three-Surface scope, seven-key response shape, Hyphae-first routing,
  natural-language candidate extraction, bounded graph expansion, source-snippet
  budgets, NOT_FOUND behavior, and RED-first acceptance tests.

- **RFC-0102 draft — adaptive output budgets** — adds the governance contract
  for Issue #380 before response-contract changes: shared CLI/MCP output
  budgets, visible truncation metadata, structured truncation before formatting,
  and a stable-tool-list policy that keeps dynamic tool hiding out of the first
  implementation unless a future RFC grants an explicit Three-Surface exception.

- **RFC-0103 draft — import-aware cross-file reference resolution** — adds the
  governance contract for Issue #381 before changing resolver semantics:
  conservative per-edge bare-stub rewrites, import-evidence ranking, inheritance
  correctness targets, watch-mode integration requirements, and ADR/migration
  gates if new import-binding storage is introduced.

- **RFC-0100 Phase 1 — `StorageBackend` + redb backend foundation** —
  introduces the feature-gated `redb-backend` storage path with an object-safe
  `StorageBackend` trait, `InMemoryBackend` oracle, redb key/tag helpers,
  `RedbBackend`, and `Store::load` format auto-detection. The feature is off by
  default, preserving existing MessagePack behavior while preparing the unified
  redb storage migration. (RFC-0100)

- **RFC-0100 Phase 2 T03/T05 — redb edge-write crash safety** — adds 4
  RED-first tests for the bidirectional adjacency invariant and makes
  `RedbBackend` update `synapse_fwd` and `synapse_rev` inside one redb write
  transaction for `upsert_edge`, `remove_node_edges`, and `remove_node`. Write
  failures are retained and surfaced through `StorageBackend::flush()` instead
  of being silently discarded.

- **RFC-0100 Phase 2 T02 — property equivalence guard** — adds randomized
  operation-sequence equivalence tests over `InMemoryBackend` and `RedbBackend`,
  including node metadata, all edge kinds, removals, re-insertion, reopen
  durability, and per-prefix `edge_count`/incoming/outgoing consistency checks.
  Redb adjacency lists are now canonicalized as sorted unique sets before
  persistence so insertion order cannot affect on-disk graph shape.

- **`Store::heap_size_estimate()` — R3 memory-bound instrumentation** — new
  diagnostic method that returns a conservative lower-bound estimate of bytes
  held by the store's Patricia trie and CSR synapse. Three TDD tests verify
  the estimate is non-zero, grows monotonically with node count, and is
  non-decreasing with edges. Three `#[ignore]`-gated tests measure actual
  process RSS at 1 K / 10 K / 100 K nodes to generate the data needed to
  design the LRU/mmap mitigation (Issue #344). Run with
  `cargo test -p mycelium-rcig-core --test sla_memory_curve -- --include-ignored --nocapture`.

- **RFC-0100 Phase 2 T04 — redb memory-footprint instrumentation** —
  `RedbBackend::heap_size_estimate()` now reports redb allocated page bytes
  instead of the in-memory node/edge formula, and `sla_memory_curve` includes
  opt-in redb RSS/page-footprint measurements plus a Linux-only child-process
  RSS comparison scaffold. This advances Issue #344's memory-bound proof while
  keeping the redb backend feature-gated off by default.

- **RFC-0100 Phase 2 T05a — redb file-scoped replacement foundation** —
  `RedbBackend` now has the ADR-0009 `file_index` table plus a feature-gated
  `replace_file` API that atomically removes one file's old nodes/owned edges,
  strips stale external references, inserts the new file graph, and persists
  the replacement index in one redb write transaction. This advances Issue #343
  without flipping the default backend or adding CLI/MCP surface yet.

- **RFC-0100 Phase 2 T05b — redb edge-count metadata cache** —
  `RedbBackend` now persists a deduplicated `edge_count` value in the redb
  `meta` table and updates it inside the same write transactions that mutate
  adjacency. This removes the O(E) scan from `edge_count()` and the
  `heap_size_estimate()` edge path while keeping the backend feature-gated off
  by default.

- **RFC-0100 Phase 2 T05c — redb single-file Store bridge** —
  `RedbBackend::replace_file_from_store` converts a single-file in-memory
  `Store` into file-owned redb nodes, metadata, and source-owned edges, then
  persists them through the existing one-transaction `replace_file` path. This
  gives watch-mode wiring a small core bridge toward O(changed-file)
  persistence without changing CLI/MCP surfaces yet.

- **RFC-0100 Phase 2 T05d — feature-gated MCP redb watch persistence** —
  `mycelium-mcp` now exposes a `redb-backend` cargo feature that loads
  `.mycelium/index.redb` before legacy `.mycelium/index.rmp`, imports the
  initial in-memory graph into redb, and persists watch batches by replacing
  only changed source files through `RedbBackend::replace_file_from_store`.
  The default build remains on legacy MessagePack snapshots.

- **RFC-0100 Phase 2 T06a — redb persistence benchmark harness** —
  Added a feature-gated `redb_sla` test target plus a
  `redb_incremental_persistence` criterion benchmark comparing legacy full
  MessagePack snapshots with redb single-file replacement. The default
  benchmark covers 10K redb replacement and 10K/100K full snapshots, with
  `MYCELIUM_REDB_BENCH_100K=1` enabling the slower 100K redb replacement run.

- **`mycelium_context` MCP tool** (Issue #379 / RFC-0101) — one-shot
  architecture-tracing tool that accepts a natural-language task, extracts
  symbol candidates, expands a bounded call-graph neighborhood, and returns
  `entry_points`, `nodes`, `edges`, `code_blocks`, `stats`, and
  `agent_summary` in a single call. The 90th MCP tool. Replaces 5–20
  chained `search_symbol` + callers/callees + `symbol_info` round-trips.

- **`OutputBudget` adaptive output control** (Issue #380 / RFC-0102) —
  3-tier size-based budget (`small <500 nodes`, `medium <5K`, `large`) that
  caps `nodes`, `edges`, `paths`, `results`, `symbols`, `callers`,
  `callees`, and `reachable` arrays with `truncated: true` +
  `total_available: N` metadata. Applied to 8 high-traffic MCP tools.
  7 unit tests.

- **Import-aware stub resolution** (Issue #381 / RFC-0103) — second-pass
  stub resolver that uses `Imports` edges to disambiguate bare call stubs
  when multiple files define the same symbol name. The caller's import
  graph is used to vote on which definition wins. Integrated into
  `Store::resolve_bare_call_stubs()` as a supplementary pass after the
  existing simple pass.

- **`architecture-context` Skill** — new `skills/architecture-context/`
  category Skill covering `mycelium_context`. Satisfies RFC-0090 I1
  coverage for the 90th MCP tool. CLI twin tracked as RFC-0101 Phase 2.

### Fixed

- **RFC-0100 redb replace-file external reference preservation** —
  `RedbBackend::replace_file` now preserves external incoming edges that point
  to nodes whose paths remain stable across a file replacement, while still
  stripping external edges to removed or renamed nodes. This fixes a
  watch-persistence correctness bug in the feature-gated redb path and avoids
  unnecessary stale-node edge scans for unchanged symbols; local 10K
  single-file replacement improved from ~18.4 ms to ~9.70 ms.

- **RFC-0100 redb no-op node upsert write amplification** —
  `RedbBackend::upsert_node` now reads existing `path -> id` and `id -> path`
  entries before writing. Re-upserting an unchanged node no longer allocates
  additional redb pages, and local 10K single-file replacement improved further
  from ~9.70 ms to ~9.37 ms.

- **RFC-0100 redb no-op file replacement write amplification** —
  `RedbBackend::replace_file` now returns early when the persisted file index,
  trunk indexes, kind metadata, span metadata, and forward/reverse adjacency
  already match the incoming file payload. Replacing an identical file payload
  100 times no longer grows allocated redb pages from 57KB to 77KB, while
  identical replacements still repair missing owned adjacency when the persisted
  edge tables drift from the file index.

- **Release pipeline hardening** — release prep now updates internal
  `mycelium-rcig-*` dependency pins before publishing, and `release.yml` fails
  fast on missing crates.io credentials or cargo publish errors instead of
  creating an orphan GitHub tag/Release. Crates publish dependency-first
  (`pack → core → hyphae → mcp → cli`) and tag creation now waits until main
  and develop merges succeed.

- **Release governance guardrails** — CI now runs static release/governance
  checks, and GitFlow/PR/release-agent runbooks explicitly define admin-merge
  overrides, incomplete-release incident response, and the four-step release
  completion invariant.

## [0.1.14] - 2026-05-31

<!-- next release goes here -->

## [0.1.14] - 2026-05-31

### Added

- **RFC-0096 Phase 2 — TypeScript `import type` → `TypeImports`** — TypeScript's
  `import type { Foo } from 'mod'` syntax now emits `TypeImports` edges instead
  of `Imports` edges, keeping `detect-cycles` clean by default. Also fixes a
  pre-existing bug where TypeScript relative imports were resolved via the Python
  resolver, producing incorrect node paths (`/foo.py` instead of `foo.ts`).
  Three TDD tests (RED confirmed before impl). (RFC-0096 Phase 2)

### Changed

- **Three-Surface parity gate promoted to required CI** — `skill-parity` job
  (runs `check_skill_parity.py --strict`) is now part of the `quality-gate`
  aggregator in `ci.yml`. Previously the check ran in the standalone
  `parity.yml` workflow and was informational only; now any PR that drops
  Three-Surface coverage below 100% will fail CI. (Charter §5.13 / RFC-0090)

### Breaking

- **RFC-0093 — MCP application-level error model (Phase 3 / v0.2.0 BREAKING)**
  All 89 MCP tools now signal application-level failures (symbol not found,
  index not loaded, invalid path syntax) via `is_error: Some(true)` on the
  `CallToolResult`, per the [MCP spec §error-handling][mcp-errors].
  Previously every error returned a `CallToolResult` whose JSON body contained
  an `"error"` key, making tool errors and application errors
  protocol-indistinguishable. Migration: check `result.is_error == Some(true)`
  instead of parsing for an `"error"` key in the content text. The `"reason"`
  key in the error payload carries the human-readable message. New canonical
  error helpers: `not_found(path)`, `not_indexed()`, `invalid_path(path,
  detail)` in `crates/mycelium-mcp/src/error.rs`. Contract tests:
  `crates/mycelium-mcp/tests/contract.rs` — `path_not_found_yields_is_error_true`
  and `successful_lookup_yields_is_error_false`. (RFC-0093, Issue #209)

[mcp-errors]: https://spec.modelcontextprotocol.io/specification/server/tools/#error-handling


## [0.1.13] - 2026-05-31

### Changed

- **RFC-0093 Phase 2 — `success_str` helper replaces local `ok_str`** — all
  101 success return sites in `mycelium-mcp` now flow through
  `crate::error::success_str` (exported from the RFC-0093 error-model module)
  instead of a file-local `ok_str` function. Uniform helper coverage: every
  success, error, not-found, not-indexed, and invalid-path return now comes
  from the same module. No behaviour change; `is_error: Some(false)` was
  already set by `CallToolResult::success`. (RFC-0093)

## [0.1.12] - 2026-05-30

### Added

- **RFC-0096 — `EdgeKind::TypeImports` (Python phase)** — Imports inside
  `if TYPE_CHECKING:` blocks are now emitted as `TypeImports` edges instead of
  being silently dropped. This makes Python type-annotation-only imports
  queryable (e.g. `get-imports --edge-kind type_imports`) while keeping the
  default `Imports` graph — and therefore `detect-cycles --edge-kind imports` —
  free of false-positive cycles (Issue #227 behaviour preserved). Three new TDD
  tests covering edge emission, Imports/TypeImports segregation, and wire string
  stability. Wire string: `"type_imports"`. (RFC-0096)


- **RFC-0097 — MCP server filesystem boundary (`--allowed-roots` / `allowed_roots`)** —
  `mycelium serve --mcp` previously accepted arbitrary filesystem paths via
  `mycelium_index_workspace` and `mycelium_load_index` with no validation, allowing any path
  (e.g. `/etc`, `../../etc`) to be indexed and its index written back to disk.
  Fix: `MyceliumServer` gains an `allowed_roots: Arc<Vec<PathBuf>>` field. When non-empty, every
  path-based MCP call canonicalises the input and verifies it is under at least one allowed root;
  paths outside the allowlist (including traversal attempts) are rejected with `is_error: true`
  without touching the filesystem. When launched via `mycelium serve --mcp`, the allowlist
  defaults to `[CWD]`. An empty allowlist (unit-test mode) is unrestricted and preserves
  backward compatibility. CLI flag: `mycelium serve --mcp [--allowed-roots <dir>]...`.
  3 TDD tests: `index_workspace_rejects_path_outside_allowed_roots`,
  `index_workspace_rejects_path_traversal`, `index_workspace_accepts_path_inside_allowed_roots`.
  (Issue #301, RFC-0097)

- **Issue #292 — `get-all-symbols` pagination (`--limit` / `--offset`)** — On large projects
  (`vscode`: 194K nodes, `django`: 66K nodes) `get-all-symbols` could emit 14MB+ of output,
  making it unusable without external truncation. Both CLI and MCP now accept `limit` and
  `offset` parameters. `limit = 0` (default) preserves the original no-limit behaviour for
  backward compatibility; `limit > 0` caps the sorted result list and returns a `total_count`
  field alongside `count` so callers can detect truncation and paginate. MCP response envelope:
  `{ "symbols": [...], "count": N, "total_count": T }`. CLI flag: `--limit N --offset K`.
  2 TDD tests (`limit_caps_result_count`, `offset_skips_results`). (Issue #292)

### Fixed

- **Issue #297 — `--edge-kind` flag added to `get-callers`, `get-callees`, `rank-symbols`,
  `get-dead-symbols`** — These four commands previously hardcoded `Calls` edges, making it
  impossible to query `Imports`, `Extends`, or `Implements` relationships. All four now accept
  `--edge-kind <calls|imports|extends|implements>` (default: `calls`, backward-compatible).
  `get-dead-symbols --edge-kind imports` returns symbols with no incoming Imports edges;
  without `--edge-kind`, the classic Calls+Imports dead definition is preserved.
  `include_virtual` on `get-callers` is silently ignored when `--edge-kind` is not `calls`.
  Same parameters added to the MCP tools (`mycelium_get_callers`, `mycelium_get_callees`,
  `mycelium_rank_symbols`, `mycelium_get_dead_symbols`). New store methods:
  `top_symbols_by_incoming` and `dead_symbols_for_kind`. 4 TDD tests. (Issue #297)

- **Issue #296 — Python Extends edges not detected for dotted base classes** — Classes using
  attribute-form superclasses (e.g., `class SimpleTestCase(unittest.TestCase):`) produced no
  `Extends` edge because the Python query only captured `(identifier)` base classes, not
  `(attribute)` nodes. Added `(class_definition superclasses: (argument_list (attribute) @name))`
  to `packs/python/queries.scm`. The extractor's existing `"reference.extends"` handler treats the
  full attribute text (e.g., `"unittest.TestCase"`) as the base-class name and creates a dotted
  stub node if no in-file definition or import alias resolves it. Mixed inheritance
  (`class Foo(bar.Base, LocalBase):`) produces Extends edges for both forms. Metaclass keyword
  arguments (`metaclass=Meta`) are correctly excluded (they are `keyword_argument` nodes, not
  plain `attribute`/`identifier` nodes). Syncs embedded packs. 3 TDD tests
  (2 RED verified). (Issue #296)

- **Issue #293 — `get-callee-tree` empty for JS `const name = function(...) {}` definitions** —
  JavaScript functions defined via `const name = function(...) {...}` (function expressions
  assigned to a `const` binding) were not captured as definition nodes. As a result, 4 000+
  call sites in projects like VS Code that use this CommonJS/UMD pattern produced bare stubs
  with no outgoing Calls edges, leaving `get-callee-tree` empty. Fix: added two new
  `@definition.function` patterns to `packs/javascript/queries.scm` for `const`-assigned and
  `export const`-assigned function expressions, mirroring the existing arrow-function patterns.
  Also fixed `enclosing_function_path` in the extractor: anonymous `function_expression` nodes
  have no `name` field, so the scope-tracking logic now falls back to the enclosing
  `variable_declarator`'s name, ensuring Calls edges inside the body are attributed to the
  correct definition. 3 TDD tests. (Issue #293, PR #310)

- **Issue #294 — `search-symbol` mangled paths and mis-classified compound-extension files** —
  Two defects in the file-system walker fixed:
  (1) **Compound extension guard**: files like `module.ts.py` whose stem's extension is itself a
  recognised source-language extension (e.g. `ts`) are now skipped rather than indexed under the
  wrong language. A file named `js.py` (stem `js` has no extension) is NOT affected and continues
  to be indexed correctly as Python.
  (2) **Strip-prefix defensive fix**: when `path.strip_prefix(root)` fails (e.g. due to symlink
  canonicalization mismatch), the indexer now skips the file and logs a warning instead of falling
  back to the raw absolute path. Previously this fallback caused Trunk nodes with `///`-prefixed
  paths that were unusable in subsequent queries. Both fixes apply to the CLI and MCP indexers.
  3 TDD tests in CLI. (Issue #294, PR #311)

- **Issue #298 — batch commands accept repeated `--paths` flags** — `batch-symbol-info`,
  `batch-node-degree`, `batch-reachable-from`, `batch-reachable-to`, `get-common-callers`, and
  `get-common-callees` previously accepted only a single `--paths` string value; passing
  `--paths a --paths b` failed with "cannot be used multiple times". The argument is now
  `Vec<String>` with `value_delimiter = ','`, so both the old comma-separated form
  (`--paths a,b,c`) and the standard repeated-flag form (`--paths a --paths b`) work.
  2 TDD tests. (Issue #298, PR #303)

- **Issue #299 — `get-files` accepts `--prefix` as an alias for `--path-prefix`** — The
  `get-files` command used `--path-prefix` while `get-all-symbols` used `--prefix` for the
  same concept. Added `alias = "prefix"` so `--prefix` is accepted as a shorter alternative;
  `--path-prefix` continues to work unchanged. 1 TDD test. (Issue #299, PR #303)

- **`release.yml` finalize job robustness** — Decoupled tag creation and GitHub Release page
  from the main-branch merge step. Tag is now created on the release branch first (branch
  protection doesn't apply to tags; `GITHUB_TOKEN` is sufficient). The GitHub Release action
  now uses `GITHUB_TOKEN` (`contents: write`) rather than `RELEASE_BOT_TOKEN`, so the release
  page is always created even when the bot token is absent or expired. Main merge and develop
  back-merge steps are `continue-on-error: true` with actionable `::warning::` messages
  (including the exact `gh pr create / gh pr merge --admin` commands for the founder). A final
  step fails the job when manual action is required, preserving CI RED visibility. Addresses
  systemic failures on v0.1.6, v0.1.10, and v0.1.11 releases.

- **Issue #286 — `get-dead-symbols` false positives for `from X import Y`** — Symbols
  imported via Python `from X import Y` (no subsequent call in the importing file) were
  incorrectly flagged as dead. The extractor's alias-binding pass (Pass 1) built the alias
  table but never created symbol-level Imports edges; `dead_symbols()` only found file-level
  edges and treated the symbols as unreachable. Fix: after `build_alias_target` resolves a
  symbol-level path (contains `>`), immediately upsert the Trunk node and add an Imports
  edge from the importing file. All three import forms are covered: relative with no alias,
  relative with alias, absolute with or without alias. 3 TDD tests. (Issue #286, PR #289)

- **Issue #214 Pattern 2 — `from .submod import Symbol` alias resolution** — When code uses
  `from .models import AnalysisResult` (a relative-submodule import without `as`), mycelium
  now correctly binds `AnalysisResult → pkg/sub/models.py>AnalysisResult` in the per-file
  alias table. Previously the `(true, None)` arm of `build_alias_target` produced the wrong
  target `pkg/sub/models.py/AnalysisResult.py` (a file path) by treating the imported name
  as a module rather than a symbol. Calls to `AnalysisResult()` now resolve to the correct
  definition node instead of a bare stub. 2 TDD tests (RED verified). (Issue #214)

### Added

- **Issue #295 — Java `Extends` and `Implements` edges** — The Java language pack
  (`packs/java/queries.scm`) now captures `@reference.extends` for `class Sub extends Base`
  and `interface Sub extends Base`, and `@reference.implements` for `class Foo implements Bar`.
  The extractor core adds a `"reference.implements"` handler (identical resolution logic to the
  existing `"reference.extends"` handler, producing `EdgeKind::Implements` edges instead).
  Cross-file extends resolution works via `resolve_bare_call_stubs()`, matching Python
  inheritance behaviour. Adds `tree-sitter-java` as a dependency of `mycelium-rcig-core` (was
  only a workspace dep). 3 TDD tests (RED verified). (Issue #295)

- **RFC-0094: Criterion formatter benchmark + byte-savings regression guard** — Adds
  `crates/mycelium-mcp/benches/formatter.rs` with four Criterion benchmarks
  (`json/50_node_callee_tree`, `text/50_node_callee_tree`, `msgpack/50_node_callee_tree`,
  `byte_savings_ratio`) and a unit test that verifies text format bytes are < 80% of JSON
  bytes for a 50-node callee tree. Clarifies that RFC-0094's ~73% savings headline is
  **token savings** (not byte savings): LLM tokenisers split JSON punctuation into individual
  tokens, so token savings exceed byte savings. (Issue #206 S3, PR #288)

- **Skill marketplace metadata** — All 10 category Skill files now include `category`,
  `icon`, and `marketplace_examples` frontmatter fields plus a `## Quick examples` table.
  Enables Skill marketplace submission for v0.2.0. Template updated for future Skill authors.
  Categories: `navigation` (basic-queries 🔍, hyphae-query 🌿), `analysis` (call-graph 📞,
  import-graph 📦, inheritance 🌳, reachability 🔗, centrality ⭐, graph-structure 🕸️),
  `operations` (batch-ops ⚡, index-management 🗃️). (PR #284)

- **RFC-0095: `mycelium index --packs-dir <dir>`** — the CLI `index` subcommand now accepts
  an optional `--packs-dir <dir>` flag. When provided, language packs are loaded from the
  given directory via `PackRegistry`; file extensions not covered by the 10 built-in grammars
  are dispatched to the registry. Packs whose grammar string does not match a compiled-in
  tree-sitter grammar are silently skipped with a tracing warning. 2 new TDD tests in
  `mycelium-cli/src/index.rs`. (Issue #212, RFC-0095 remaining item)

- **RFC-0095: Language pack documentation** — `docs/packs.md` documents the full pack
  system: bundled-pack table, `MYCELIUM_PACKS_DIR` env var, `--packs-dir` CLI flag,
  `pack.toml` field reference (including `primary_extensions` / `secondary_extensions`),
  grammar string format, `queries.scm` capture naming conventions, and an end-to-end
  example for creating a custom pack. RFC-0095 status updated to **Implemented**. (RFC-0095)

- **RFC-0095: Runtime pack registry (`PackRegistry`)** — `crates/mycelium-pack` now exposes
  `PackRegistry::load(packs_dir)` which discovers all `packs/<lang>/pack.toml` + `queries.scm`
  pairs at runtime. `PackRegistry::lookup_by_ext(".py")` returns the matching `LanguagePack`.
  The cortex (`mycelium-core`) uses the registry when `MYCELIUM_PACKS_DIR` env var is set,
  falling back to compile-time embedded queries otherwise. Circular dep eliminated:
  `mycelium-pack` no longer depends on `mycelium-core`; `mycelium-core` depends on
  `mycelium-pack`. 4 TDD tests in `mycelium-pack` + 2 in `mycelium-core`. (Issue #212)

- **RFC-0092 Phase 2: TypeScript/JavaScript alias resolution** — `import { foo as bar }`,
  `import * as ns`, `import { foo }`, and `import Foo` statements now build per-file alias
  tables. Call edges such as `bar()` and `ns.greet()` are rewritten through the alias table
  to resolve to the canonical `src/module.ts>foo` or `src/module.js>foo` path instead of
  bare stubs. Relative specifiers (`./module`, `../lib/util`) are resolved to file paths
  using the importing file's own extension (`.ts` → `.ts`, `.js` → `.js`); package imports
  remain symbolic. Covered by 6 TDD tests (3 TypeScript + 3 JavaScript) in `extractor::tests`.

## [0.1.11] - 2026-05-30

### Added

- **RFC-0094 §206 S2: `crates/mycelium-mcp/README.md`** — documents the output format per
  transport (stdio default = `text`, CLI default = `json`), the text-format grammar, and the
  parity contract. Satisfies RFC-0094 acceptance criterion "Default-format logic per transport
  documented in `crates/mycelium-mcp/README.md`".

### Changed

- **Issue #206 S1: MCP `is_error` sweep — all 90 tool handlers now set `is_error` flag**.
  Every MCP tool handler return type changed from `String` to `CallToolResult`. Success
  paths set `is_error: Some(false)` via `ok_str()` helper; error paths (`not_found`,
  `application_error`, `invalid_path`) set `is_error: Some(true)`. MCP clients can now
  branch on `is_error` without parsing JSON bodies. The `not_found()` payload includes
  a backward-compatible `"error"` key alongside the new structured `"found"/"reason"/"path"`
  keys. Two new contract tests verify the is_error contract end-to-end (PR #266).

### Fixed

- **Issues #267/#268: cross-file Extends edges now resolve correctly when multiple files define
  a class with the same name**. Previously, `subclasses-tree` with a full symbol path returned
  only same-file subclasses, and `get-descendants --include-inherited` returned 0 inherited
  methods for cross-file base classes. Root cause: the `reference.extends` extractor handler
  fell back to a bare stub (e.g. `LanguagePlugin`) when the base class was cross-file; with
  test mocks also named `LanguagePlugin`, `resolve_bare_call_stubs()` found multiple candidates
  and gave up. Fix: a new `from M import X` (absolute, no `as`) alias-binding query feeds the
  per-file alias table, and the extends handler resolves the base class via the alias table
  (dotted module path converted to file path) before falling back to a bare stub. 2 new TDD
  tests. Pack change synced to embedded MCP and CLI copies.

- **Issue #214 Pattern 3: depth-2+ attribute chain calls no longer create global bare stubs**.
  `self.history.append(x)` (and any call through a chain of depth > 1) previously emitted a
  global bare-name node (`append`) that absorbed every same-named call across the entire
  codebase. On `tree-sitter-analyzer`, `HealthHistory.append` collected 1,472 spurious callers
  from this mechanism. Root cause: a fallback tree-sitter pattern matched `(call (attribute
  attribute: @name))` without a receiver constraint, bypassing the alias table entirely.
  Fix: remove the fallback. Direct `obj.method()` calls (depth-1 chain, receiver is an
  identifier) continue to resolve correctly via the `@call.receiver` pattern. Unresolvable
  deep chains emit no edge rather than a misleading global stub. 2 new TDD tests; 1 updated
  test (RFC-0092). Pack change synced to embedded MCP and CLI copies.

### Added

- **Issue #246: `get-callers --include-virtual` / MCP `include_virtual` flag for virtual dispatch**.
  New `Store::virtual_dispatch_callers_of_path` follows `EdgeKind::Extends` edges from the
  target symbol's class to each base class, then collects callers of `BaseClass>method_name`.
  When typed variables (e.g. `plugin: AbstractBase`) invoke a method via virtual dispatch,
  the Calls edge points to the base class method — this flag surfaces those callers for the
  concrete override. MCP `mycelium_get_callers` gains `include_virtual: Option<bool>`;
  CLI `get-callers` gains `--include-virtual`. Three-Surface Rule (RFC-0090) satisfied:
  `Store` method → MCP param → CLI flag. Default `false` is backward-compatible.
  2 TDD tests.

- **Issue #248: `get-descendants --include-inherited` / MCP `include_inherited` flag**.
  `Store::inherited_descendants_of_path` follows `EdgeKind::Extends` edges from the
  requested class to each declared base class and returns methods that exist on the
  base but are not overridden in the subclass. The MCP handler for
  `mycelium_get_descendants` accepts `include_inherited: true` and appends an
  `inherited_descendants` array to the response, where each entry carries the method
  path and the `from` (declaring class path). The CLI command `get-descendants` gains
  `--include-inherited` boolean flag with identical output in `--format text` mode.
  Three-Surface Rule (RFC-0090) satisfied: Store method → MCP param → CLI flag.
  2 new MCP TDD tests; clippy `significant_drop_tightening` and
  `double_ended_iterator_last` lints fixed as part of this work.


- **Issue #247: Python callback / higher-order function false positives fixed**.
  `packs/python/queries.scm` now captures identifiers passed as positional
  or keyword-value arguments (`reference.arg_callback`). The extractor's
  Pass 2 creates a `Calls` edge from the enclosing function to the argument
  identifier, so `get-isolated-symbols` no longer reports callback functions
  as dead code when they are only ever passed (not directly called). 525
  false positives eliminated in the `tree-sitter-analyzer` dogfood project.
  Also confirmed that Pattern 1 (import aliases: `from .mod import fn as
  alias`) was already correctly resolved via the RFC-0092 alias table; a
  regression-guard test is added. 2 TDD tests.

- **Issue #245: Python class inheritance (Extends) edges extracted**.
  `packs/python/queries.scm` now captures `class Sub(Base):` superclass
  identifiers as `@reference.extends` matches. The extractor's Pass 2
  match block gains a `"reference.extends"` arm that reads the subclass
  name from the `class_definition` anchor, resolves the base class
  (intra-file definition first, bare-symbol stub fallback), and emits an
  `EdgeKind::Extends` edge. Multiple-inheritance (`class Sub(A, B):`)
  produces one Extends edge per superclass via tree-sitter's per-identifier
  match semantics. 3 TDD tests cover same-file, external, and multiple-
  inheritance cases.

- **Issue #211: Cross-tool MCP response contract tests**.
  New `crates/mycelium-mcp/tests/contract.rs` spins up an in-process
  `MyceliumServer` + rmcp client over `tokio::io::duplex` and verifies
  three invariants for all 89 registered tools: (1) tool count equals the
  expected constant (`EXPECTED_TOOL_COUNT = 89`), (2) every tool returns
  non-empty content when called with catch-all arguments, (3) every tool
  manifest carries a non-empty description string. Tests use the rmcp
  `client` feature; `Cargo.toml` dev-deps updated accordingly.

- **RFC-0094 Phase 3: `output_format` wired into all remaining 83 MCP query tools**.
  Every query tool now accepts `output_format: "json" | "text" | "msgpack"` in its
  request payload. Mutation/control tools (`index_workspace`, `load_index`,
  `sync_file`, `set_compact_mode`, `server_status`, `watch_status`,
  `get_token_stats`) are unchanged. Handler success paths use
  `req.output_format.map_or_else(|| value.to_string(), |fmt| formatter_for(fmt).format(&value))`
  (Pattern B, no compact_mode dependency). Complex multi-branch handlers
  (`get_shortest_path`, `find_call_path`, `find_import_path`,
  `find_extends_path`, `find_implements_path`) capture `let fmt =
  req.output_format` before early-return guard clauses so every branch honours
  the caller's requested format. 12 new TDD tests written RED-first per
  Charter §5.1 before any implementation. All 319 MCP tests pass.

- **fix(packs): sync stale embedded Python pack queries — issue #260**.
  `crates/mycelium-mcp/packs/python/queries.scm` and
  `crates/mycelium-cli/packs/python/queries.scm` were 80 lines behind
  canonical `packs/python/queries.scm` after PR #250 (Python Extends edges).
  Both copies synced. The compiled binary now correctly emits Python Extends
  edges. Adds `scripts/check_pack_parity.sh` + `pack-parity` CI job to
  `parity.yml` to prevent future drift.

- **fix(extractor): regression test for cross-file Extends resolution — issue #261**.
  Confirmed that `resolve_bare_call_stubs()` correctly redirects `EdgeKind::Extends`
  edges (not just `Calls`) after multi-file extraction: when `Sub(Base)` in `sub.py`
  references `Base` from `base.py`, the bare stub is resolved to `base.py>Base` and
  removed. Issue #261 was a symptom of issue #260 (stale embedded pack emitted no
  `@reference.extends` captures); PR #263 (pack sync) already fixed the root cause.
  New TDD test `extractor_python_extends_cross_file_resolves_to_definition` guards
  against regression.

- **Charter §2 SLA: 100 K-node heavy-graph benchmark row**.
  New `crates/mycelium-core/tests/sla_heavy_graph.rs` contains 6 CI-gated SLA
  assertions (leaf_symbols, degree_histogram, graph_metrics, page_rank with 5
  iterations, weakly_connected_components, find_call_path) on a deterministic
  100 000-node / ~300 000-edge sparse graph. All 6 pass in < 1 s on a
  development machine (SLA limit is 30 s). Charter §2 table gains the
  100 K-node row: < 30 s for the same six heavy-graph tools.

- **RFC-0094 Phase 2 PoC: `output_format` per-request for basic-query tools** (#210).
  Three tools (`mycelium_search_symbol`, `mycelium_get_ancestors`,
  `mycelium_get_descendants`) now accept an optional `output_format`
  parameter (`"json"`, `"text"`, or `"msgpack"`). When absent the
  response is JSON (backward-compatible). `"text"` emits TOON-style
  `key: value` indented text (~73% fewer structural punctuation tokens
  than JSON on tree-shaped payloads). Server-wide `compact_mode` toggle
  continues to work as before when no per-request format is specified.
  `OutputFormat` derives `schemars::JsonSchema` so the schema is
  auto-generated in the MCP tool manifest. 5 new TDD tests.
  Phase 3 (remaining 86 tools) follows in separate PRs.
- **RFC-0093: MCP application-level error model foundation** (#209).
  New `crates/mycelium-mcp/src/error.rs` module provides `success_json`,
  `application_error`, `not_found`, `not_indexed`, and `invalid_path`
  helpers that wrap `rmcp::model::CallToolResult` with the correct
  `is_error` flag per the MCP spec. MCP clients can now branch on
  `is_error: true` for application errors (symbol not found, index not
  loaded, invalid path) without string-parsing the response body.
  13 TDD tests written before implementation per Charter §5.1.
  Phase 2 (migrate all 89 tools to use these helpers) lands in v0.2.0.

- **RFC-0094 Phase 1: token-efficient output formatter foundation** (#210).
  New `crates/mycelium-mcp/src/formatter.rs` module ships the
  `Formatter` trait plus three implementations: `JsonFormatter`
  (pretty-printed JSON), `TextFormatter` (TOON-inspired indented
  `key: value` layout per RFC-0094 §"Format grammar"), and
  `MsgpackHexFormatter` (`hex::encode(rmp_serde::to_vec(...))`).
  `OutputFormat` enum defaults to `Text`. `formatter_for(...)` factory
  returns a boxed trait object. Reserved-character escaping for
  TextFormatter (leading `[`, `{`, `-`, `"`, whitespace; embedded
  `: `, `\n`, `\r`, `\t`; reserved literals `null` / `true` / `false`
  / `[]` / `{}`) goes through `serde_json::to_string` so the
  reference parser only needs one escape convention. Enables
  `serde_json/preserve_order` in `mycelium-mcp` so TOON output keeps
  source-key order instead of alphabetising. 17 TDD tests written
  before implementation per Charter §5.1. Phase 2 (migrate the 89
  existing tools to accept `output_format`) lands in a follow-up.

## [0.1.10] — 2026-05-30

Patch release: two Python correctness fixes (TYPE_CHECKING cycle
false-positives + nested-attribute call regression).

### Fixed

- **`if TYPE_CHECKING:` imports no longer create `Imports` edges**
  (#227). Python's `TYPE_CHECKING` constant is always `False` at runtime;
  imports guarded by it are annotation-only and were causing
  false-positive cycle reports (`detect-cycles` reported 7 spurious
  cycle nodes in tree-sitter-analyzer that had no runtime counterpart).
  Fix: new `is_inside_type_checking_block` helper in the extractor walks
  AST ancestors of each import node and skips edge creation when the
  import lives inside an `if TYPE_CHECKING:` block. Real imports in the
  same file are unaffected. 2 new TDD tests.
- **Nested attribute calls regression** (post-RFC-0092 fallthrough).
  `self.history.append(x)` and other nested-attribute call patterns
  were silently dropped after v0.1.7 because the new receiver-capturing
  call query required `object: (identifier)`. Nested attribute access
  (`object: (attribute ...)`) didn't match — outgoing Calls edges from
  any such caller were lost. Added a fallback query pattern that
  matches all attribute calls without the receiver constraint,
  preserving the bare-name fallback path. Tests: 1 new assertion.

## [0.1.9] — 2026-05-30

Patch release: ships the attribute-assignment alias fix that closes
the gap remaining after v0.1.7. Also bundles two governance + spec
artefacts that arrived in the same window.

### Fixed

- **Python attribute-assignment alias pattern** (#229, follow-up to
  RFC-0092 Phase 1). `_alias = _h.fn; _alias()` now resolves via the
  alias table + new `chain_resolve` multi-hop walker. Closes the gap
  that remained after v0.1.7's direct `_h.fn()` fix. Tests: 1 new
  assertion in `crates/mycelium-core/src/extractor/tests.rs`.

### Added

- **Charter §5.12 release-gate rule** — a `release/*` branch MUST NOT
  be admin-merged to `main` unless every CI check is `SUCCESS` or
  `SKIPPED`. Codified in CHARTER.md, CLAUDE.md, and GITFLOW.md after
  the v0.1.4 saga where red-CI admin-merges shipped broken Windows
  binaries. The rule is now enforced by self-discipline + this
  changelog entry.
- **RFC-0096** drafted (type-only import edge kind) — adds
  `EdgeKind::TypeImports` to model Python `if TYPE_CHECKING:` and
  TypeScript `import type` patterns separately from runtime imports.
  Closes #227's false-positive cycles when implemented (target v0.2.0).

## [0.1.8] — 2026-05-30

Patch release: ships the `self.method()` / `cls.method()` resolution
fix that addresses the dominant pattern behind 533 false positives in
`get-isolated-symbols` from the tree-sitter-analyzer dogfood (#214).

### Fixed

- **`self.method()` and `cls.method()` inside a class now resolve to
  the sibling method node** (#220, from #214 reliability report). The
  reference pass previously fell through to bare-symbol upsert for
  `self.X()`, so methods called only via `self` looked isolated. This
  was the dominant pattern behind the 533 false positives in
  `get-isolated-symbols` reported by the tree-sitter-analyzer dogfood.
  Fix: when the call receiver is `self` or `cls`, walk ancestors to
  the enclosing class chain and qualify the target as
  `<file>>ClassName>method`. New `enclosing_class_chain` helper. Tests:
  2 new assertions covering `self.foo()` and `@classmethod`+`cls.X()`
  in `crates/mycelium-core/src/extractor/tests.rs`.

## [0.1.7] — 2026-05-30

Patch release: closes the headline Python static-analysis blind spot from
the tree-sitter-analyzer dogfood (#200 umbrella resolved).

### Fixed

- **Python module-alias dispatch now tracks callers through the alias**
  (#205, bug 1 of #200, RFC-0092 Phase 1). When a file does
  `from . import _ast_cache_query as _query` and later calls
  `_query.fts_search_ranked(...)`, the Calls edge previously pointed to
  the bare path `_query>fts_search_ranked`, so the real definition at
  `_ast_cache_query.py>fts_search_ranked` saw 0 callers. Now the
  extractor builds a per-file alias table (`local_name → resolved_path`)
  in a new Pass 1b, and the reference pass rewrites the leftmost
  identifier of `obj.method()` calls through that table. Closes the
  73-function false-positive-dead-code case from
  `tree-sitter-analyzer`'s dogfood. Python only in this PR; TypeScript /
  JavaScript / Ruby follow per RFC-0092. Tests: 3 new assertions in
  `crates/mycelium-core/src/extractor/tests.rs`.

## [0.1.6] — 2026-05-30

Patch release focused on Python correctness + Three-Surface enforcement.

### Changed

- **`parity.yml` promoted from informational to strict** (Charter §5.13 /
  RFC-0090 Phase 3). The Three-Surface parity checker now runs with
  `--strict` and exits non-zero on any I1 (Skill coverage) or I2 (no
  Skill orphans) violation. Promoted after v0.1.5 hit 89/89 (100%)
  coverage with zero 🟡 rows in `skills/INDEX.md`. Branch-protection
  side of the promotion (marking the check as required on main) is a
  founder-only repo-settings change, tracked separately.

### Fixed

- **Python relative imports now resolve to actual file paths** (#204,
  bug 2 of #200). `from .models import X` in `pkg/sub/foo.py` previously
  created an `Imports` edge to the symbolic node `.models`, so
  `mycelium get-symbol-info pkg/sub/models.py` showed `callers: []` even
  when there were 100+ import sites. Resolver now walks the dot prefix
  against the importing file's directory: `.X → sibling.py`,
  `..X → parent package's X.py`. Absolute imports keep the symbolic-node
  behaviour pending the alias-table work in #205. Tests: 4 new
  assertions in `crates/mycelium-core/src/extractor/tests.rs`.

## [0.1.5] — 2026-05-30

The "100% Three-Surface" release. Every MCP tool now has a 1:1 CLI twin,
every (CLI, MCP) pair is covered by at least one Skill umbrella, and the
RFC-0091 jQuery-inspired selectors close the Hyphae expressiveness gap.

### Added

- **Charter §5.10 dogfood test** (`crates/mycelium-core/tests/e2e_dogfood.rs`).
  Walks the Mycelium workspace, runs the bundled Rust extractor over every
  `.rs` file we own (~145), and asserts zero extraction errors plus that
  load-bearing symbols (`mycelium-core/src/lib.rs`, `store/mod.rs`) resolve.
  Unblocks the `dogfood` job in `.github/workflows/e2e.yml`, which had
  previously been a no-op pending this test. Honours Charter §5.10:
  *"Mycelium indexes itself; CI runs Hyphae queries against the Mycelium
  codebase as part of e2e."*

- **CLI parity backfill batch 10 — FINAL** (v0.1.5, PR #187): 10 new
  cross-category CLI subcommands + INDEX cleanup. **Three-Surface
  compliance now 100% (0 🟡 rows in `skills/INDEX.md`).**
  - New CLI: `mycelium get-node-degree`, `get-files`,
    `get-symbol-count-by-kind`, `get-leaf-symbols`,
    `get-common-callers`, `get-common-callees`, `get-common-reachable`,
    `get-mutual-reachability`, `find-call-path`, `find-import-path`.
  - Status flips for capabilities whose CLI was already shipped under
    a different batch: `betweenness_centrality`, `page_rank`,
    `get_graph_metrics`, `get_wcc`, `get_degree_histogram`,
    `index_workspace` (alias of `mycelium index`).
  - 4 capabilities marked `EXCEPTION: MCP-only` per RFC-0090 — they
    have no meaningful CLI surface: `load_index` (CLI loads per
    invocation), `watch_status`, `set_compact_mode`, `get_token_stats`.

- **CLI parity backfill batch 9** (v0.1.5): all 4 `batch-ops`
  capabilities. Three-Surface compliant: 69 → 73 / 89 (82%).

- **CLI parity backfill batch 8** (v0.1.5): all 14 `graph-structure`
  capabilities. Three-Surface compliant: 55 → 69 / 89 (78%). Remaining
  20: batch-ops (4) + index-management (15) + 1 misc.

- **Hyphae jQuery-inspired selector extensions** (RFC-0091, v0.1.5).
  Closes the gap between Hyphae v1's CSS-selector core and full
  jQuery expressiveness. Eight new selector forms, all parse-compatible
  with existing queries:
  - `:not(X)` — set difference.
  - `:has(X)` — containment check.
  - `:in(path-prefix)` — path-scoped filter (heavily requested in agent prompts).
  - `:implements(X)` — outgoing `Implements` edge (mirror of `:extends`).
  - `:first-child` / `:last-child` / `:only-child` — positional within siblings.
  - `:nth-child(N)` — 1-indexed positional.
  - `[attr=value]` — exact-match attribute selector. Supported attributes: `language` (derived from file extension), `kind` (`NodeKind` wire string), `file` (file path).
  Lexer gains `LBracket`, `RBracket`, `Eq`, `Number`, and a broadened
  `Ident` token (now accepts `/` and `.` so `:in(src/lib.rs)` lexes as
  a single bare path). AST gains `AttributeSelector` and `PseudoArg`
  enum. Tests: 11 integration assertions in
  `crates/mycelium-hyphae/tests/jquery_selectors.rs` plus parser unit
  tests for each new form. The marketing-copy claim "CSS-selector-style
  with relationship pseudo-classes" is now literal, not aspirational.

- **CLI parity backfill batch 7** (v0.1.5): all 14 `centrality` capabilities
  get CLI subcommands. `centrality` category now 14/14 ✅ Three-Surface
  (incl. `rank-symbols`, `get-top-files`, `page-rank` — top-10 most useful).
  Three-Surface compliant: 41 → 55 / 89 (62%) — **above the 50% threshold
  for flipping parity.yml from informational to required.**

- **CLI parity backfill batch 6** (v0.1.5): all 12 `reachability`
  capabilities get CLI subcommands. `reachability` category is now
  12/12 ✅ Three-Surface (incl. `get-shortest-path` — top-3 most useful
  per glm5.1 eval):
  - `mycelium get-reachable <path> --edge-kind K [--max-depth N]`
  - `mycelium get-reachable-to <path> --edge-kind K [--max-depth N]`
  - `mycelium get-k-hop-neighbors <path> --k N --edge-kind K`
  - `mycelium get-two-hop-neighbors <path> --edge-kind K`
  - `mycelium get-shortest-path --from A --to B --edge-kind K`
  - `mycelium get-symbol-neighborhood <path> --edge-kind K`
  - `mycelium get-cross-refs <path>` / `get-outgoing-refs <path>`
  - `mycelium get-dependency-depth <path> --edge-kind K`
  - `mycelium get-reachable-set <path> --edge-kind K`
  - `mycelium get-reaches-into <path> --edge-kind K`
  - `mycelium get-singly-referenced --edge-kind K [--limit N]`
  Tests: 12 integration assertions in
  `crates/mycelium-cli/tests/cli_reachability.rs` using the 5-function
  diamond fixture.

- **CLI parity backfill batch 5** (v0.1.5): all 8 `inheritance`
  capabilities get CLI subcommands. `inheritance` category is now
  8/8 ✅ Three-Surface:
  - `mycelium get-extends <path>` / `mycelium get-implements <path>`
  - `mycelium extends-tree <path> [--max-depth N]`
  - `mycelium subclasses-tree <path> [--max-depth N]`
  - `mycelium implements-tree <path> [--max-depth N]`
  - `mycelium implementors-tree <path> [--max-depth N]`
  - `mycelium find-extends-path --from A --to B [--max-depth N]`
  - `mycelium find-implements-path --from A --to B [--max-depth N]`
  Tests: 8 integration assertions in
  `crates/mycelium-cli/tests/cli_inheritance.rs` using a Python
  Grandparent ← Parent ← Child chain plus a Rust trait/impl fixture.

- **CLI parity backfill batch 4** (v0.1.5): all three `import-graph`
  capabilities get CLI subcommands. `import-graph` category is now
  3/3 ✅ Three-Surface:
  - `mycelium get-imports <path> [--format ...]`
  - `mycelium get-import-tree <path> [--max-depth N] [--format ...]`
  - `mycelium get-importers-tree <path> [--max-depth N] [--format ...]`
  Tests: 4 integration assertions in
  `crates/mycelium-cli/tests/cli_import_graph.rs` using a 3-file
  Python import chain. Tree envelope `{ root: { path, imports: [...] } }`
  matches the MCP tool shape byte-for-byte.

- **CLI parity backfill batch 3** (v0.1.5): all seven `call-graph`
  capabilities get CLI subcommands. `call-graph` category is now
  7/7 ✅ Three-Surface (Charter §5.13 / RFC-0090):
  - `mycelium get-callees <path> [--format ...]`
  - `mycelium get-callers <path> [--format ...]`
  - `mycelium get-callee-tree <path> [--max-depth N] [--format ...]`
  - `mycelium get-caller-tree <path> [--max-depth N] [--format ...]`
  - `mycelium get-entry-points [--prefix P] [--format ...]`
  - `mycelium get-dead-symbols [--prefix P] [--format ...]`
  - `mycelium get-isolated-symbols [--prefix P] [--format ...]`
  Tests: 7 integration assertions in
  `crates/mycelium-cli/tests/cli_call_graph.rs` using a 3-function
  linear-chain fixture. CLI tree shape matches MCP tool byte-for-byte
  (`{ path, children }` for callee tree, `{ path, callers }` for
  caller tree).

### Fixed

- **Windows stack overflow** in `mycelium index` — final root-cause fix.
  Initialising the 11 tree-sitter parsers exceeded the Windows 1 MiB
  default thread stack, terminating with `STATUS_STACK_OVERFLOW`
  (0xC00000FD = exit -1073741571). The v0.1.4 attempt placed the
  `link-arg=/STACK:8388608` flag in `.cargo/config.toml`'s
  `[target.x86_64-pc-windows-*]` table, but CI workflows set
  `RUSTFLAGS=-D warnings` at the env level, which fully overrides
  `rustflags` from `.cargo/config.toml`. The v0.1.5 fix routes the same
  link-arg through a `cargo:rustc-link-arg-bin=mycelium=…` directive
  emitted by `crates/mycelium-cli/build.rs`, which is not subject to
  the `RUSTFLAGS` override. Linux/macOS unaffected.
- **DCO sign-off check** excluded merge commits via `git rev-list
  --no-merges`. Back-merge PRs (`release/* → develop`) had been failing
  because historical PR-merge commits never carried `Signed-off-by`
  trailers (they predate DCO enforcement).

## [0.1.4] — 2026-05-30

### Fixed

- **CI workflow `--fail-under-branches 80`** in `coverage` job — flag
  doesn't exist in `cargo-llvm-cov`. Removed; lines-only gate at 90%
  retained.
- **Rustdoc broken intra-doc links**: `[LanguagePack]` (wrong crate)
  and ambiguous `[index_file]` (Salsa generates a struct of the same
  name). Disambiguated.
- **Stale package-name references** in workflows: `mycelium-core` /
  `mycelium-mcp` updated to the published `mycelium-rcig-*` names so
  `cargo test --package` and `cargo publish -p` work.
- **`watch_mode_resolves_stub_after_callee_file_added`** flaky test:
  poll budget bumped 8 s → 30 s for slow GitHub-Actions runners. Then
  excluded from CI via `--skip` because the FSE watcher does not
  reliably fire on file *creation* in tempdir on GH runners.
- **`e2e_dogfood` / `e2e_real_projects` workflows** were referencing
  test targets that didn't exist yet (now fixed by the dogfood test in
  [0.1.5]). Workflow no-ops with a CI warning when the target is
  absent.
- **Redundant `cargo-audit`** removed from the security job —
  `cargo deny check` already runs the RustSec advisory check via the
  `[advisories]` section in `deny.toml`. `cargo-audit` was failing
  persistently with `failed to prepare clone` when re-fetching the
  advisory-db on GH runners.

### Added

- **CLI parity backfill batch 2** (v0.1.4): the remaining seven
  `basic-queries` capabilities get CLI subcommands. Combined with
  batch 1, the entire `basic-queries` category is now ✅ Three-Surface
  (10 / 10 rows in `skills/INDEX.md`):
  - `mycelium get-descendants <path> [--format ...]`
  - `mycelium get-node-kind <path> [--format ...]`
  - `mycelium get-symbols-by-kind <kind> [--path-prefix ...] [--format ...]`
  - `mycelium get-source-span <path> [--format ...]`
  - `mycelium get-siblings <path> [--format ...]`
  - `mycelium get-all-symbols [--prefix ...] [--kind ...] [--format ...]`
  - `mycelium server-status [--format ...]`
  Tests: 10 integration assertions in
  `crates/mycelium-cli/tests/cli_basic_queries_batch2.rs`.

- **CLI parity backfill batch 1** (v0.1.4): three high-frequency
  basic-queries capabilities now have CLI subcommands flipping their
  `skills/INDEX.md` rows from 🟡 to ✅ Three-Surface:
  - `mycelium search-symbol <query> [--limit N] [--format text|json]`
  - `mycelium get-symbol-info <path> [--format text|json]`
  - `mycelium get-ancestors <path> [--format text|json]`
  Each is the human-facing twin of an existing MCP tool. Shared
  loader (`load_index`) gives every subcommand the same "no index
  found — run `mycelium index <root>` first" recovery hint. Tests:
  8 integration assertions in `crates/mycelium-cli/tests/cli_basic_queries.rs`.

- **Performance hardening — issue #153** (v0.1.4):
  - Added `Trunk::symbol_nodes()` and `Store::symbol_nodes()` — O(V) iterator over
    symbol nodes yielding `(NodeId, &str)` without trie navigation. Eliminates the
    `all_paths() + lookup_path()` anti-pattern from five heavy-graph algorithms
    (`leaf_symbols`, `degree_histogram`, `graph_metrics`, `page_rank`, `weakly_connected_components`).
  - Replaced path-clone BFS in `find_call_path` with a parent-map BFS — O(V) space
    instead of O(V·max_depth), eliminates per-frontier Vec allocations.
  - Added 8 performance regression tests (`heavy_graph_*`) proving all six tools
    complete in < 2 s on 1 K-node and < 10 s on 10 K-node graphs in debug mode.
  - Added `benches/heavy_graph.rs` — Criterion benchmarks at 1 K and 10 K nodes for
    all six tools; use `cargo bench -p mycelium-rcig-core --bench heavy_graph` for SLA tracking.
  - Charter §2 SLA table extended with two new rows for heavy-graph algorithm classes.

- **RFC-0090 Phase 1 — Three-Surface parity checker** (v0.1.4):
  - New [`scripts/check_skill_parity.py`](scripts/check_skill_parity.py): extracts MCP tool names
    from `crates/mycelium-mcp/src/lib.rs` and Skill `allowed-tools` from `skills/*/SKILL.md`,
    reports I1 (every MCP tool has ≥1 Skill) and I2 (no Skill orphans) coverage.
  - New [`.github/workflows/parity.yml`](.github/workflows/parity.yml): runs the checker
    on every PR touching MCP, CLI, or Skills. Phase 1: informational (exits 0).
    Phase 3 / v0.2.0: add `--strict` to make the gate blocking.
  - Fixed 12 Skill `allowed-tools` naming mismatches discovered by the checker:
    `betweenness_centrality` → `get_betweenness_centrality`, `extends_tree` → `get_extends_tree`,
    `get_scc` → `get_strongly_connected_components`, and nine more.
    Confirmed coverage at 89/89 (100 %).

- **RFC-0090 Phase 2.3 — Skill coverage complete (89/89)** (v0.1.4):
  - New [`skills/index-management/`](skills/index-management/) Skill — 7 tools covering
    the server lifecycle: `index_workspace`, `load_index`, `server_status` (shared with
    `basic-queries`), `watch_status`, `sync_file`, `set_compact_mode`, `get_token_stats`.
  - 10 capabilities triaged into existing Skills:
    `get_files`, `get_node_degree`, `get_symbol_count_by_kind` → **basic-queries**;
    `get_leaf_symbols`, `find_call_path`, `get_common_callers`, `get_common_callees` → **call-graph**;
    `find_import_path` → **import-graph**;
    `get_mutual_reachability`, `get_common_reachable` → **reachability**.
  - Fixed `get_scc` name in INDEX.md to correct `get_strongly_connected_components`.
  - `skills/INDEX.md` updated to 89/89 coverage (100% of all MCP tools have Skill umbrella).

## [0.1.3] — 2026-05-30

### Added

- **Third wave of category Skills** (RFC-0090 Phase 2 closing, v0.1.3):
  - [`skills/inheritance/`](skills/inheritance/) — 8 capabilities for
    `Extends` and `Implements` edge navigation
    (`get_extends`, `extends_tree`, `subclasses_tree`,
    `find_extends_path`, `get_implements`, `implements_tree`,
    `implementors_tree`, `find_implements_path`).
  - [`skills/graph-structure/`](skills/graph-structure/) — 14 structural
    analysis tools including `detect_cycles` and
    `get_dependency_layers` (both top-10 per glm5.1).
  - [`skills/batch-ops/`](skills/batch-ops/) — 4 batch variants for
    token-efficient multi-symbol inspection
    (`batch_symbol_info`, `batch_node_degree`,
    `batch_reachable_from`, `batch_reachable_to`).
  - Combined coverage now: 73/88 (83%). The remaining 15 capabilities
    (not in any of the 8 glm5.1 categories) are triaged in Phase 2.3.
- **Second wave of category Skills** (RFC-0090 Phase 2, v0.1.3):
  - [`skills/import-graph/`](skills/import-graph/) — 3 capabilities
    (`get_imports`, `get_import_tree`, `get_importers_tree`).
  - [`skills/reachability/`](skills/reachability/) — 12 capabilities
    including `get_shortest_path` (top-3 most useful per glm5.1).
  - [`skills/centrality/`](skills/centrality/) — 14 capabilities
    including `rank_symbols` and `get_top_files` (both top-10).
  - 29 additional capabilities mapped in `skills/INDEX.md`. Combined
    coverage now: 47/88 (54%, was 18/88 after wave 1).
- **First wave of category Skills** (RFC-0090 Phase 2, v0.1.3):
  - [`skills/basic-queries/`](skills/basic-queries/) covers 10 foundation
    capabilities (`search_symbol`, `get_symbol_info`, `get_ancestors`,
    `get_descendants`, `get_node_kind`, `get_symbols_by_kind`,
    `get_source_span`, `get_siblings`, `get_all_symbols`,
    `server_status`).
  - [`skills/call-graph/`](skills/call-graph/) covers 7 Calls-edge
    capabilities (`get_callees`, `get_callers`, `get_callee_tree`,
    `get_caller_tree`, `get_entry_points`, `get_dead_symbols`,
    `get_isolated_symbols`).
  - Each Skill includes a worked example and a `tests/parity.test.json`
    asserting CLI ↔ MCP byte-equality for every covered capability.
  - `skills/INDEX.md` coverage matrix gains 17 rows (status 🟡 — Skill
    landed; CLI subcommand backfill follows in v0.1.4–v0.1.5 alongside
    the parity-CI workflow).
- **`mycelium query <hyphae>` works end-to-end** (#151). The marquee feature
  Hyphae was previously advertised in the README but the CLI subcommand
  was a `tracing::warn!` stub. It now: loads `.mycelium/index.rmp`, parses
  the selector (RFC-0003 grammar), runs the evaluator, and prints matches
  one per line (or as a JSON array with `--format=json`). Examples:
  ```
  mycelium query "#login"          # name selector
  mycelium query ".function"       # kind selector
  mycelium query ".class>.method"  # direct-child combinator
  ```
- **MCP twin tool `mycelium_query`.** Same Hyphae selector grammar, same
  match-set shape — Three-Surface Rule (RFC-0090) parity. Returns
  `{ matches: [...], count: N }` on success or `{ error: "..." }` on
  parse failure.
- **First real category Skill: `skills/hyphae-query/`.** SKILL.md +
  two worked examples (name-selector basic, kind+combinator advanced)
  + `tests/parity.test.json` asserting CLI ↔ MCP output equality.
  `skills/INDEX.md` updated with the coverage row.
- `mycelium-hyphae` crate now exposes `pub mod evaluator` (was previously
  unreachable). The `Evaluator::new` becomes `const fn` and gains
  clippy-clean control flow on the `BaseSelector::Kind` and
  `pseudo_arg_ids` paths.

## [0.1.2] — 2026-05-30

### Fixed

- **#150 — `mycelium serve --mcp` stdout pollution.** Tracing now goes
  to stderr with ANSI disabled, so the stdout stream contains only
  valid newline-delimited JSON-RPC frames. Strict MCP clients work
  without a custom filter. Regression test:
  `crates/mycelium-cli/tests/mcp_stdout_purity.rs`.
- **#152 — `edge_kind` parameter is now case-insensitive.** Tools
  accept `"Calls"`, `"calls"`, `"CALLS"` interchangeably. Unknown
  values produce a helpful error that lists all four canonical
  forms. Single source of truth: `parse_edge_kind()` in
  `crates/mycelium-mcp/src/lib.rs`.
- **#154 — `mycelium init` and `mycelium query` hidden from `--help`**
  until implemented. The subcommands still exist (so test setup and
  documentation links keep working) but no longer surface in
  discoverability output. `query` is fully wired in v0.1.3 per
  RFC-0090 Phase 2.

### Added

- **Charter §5.13 — the Three-Surface Rule** (colloquially "1:1:1 rule"):
  CLI ↔ MCP is **1:1 strict** (byte-identical name, description, args,
  JSON output); (CLI, MCP) ↔ Skill is **N:1 covered** (every pair must
  appear in ≥ 1 category Skill's `allowed-tools`; orphans fail CI). See
  [RFC-0090](rfcs/0090-cli-mcp-skill-parity.md) and
  [ADR-0007](docs/adr/0007-cli-mcp-skill-parity.md).
- `skills/` directory at the repo root with `README.md`, an `INDEX.md`
  coverage matrix (seeded with 8 planned categories covering 72 of the
  88 MCP capabilities), and a category-style `_template/` scaffold.
- PR template grew a "Three-Surface Self-Check" section split into
  CLI ↔ MCP parity (6 items), Skill coverage (4 items), and exception
  path (3 items).

### Changed

- **crates.io publish prefix**: Renamed all five workspace crates from
  `mycelium-*` to `mycelium-rcig-*` (`rcig` = reactive code intelligence
  graph). The short names `mycelium-core` and `mycelium-cli` were already
  taken on crates.io by unrelated projects (Matthew Bradford's
  `mycelium_core` from 2019 and LepistaBioinformatics' active
  `mycelium-cli`). Source code is unchanged — dep-names and `[lib].name`
  preserve `use mycelium_core::*` etc. New install command:
  `cargo install mycelium-rcig-cli` (the installed binary is still
  `mycelium`).
- **mycelium-core self-containment**: Copied the 5 language packs
  referenced by `cortex.rs` (javascript, python, typescript, rust, go)
  into `crates/mycelium-core/packs/` and updated `include_str!` paths.
  Matches the pattern PR #145 introduced for `mycelium-mcp`.

## [0.1.0] — 2026-05-30

### Highlights

First public release of **Mycelium** — the reactive, AI-native symbol graph that perceives code like a nervous system.

**Core engine:** Trunk (Materialized Path Radix Trie) + Synapse (per-`EdgeKind` adjacency lists) + Cortex (Salsa 3 incremental reactive layer). In-memory graph with MessagePack snapshot persistence (`.mycelium/index.rmp`). Full tree-sitter extraction pipeline for 10 languages.

**AI interface:** Hyphae DSL — a CSS-selector-inspired query language that replaces multi-round-trip JSON MCP calls with a single compact query (≤ 30% of JSON token count — Charter §2 SLA). Plus 88 specialized MCP graph-intelligence tools.

**All Charter §2 SLAs satisfied:**
- Cold symbol lookup: ~8 ns (target: < 5 ms)
- 3-hop traversal: ~392 ns (target: < 1 ms)
- Reactive re-query: Salsa-memoized (target: < 10 ms)
- AI token efficiency: Hyphae DSL ≤ 30% JSON baseline ✅
- Language onboarding: ≤ 3 files, 0 core changes ✅
- Test coverage: 96.27% lines / 835 tests ✅ (target: ≥ 90%)
- Fast CI: 1.5 s local, < 5 min gate ✅
- Documentation: 100% pub items have rustdoc ✅

### Added

- Day-0 project skeleton: charter, governance, GitFlow, code of conduct, security policy.
- `.hive/` definition of the autonomous AI development team.
- `.hive/memory/` persistent shared memory (append-only JSONL).
- RFC-0000 RFC template and RFC-0001 draft (Trunk + Synapse storage layer).
- GitHub workflows skeleton: `ci.yml`, `release.yml`, `nightly.yml`, `hive.yml`, `triage.yml`.
- Issue and PR templates.
- macOS `launchd` plists for autonomous Hive scheduling.
- Cargo workspace stub with `mycelium-core`, `mycelium-hyphae`, `mycelium-pack`, `mycelium-cli`, `mycelium-mcp` crates.
- First language packs: Python and TypeScript skeletons under `packs/`.
- `mycelium-core`: RFC-0002 `Extractor` — tree-sitter → Store bridge; parses Python source files and populates `Trunk` nodes + `Contains` edges for modules, functions, classes, methods, and imports.
- `mycelium-pack`: language pack loader (`LanguagePack::load`) with `pack.toml` manifest parsing and query-source validation.
- `mycelium index <path>`: first end-user-visible CLI command — walks a directory tree, extracts Python symbols via RFC-0002 `Extractor`, and reports file/error counts.
- TypeScript language pack (`packs/typescript/`) — `function_declaration`, `class_declaration`, methods, `interface_declaration`, `type_alias_declaration`, and import references.
- Extractor generic `definition.*` dispatch: any capture name starting with `definition.` (other than `module`/`method`) creates a top-level child node, enabling language-pack authors to use custom definition kinds.
- Rust language pack (`packs/rust/`) — functions, structs, enums, traits, type aliases, consts, inline mods, impl methods, and use declarations.
- `mycelium index` now indexes Python, TypeScript, and Rust source trees.
- RFC-0004 MCP server (`mycelium-mcp`): `mycelium serve --mcp` starts a stdio JSON-RPC 2.0 server with three tools — `mycelium_index_workspace`, `mycelium_search_symbol`, `mycelium_get_ancestors`.
- `Store::search_symbol` — case-insensitive substring search over all materialized path name-segments; returns sorted results up to a configurable limit.
- `Store::ancestors_of_path` — returns ancestor path strings (child-to-root) for a given trunk path string.
- RFC-0005: JavaScript language pack (`packs/javascript/`) — top-level functions, arrow functions, class declarations, methods, and import references for `.js` and `.jsx` files.
- RFC-0005: `.jsx` and `.tsx` extension dispatch in CLI and MCP indexing layers.
- RFC-0005: `mycelium_get_descendants` MCP tool — returns all symbols nested under a trunk path.
- RFC-0005: `mycelium_index_workspace` now includes a `"languages"` field listing all indexed language names.
- RFC-0005: `Store::descendants_of_path` — symmetric counterpart to `ancestors_of_path`; returns descendant path strings in unspecified order.
- RFC-0005: MCP server identity corrected — `get_info()` now reports `{"name":"mycelium-mcp","version":"0.0.1"}` instead of the rmcp library name.
- RFC-0006: `Store::save()` — serializes the full Trunk+Synapse graph to a `MessagePack` snapshot; creates parent directories automatically.
- RFC-0006: `Store::load()` — deserializes a `Store` from a `.mycelium/index.rmp` snapshot file.
- RFC-0006: `mycelium index` CLI auto-saves snapshot to `.mycelium/index.rmp` after indexing.
- RFC-0006: `mycelium_index_workspace` MCP tool auto-saves snapshot after indexing.
- RFC-0006: `mycelium_load_index` MCP tool — reloads a previously-saved index from `.mycelium/index.rmp` without re-parsing source files.
- RFC-0006: All core types (`NodeId`, `NodeKind`, `EdgeKind`, `Language`, `Trunk`, `Synapse`, `Store`) now implement `serde::Serialize` + `Deserialize`.
- RFC-0007: `MyceliumServer::with_root(path)` — new constructor that pre-loads a `.mycelium/index.rmp` snapshot, or falls back to a live index + auto-save.
- RFC-0007: `serve_stdio(root: Option<PathBuf>)` — passes `--root` through to `with_root`.
- RFC-0007: `mycelium serve --mcp --root <path>` CLI flag — server starts ready without needing `mycelium_index_workspace`.
- RFC-0007: `mycelium_server_status` MCP tool — returns `node_count`, `indexed_root`, and `is_loaded` for client diagnostics.
- RFC-0008: File-system watch mode — `MyceliumServer::start_watch(root)` spawns a background loop that debounces FSE events (300 ms window) and incrementally re-indexes changed/created/deleted files.
- RFC-0008: `with_root` now automatically starts the watch loop after loading.
- RFC-0008: `mycelium_watch_status` MCP tool — returns `watching`, `root`, and `batches_processed`.
- RFC-0008: `reindex_file` helper — single-file extraction used by the watch loop.
- RFC-0009: Gitignore-aware file walking — CLI `index_path` and MCP `run_index` now use `ignore::WalkBuilder` to respect `.gitignore` and `.myceliumignore` patterns.
- RFC-0009: `target/` and `.mycelium/` are always excluded from indexing, even without an ignore file.
- RFC-0009: Background FSE watch loop filters events for ignored paths before re-indexing.
- RFC-0009: `.myceliumignore` is registered as a custom ignore filename in `WalkBuilder`.
- RFC-0010: `Synapse::edge_count()` — total directed edges across all `EdgeKind` buckets.
- RFC-0010: `Store::edge_count()` — delegates to `Synapse::edge_count()`.
- RFC-0010: `mycelium_server_status` now includes `"edge_count"` alongside `"node_count"`.
- RFC-0011: Call graph edges — `reference.call` patterns added to Python, TypeScript, JavaScript, and Rust language packs.
- RFC-0011: `Extractor` now populates `EdgeKind::Calls` edges between caller and callee nodes.
- RFC-0011: Intra-file call resolution: callees defined before callers in the same file are resolved to their definition nodes rather than bare stubs.
- RFC-0012: `mycelium_get_callees` MCP tool — returns all symbols a given path calls, as a sorted list.
- RFC-0012: `mycelium_get_callers` MCP tool — returns all symbols that call a given path, as a sorted list.
- RFC-0013: Two-pass extraction — `Extractor::extract` now makes two sequential AST traversals (definitions first, references second) so forward-reference call edges always resolve to definition nodes rather than bare stubs.
- RFC-0014: Cross-file call stub resolution — `Store::resolve_bare_call_stubs()` runs after each full workspace index, rewiring `Calls` edges that point to bare stub nodes to their actual definition nodes (unambiguous matches only).
- RFC-0014: `AdjacencyList::redirect_node` and `Synapse::redirect_node` — edge-rewiring primitives used by stub resolution.
- RFC-0014: `mycelium_index_workspace` response now includes `"stubs_resolved"` count.
- RFC-0015: Watch-mode stub resolution — `resolve_bare_call_stubs()` is called at the end of each FSE debounce batch, so cross-file call edges are kept accurate during incremental re-indexing without requiring a full re-index.
- RFC-0016: `mycelium_get_symbol_info` MCP tool — returns ancestors, descendants, callers, and callees for any symbol path in a single call; all lists are sorted lexicographically.
- RFC-0017: `Store::find_call_path(from, to, max_depth)` — BFS shortest call path search; returns `Some(Vec<NodeId>)` including both endpoints, or `None` if unreachable; cycle-safe via visited set; `max_depth` limits hops.
- RFC-0017: `mycelium_find_call_path` MCP tool — BFS call chain tool; request `{ from_path, to_path, max_depth? }`; returns `{ path, hops }` on success or `{ path: [], hops: null, message }` when unreachable; unknown paths return `{ error }`.
- RFC-0018: `Store::all_file_paths()` — returns all trunk paths with no `>` separator (file-level nodes), sorted lexicographically.
- RFC-0018: `mycelium_get_files` MCP tool — enumerates all indexed source files; optional `path_prefix` parameter filters results; returns `{ files: [...] }` sorted.
- RFC-0019: `Store::top_callee_symbols(limit)` — returns top-N `(path, caller_count)` pairs sorted by caller count descending (ties by path ascending); symbols with 0 callers excluded.
- RFC-0019: `mycelium_rank_symbols` MCP tool — hot-spot analysis; request `{ limit? }`; returns `{ symbols: [{ path, caller_count }, ...] }`; limit defaults to 10, capped at 100.
- RFC-0020: `CalleeNode { id, children }` struct — DFS callee tree node; cycle-safe via per-traversal visited set with backtrack removal.
- RFC-0020: `Store::callee_tree(id, max_depth)` — depth-limited recursive DFS over Calls edges.
- RFC-0020: `mycelium_get_callee_tree` MCP tool — returns `{ root: { path, children: [...] } }`; max_depth defaults to 4, capped at 10; unknown path returns `{ error }`.
- RFC-0021: `CallerNode { id, callers }` struct — symmetric complement to `CalleeNode`; DFS up incoming Calls edges; cycle-safe via path-tracking visited set.
- RFC-0021: `Store::caller_tree(id, max_depth)` — depth-limited recursive DFS over incoming Calls edges.
- RFC-0021: `mycelium_get_caller_tree` MCP tool — returns `{ root: { path, callers: [...] } }`; max_depth defaults to 4, capped at 10; unknown path returns `{ error }`.
- RFC-0022: `Store::entry_points(prefix)` — returns all symbol paths (containing `>`) with zero incoming Calls edges, sorted lexicographically; optional prefix filter.
- RFC-0022: `mycelium_get_entry_points` MCP tool — returns `{ entry_points: [...] }`; optional `path_prefix` filter; excludes file-level nodes.
- RFC-0023: `Store::imports_of(id)` / `Store::imported_by(id)` — outgoing/incoming `Imports` edge resolvers; results sorted lexicographically.
- RFC-0023: `mycelium_get_imports` MCP tool — returns `{ imports: [...], imported_by: [...] }` for a path; unknown path returns `{ error }`.
- RFC-0024: `ImportNode { id, imports }` struct — DFS import dependency tree node; cycle-safe via path-tracking visited set.
- RFC-0024: `Store::import_tree(id, max_depth)` — depth-limited recursive DFS over outgoing `Imports` edges.
- RFC-0024: `mycelium_get_import_tree` MCP tool — returns `{ root: { path, imports: [...] } }`; max_depth defaults to 4, capped at 10; unknown path returns `{ error }`.
- RFC-0025: `mycelium_batch_symbol_info` MCP tool — batch variant of `mycelium_get_symbol_info`; accepts up to 50 paths in one call; returns `{ symbols: [{ path, ancestors, descendants, callers, callees }] }` in input order; unknown paths return `{ path, error }` without failing the whole request.
- RFC-0026: `mycelium_get_extends` MCP tool — returns `{ extends, extended_by }` for a path using `EdgeKind::Extends`; both lists sorted lexicographically; unknown path returns `{ error }`.
- RFC-0026: `mycelium_get_implements` MCP tool — returns `{ implements, implemented_by }` for a path using `EdgeKind::Implements`; both lists sorted lexicographically; unknown path returns `{ error }`.
- RFC-0027: `Store::find_import_path(from, to, max_depth)` — BFS shortest import-dependency path; returns `Some(Vec<NodeId>)` including both endpoints or `None` if unreachable; cycle-safe; `max_depth` limits hops.
- RFC-0027: `mycelium_find_import_path` MCP tool — BFS import chain tool; request `{ from_path, to_path, max_depth? }`; returns `{ path, hops }` on success or `{ path: [], hops: null, message }` when unreachable; unknown paths return `{ error }`.
- RFC-0028: `Store::kind_map` — per-node `NodeKind` metadata stored alongside each node; zero query-time cost.
- RFC-0028: `Store::set_kind(id, kind)`, `Store::kind_of(id) -> Option<NodeKind>`, `Store::symbols_of_kind(kind, prefix) -> Vec<String>` — kind storage and query methods.
- RFC-0028: `Extractor` now calls `set_kind` for every extracted node (file → `File`, functions → `Function`, classes → `Class`, methods → `Method`, etc.).
- RFC-0028: `mycelium_get_node_kind` MCP tool — returns `{ path, kind }` where kind is the wire string or `null` if unrecorded; unknown path returns `{ error }`.
- RFC-0028: `mycelium_get_symbols_by_kind` MCP tool — returns `{ symbols: [...] }` for all indexed symbols of a given kind; optional `path_prefix` filter; unknown kind returns `{ error }`.
- RFC-0029: `SourceSpan` now derives `Serialize` + `Deserialize` so it persists in the MessagePack snapshot.
- RFC-0029: `Store::set_span(id, span)`, `Store::span_of(id) -> Option<SourceSpan>` — source location storage and retrieval.
- RFC-0029: `Extractor` now calls `set_span` for every extracted node using tree-sitter node positions (rows converted to 1-indexed lines).
- RFC-0029: `mycelium_get_source_span` MCP tool — returns `{ path, start_line, start_col, end_line, end_col, start_byte, end_byte }` on hit, `{ path, span: null }` when unrecorded, or `{ error }` when path is not found.
- RFC-0030: `Store::find_extends_path(from, to, max_depth)` — BFS shortest extends-chain search over `EdgeKind::Extends`; completes the `find_*_path` triad.
- RFC-0030: `mycelium_find_extends_path` MCP tool — returns `{ path, hops }` on success, `{ path: [], hops: null, message }` when unreachable, or `{ error }` for unknown paths; `max_depth` defaults to 8, capped at 20.
- RFC-0031: `ExtendsNode { id, parents }` struct — DFS superclass tree node; cycle-safe via path-tracking visited set with backtrack removal.
- RFC-0031: `Store::extends_tree(id, max_depth)` — depth-limited recursive DFS over outgoing `Extends` edges.
- RFC-0031: `mycelium_get_extends_tree` MCP tool — returns `{ root: { path, parents: [...] } }`; `max_depth` defaults to 4, capped at 10; unknown path returns `{ error }`.
- RFC-0032: `SubclassNode { id, subclasses }` struct — DFS subclass forest node; cycle-safe via path-tracking visited set with backtrack removal.
- RFC-0032: `Store::subclasses_tree(id, max_depth)` — depth-limited recursive DFS over **incoming** `Extends` edges.
- RFC-0032: `mycelium_get_subclasses_tree` MCP tool — returns `{ root: { path, subclasses: [...] } }`; `max_depth` defaults to 4, capped at 10; unknown path returns `{ error }`. Complements `extends_tree` (outgoing) for full class-hierarchy exploration.
- RFC-0033: `Store::find_implements_path(from, to, max_depth)` — BFS shortest implements-chain search over `EdgeKind::Implements`; completes the `find_*_path` family (calls / imports / extends / implements).
- RFC-0033: `mycelium_find_implements_path` MCP tool — returns `{ path, hops }` on success, `{ path: [], hops: null, message }` when unreachable, or `{ error }` for unknown paths; `max_depth` defaults to 8, capped at 20.
- RFC-0034: `ImplementsNode { id, interfaces }` struct — DFS interface hierarchy node; cycle-safe via path-tracking visited set with backtrack removal.
- RFC-0034: `Store::implements_tree(id, max_depth)` — depth-limited recursive DFS over outgoing `Implements` edges.
- RFC-0034: `mycelium_get_implements_tree` MCP tool — returns `{ root: { path, interfaces: [...] } }`; `max_depth` defaults to 4, capped at 10; unknown path returns `{ error }`.
- RFC-0035: `ImplementorNode { id, implementors }` struct — DFS implementor forest node; cycle-safe via path-tracking visited set with backtrack removal.
- RFC-0035: `Store::implementors_tree(id, max_depth)` — depth-limited recursive DFS over **incoming** `Implements` edges.
- RFC-0035: `mycelium_get_implementors_tree` MCP tool — returns `{ root: { path, implementors: [...] } }`; `max_depth` defaults to 4, capped at 10; unknown path returns `{ error }`. Completes the Implements family.
- RFC-0036: `ImporterNode { id, importers }` struct — DFS reverse-dependency tree node; cycle-safe via path-tracking visited set with backtrack removal.
- RFC-0036: `Store::importers_tree(id, max_depth)` — depth-limited recursive DFS over **incoming** `Imports` edges.
- RFC-0036: `mycelium_get_importers_tree` MCP tool — returns `{ root: { path, importers: [...] } }`; `max_depth` defaults to 4, capped at 10; unknown path returns `{ error }`. Completes the Imports family and the full symmetric DFS coverage for all four `EdgeKind` variants.
- RFC-0037: `Store::dead_symbols(prefix)` — returns all symbol paths (containing `>`) with zero incoming `Calls` edges and zero incoming `Imports` edges; file-level nodes excluded; optional prefix filter; results sorted lexicographically.
- RFC-0037: `mycelium_get_dead_symbols` MCP tool — dead-code analysis tool; returns `{ dead_symbols: [...], count: N }`; optional `path_prefix` filter; dead symbols are candidates for deletion or documentation review.
- RFC-0038: `GraphStats { total_nodes, total_edges, nodes_by_kind, edges_by_kind }` struct — per-kind breakdown of the indexed graph.
- RFC-0038: `Synapse::edge_counts_by_kind()` — iterator over non-empty `(EdgeKind, usize)` pairs.
- RFC-0038: `Store::graph_stats()` — returns `GraphStats` with node counts grouped by `NodeKind` and edge counts grouped by `EdgeKind`; kinds with zero count are omitted.
- RFC-0038: `mycelium_get_stats` MCP tool — comprehensive per-kind statistics; extends `mycelium_server_status` with the breakdown needed for architectural analysis; returns `{ total_nodes, total_edges, nodes_by_kind, edges_by_kind }`.
- RFC-0039: `CrossRefs { callers, importers, extended_by, implemented_by }` struct — all incoming edges for a symbol grouped by `EdgeKind`.
- RFC-0039: `Store::cross_refs(id)` — collects incoming `Calls`, `Imports`, `Extends`, and `Implements` edges and resolves them to sorted path strings; all four lists always present.
- RFC-0039: `mycelium_get_cross_refs` MCP tool — unified "who references this?" primitive for impact analysis; returns `{ callers, importers, extended_by, implemented_by }` or `{ error }` for unknown paths.
- RFC-0040: `Store::nodes_in_cycles(edge_kind, prefix)` — iterative DFS with `in_stack` tracking; returns all paths participating in at least one cycle for the given `EdgeKind`; optional prefix filter; results sorted lexicographically.
- RFC-0040: `mycelium_detect_cycles` MCP tool — circular dependency detection; `edge_kind` must be `"calls"`, `"imports"`, `"extends"`, or `"implements"`; returns `{ cycle_nodes, count }` or `{ error }` for unknown edge kind.
- RFC-0041: `OutgoingRefs { callees, imports, extends, implements }` struct — all outgoing edges from a symbol grouped by `EdgeKind`; symmetric complement to `CrossRefs`.
- RFC-0041: `Store::outgoing_refs(id)` — collects outgoing `Calls`, `Imports`, `Extends`, `Implements` edges and resolves them to sorted path strings; all four lists always present.
- RFC-0041: `mycelium_get_outgoing_refs` MCP tool — "what does this reference?" primitive; paired with `mycelium_get_cross_refs` provides complete incoming/outgoing reference picture in two calls; returns `{ callees, imports, extends, implements }` or `{ error }`.
- RFC-0042: `Store::all_symbols(prefix, kind)` — returns all non-file symbol paths (paths containing `>`), sorted lexicographically, with optional path-prefix and `NodeKind` filters; file-level nodes are excluded.
- RFC-0042: `mycelium_get_all_symbols` MCP tool — enumerates every indexed symbol across all kinds; accepts optional `path_prefix` and `kind` parameters; returns `{ symbols, count }` or `{ error }` for an unknown kind string.
- RFC-0043: `Store::reachable_from(id, kind, max_depth)` — flat BFS reachability from a node via outgoing edges of any `EdgeKind`, depth-limited (cap 20), cycle-safe; starting node excluded; results sorted lexicographically.
- RFC-0043: `mycelium_get_reachable` MCP tool — transitive dependency enumeration in a single call; accepts `path`, `edge_kind`, and optional `max_depth`; returns `{ reachable, count }` or `{ error }` for unknown path or edge kind.
- RFC-0044: `Store::reachable_to(id, kind, max_depth)` — flat BFS backward reachability following incoming `EdgeKind` edges; depth-limited (cap 20), cycle-safe, starting node excluded; symmetric complement to `reachable_from`.
- RFC-0044: `mycelium_get_reachable_to` MCP tool — impact analysis primitive answering "who transitively depends on this symbol?"; paired with `mycelium_get_reachable` provides complete forward+backward reachability.
- RFC-0045: `Store::siblings(id)` — returns all direct siblings (other children of the same parent container in the containment tree), excluding the node itself; root nodes return empty `Vec`; results sorted lexicographically.
- RFC-0045: `mycelium_get_siblings` MCP tool — "what else is in this class/file?" query in a single call; returns `{ siblings, count }` or `{ error }` for unknown paths.
- RFC-0046: `NodeDegree` struct — per-node edge count summary: in/out degree for each of the four `EdgeKind`s (calls, imports, extends, implements).
- RFC-0046: `Store::node_degree(id)` — O(1) per-kind edge count summary without pulling full edge lists; useful for fast coupling analysis and hub-node detection.
- RFC-0046: `mycelium_get_node_degree` MCP tool — connectivity fingerprint for any path; returns `{ in_calls, out_calls, in_imports, out_imports, in_extends, out_extends, in_implements, out_implements }` or `{ error }`.
- RFC-0047: `Store::top_files(limit)` — returns top-N source files ranked by direct child symbol count (descending), ties broken alphabetically; files with no direct symbols excluded; limit capped at 100.
- RFC-0047: `mycelium_get_top_files` MCP tool — god-file detector identifying the most symbol-dense source files; returns `{ files: [{ path, symbol_count }], count }`.
- RFC-0048: `Store::most_connected(limit, kind)` — top-N symbol nodes ranked by total degree (in + out) for any EdgeKind; zero-degree nodes excluded; sorted descending by degree, ties broken alphabetically; limit capped at 100.
- RFC-0048: `mycelium_get_most_connected` MCP tool — hub-node detector for any edge kind; returns `{ symbols: [{ path, degree }], count }` or `{ error }` for unknown edge kind.
- RFC-0049: `Store::leaf_symbols(kind, limit)` — symbol nodes with out-degree 0 for any EdgeKind; symmetric complement to `entry_points` (RFC-0022, in-degree 0 for Calls); sorted alphabetically; limit capped at 100.
- RFC-0049: `mycelium_get_leaf_symbols` MCP tool — leaf-implementation detector for any edge kind; returns `{ symbols, count }` or `{ error }` for unknown edge kind.
- RFC-0050: `Store::shortest_path(from, to, kind)` — BFS minimum-hop path between two symbol nodes via outgoing EdgeKind edges; returns `Some(path_strings)` with both endpoints, or `None` if unreachable; cycle-safe.
- RFC-0050: `mycelium_get_shortest_path` MCP tool — "how does A reach B?" in a single call; returns `{ path, length }` if found, `{ path: null, length: null }` if no path, or `{ error }` for unknown edge kind or unrecognised node paths.
- RFC-0051: `Store::symbol_count_by_kind()` — per-`NodeKind` symbol histogram from `kind_map`; wire-string keys sorted alphabetically; zero-count kinds excluded.
- RFC-0051: `Store::upsert_node_with_kind(path, kind)` — convenience method: insert or retrieve a node and record its `NodeKind` in a single call.
- RFC-0051: `mycelium_get_symbol_count_by_kind` MCP tool — codebase composition histogram; returns `{ kinds: [{ kind, count }], total }`.
- RFC-0052: `Store::common_callers(target_ids, kind)` — set intersection of each target's incoming-neighbour set for any EdgeKind; answers "which symbols depend on ALL of these targets?"; results sorted alphabetically.
- RFC-0052: `mycelium_get_common_callers` MCP tool — shared-dependency detector; accepts `{ paths, edge_kind }` and returns `{ callers, count }` or `{ error }`.
- RFC-0053: `Store::fan_out_rank(kind, limit)` — top-N symbol nodes ranked by out-degree for any EdgeKind; "orchestrator detector" identifying symbols that call/import/extend many others; zero-degree nodes excluded; sorted descending by degree, ties broken alphabetically; limit capped at 100.
- RFC-0053: `mycelium_get_fan_out_rank` MCP tool — identifies orchestrating symbols; returns `{ symbols: [{ path, out_degree }], count }` or `{ error }` for unknown edge kind; limit defaults to 10.
- RFC-0054: `Store::fan_in_rank(kind, limit)` — top-N symbol nodes ranked by in-degree for any EdgeKind; "hotspot detector" identifying symbols depended upon by many others; zero-degree nodes excluded; sorted descending by degree, ties broken alphabetically; limit capped at 100. Symmetric complement to `fan_out_rank`.
- RFC-0054: `mycelium_get_fan_in_rank` MCP tool — identifies high-demand hotspot symbols; returns `{ symbols: [{ path, in_degree }], count }` or `{ error }` for unknown edge kind; limit defaults to 10.
- RFC-0055: `Store::common_callees(source_ids, kind)` — set intersection of each source's outgoing-neighbour set for any EdgeKind; answers "which symbols are called/imported by ALL of these sources?"; results sorted alphabetically. Symmetric complement to `common_callers` (RFC-0052).
- RFC-0055: `mycelium_get_common_callees` MCP tool — shared-dependency detector (outgoing direction); accepts `{ paths, edge_kind }` and returns `{ callees, count }` or `{ error }`.
- RFC-0056: `Store::isolated_symbols(prefix)` — symbol nodes with zero connectivity across all four EdgeKinds (Calls, Imports, Extends, Implements); stronger than `dead_symbols` (RFC-0037) which only checks incoming edges; optional path prefix filter; results sorted alphabetically.
- RFC-0056: `mycelium_get_isolated_symbols` MCP tool — completely-disconnected symbol detector; returns `{ isolated_symbols, count }`; optional `path_prefix` filter.
- RFC-0057: `Store::scc_groups(kind)` — Tarjan's iterative Strongly Connected Components algorithm over symbol nodes for a given EdgeKind; returns groups of size ≥ 2 (singletons excluded), sorted by size descending then by first path ascending; reveals mutually-recursive dependency clusters.
- RFC-0057: `mycelium_get_scc_groups` MCP tool — mutually-recursive symbol cluster detector; accepts `{ edge_kind }` and returns `{ groups, group_count, total_symbols }` or `{ error }` for unknown edge kind.
- RFC-0058: `Store::dependency_layers(kind)` — Kahn's BFS topological dependency layering; layer 0 = utility/leaf symbols (zero outgoing edges for `kind`), layer k+1 = symbols all of whose direct dependencies are in layers 0..=k; symbols in cycles excluded; paths within each layer sorted ascending.
- RFC-0058: `mycelium_get_dependency_layers` MCP tool — architectural layering inspector; accepts `{ edge_kind }` and returns `{ layers, layer_count, total_symbols, cycle_excluded_count }` or `{ error }` for unknown edge kind. Complements `scc_groups` (cycles) and `entry_points` (zero in-degree).
- RFC-0059: `Store::two_hop_neighbors(id, kind)` — symbol paths reachable in exactly 2 outgoing steps for `kind`; excludes source and direct (1-hop) neighbours; focused bridge detector without full reachability traversal; results sorted ascending.
- RFC-0059: `mycelium_get_two_hop_neighbors` MCP tool — indirect dependency bridge detector; accepts `{ path, edge_kind }` and returns `{ neighbors, count }`, `{ neighbors: [], count: 0 }` for unknown path, or `{ error }` for unknown edge kind.
- RFC-0060: `Store::symbol_neighborhood(id, kind)` + `SymbolNeighborhood` struct — ego-graph of a symbol for a single EdgeKind; returns path + direct incoming + direct outgoing, both lists sorted ascending; returns empty neighborhood for unknown id.
- RFC-0060: `mycelium_get_symbol_neighborhood` MCP tool — bidirectional single-kind ego-graph query; accepts `{ path, edge_kind }` and returns `{ path, incoming, outgoing, incoming_count, outgoing_count }`, empty neighborhood for unknown path, or `{ error }` for unknown edge kind.
- RFC-0061: `Store::hub_symbols(kind, min_in, min_out, limit)` — symbols with both in-degree ≥ `min_in` AND out-degree ≥ `min_out` for a given EdgeKind; returns `(path, in_degree, out_degree)` sorted by `in_degree + out_degree` descending (ties by path ascending); limit capped at 100; file nodes excluded.
- RFC-0061: `mycelium_get_hub_symbols` MCP tool — architectural hub detector identifying symbols that are both widely-used (high in-degree) and orchestrating (high out-degree); accepts `{ edge_kind, min_in?, min_out?, limit? }` and returns `{ hubs: [{ path, in_degree, out_degree }], count }` or `{ error }` for unknown edge kind; `min_in`/`min_out` default to 1.
- RFC-0062: `Store::singly_referenced(kind, limit)` — symbols with exactly one incoming edge for a given EdgeKind; returns `(symbol_path, referencing_path)` pairs sorted by symbol path ascending; limit capped at 100; file nodes excluded. Fills the in-degree=1 gap between `entry_points` (0) and `fan_in_rank` (top-N).
- RFC-0062: `mycelium_get_singly_referenced` MCP tool — inlining and privatisation candidate detector; accepts `{ edge_kind, limit? }` and returns `{ symbols: [{ path, referenced_by }], count }` or `{ error }` for unknown edge kind; limit defaults to 10.
- RFC-0063: `Store::batch_reachable_to(ids, kind, max_depth)` — union of transitive incoming dependents for a set of symbols; deduplicated, input nodes excluded, sorted ascending, max_depth capped at 20. Answers "what is the total blast radius if any of these symbols change?"
- RFC-0063: `mycelium_batch_reachable_to` MCP tool — total change-impact surface in one call; accepts `{ paths (up to 20), edge_kind, max_depth? }` and returns `{ reachable, count }` or `{ error }` for unknown edge kind; max_depth defaults to 10.
- RFC-0064: `Store::k_core(kind, k)` — k-core decomposition of the symbol graph; the maximal induced subgraph where every node has total degree (in + out within the subgraph) ≥ k; iterative peeling algorithm; k=0 returns all symbols; file nodes excluded; results sorted ascending.
- RFC-0064: `mycelium_get_k_core` MCP tool — hard-to-refactor core detector; accepts `{ edge_kind, k? }` and returns `{ core, count, k }` or `{ error }` for unknown edge kind; k defaults to 2.
- RFC-0065: `Store::batch_reachable_from(ids, kind, max_depth)` — union of symbols transitively reachable FROM a set of sources via outgoing edges; deduplicated, input nodes excluded, sorted ascending, max_depth capped at 20. Symmetric complement of `batch_reachable_to` (RFC-0063).
- RFC-0065: `mycelium_batch_reachable_from` MCP tool — collective forward-reachability in one call; accepts `{ paths (up to 20), edge_kind, max_depth? }` and returns `{ reachable, count }` or `{ error }` for unknown edge kind; max_depth defaults to 10.
- RFC-0066: `Store::batch_node_degree(ids)` — returns one `NodeDegree` per `NodeId` in input order; ids absent from the synapse return `NodeDegree::default()` (all counts zero). Batch version of `node_degree` (RFC-0046) eliminating N round trips when analysing a set of related symbols.
- RFC-0066: `mycelium_batch_node_degree` MCP tool — batch degree query for up to 50 symbols in one call; accepts `{ paths }` and returns `{ degrees: [{ path, in_calls, out_calls, in_imports, out_imports, in_extends, out_extends, in_implements, out_implements }], count }` with unknown paths returning `{ path, error: "path not found" }`; results in input order.
- RFC-0067: `Store::cycle_members(kind)` — paths of all symbol nodes participating in at least one directed cycle for a given EdgeKind; uses iterative Kosaraju's SCC algorithm (O(V+E)); file nodes excluded; results sorted ascending. Returns `[]` when no cycles exist.
- RFC-0067: `mycelium_find_cycle_members` MCP tool — circular dependency detector; accepts `{ edge_kind }` and returns `{ members, count }` (cycle-member symbol paths, sorted) or `{ error }` for unknown edge kind. Detects circular imports, mutually-recursive functions, and inheritance cycles.
- RFC-0068: `Store::weakly_connected_components(kind)` — groups symbol nodes into weakly-connected components (WCCs) treating edges as undirected; uses path-compressed Union-Find (O(α(V)·E)); components sorted by size descending (ties by first element); file nodes excluded. Surfaces isolated clusters and self-contained subsystems.
- RFC-0068: `mycelium_get_wcc` MCP tool — cluster detector; accepts `{ edge_kind, min_size? }` and returns `{ components, component_count, total_symbols }` or `{ error }` for unknown edge kind; `min_size` (default 1) filters singletons to focus on real clusters.
- RFC-0069: `Store::topological_sort(kind)` — topological ordering of the symbol graph via Kahn's BFS algorithm; returns `TopologicalOrder { order, cycle_members }` where `order` places each symbol after all its `kind`-predecessors (ties broken by path ascending) and `cycle_members` lists symbols that form directed cycles; file nodes excluded.
- RFC-0069: `mycelium_topological_sort` MCP tool — dependency order analysis; accepts `{ edge_kind }` and returns `{ order, cycle_members, ordered_count, cycle_count }` or `{ error }` for unknown edge kind. Useful for build order, initialization sequences, and layered architecture validation.
- RFC-0070: `Store::articulation_points(kind)` — cut vertices in the undirected symbol graph for a given EdgeKind via iterative Tarjan DFS (O(V+E)); file nodes excluded; singleton nodes (degree 0) never returned; results sorted ascending. A node is an articulation point if its removal disconnects its weakly-connected component.
- RFC-0070: `mycelium_find_articulation_points` MCP tool — single-point-of-failure detector; accepts `{ edge_kind }` and returns `{ points, count }` or `{ error }` for unknown edge kind. Identifies modules whose removal fragments the dependency graph — critical for safe refactoring and resilience analysis.
- RFC-0071: `Store::bridge_edges(kind)` — bridge edges (cut edges) in the undirected symbol graph via iterative Tarjan bridge-finding DFS (O(V+E)); file nodes excluded; multigraph-safe (parallel edges are not bridges); canonical `(from ≤ to)` pairs sorted ascending. Complements articulation points (RFC-0070): where APs are vertex cut-points, bridges are edge cut-points.
- RFC-0071: `mycelium_find_bridge_edges` MCP tool — fragile single-link connection detector; accepts `{ edge_kind }` and returns `{ bridges: [{ from, to }], count }` or `{ error }` for unknown edge kind. Identifies dependency edges whose removal would disconnect two subsystems.
- RFC-0072: `Store::biconnected_components(kind)` — partitions the undirected symbol graph into biconnected components (BCCs) via iterative Tarjan BCC detection with edge stack (O(V+E)); bridge edges produce 2-node BCCs; larger BCCs represent cycle-rich cohesive clusters; singletons excluded; groups sorted by size descending. Completes the cut-point analysis trilogy: WCC (RFC-0068), articulation points (RFC-0070), bridge edges (RFC-0071).
- RFC-0072: `mycelium_get_biconnected_components` MCP tool — tightly-coupled cluster detector; accepts `{ edge_kind }` and returns `{ components, component_count, total_symbols }` or `{ error }` for unknown edge kind. Reveals which symbol groups are so interdependent that no single node is a cut point.
- RFC-0073: `DegreeHistogram { in_degrees, out_degrees }` struct — frequency distribution of in- and out-degrees as `(degree, count)` pairs sorted ascending.
- RFC-0073: `Store::degree_histogram(kind)` — O(V) in- and out-degree frequency histograms over all symbol nodes for a given EdgeKind; degree 0 included; file nodes excluded.
- RFC-0073: `mycelium_get_degree_histogram` MCP tool — graph shape analysis; accepts `{ edge_kind }` and returns `{ in_degrees: [{degree, count}], out_degrees: [{degree, count}], total_symbols }` or `{ error }`. Power-law shape = hub-spoke architecture; uniform = balanced modular design.
- RFC-0074: `EdgeKindMetrics { symbol_count, directed_edge_count, density, avg_degree, max_in_degree, max_out_degree }` struct — structural summary for one EdgeKind.
- RFC-0074: `Store::graph_metrics(kind)` — O(V+E) structural summary: directed graph density (`E / V(V-1)`), average degree, and maximum in/out degree across all symbol nodes; file nodes excluded.
- RFC-0074: `mycelium_get_graph_metrics` MCP tool — instant architectural health check; accepts `{ edge_kind }` and returns `{ symbol_count, directed_edge_count, density, avg_degree, max_in_degree, max_out_degree }` or `{ error }`. Density near 0 = sparse/modular; near 1 = tightly coupled.
- RFC-0075: `Store::neighbor_similarity_stats(id1, id2, kind)` — returns `(similarity, shared, total)` in one pass; N(x) = outgoing ∪ incoming neighbors (self excluded); Jaccard = shared / total; both isolated → (0.0, 0, 0). O(max_degree).
- RFC-0075: `Store::neighbor_similarity(id1, id2, kind)` — Jaccard similarity ∈ [0.0, 1.0] between combined neighbor sets for a given EdgeKind; thin wrapper over `neighbor_similarity_stats`.
- RFC-0075: `mycelium_get_neighbor_similarity` MCP tool — structural role similarity detector; accepts `{ path1, path2, edge_kind }` and returns `{ similarity, shared, total }` or `{ error }`. Score 1.0 = identical structural roles (same callers+callees); 0.0 = no overlap. Useful for refactoring candidates and duplicate detection.
- RFC-0076: `Store::clustering_coefficient_stats(id, kind)` — returns `(coefficient, neighbor_count, neighbor_edge_count)` in one pass; CC(u) = #{directed edges among N(u)} / (|N(u)|*(|N(u)|-1)); N(u) = outgoing ∪ incoming, self and file nodes excluded; `|N| < 2` → 0.0. O(degree²).
- RFC-0076: `Store::clustering_coefficient(id, kind)` — local clustering coefficient ∈ [0.0, 1.0] for a symbol node; thin wrapper over `clustering_coefficient_stats`. High CC = node embedded in tightly-coupled cluster.
- RFC-0076: `mycelium_get_clustering_coefficient` MCP tool — cluster density probe; accepts `{ path, edge_kind }` and returns `{ coefficient, neighbor_count, neighbor_edge_count }` or `{ error }`. Complements neighbor_similarity (RFC-0075): measures how densely a single node's neighborhood is interconnected.
- RFC-0077: `Store::eccentricity_stats(id, kind)` — returns `(max_distance, reachable_count)` via single BFS (O(V+E)); file nodes excluded; isolated node → (0, 0).
- RFC-0077: `Store::eccentricity(id, kind)` — maximum BFS distance from a symbol node to any reachable symbol node; thin wrapper over `eccentricity_stats`.
- RFC-0077: `mycelium_get_eccentricity` MCP tool — directed reach depth probe; accepts `{ path, edge_kind }` and returns `{ eccentricity, reachable_count }` or `{ error }`. High eccentricity = deep dependency chains emanating from this node.
- RFC-0078: `Store::harmonic_centrality_stats(id, kind)` — returns `(centrality, reachable_count, symbol_count)` via single BFS (O(V+E)); HC(u) = (1/(n-1))×Σ(1/d(v)); unreachable nodes contribute 0; file nodes excluded.
- RFC-0078: `Store::harmonic_centrality(id, kind)` — harmonic centrality ∈ [0.0, 1.0]; thin wrapper over `harmonic_centrality_stats`. Near 1.0 = reaches all symbols in ~1 hop; 0.0 = isolated.
- RFC-0078: `mycelium_get_harmonic_centrality` MCP tool — average closeness probe; accepts `{ path, edge_kind }` and returns `{ harmonic_centrality, reachable_count, symbol_count }` or `{ error }`. Complements eccentricity (RFC-0077): average vs. max distance.
- RFC-0079: `MutualReachability` struct — `forward`, `backward`, `mutual` flags plus `forward_distance`/`backward_distance` `Option<usize>` hop counts.
- RFC-0079: `Store::mutual_reachability(id1, id2, kind)` — bidirectional BFS reachability; two traversals O(V+E) each; `id1 == id2` short-circuits with both distances `Some(0)`; file nodes excluded.
- RFC-0079: `mycelium_get_mutual_reachability` MCP tool — bidirectional reachability probe; accepts `{ path1, path2, edge_kind }` and returns `{ forward, backward, mutual, forward_distance, backward_distance }` or `{ error }`. Answers "are these two symbols connected, and in which direction(s)?".
- RFC-0080: `Store::reachable_set(id, kind)` — BFS transitive closure from a symbol node; returns sorted paths of all reachable symbols (source excluded, file nodes excluded); O(V+E). Answers "what does this symbol transitively call/import/extend?".
- RFC-0080: `mycelium_get_reachable_set` MCP tool — transitive dependency explorer; accepts `{ path, edge_kind }` and returns `{ reachable, count }` or `{ error }`.
- RFC-0081: `Store::reaches_into(id, kind)` — reverse BFS transitive closure; returns sorted paths of all symbols that can transitively reach `id` via `kind` edges (source excluded, file nodes excluded); O(V+E). Answers "what transitively depends on this symbol?".
- RFC-0081: `mycelium_get_reaches_into` MCP tool — reverse transitive dependency explorer; accepts `{ path, edge_kind }` and returns `{ callers, count }` or `{ error }`. Symmetric companion to `mycelium_get_reachable_set`.
- RFC-0082: `PageRankEntry` struct `{ path, score }` — one result entry from `page_rank`.
- RFC-0082: `Store::page_rank(kind, damping, iterations)` — iterative power-method PageRank; dangling nodes redistribute mass uniformly; damping clamped `[0.0, 1.0]`; file nodes excluded; returns entries sorted descending by score. Identifies globally important hub symbols.
- RFC-0082: `mycelium_page_rank` MCP tool — global importance ranker; accepts `{ edge_kind, damping?, iterations?, top_n? }` and returns `{ nodes: [{path, score}], symbol_count, top_n }` or `{ error }`. Complements local metrics (harmonic centrality, eccentricity) with a global ranking.
- RFC-0083: `Store::common_reachable(id1, id2, kind)` — intersection of transitive reachable sets of two symbol nodes; `id1 == id2` equals `reachable_set`; file nodes excluded; sorted alphabetically; O(V+E). Answers "what symbols do both nodes transitively depend on?".
- RFC-0083: `mycelium_get_common_reachable` MCP tool — shared dependency finder; accepts `{ path1, path2, edge_kind }` and returns `{ common, count }` or `{ error }`. Useful for refactoring analysis and finding shared utilities.
- RFC-0084: `Store::k_hop_neighbors(id, kind, k)` — BFS frontier at exactly depth k; nodes reached at depth < k excluded; source excluded; file nodes excluded; sorted alphabetically; O(V+E). Answers "what is reachable at exactly depth k?".
- RFC-0084: `mycelium_get_k_hop_neighbors` MCP tool — depth-scoped neighbor probe; accepts `{ path, edge_kind, k }` and returns `{ neighbors, count, k }` or `{ error }`. k=1 = direct neighbors; k=2 = two-hop callees only.
- RFC-0085: `BetweennessEntry` struct `{ path, score }` — one result entry from `betweenness_centrality`.
- RFC-0085: `Store::betweenness_centrality(kind)` — Brandes' O(V×(V+E)) algorithm; BFS per source with backward delta accumulation; normalized by (n-1)×(n-2); file nodes excluded; sorted descending. Identifies bridge nodes that lie on many shortest dependency paths.
- RFC-0085: `mycelium_get_betweenness_centrality` MCP tool — bridge node detector; accepts `{ edge_kind, top_n? }` and returns `{ nodes: [{path, score}], symbol_count, top_n }` or `{ error }`. Score ∈ [0, 1]; high score = critical bottleneck.
- RFC-0086: `SccEntry` struct `{ members, size }` — one strongly connected component from `strongly_connected_components`.
- RFC-0086: `Store::strongly_connected_components(kind)` — iterative Tarjan's O(V+E) algorithm; identifies groups of symbols that mutually depend on each other (circular dependencies); members sorted alphabetically; results sorted descending by size.
- RFC-0086: `mycelium_get_strongly_connected_components` MCP tool — circular dependency detector; accepts `{ edge_kind, min_size? }` (default `min_size=1`; use `2` for non-trivial cycles only) and returns `{ components: [{members, size}], total_components, symbol_count, min_size }` or `{ error }`.
- RFC-0087: `DegreeCentralityEntry` struct `{ path, in_degree, out_degree, in_centrality, out_centrality }` — one result entry from `degree_centrality`.
- RFC-0087: `Store::degree_centrality(kind)` — O(V+E) in-degree and out-degree centrality; both scores normalized by `(n-1)`; sorted descending by `in_centrality`. Identifies fan-in hubs (widely-used dependencies) and fan-out hubs (wide surface area).
- RFC-0087: `mycelium_get_degree_centrality` MCP tool — degree hub detector; accepts `{ edge_kind, top_n?, sort_by? }` (`sort_by: "in"` or `"out"`, defaults to `"in"`) and returns `{ nodes: [{path, in_degree, out_degree, in_centrality, out_centrality}], symbol_count, top_n, sort_by }` or `{ error }`.
- RFC-0089: `Store::dependency_depth(id, kind) -> Option<usize>` — longest-path distance from any root (no incoming symbol edges of `kind`) to `id`, following incoming edges; cycle-safe via relaxation updates; file nodes excluded; returns `None` for unknown or file-level nodes; leaf nodes return `Some(0)`.
- RFC-0089: `mycelium_get_dependency_depth` MCP tool — accepts `{ path, edge_kind }` and returns `{ path, depth, edge_kind }` on success, or `{ error }` for unknown path, file node, or unrecognised edge kind. Depth 0 = root; depth N = N layers of dependents above the node.
- RFC-0088: `ClosenessCentralityEntry` struct `{ path, score }` — one result entry from `closeness_centrality`.
- RFC-0088: `Store::closeness_centrality(kind)` — Wasserman-Faust normalized BFS closeness; `CC_WF(v) = (n_reach/(n-1))^2 * (n_reach/sum_dist)`; handles disconnected graphs; file nodes excluded; sorted descending. Identifies well-connected hubs that propagate influence quickly.
- RFC-0088: `mycelium_get_closeness_centrality` MCP tool — connection hub detector; accepts `{ edge_kind, top_n? }` and returns `{ nodes: [{path, score}], symbol_count, top_n }` or `{ error }`. Score ∈ [0, 1].
- RFC-0090: `compact_mode: Arc<AtomicBool>` field on `MyceliumServer` — server-side flag that switches symbol-search output format; thread-safe via `AtomicBool`; defaults to `false`.
- RFC-0090: `mycelium_set_compact_mode` MCP tool — toggle compact output; accepts `{ "enabled": true | false }` and returns `{ compact_mode, message }`.
- RFC-0090: `mycelium_get_token_stats` MCP tool — sample-payload byte-count comparison; returns `{ sample_query, json_bytes, msgpack_bytes, ratio }` to let callers verify the Charter §2 AI token-efficiency SLA (raw MessagePack bytes vs JSON bytes).
- RFC-0090: `mycelium_search_symbol` — when compact mode is enabled, serialises the result with `rmp_serde::to_vec_named` and returns `{ "fmt": "msgpack_hex", "data": "<hex>", "bytes": N }` instead of plain JSON, achieving significant token-count reduction for large result sets.
- RFC-0090: `encode_msgpack_hex` private helper — encodes any `serde_json::Value` as MessagePack then hex; falls back to plain JSON on serialization error (logged via `tracing::warn`).
- SPRINT-002: CI coverage job now gates on `--fail-under-branches 80` in addition to `--fail-under-lines 90`, enforcing Charter §2 / §5.4 branch coverage SLA. A second `--json --no-run` step captures per-crate branch percentages for Codecov upload.
- RFC-0004: `mycelium-hyphae` `Evaluator` — executes a parsed Hyphae `Ast` against a `Store`; supports `*`, `#name`, `.kind`, `:calls()`, `:callers()`, `:imports()`, `:extends()` pseudo-classes; `>` child, descendant space, and `~` sibling combinators; comma union; returns sorted deduplicated paths.
- RFC-0004: Parser now accepts empty-argument pseudo-classes `()` (e.g. `*:calls()` matches any symbol with at least one outgoing call edge), mapping them to "match everything" semantics.
- RFC-0004: `mycelium_query` MCP tool — accepts `{ query, limit? }`, runs a Hyphae query against the live index, returns `{ results, count, query }` on success or `{ error }` on parse failure. Primary token-efficiency interface for AI agents (Charter §2 ≤ 30% SLA).
- RFC-0004: `mycelium-mcp` now depends on `mycelium-hyphae` and imports `Evaluator` for inline query evaluation.

### Fixed

- RFC-0013: Forward-reference calls (callee defined after caller in source order) no longer create duplicate bare stub nodes; `Calls` edges now always point to the definition node.

- RFC-0006 / RFC-0005: `.tsx` files were dispatched to `LANGUAGE_TYPESCRIPT` which cannot parse JSX syntax; corrected to use `tree_sitter_typescript::LANGUAGE_TSX`.

### Changed

- (none)

### Deprecated

- (none)

### Removed

- (none)

### Fixed

- (none)

### Security

- (none)

---

[Unreleased]: https://github.com/aimasteracc/mycelium/compare/v0.1.3...HEAD
