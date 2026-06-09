# RFC-0122: Phase 2b — Cross-file call-site context resolution

- **Status**: Draft
- **Created**: 2026-06-09
- **Author**: orchestrator (PM dispatch v148)
- **Depends on**: RFC-0118 (Phase 2a — pack captures present on `packs/rust/queries.scm`)
- **Tracked by**: Issue #612 Item 1
- **Blocked by**: PR #568 back-merge to develop (branch baseline prerequisite)

---

## Summary

RFC-0118 Phase 2a added static receiver-type inference via `disambiguate()` inside
`resolve_call_site_contexts()`. This works for **same-file** calls where the type
definition has already been ingested when the call site is processed.

For **cross-file** calls (e.g. `let s = Store::new(); s.upsert_node()` where `Store`
is defined in a different file processed later), `disambiguate()` finds no candidate,
falls back to the shared bare stub, and the receiver binding is lost. The post-pass
conservative resolver still cannot resolve the multi-match method call — `get-callers`
returns 0 for those call sites. This is `F5` in the RFC-0118 risk table.

This RFC adds a **two-phase extraction model** that makes cross-file resolution
order-independent.

---

## Problem

### Observed symptom

```
$ mycelium get-callers Store::upsert_node --format json
{"caller_paths": [], ...}
```

Expected: should return all callers across all files in the repo.

### Root cause (from Issue #612 Item 1 analysis, v144 PM dispatch)

`resolve_call_site_contexts()` runs *during extraction* (per-file). When file `a.rs`
calls `Store::upsert_node()` and `Store` is defined in `b.rs` which hasn't been
extracted yet:

1. `disambiguate()` queries the `Store` (only partial data in store).
2. Gets 0 candidates → falls back to bare stub.
3. Receiver type binding is discarded.
4. Even when `b.rs` is later extracted, the call site in `a.rs` has no receiver
   context to re-evaluate.

The pack captures required for Phase 2b are already present (verified PM v144):
- `@call.receiver` (queries.scm:158)
- `@binding.local` / `@binding.ctor` (queries.scm:183, 186)
- `@param.type` (queries.scm:195)

The gap is entirely in the **resolver algorithm**, not the pack layer.

---

## Proposed solution

### Phase 2b: Deferred call-site context table

Add a **`CallSiteContext` table** persisted alongside the store that records
unresolved receiver bindings during per-file extraction. After all files are
extracted (or on the next full-index cycle), a **post-merge pass** re-evaluates
every entry in the table against the now-complete store.

```
Extract file A → receiver binding unresolved → append to CallSiteContext table
Extract file B → Store defined → append resolved types to store
Post-merge pass → re-evaluate CallSiteContext entries → back-patch edges
```

### Data model

```rust
/// Persisted in redb under TABLE_CALL_SITE_CONTEXT.
pub struct CallSiteContext {
    /// The stub node path for the unresolved call (e.g. ">upsert_node").
    pub stub_path: String,
    /// Inferred receiver type name from @call.receiver capture at extraction time.
    pub receiver_type_hint: String,
    /// File where the call site lives.
    pub call_site_file: String,
    /// All candidate definitions from a previous multi-match.
    pub candidates: Vec<String>,
}
```

### Post-merge pass algorithm

```
for each entry in CallSiteContext:
    resolved = disambiguate(entry.receiver_type_hint, entry.candidates, store)
    if resolved.is_some():
        re-point stub → resolved definition (update synapse fwd/rev edges)
        remove entry from CallSiteContext
    else:
        leave in table for next pass (incremental: re-evaluate on next index cycle)
```

### Trigger points

| Trigger | Action |
|---|---|
| `mycelium index` completes all files | Run post-merge pass once |
| `mycelium watch` detects a type definition file changed | Re-run pass for affected receiver types |
| `mycelium index --incremental` | Run pass on newly added CallSiteContext entries only |

### API surface (Three-Surface Rule, RFC-0090 / Charter §5.13)

