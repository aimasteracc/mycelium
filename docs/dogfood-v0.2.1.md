# Dogfood Report — v0.2.1 (RFC-0119 AC-12/AC-13)

> RFC-0119 AC-12 + AC-13 real-corpus validation of the context importance
> ranking fix (gerund expansion).
>
> Run: 2026-06-19 · binary: `target/release/mycelium` (develop, post-fix) ·
> target repo: this workspace.

## Regression found before the fix

Before the gerund-expansion patch, running `mycelium context` against the
Mycelium self-index returned **only test functions** for the query
`"how does indexing work"`:

```
candidates: ["indexing"]
entry_points:
  - crates/mycelium-core/src/context/tests.rs>context_indexing_query_ranks_subsystem_over_test_fixture
  - crates/mycelium-core/src/store/tests.rs>merge_carries_call_site_contexts_for_parallel_indexing
```

**Root cause**: `search_symbol("indexing")` only matches the leaf segment of
trunk paths. Production symbols (`index_path`, `index_file_into`, `IndexStats`)
use the root form "index", not the gerund "indexing". Only test functions that
*test the indexing subsystem* happened to contain "indexing" in their names.
With an empty `non_test` bucket, the never-empty guarantee in `rank_entry_points`
fell back to returning the test functions.

## Fix: gerund-to-stem expansion in `extract_symbol_candidates`

`context/mod.rs` `extract_symbol_candidates` now appends bare stems for `-ing`
gerund tokens (≥ 7 chars, stem ≥ 4 chars, not a stop word):

- `"indexing"` → also tries `"index"`
- `"searching"` → also tries `"search"`
- `"filtering"` → also tries `"filter"`

## Post-fix dogfood run

```bash
cargo build -p mycelium-rcig-cli --release
./target/release/mycelium index .          # 124 files indexed, 0 errors
./target/release/mycelium context --task "how does indexing work"
```

**Result** (key fields):

```json
{
  "candidates": ["indexing", "index"],
  "entry_points": [
    "bindings/python/mycelium_rcig/_client.py>Mycelium>index",
    "crates/mycelium-cli/src/index.rs>index_path",
    "crates/mycelium-cli/src/index.rs>index_file_into",
    "crates/mycelium-cli/src/index.rs",
    "crates/mycelium-cli/src/index.rs>IndexStats"
  ],
  "stats": { "entry_points": 7, "nodes": 30, "edges": 25, "code_blocks": 6 }
}
```

Production code (`index_path`, `index_file_into`, `IndexStats`) now leads.
No test functions from `tests.rs` files appear. AC-12 passes.

## AC-12 assertion

`index.rs` (the real indexing implementation) appears as an entry point above
any test fixtures. `IndexStats`, `index_path`, and `index_file_into` are all
production symbols from `crates/mycelium-cli/src/index.rs`.

## TDD notes

- **RED**: Two new tests added to `context/tests.rs` before the fix
  - `extract_expands_gerund_to_stem` — asserts `["indexing","index"]` in candidates
  - `seed_finds_production_code_from_gerund_query` — asserts `index_file` ranks above test fn
- **GREEN**: Both pass after the one-function patch to `extract_symbol_candidates`
- **Regression guard**: existing `context_indexing_query_ranks_subsystem_over_test_fixture`
  still passes (still tests the path-based demotion logic with synthetic store)

## Limitations observed

Inline test functions inside production files (e.g.,
`index_path_always_skips_mycelium_dir` in `src/index.rs`) are not demoted
because `classify_test_path` is file-path-based and cannot detect
`#[cfg(test)]` module content. This is a known limitation tracked under
the RFC-0119 "Phase 3" notes (attribute-based detection).
