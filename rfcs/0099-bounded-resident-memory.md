# RFC-0099: Bounded Resident Memory (measurement-first; streaming index + segment eviction)

**Status:** Draft
**Author:** orchestrator (Hive AI agent)
**Created:** 2026-05-31
**Decision gate:** Charter §3 (storage format / memory strategy) — **requires founder sign-off**
**Tracking issue:** [#344](https://github.com/aimasteracc/mycelium/issues/344) (R3 — scale-gap)
**Supersedes / superseded by:** —
**Depends on:**
- RFC-0001 (NodeId = content hash; low-8-bit **shard-tag reserved, always 0** — `store/node.rs:56`, reservation comment `store/node.rs:40-43`)
- **RFC-0098 (Incremental Persistence)** — Phase 2 (segment eviction) reuses RFC-0098's on-disk per-file segment format. **R3 Phase 2 MUST NOT be implemented until RFC-0098 is ratified and merged.** Phase 0 (measurement) and Phase 1 (streaming index) do **not** depend on RFC-0098.

---

## Summary

Today the **entire `Store`** — `Trunk` (radix trie + `by_id` reverse map), `Synapse`
(forward + reverse adjacency per `EdgeKind`), `kind_map`, and `span_map` — is held
resident in RAM (`store/mod.rs:395`). There is **no memory bound**: no `lru`, no
`evict`, no `mmap` anywhere in `crates/`. A large Java repo (symbols + edges in the
millions) is held entirely in memory, and beyond a certain size the process OOMs —
**both while indexing and while serving queries.**

This RFC sequences the fix to match risk and the R2 dependency:

1. **Phase 0 — Measure (no behaviour change).** Produce the RSS-vs-node-count curve at
   100K / 500K / 1M nodes so we size the problem with evidence, not guesses.
2. **Phase 1 — Bounded-peak streaming index (independent of R2).** Solve the *acute*
   pain — OOM **while indexing** — by capping the indexer's working set: spill
   per-shard sub-`Store`s to disk and merge externally, so peak RSS during a full
   index is bounded regardless of repo size. The resident query store is unchanged.
3. **Phase 2 — Segment eviction for queries (gated on RFC-0098).** Solve the *larger*
   problem — *querying* a graph larger than RAM — with LRU eviction of cold per-file
   segments, reusing RFC-0098's on-disk segment format, reloading on demand.

The default all-in-RAM fast path stays the default and is **feature-flagged off** for
Phases 1–2. No perf regression for normal-size repos.

---

## 1. Context & Problem

### 1.1 What is resident today

Verified against source (`crates/mycelium-core/src/`):

```
Store (store/mod.rs:395)
├── trunk: Trunk (trunk/mod.rs:136)
│   ├── root: TrieNode               // the radix trie
│   └── by_id: HashMap<NodeId,String>  // reverse index, one String per node
├── synapse: Synapse (synapse/mod.rs:145)
│   └── by_kind: HashMap<EdgeKind, AdjacencyList>
│       └── AdjacencyList (synapse/mod.rs:26)
│           ├── forward: HashMap<NodeId, Vec<NodeId>>
│           └── reverse: HashMap<NodeId, Vec<NodeId>>   // edges stored twice
├── kind_map: HashMap<NodeId, NodeKind>
└── span_map: HashMap<NodeId, SourceSpan>   // file + line/col/byte per node
```

Every node carries: a trie entry, a `by_id` `String` (full path), a `kind_map` entry,
a `span_map` entry, and its incident edges stored **twice** (forward + reverse). None
of it is ever released. `Store::save`/`load` (`store/mod.rs:421`/`:439`) read and write
the **whole** thing via `rmp-serde`.

### 1.2 Two distinct failure modes

These are usually conflated; they have different fixes:

| Failure | When | Cause | Fixed by |
|---|---|---|---|
| **Index-time OOM** | building the index | the full `Store` plus per-thread sub-`Store`s (R1 parallel index) are all resident before the final merge | **Phase 1** (streaming index) — independent of R2 |
| **Query-time OOM** | serving queries / watch mode | the full resident graph exceeds RAM even at rest | **Phase 2** (segment eviction) — needs RFC-0098 format |

R1 (parallel index, shipped v0.1.14) **increased** index-time peak: N thread-local
sub-`Store`s coexist with the merge target. Phase 1 directly addresses that.

### 1.3 Why not "just guess a number"

The issue (#344) and `docs/scale-gap-analysis.md` (R3) are explicit: **measurement
first**. We do not know the bytes-per-node constant today. Committing to mmap vs LRU vs
sharding before measuring would be the kind of fantasy the project's QA discipline
exists to prevent. Phase 0 is a hard prerequisite for Phases 1–2.

---

## 2. Goals / Non-Goals

### Goals
- G1. A documented, reproducible **RSS-vs-node-count curve** (100K / 500K / 1M nodes).
- G2. **Bounded peak RSS while indexing** a repo whose final graph exceeds a deliberately
  small cap — without changing query results.
- G3. **Bounded resident RSS while querying** a graph larger than RAM (Phase 2).
- G4. **Zero change to the default path.** All-in-RAM stays default; bounding is opt-in
  behind a cargo feature + a runtime knob. No measurable regression for normal repos.
- G5. **Identical query results** in bounded mode vs in-RAM mode (semantic equivalence,
  the same contract R1 used — not byte-identity).

### Non-Goals
- NG1. Distributed / multi-process sharding. (The reserved NodeId shard-tag byte is the
  *long-term* hook; activating it is out of scope here — see §5.4.)
- NG2. Changing the in-RAM data structures themselves (trie, adjacency) — this RFC bounds
  *residency*, not representation. A separate RFC may later shrink per-node overhead.
- NG3. Replacing `rmp-serde`. Phase 1 spill and Phase 2 segments reuse existing formats
  (Phase 1: ephemeral `Store` snapshots; Phase 2: RFC-0098 segments).
- NG4. Implementing Phase 2 before RFC-0098 is ratified. Explicitly forbidden.

---

## 3. Prior Art

- **codegraph** (referenced in `anti-patterns.jsonl`): recycles the tree-sitter parse
  worker every ~250 files because WASM linear memory only grows. We already know parse-
  side memory is a real hazard; this RFC bounds the *graph* side.
- **rust-analyzer**: keeps salsa-interned data resident but evicts derived query results
  under memory pressure; on-disk is incremental. Mirrors our Phase 2 + RFC-0098 split.
- **External-memory / streaming merge** (classic): bound peak by processing in shards and
  k-way merging spilled runs. Phase 1 is a direct application over `Store::merge`
  (shipped in v0.1.14, order-independent because NodeId is a content hash).
- **mmap-backed stores** (LMDB, sled, rkyv archives): OS page cache bounds RSS for reads.
  Considered (§4 Option A) and deferred — too large a format break for now.

---

## 4. Design Options Considered

### Option A — mmap-backed zero-copy store
Serialize the `Store` to a zero-copy archive (e.g. `rkyv`), `mmap` it, let the OS page
cache bound RSS. **Pros:** eviction is free (the kernel does it); reads of a graph larger
than RAM "just work." **Cons:** replaces the `rmp-serde` on-disk format with a zero-copy
archive — a large Charter §3 break; the write/update path becomes complex; conflicts with
RFC-0098's append-journal model. **Verdict: deferred** (documented, not chosen).

### Option B — LRU segment eviction over RFC-0098 segments
Partition the graph into per-file segments (exactly RFC-0098's unit). Keep an LRU set of
hot segments resident; evict cold segments (they already live on disk as RFC-0098
segments); fault them back in on query miss. **Pros:** reuses the R2 substrate; no new
on-disk format; naturally incremental. **Cons:** requires RFC-0098 first; query path must
handle "segment not resident → load." **Verdict: chosen for Phase 2.**

### Option C — Bounded-peak streaming index (spill + external merge)
During indexing, cap the number of nodes held in any working sub-`Store`; when a shard
fills, spill it to a temp `Store` snapshot on disk and start a fresh one; at the end,
fold all spilled snapshots via `Store::merge`. **Pros:** bounds the *acute* index-time
peak; independent of RFC-0098; reuses shipped `Store::merge`; low risk; biggest immediate
win. **Cons:** does not help query-time residency (that's Phase 2). **Verdict: chosen for
Phase 1.**

---

## 5. Recommended Design

A three-phase rollout. Each phase is independently shippable and independently
feature-gated; **Phase 2 is blocked on RFC-0098**.

### 5.0 Phase 0 — Measurement (ships first, no behaviour change)

- New bench `benches/memory_curve.rs` (or `xtask mem-curve`): build synthetic graphs of
  100K / 500K / 1M nodes with a realistic edge:node ratio (measured from a real Java repo,
  not invented), record peak RSS (`/proc/self/status` `VmHWM` on Linux; `ru_maxrss` via
  `getrusage` cross-platform) and resident-at-rest RSS.
- Output committed to `docs/scale-gap-analysis.md` as a table + the bytes-per-node
  constant. This number sizes Phases 1–2 caps.
- **Acceptance:** the curve exists and is reproducible. No code path changes.

### 5.1 Phase 1 — Bounded-peak streaming index (independent of RFC-0098)

- Cargo feature `bounded-index` (off by default).
- Runtime knob: `mycelium index --max-resident-nodes <N>` (CLI) and the 1:1 MCP arg
  (RFC-0090 Three-Surface Rule — the MCP `index` tool gets the byte-identical arg; both
  appear in the indexing category Skill).
- Mechanism, layered on the shipped R1 parallel indexer:
  1. Each worker accumulates into a thread-local `Store` as today.
  2. When a worker's node count crosses `max_resident_nodes / num_workers`, it **spills**:
     `store.save(tmp_segment_k.rmp)`, then `*store = Store::new()`.
  3. After the walk, fold all spilled segments + in-flight stores with `Store::merge`
     (already order-independent — NodeId is a content hash, RFC-0001), then run
     `resolve_bare_call_stubs()` once, exactly as the serial/parallel paths do today.
- **Result:** peak RSS during indexing ≈ `max_resident_nodes` worth of `Store`, plus one
  merge target, regardless of total repo size.
- **Default path unchanged:** with the feature off (or `--max-resident-nodes` unset), the
  shipped in-RAM parallel index runs verbatim.

### 5.2 Phase 2 — Segment eviction for queries (**gated on RFC-0098**)

- Cargo feature `bounded-resident` (off by default).
- Runtime knob: `--max-resident-segments <N>` / `--max-rss <bytes>`.
- An `EvictingStore` wraps the resident `Store` with an LRU of hot per-file segments
  (RFC-0098 unit). On a query touching a non-resident segment, load it from its RFC-0098
  segment file; when over budget, evict the coldest (it's already durable on disk, so
  eviction is a drop, not a write).
- Query results MUST be identical to the all-resident path (G5). The eviction layer is
  transparent to Hyphae.
- **Blocked:** this phase reads RFC-0098's on-disk segment format. It MUST NOT be built or
  merged until RFC-0098 is `Implemented`. Until then this section is design-only.

### 5.3 Configuration surface (Three-Surface Rule, RFC-0090)

Every new knob ships on **all three surfaces** or none:
- CLI: `mycelium index --max-resident-nodes`, query-side `--max-resident-segments` (Phase 2).
- MCP: byte-identical tool args.
- Skill: both appear in `skills/<indexing|query>/SKILL.md` `allowed-tools`.

### 5.4 Long-term hook (out of scope, noted for continuity)

The NodeId low-8-bit **shard-tag** (reserved at RFC-0001, always 0 today,
`store/node.rs:56`) is the seam for future real sharding (segment routing by shard tag).
This RFC does **not** activate it (NG1); Phase 1/2 partition by *file*, not by shard tag.
A future RFC may map files → shard tags to make Phase 2 segments self-routing.

---

## 6. Crash Safety & Durability

- **Phase 0:** none (read-only benches).
- **Phase 1:** spilled `tmp_segment_k.rmp` files are written to a temp dir and `fsync`ed
  before the worker frees its in-RAM store; on crash mid-index, temp segments are orphaned
  and cleaned on next run (the index is rebuilt from source anyway — indexing is not the
  source of truth). The final atomic `rename` of the merged index is unchanged.
- **Phase 2:** eviction is a pure drop of already-durable RFC-0098 segments — no new write
  path, so it inherits RFC-0098's crash-safety (append + fsync + CRC tail-truncation).

---

## 7. Migration & Compatibility

- **Phase 0 / Phase 1:** no on-disk format change. Existing `index.rmp` loads unchanged.
  Features off → byte-for-byte the current behaviour.
- **Phase 2:** consumes RFC-0098's format; migration is governed by RFC-0098. No
  *additional* migration beyond R2.
- Reverting any phase = compiling with the feature off. No data migration to undo.

---

## 8. Charter §3 Amendment Note

R3 touches the **memory strategy** and (Phase 2) the **on-disk segment format**, both
Charter §3 decision-gate matters. This RFC requests founder sign-off for:

1. The phased plan and the principle that **bounding is opt-in, default is in-RAM**.
2. Phase 1's temp-spill mechanism (no persistent format change).
3. A binding constraint: **Phase 2 is not implementable until RFC-0098 is `Implemented`.**

No Charter *text* amendment is proposed; this is a §3 decision record. If founder prefers
mmap (Option A) over segment eviction (Option B), that is a different §3 format break and
this RFC would be revised before any Phase 2 work.

---

## 9. Acceptance Criteria

**Phase 0 (measurement):**
- [ ] Reproducible RSS-vs-node-count curve at 100K / 500K / 1M nodes, committed to
      `docs/scale-gap-analysis.md`, with the measured bytes-per-node constant.
- [ ] Edge:node ratio used is measured from a real repo and cited (not invented).
- [ ] No behaviour change; default path untouched.

**Phase 1 (streaming index — independent of RFC-0098):**
- [ ] `bounded-index` cargo feature, off by default.
- [ ] `--max-resident-nodes` on CLI **and** byte-identical MCP arg (RFC-0090) **and**
      present in the indexing-category Skill.
- [ ] TDD: a test that indexes a synthetic graph exceeding a small `--max-resident-nodes`
      cap and asserts the resulting graph is **semantically equal** to the unbounded index
      (same node set, same edge set) — written RED first.
- [ ] Bench shows peak RSS during a bounded index is bounded (≈cap) while the unbounded
      index's peak grows with repo size.
- [ ] Feature off ⇒ shipped parallel index runs verbatim (no regression; existing tests green).

**Phase 2 (segment eviction — BLOCKED on RFC-0098):**
- [ ] (blocked) `bounded-resident` cargo feature, off by default.
- [ ] (blocked) Query results in eviction mode are identical to all-resident mode.
- [ ] (blocked) Bench: query a graph larger than a deliberately small `--max-rss` cap with
      bounded peak RSS.
- [ ] (blocked) RFC-0098 is `Implemented` before any Phase 2 code merges.

---

## 10. Risks & Mitigations

| Risk | Severity | Mitigation |
|---|---|---|
| Measurement (Phase 0) reveals bytes-per-node is small enough that R3 is premature | Low | Good outcome — we'd defer Phases 1–2 with evidence and document the real ceiling. Phase 0 is cheap. |
| Phase 1 spill I/O slows indexing | Medium | Off by default; only engaged above a cap a normal repo never hits; spill is sequential `rmp` writes. |
| Phase 2 query-miss latency (segment fault-in) | Medium | LRU sized from Phase 0 data; segment = one file (small); measure p99 in bench. |
| Building Phase 2 against an unratified R2 format | High | **Hard constraint §8/§9:** Phase 2 blocked until RFC-0098 `Implemented`. |
| Feature-flag drift (bounded path bit-rots) | Medium | CI matrix builds + runs the equivalence test with features on; not just default. |

---

## 11. Rollout Plan

1. **RFC review** (this doc) → founder §3 decision: approve phased plan; confirm Option B
   (segment eviction) over Option A (mmap) for Phase 2, or send back.
2. **Phase 0** — measurement bench + curve in `docs/scale-gap-analysis.md`. Ship to develop.
   *This is the immediately-actionable piece and needs no R2.*
3. **Phase 1** — `bounded-index` + `--max-resident-nodes` + streaming spill/merge, TDD.
   Ship behind the off-by-default feature once Phase 0 sizes the cap.
4. **Phase 2** — only after RFC-0098 is `Implemented`: `bounded-resident` segment eviction.
5. Update `docs/scale-gap-analysis.md` R3 row to "in progress / done per phase" and append
   `.hive/memory/decisions.jsonl`.

---

*Companion to RFC-0098 (R2). Together they make the "handles a hundreds-of-thousands-of-files
Java project" claim defensible — R2 makes writes incremental, R3 makes residency bounded.
Phase 0 + Phase 1 are actionable now; Phase 2 waits on R2.*