No new CLI/MCP tool. This is an internal store operation — transparent to callers.
`get-callers` / `mycelium_get_callers` automatically returns more results after the
post-merge pass runs. No surface change = no parity obligation.

---

## Implementation plan

### Step 1 — Schema (redb table)
- Add `TABLE_CALL_SITE_CONTEXT: TableDefinition<&str, &[u8]>` to
  `crates/mycelium-core/src/store/redb_codec.rs`.
- Define `CallSiteContext` struct + redb codec (serde + rmp-serde).

### Step 2 — Extraction integration
- In `crates/mycelium-core/src/extractor/mod.rs`, after `disambiguate()` returns
  `None` for a multi-match call site, instead of falling back silently: write a
  `CallSiteContext` entry to the table (within the same `WriteTransaction`).

### Step 3 — Post-merge pass
- Add `Store::resolve_deferred_call_sites(&mut self) -> Result<usize>` in
  `crates/mycelium-core/src/store/mod.rs`.
- Call from `index` command after all files extracted.
- Return count of resolved entries (logged at `debug!`).

### Step 4 — Watch integration
- In `WatchEngine::on_batch`, after re-extracting changed files, call
  `resolve_deferred_call_sites()` for affected receiver type paths.

### Step 5 — Tests (TDD: RED first)
- Unit: `deferred_context_persists_on_cross_file_extraction` — two in-memory stores,
  first file has call, second has definition; assert table populated.
- Integration: `cross_file_caller_resolved_after_post_pass` — index a two-file
  fixture; assert `get-callers` returns the cross-file caller before and after pass.
- E2E: re-run dogfood against this repo and assert Mycelium's own cross-file callers
  (e.g. `Store::upsert_node` called from CLI) appear in `get-callers` output.

---

## Acceptance criteria

- [ ] AC-1: `CallSiteContext` table defined in `redb_codec.rs` with codec tests.
- [ ] AC-2: Extractor writes deferred entries for cross-file unresolved multi-match calls.
- [ ] AC-3: `Store::resolve_deferred_call_sites()` re-evaluates entries against full store.
- [ ] AC-4: `mycelium index` calls the post-merge pass; count logged.
- [ ] AC-5: Watch engine triggers pass on definition-file changes.
- [ ] AC-6: `cross_file_caller_resolved_after_post_pass` integration test GREEN.
- [ ] AC-7: Dogfood `get-callers Store::upsert_node` returns ≥ 1 caller.
- [ ] AC-8: `cargo test --all` green; coverage ≥ 90%.
- [ ] AC-9: No new unsafe, no SQLite, no graph-DB dependency.

---

## Alternatives considered

**Alt A: Re-run full extraction on every file change.**
Correct but O(N) per change. Violates the `< 10 ms` reactive re-query SLA.

**Alt B: Topological sort of files before extraction.**
Requires a pre-pass over all files to build a dependency graph (expensive). Also
breaks for circular imports (common in Python, TypeScript). Rejected.

**Alt C: LSP-based semantic resolution.**
Explicitly rejected by ADR-0010 (no live LSP). Cold-start latency (seconds) violates
Charter §2 SLA. Rejected.

**Chosen: Deferred context table (this RFC).**
Incremental, bounded per entry, compatible with Salsa reactivity model, and
self-healing (entries retry on next pass automatically).

---

## Risks

| Risk | Mitigation |
|---|---|
| Post-merge pass adds latency to `mycelium index` | Gate on `MYCELIUM_RESOLVE_DEFERRED=1` env flag until benchmarked; add to Charter §2 SLA table when data available |
| redb schema bump breaks existing index files | Handle with `open_existing` migration: if `TABLE_CALL_SITE_CONTEXT` absent, treat as empty (no entries to process) |
| Circular receiver inference loops | Limit pass iterations to `min(entries.len(), 3)` per index cycle |

---

## Status history

| Date | Status | Note |
|---|---|---|
| 2026-06-09 | Draft | Authored by PM dispatch v148; pack captures confirmed present (PM v144) |

*Next step: Architect review → rust-implementer implementation (after PR #568 back-merges to develop).*
