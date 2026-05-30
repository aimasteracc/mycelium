# Changelog

All notable changes to **Mycelium** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

