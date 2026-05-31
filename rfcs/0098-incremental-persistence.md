# RFC-0098: Incremental Persistence

- **RFC**: 0098
- **Title**: Incremental Persistence (base snapshot + append-only per-file delta journal)
- **Status**: Draft
- **Author**: Spec Author (Hive)
- **Created**: 2026-05-31
- **Tracking issue**: #343 (scale-gap R2)
- **Revises**: [RFC-0006: Persistence](0006-persistence.md)
- **Depends on / pairs with**: [RFC-0008: Watch Mode](0008-watch-mode.md), PR #345 (`Store::merge`)

> **Governance note.** This RFC changes the on-disk storage layout, which is an
> architecture decision under **Charter §3** and a **non-trivial change** under
> CLAUDE.md Hard Rules (storage format + performance SLA). It therefore requires
> founder sign-off before leaving `Draft`, **and a new ADR in `docs/adr/`**
> (following the precedent of [ADR-0005: MessagePack wire format](../docs/adr/0005-messagepack-wire-format.md))
> recording the layout, the edge-ownership rule, the manifest schema, and the
> crash-recovery protocol. **This RFC must NOT be autonomously implemented** —
> the ADR + sign-off are a prerequisite gate.

---

## Charter §3 Amendment Note

Charter §3 (Tech Stack, locked) specifies the Persistence row as:
> "Single-file `.myc`: WAL + periodic snapshot; HAMT structural sharing — Time-travel queries free"

This RFC proposes a **v0.1 approximation** that deliberately defers the `.myc`
single-file container, HAMT structural sharing, and time-travel semantics. The
rationale: those features require substantial additional engineering
(HAMT implementation, `.myc` container format, time-travel query API) that is
out of scope for the R2 scale-gap fix. The three-file layout proposed here
(base.rmp + journal.log + manifest.rmp) is the pragmatic stepping stone.

**This RFC supersedes the Charter §3 Persistence row for v0.1.** The long-term
`.myc`/HAMT/time-travel target remains the architectural north star and must be
addressed in a subsequent `meta` RFC when the engineering capacity exists.
**Founder sign-off on this amendment is required before the ADR is filed.**
The ADR must record this deviation explicitly and reference this RFC.

---

## Summary

