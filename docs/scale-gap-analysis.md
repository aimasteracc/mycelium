# Scale Gap Analysis — can Mycelium handle a 100K+-file Java project?

> **Short answer: not today, and we should stop implying otherwise.**
>
> This document records a source-verified audit (2026-05-31) prompted by an
> external review. Every claim below was checked against the actual code, with
> file:line citations. Where we have over-stated our scale story, it is
> corrected here.

---

## TL;DR

An external reviewer (who read both Mycelium and tree-sitter-analyzer / "TSA"
source) asserted that Mycelium is **not** built for hundreds-of-thousands of
files. **The audit confirms this — all five claims are true.** Our headline
"100K-node SLA ✅" is a *query-latency* number on a *synthetic in-memory graph*;
it says nothing about indexing throughput, persistence cost, or memory ceiling
on a real large repository.

The good news: the *ideas* are right (per-file invalidation via Salsa, radix
trie, tree-sitter-only). The gap is **engineering maturity of the index path**:
it is serial, it snapshots the whole graph on every change, and it holds the
entire graph in memory with no bound.

---

## Verified claims (source-checked)

| # | External claim | Verdict | Evidence (file:line) |
|---|---|---|---|
| 1 | Initial index is **serial, no parallelism** | ✅ TRUE | `crates/mycelium-cli/src/index.rs:167` uses `WalkBuilder…build()` (not `build_parallel()`); `:174` `for entry in walker` extracts one file at a time. `rayon`/`par_iter` appear **0 times** in the whole workspace. |
| 2 | Persistence is a **full msgpack snapshot**, rewritten on every change | ✅ TRUE | `crates/mycelium-core/src/store/mod.rs:429` `rmp_serde::encode::write(&mut writer, self)` serializes the **entire `Store`**. The watch loop calls `store.save(&snap)` after each batch (`mycelium-mcp/src/lib.rs` start_watch). Editing one file rewrites the whole `.mycelium/index.rmp`. |
| 3 | Incremental: **Salsa covers only the "symbol extraction" half-step; edges still fully re-extracted** | ✅ TRUE | `cortex.rs` is explicitly "Phase 1"; its own header says *"Phase 2 (separate RFC) will propagate Salsa invalidation signals to full-graph Store mutations."* The watch loop applies the memoised `FileIndex` **and then still calls `reindex_file()`** as a fallback for edges (calls/imports) — comment: *"Phase 2 will remove this once FileIndex is complete."* |
| 4 | **No memory bound** — full graph resident, no LRU/sharding | ✅ TRUE | No `lru`/`evict`/`mmap`/`memmap` anywhere in `crates/`. The only "shard" hits are the NodeId **shard-tag byte that is reserved but always 0** (`trunk/mod.rs:308`). The whole `Store` lives in RAM. |
| 5 | **Benchmarks cap at ~10K nodes; no hundreds-of-thousands real-file test** | ✅ TRUE (with nuance) | Criterion benches (`benches/heavy_graph.rs:42…`) run `1_000` and `10_000`. SLA tests (`tests/sla_heavy_graph.rs:21` `const N = 100_000`, `tests/sla_trunk.rs`) **do** use 100K — but they build a **synthetic in-memory graph** and measure *query* latency. **The full index pipeline (walk → tree-sitter parse → extract → persist) has never been measured on hundreds of thousands of real files.** |

---

## Direct answers

### Does our index construction have a performance problem?

**Yes — structural, not incidental.** Three compounding bottlenecks:

1. **Serial extraction.** One thread, one file at a time, through tree-sitter.
   N files = N serial parses. On a 100K+-file repo this dominates wall-clock.
2. **Whole-graph snapshot.** Every `save()` rewrites the entire graph. The
   reactive watch loop, which is supposed to be the cheap incremental path,
   still pays an O(total-graph) disk write per change batch.
3. **Unbounded resident memory.** No LRU, no sharding, no mmap. A large Java
   project (symbols + edges easily in the millions) is held entirely in RAM.

### Can a 100K+-file Java project run "with no pressure"?

**No.** Honestly:

- Our "100K-node SLA ✅" is a **synthetic in-memory query-latency** figure, not
  real indexing throughput. Representing it as proof of large-repo readiness is
  misleading — corrected in `docs/vision-vs-reality.md` alongside this doc.
- We have **no** benchmark of the *index pipeline* at hundreds-of-thousands of
  files, so we cannot claim that scale. The defensible statement is: "fast
  graph queries up to 100K nodes in-memory; index-pipeline scale unproven."

---

## What the reviewer got right that we should adopt

- **Per-file invalidation is the right idea** — and it's already our direction
  (Salsa `cortex.rs`). The gap is finishing it (Phase 2: propagate invalidation
  to edges, not just symbol extraction).
- **Don't conflate query latency with index throughput** — our SLA wording did.

## What this means for the roadmap

Three independent, Rust-only, low-risk workstreams — none requires a new
dependency beyond `rayon`:

| Workstream | Change | Risk | Payoff |
|---|---|---|---|
| **R1 Parallel index** | `WalkBuilder::build_parallel()` + per-file extract on a thread pool, merge into `Store` under a lock (or per-thread sub-stores merged at the end) | Low | Near-linear speedup on initial index |
| **R2 Incremental persistence** | Stop full-snapshot-on-every-change. Either append-only edit log + periodic compaction, or per-file segment files keyed by path hash | Medium | Removes O(total-graph) write per edit — the thing that kills the reactive path at scale |
| **R3 Memory bound** | Optional LRU/segment eviction or mmap-backed store behind a feature flag; activate the reserved NodeId shard-tag byte for sharding | Medium | Lets the graph exceed RAM; unblocks truly large repos |

Plus the already-scoped **Salsa Phase 2** (edge-level invalidation) which makes
R2's incremental story actually incremental end-to-end.

These should become tracked issues before any "large Java project" claim is made.

---

## Honesty note

Prior wording (including the first draft of `docs/vision-vs-reality.md`) leaned
on "Charter §2 SLA 100K-node ✅" as if it demonstrated large-repo readiness. It
does not. This audit corrects the record: **the reactive core works and queries
are fast in-memory, but the index pipeline is not yet engineered for
hundreds-of-thousands of files.** Claiming otherwise would be the kind of
fantasy this project's QA discipline (Reality Checker / Evidence Collector)
exists to prevent.