Replace the *full-snapshot-on-every-change* persistence model with **incremental
persistence**: a full MessagePack **base snapshot** (`base.rmp`, byte-identical
to today's `index.rmp` encoding of `Store`) plus an **append-only per-file delta
journal** (`journal.log`) and a tiny **manifest** (`manifest.rmp`). Each
`save_file()` appends one `DeltaRecord` whose size is proportional to the
*changed file's* nodes + edges — **O(changed-file)**, not **O(total-graph)**.
Load reconstructs the in-memory `Store` by decoding the base and replaying the
journal through the existing order-independent `Store::merge` / `remove_file`
primitives. A background **compaction** folds the journal back into a fresh base
when it grows past a configurable threshold. All writes are **atomic and
crash-safe** (append + `fsync` + CRC tail-truncation; temp-file + `fsync` +
`rename` for base/manifest), fixing the current non-atomic truncate-in-place
`File::create` corruption hole.

## Motivation

[`docs/scale-gap-analysis.md`](../docs/scale-gap-analysis.md) (claim #2, R2 /
issue #343) identifies the persistence path as *"the thing that kills the
reactive path at scale."* The on-disk unit of persistence today is the **entire
`Store`**:

- `store/mod.rs` (`save`) calls `rmp_serde::encode::write(&mut writer, self)`,
  which walks every field of `Store`: the full Patricia trie + `by_id` map in
  `Trunk`, every forward+reverse `AdjacencyList` for every `EdgeKind` in
  `Synapse`, plus the whole `kind_map` and `span_map`.
- The watch loop ([RFC-0008](0008-watch-mode.md)) mutates the in-memory store
  per file (`remove_file` + reindex) but then re-serializes the **whole** graph
  and `File::create`-truncates `index.rmp` from scratch each batch.

Consequences:

1. **Write cost scales with total nodes+edges N, not the changed-file delta d.**
   On a 100K+-file repo (millions of symbols/edges) every save rewrites the
   entire multi-hundred-MB snapshot. The watch debounce was cut 300ms → 5ms to
   meet the **<10ms reactive SLA** (Charter §2); an O(N) write per edit defeats
   that headroom.
2. **The write is not atomic.** `File::create` truncates `index.rmp` in place,
   so a crash mid-encode destroys the prior valid index — there is no temp file,
   no journal, no fallback.

There is no index, no segmentation, no journal, and no dirty-set. The scale-gap
doc prescribes the fix: stop full-snapshot-on-every-change via either an
append-only edit log + periodic compaction, **or** per-file segment files. This
RFC adopts the append-only journal variant (see *Alternatives considered* for
why over per-file segments).

## Guide-level explanation

After this RFC, `<root>/.mycelium/` contains:

```
.mycelium/
  manifest.rmp     # tiny: format_version, base content hash, journal seq state
  base.rmp         # full rmp-serde encoding of a Store (today's index.rmp format)
  journal.log      # append-only sequence of length-prefixed MessagePack DeltaRecords
```

- **Editing one file** appends a single `DeltaRecord` (that file's node-subtree +
  its owned edges + a CRC) to `journal.log` and `fsync`s — a few KB, independent
  of total graph size.
- **Opening the index** decodes `base.rmp`, then replays `journal.log` on top.
- **When the journal grows large**, a background compaction writes a new
  `base.rmp` (base + replayed deltas) and truncates the journal — never on the
  reactive hot path, never required for correctness.
- **A crash** never corrupts a valid index: a torn journal tail is detected by
  length-prefix + CRC + monotonic `seq` and truncated back to the last good
  record; base/manifest swaps are atomic.

No query semantics change. `NodeId` (BLAKE3-of-path) stays stable; the
order-independent `Store::merge` (PR #345) is the replay/compaction primitive.

## Detailed design

### On-disk layout & schemas

All structs serialize via `serde` + `rmp-serde` (MessagePack) only — **no
SQLite, no graph DB, no new codec** (CLAUDE.md Tool Preferences; reaffirmed by
RFC-0006 §4.1 and ADR-0005).

```rust
// manifest.rmp — small, rewritten atomically on compaction only
struct Manifest {
    format_version: u16,   // start at 1; unknown/newer (> 1) => fall back to re-index
    base_hash: [u8; 32],   // BLAKE3 of base.rmp content; integrity + compaction linkage
    base_node_count: u64,  // for benchmarking / sanity
    next_seq: u64,         // next DeltaRecord seq to assign
}

// One per save_file() call, appended to journal.log with a u32 length prefix
struct DeltaRecord {
    seq: u64,              // monotonic; a gap on replay == corruption
    path: String,          // the changed file's canonical path (the segment key)
    op: DeltaOp,           // Upsert | Remove
    // present iff op == Upsert — the file's full id-subtree as self-contained rows:
    // String is the full TrunkPath string (e.g. "src/auth.rs>AuthService>login"),
    // exactly as returned by trunk.path_of(id); replay calls TrunkPath::parse(s).
    nodes: Vec<(NodeId, String, NodeKind, SourceSpan)>, // trunk path + kind_map + span_map
    // Only forward edges are stored (src -> dst). Reverse adjacency is NOT stored
    // on disk; it is re-materialized in memory during replay by upsert_edge/merge,
    // matching the base.rmp round-trip behavior.
    edges: Vec<(EdgeKind, NodeId /*src*/, NodeId /*dst*/)>, // edges OWNED by this file
    // CRC32 (IEEE 802.3 polynomial, crc32fast crate) of the rmp-serde encoding
    // of all preceding fields in this record (seq + path + op + nodes + edges).
    // Must be verified before applying the record; failure = torn tail, truncate.
    crc:   u32,
}

enum DeltaOp { Upsert, Remove }
```

**TrunkPath replay contract**: The `String` in each `nodes` tuple is a raw
TrunkPath-encoded string (separator `>`). On replay, each string is passed to
`TrunkPath::parse(s)`. If parse fails for any node in a record, that record is
treated as corrupted (equivalent to CRC mismatch): journal replay stops at that
record and the tail is truncated, with a warning logged. This prevents a
malformed node path from causing a panic and ensures the index degrades
gracefully to the last good state.

### Edge ownership rule (resolves the cross-file-edge wrinkle)

A synapse edge `src -> dst` can connect two different files (`Calls`, `Imports`,
`Extends`, `Implements` span files), so edges do **not** partition cleanly by a
single file. We adopt **src-ownership**: an edge belongs to the segment/record of
the file containing its **`src`** node. This gives every edge exactly one owner
and a well-defined home, deterministically. Only forward edges `(EdgeKind, src, dst)`
are stored in the journal; reverse adjacency is **not persisted**. On replay,
each `upsert_edge` / `merge` call re-materializes both forward and reverse
adjacency in the in-memory `AdjacencyList` exactly as today's full-snapshot load
does. No separate reverse-derivation pass is needed after replay. This halves
on-disk edge bytes and removes the dangling-reverse-edge problem when a `dst`
file is deleted.

On `Upsert(path)`, replay first **deletes every edge previously owned by `path`**
(computed from the file's current id-set via `trunk.descendants`), then inserts
the record's `edges`. A file's edge set is thus fully replaceable from one
record, exactly mirroring `remove_file` + reindex.

### Write path — `save_file(path, op)` (the hot path; must be <10ms, atomic)

On a batch of changed files from the watch loop ([RFC-0008](0008-watch-mode.md)):

1. Mutate the in-memory `Store` as today (`remove_file` + reindex per changed
   file). **Unchanged** — this keeps queries correct within the 5ms debounce.
2. Build the `DeltaRecord` by reusing the id-set `remove_file`/reindex already
   compute: `trunk.descendants(root_id)` for nodes + `kind_map`/`span_map`
   lookups, and `synapse.all_edges()` filtered to edges whose `src` is in the
   file's id-set.
3. Serialize the record with `rmp_serde`, prepend a `u32` length, compute CRC32
   over the encoded record bytes, append `[u32 length][record bytes][u32 crc]`
   to `journal.log`, then a **single `fsync`** of the journal.

Cost is **O(d)** — the changed file's nodes + edges — with no touch of the
trunk, synapse adjacency, `kind_map`, `span_map`, or `base.rmp`. This restores
headroom under the <10ms reactive SLA. (See *Single-file write cost* below.)

The journal file handle has its own mutex (separate from the store `RwLock`),
so the store read lock can be released before serialization and the journal
append, enabling future parallel indexing (R1) without holding the store write
lock across I/O.

### Load path — `Store::load`

1. Read `manifest.rmp`. If `format_version > 1` (unknown newer version), log
   a warning and fall back to full re-index (in-spec per RFC-0006 §3, which
   disclaims v0.1 snapshot stability).
2. Decode `base.rmp` into a `Store` (existing `rmp_serde::decode::from_read`).
3. Stream `journal.log`: for each record, validate length-prefix + CRC32 +
   monotonic `seq`. For each `nodes` entry, validate `TrunkPath::parse(s)`;
   on parse failure treat the record as corrupted (same as CRC mismatch).
   Apply `Upsert` (delete edges owned by `path`, then `merge` the mini-Store's
   nodes/edges via the existing `upsert` / `all_edges` primitives) or `Remove`
   (existing `remove_file(path)`).
4. **Torn-tail recovery:** stop at the first record failing CRC, a `seq` gap,
   or a `TrunkPath::parse` failure, and truncate `journal.log` there. Base +
   all prior deltas are untouched.
5. Reverse adjacency is fully materialized as a side-effect of step 3 via
   `upsert_edge`/`merge` — no separate derivation pass required.

Result is a normal in-memory `Store`. The RFC-0006 round-trip equality test
(`save -> load -> equal Store`) is preserved and extended to the incremental
path.

### Compaction

When `journal entry count >= 512` **OR** `journal.log bytes >= base.rmp bytes`
(both configurable; whichever first), spawn compaction **off the reactive hot
path** (`tokio::task::spawn_blocking` against a consistent snapshot):

1. Load base + journal into a `Store`.
2. Write `base.rmp.tmp`, `flush`, `fsync`.
3. Write `manifest.rmp.tmp` pointing at the new base with `next_seq` carried
   forward and an empty-journal marker; `fsync`.
4. Atomic-`rename` `base.rmp.tmp -> base.rmp`, then `manifest.rmp.tmp ->
   manifest.rmp` (manifest last = single linearization/commit point).
5. `fsync` the `.mycelium/` directory to make the renames durable.
6. Truncate `journal.log` to zero, then **`fsync` `journal.log`** to make the
   truncation durable.

This six-step sequence is the only safe ordering. See the crash-safety table
below for what each failure point means.

Compaction is idempotent and **never required for correctness** — if it never
runs, load still works (just slower replay). It transiently doubles on-disk
footprint (old + new base) and CPU-spikes a full re-serialize, hence off-thread.

### Crash safety (load-after-crash defined)

| Failure point | What is durable | Recovery on next load |
|---|---|---|
| Journal append crash (mid-write) | Last fully-written record | Torn tail detected via length-prefix + CRC + seq gap; tail truncated. Base and all prior records intact. |
| Compaction crash before step 4 (rename) | Old base + full journal | Old base + journal loaded normally. Temp files ignored (not referenced by manifest). |
| Compaction crash between step 4 and step 5 (dir fsync) | Renames may not be durable (OS cache) | On most file systems the rename is visible after crash even without dir-fsync; worst case: old base + journal re-loaded. Both are correct. |
| Compaction crash after step 5 (dir fsync) but before step 6 (journal fsync) | New base + manifest committed; journal NOT yet durably empty | Next load: new base + unreplayed journal records (already folded in). Records are replayed again on top of new base. This is safe because `merge` and `upsert_edge` are idempotent. |
| Compaction crash after step 6 (journal fsync) | Fully committed new state | Clean load. |

**Key invariant**: A crash never destroys a valid prior index. The worst case
of the idempotent-replay scenario (row 4 above) produces a correct store
because merge is order-independent and duplicate-edge insertions are no-ops.

### Concurrency

The hot-path append holds the store lock only long enough to read the changed
file's id-set; serialization of the small record occurs after releasing the
store lock, guarded by the journal file-handle mutex. Compaction reads a
consistent snapshot (clone or `Arc<Store>` swap) and runs in `spawn_blocking`
so the encode never blocks the reactor or is held across an `.await` (honors
the recorded async anti-pattern in `.hive/memory/anti-patterns.jsonl`).

### Determinism preserved

`NodeId` = BLAKE3(path) truncated to u64 (low 8 bits reserved as shard-tag,
currently 0) is **unchanged**. The journal stores `NodeId`s and `path`s exactly
as today. `Store::merge` remains the order-independent union (PR #345), so replay
order is irrelevant and last-write-wins per `path` holds.

## Migration

Existing on-disk format is a bare `rmp-serde` encoding of the whole `Store` at
`<root>/.mycelium/index.rmp`.

- **No `manifest.rmp` present, `index.rmp` present (legacy):** treat `index.rmp`
  as `base.rmp` with an **empty journal** — **zero-cost, no re-index**, opens the
  user's existing index unchanged. On the first incremental save we materialize
  the new layout (copy/rename `index.rmp -> base.rmp`, write `manifest.rmp` with
  `format_version = 1`, start `journal.log`). The original `index.rmp` is kept as
  `index.rmp.migrated` for one release as a safety net.
- **Automatic cleanup of `index.rmp.migrated`:** after 10 successful incremental
  save+load cycles post-migration (tracked via a counter in `manifest.rmp`), the
  migrated file is automatically deleted. This is tested by the migration
  acceptance criterion below.
- **`manifest.rmp` present:** new format, load normally.
- **Neither present:** fresh empty index.
- **Legacy blob fails to decode:** fall back to full re-index with a logged
  warning (acceptable per RFC-0006 §3).

This satisfies the constraint to **either** load the existing `index.rmp` **or**
ship a documented migration — here we do both, with no silent corruption.

## Drawbacks

- **Load cost grows with journal length** until compaction runs (base decode + N
  delta replays); bounded by the compaction threshold but a slower cold start
  for a long-uncompacted journal.
- **Two write surfaces** (append path + compaction path) plus tail-recovery logic
  add code and test burden vs a single snapshot writer.
- **Compaction transiently doubles on-disk footprint** and CPU-spikes a full
  re-serialize, so it must be scheduled off the reactive hot path.
- **Dangling `dst`**: removing a `dst` file leaves the `src`'s record referencing
  an absent `dst` until that `src` is re-saved (already true of `merge`
  semantics; must be tested explicitly).
- **Per-file granularity** assumes the file is the right unit; a single huge
  generated file still produces a large delta record (bounded by that file, not
  N — acceptable).
- **Charter §3 deviation**: WAL/HAMT/time-travel deferred (see *Charter §3
  Amendment Note* above).

## Alternatives considered

### A. Per-file segment store (one rmp segment per source file + manifest)
Replace `index.rmp` with `segments/<aa>/<hash>.rmp` (path-hash keyed,
256-way-sharded) + a manifest; a single-file change rewrites only that segment +
the manifest. **Pros:** O(d) writes, parallelizable cold start, per-file
addressability. **Cons:** 100K+ tiny files (inode pressure, per-file `fsync`
amplification, orphan GC, collision chaining); a hub-file change forces
rewriting every segment owning an in-edge (reverse-index dirty-tracking on the
hot path); migration requires a one-time SPLIT pass writing all segments before
the user can proceed. **Rejected for now** in favor of B's lower complexity,
near-zero migration, and single-log crash story — but A is the **preferred
upgrade path** if load-time replay or per-file addressability later dominates.

### C. Dirty-tracking + debounced full snapshot (atomic)
Keep `index.rmp` byte-for-byte; stop saving on every change, flush one **atomic**
full snapshot on an idle/debounce timer + on shutdown. **Pros:** zero migration,
smallest diff, fixes the crash-corruption bug immediately. **Cons (fatal):**
per-write cost stays **O(N)** — it coalesces *frequency*, not cost, so it does
**not** solve the scale-gap R2 bottleneck; widens the durability window (edits in
the unflushed interval lost on hard crash). **Rejected as the full solution.**
However, C's **atomic temp+`fsync`+`rename`+dir-`fsync` write protocol is adopted
into this RFC** for the base/manifest writes, and C's standalone atomic-write fix
may ship first as an independent crash-safety patch.

## Acceptance criteria

- [ ] **ADR landed** in `docs/adr/` (precedent ADR-0005) recording the layout,
      src-edge-ownership rule, manifest/`DeltaRecord` schemas, CRC32 variant
      (IEEE 802.3, `crc32fast`), and crash-recovery protocol — **merged before
      any implementation PR**. ADR must note the Charter §3 deviation and
      reference this RFC.
- [ ] `DeltaRecord` and `Manifest` serialize/deserialize via `serde` +
      `rmp-serde` only (no SQLite, no graph DB, no new codec).
- [ ] `save_file(path, Upsert)` appends exactly one length-prefixed, CRC'd
      `DeltaRecord` and does **not** rewrite base/trunk/synapse/maps.
- [ ] **Single-file persist is O(changed-file), not O(total-graph)** — verified
      by a benchmark measuring persist latency for a single-file edit at **10K**
      and **100K** nodes, reporting **before vs after**; after-latency must be
      effectively flat across the two sizes (within noise) while before-latency
      scales with N.
- [ ] Per-edit persist latency stays within the **<10ms reactive SLA** (Charter
      §2) at 100K nodes on the watch path.
- [ ] **Round-trip equality** (extends RFC-0006): apply N deltas, `load`, assert
      the resulting `Store` equals an equivalent fully-reindexed `Store`.
- [ ] **Crash-safety test**: inject a mid-append failure (torn record) →
      `load` recovers to the last good record and the index is intact; inject a
      mid-compaction failure at each step (before rename, after rename but before
      dir-fsync, after dir-fsync but before journal-fsync) → the correct
      recovery behavior from the crash-safety table holds for each case.
- [ ] **Idempotent-replay test**: simulate the "crash after dir-fsync, before
      journal-fsync" scenario by loading a new base + the pre-compaction journal;
      assert the resulting `Store` is equal to a clean load (no duplicate edges).
- [ ] **Load-compat / migration test**: a legacy `.mycelium/index.rmp` opens
      unchanged (reused as base, empty journal), first save materializes the new
      layout, `index.rmp.migrated` is preserved; after 10 successful save+load
      cycles, `index.rmp.migrated` is automatically deleted.
- [ ] **TrunkPath parse-failure test**: a `DeltaRecord` containing a malformed
      node path string causes journal tail truncation at that record, not a panic;
      the index state before that record is intact.
- [ ] `Remove` op replays via existing `remove_file`; dangling-`dst` case covered
      by a test.
- [ ] Compaction triggers at the configured threshold, produces a fresh base +
      empty (durably-fsynced) journal atomically, and is a no-op for correctness
      if skipped.
- [ ] Coverage **≥ 90%** on new persistence paths (`cargo llvm-cov --workspace
      --fail-under-lines 90`); quality gate green (`fmt`, `clippy -D warnings`,
      `test --all`, `deny`, `audit`).
- [ ] RFC acceptance checkboxes updated and Status moved to `Implemented` only
      after all of the above.

## Rollout plan

1. **Founder sign-off + ADR** (Charter §3 gate, including sign-off on the Charter
   §3 Persistence row deviation). No code before this.
2. **Phase 1 — atomic write (shippable independently):** convert `Store::save`
   to temp+`fsync`+`rename`+dir-`fsync` (the C sub-component), closing the
   corruption hole without changing the format.
3. **Phase 2 — journal + manifest:** implement `DeltaRecord`/`Manifest`,
   `save_file` append path, and `load` replay with torn-tail + parse-failure
   recovery; wire the watch loop ([RFC-0008](0008-watch-mode.md)) to call
   `save_file` instead of full save.
4. **Phase 3 — compaction:** background threshold-driven compaction with the
   six-step atomic sequence (including journal fsync after truncation).
5. **Phase 4 — migration:** legacy `index.rmp` → base-with-empty-journal,
   `index.rmp.migrated` safety net with auto-cleanup after 10 cycles.
6. **Phase 5 — benchmark + crash tests + coverage**, then flip RFC Status.

**End-to-end incremental story.** This RFC makes *persistence* incremental.
Pairing it with future **Salsa-style edge-level invalidation** (recompute only
the queries affected by a changed file) closes the loop: incremental compute +
incremental persistence give an O(changed-file) reactive path end to end. That
invalidation work is out of scope here and should be its own RFC.

## References

- Issue **#343** (scale-gap R2) — tracking issue.
- **PR #345** — `Store::merge` (order-independent union; the replay/compaction
  primitive).
- [`docs/scale-gap-analysis.md`](../docs/scale-gap-analysis.md) — claim #2,
  R2/#343, the bottleneck this RFC resolves.
- [RFC-0006: Persistence](0006-persistence.md) — the original design this
  revises (§3 non-goals: v0.1 snapshot stability not guaranteed; §4.1
  MessagePack).
- [RFC-0008: Watch Mode](0008-watch-mode.md) — the watch loop that calls `save`.
- [ADR-0005: MessagePack wire format](../docs/adr/0005-messagepack-wire-format.md)
  — precedent for the required new ADR.
- CLAUDE.md Tool Preferences — no SQLite, no graph DB, `serde` + `rmp-serde`
  only; Charter §2 (<10ms reactive SLA, 90% coverage), §3 (ADR requirement,
  Persistence row deviation noted above).
