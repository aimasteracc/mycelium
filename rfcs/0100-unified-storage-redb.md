# RFC-0100: Unified Storage Layer on redb (memory-mapped embedded engine)

- **RFC**: 0100
- **Title**: Unified Storage Layer on redb — one backend that makes writes incremental (R2) and resident memory bounded (R3)
- **Status**: Draft
- **Author**: orchestrator (Hive AI agent)
- **Created**: 2026-05-31
- **Decision gate**: Charter §3 (Tech Stack, *locked*) — **founder-authorized 2026-05-31** ("允许引入 redb（方案 A）"); this RFC carries the §3 amendment text + requires a new ADR before implementation
- **Tracking issues**: [#343](https://github.com/aimasteracc/mycelium/issues/343) (R2), [#344](https://github.com/aimasteracc/mycelium/issues/344) (R3)
- **Supersedes**: [RFC-0006 Persistence](0006-persistence.md), [RFC-0098 Incremental Persistence](0098-incremental-persistence.md), [RFC-0099 Bounded Resident Memory](0099-bounded-resident-memory.md)

> **Governance.** This changes the §3 *locked* Storage and Persistence rows and adds an
> external dependency — the highest-gate change in the project. The founder authorized the
> direction on 2026-05-31 (choosing "Option A: allow redb" over "build our own mmap
> engine"). This RFC therefore (a) proposes the exact Charter §3 amendment text (§2 below),
> and (b) requires a new **ADR in `docs/adr/`** recording the table schema, the
> value-encoding rule, the migration protocol, and the crash-recovery contract before any
> code merges. **Do NOT autonomously implement before the ADR lands.**

---

## 1. Summary

Replace the bespoke full-snapshot storage layer with **[redb](https://github.com/cberner/redb)** —
a pure-Rust, memory-mapped, ACID, copy-on-write B-tree key-value store — as the backing
engine for the `Store` (Trunk + Synapse + kind/span maps).

This single change solves **both** open scale-gap problems at once, because they are the
same problem (the storage layer) seen from two sides:

| Problem | Today | With redb |
|---|---|---|
| **R2 — writes** (#343) | every change rewrites the **whole** graph to disk (`Store::save`, O(total)) | one write transaction touches only the changed file's keys — **O(changed)** |
| **R3 — memory** (#344) | the **entire** graph is resident in RAM; no bound; OOM on large repos | redb mmaps the file; the **OS page cache** bounds RAM — graphs larger than RAM "just work" |

The logical model (Trunk = radix trie, Synapse = adjacency, NodeId = content hash) is
**unchanged** — only the bytes-on-disk and the residency model change. MessagePack
(`rmp-serde`) is **retained as the per-record value encoding inside redb**, so the Charter
"Wire format: MessagePack" row is preserved.

Why this supersedes RFC-0098 and RFC-0099: those two proposed *hand-built* mechanisms (an
append-only delta journal; an LRU segment-eviction cache) that, combined, reimplement what
a memory-mapped transactional B-tree already does — correctly, crash-safely, and battle-
tested. With founder approval to take an external engine, the honest engineering choice is
to adopt it rather than reinvent it.

---

## 2. Charter §3 Amendment (proposed text)

Charter §3 (Tech Stack, *locked*) currently reads (verbatim, lines 56–57):

```
| Storage     | Self-built: trunk (radix trie) + synapse (CSR) + Apache Arrow columnar attrs | See RFC-0001 |
| Persistence | Single-file `.myc`: WAL + periodic snapshot; HAMT structural sharing         | Time-travel queries free |
```

**Proposed replacement:**

```
| Storage     | **redb** (pure-Rust mmap B-tree) backing the RCIG model (Trunk + Synapse) | Not SQLite, not a graph DB; embedded KV engine. mmap bounds RAM; ACID txns make writes incremental. We still own the logical model + value schema. |
| Persistence | redb ACID transactions (copy-on-write B-tree) | Crash-safe by construction; per-file incremental writes; mmap residency |
```

**What the amendment does and does NOT relax:**
- ✅ Still **no SQLite.** redb is a pure-Rust KV store, not SQLite, not a SQL engine.
- ✅ Still **no graph database.** redb is a key-value B-tree; the graph semantics remain
  ours (Trunk/Synapse on top of KV tables).
- ✅ Still **MessagePack** for values (Charter Wire-format row unchanged).
- ✅ Still **our own logical format/schema** — we define the tables and value layout.
- 🔄 **Relaxed:** "we own the *format*" (byte layout on disk) → we now own the *schema*,
  and delegate the *byte container + page cache + transactions* to redb. This is the one
  thing the founder explicitly authorized.

---

## 3. Why redb specifically

Selection criteria for a code-intelligence graph that must scale to 10⁵–10⁶ files:

| Criterion | redb | Why it matters here |
|---|---|---|
| Pure Rust, no C deps | ✅ | Cross-platform single binary; **no Windows build breakage** (the v0.1.4 saga was a Windows-binary incident — a C-linked engine would re-open that risk) |
| Memory-mapped | ✅ | OS page cache bounds RAM → **R3** without a hand-rolled LRU |
| ACID transactions | ✅ | Per-file write txn → **R2**; crash safety for free → deletes RFC-0098's fsync+CRC code |
| Copy-on-write / MVCC | ✅ | Read snapshots → near-free **time-travel** (the original Charter vision: "time-travel queries free") |
| Embedded, single file | ✅ | Matches the current "one index file" UX; no server |
| License MIT/Apache-2.0 | ✅ | `cargo deny` clean |
| Not SQLite / not a graph DB | ✅ | Honours the Charter spirit after the amendment |

Alternatives weighed: **LMDB (`heed`)** — excellent, but C dependency (Windows/cross-compile
risk) → rejected on the same ground that bit us before. **`sled`** — pure Rust but
effectively unmaintained / beta-stalled → rejected. **Build our own mmap engine** — the
founder's "Option B"; maximal control but re-invents redb at large cost and risk →
not chosen.

---

## 4. Design

### 4.1 Logical model unchanged

`Store` keeps its public API. Internally, instead of holding everything in `HashMap`s and
serializing the whole thing, it reads/writes redb tables. The current in-RAM types
(`Trunk`, `Synapse`, `AdjacencyList`) become **caches over** redb, not the source of truth.

### 4.2 Table schema (redb tables)

All keys/values are MessagePack-encoded (`rmp-serde`) unless a native redb type is simpler.

| Table | Key | Value | Replaces |
|---|---|---|---|
| `trunk_id_to_path` | `NodeId` (u64) | path `String` | `Trunk.by_id` |
| `trunk_path_to_id` | path `String` (ordered) | `NodeId` | trie lookups + **prefix scans via range queries** |
| `kind_map` | `NodeId` | `NodeKind` | `Store.kind_map` |
| `span_map` | `NodeId` | `SourceSpan` | `Store.span_map` |
| `synapse_fwd` | `(EdgeKind, NodeId)` | `Vec<NodeId>` | `AdjacencyList.forward` |
| `synapse_rev` | `(EdgeKind, NodeId)` | `Vec<NodeId>` | `AdjacencyList.reverse` |
| `file_index` | file path | list of `NodeId` owned by that file | enables O(changed) per-file rewrite (the R2 unit) |

Prefix/structural Hyphae queries that today walk the radix trie are served by **ordered
range scans** on `trunk_path_to_id` (redb keeps keys sorted). A small bounded in-memory
trie MAY be kept as a hot cache, rebuilt lazily — but it is no longer required for
correctness, so it can never cause an OOM.

### 4.3 Read path

- Open the redb file (mmap). Point lookups and range scans hit the B-tree; the OS page
  cache holds hot pages; cold pages are evicted by the kernel. **Resident RAM is bounded
  by the page cache, not by graph size** → R3.
- Query results are **semantically identical** to today (same NodeIds, same edges) — the
  R1/R-series equivalence contract.

### 4.4 Write path (the R2 hot path)

- A file change (watch mode, RFC-0008) opens **one redb write transaction**:
  1. read `file_index[path]` → old NodeIds for that file
  2. delete those nodes/edges from the tables (using `synapse_rev` to find back-edges)
  3. insert the freshly-extracted nodes/edges
  4. update `file_index[path]`
  5. `commit()` — ACID, atomic, crash-safe
- Cost = **O(symbols+edges in the changed file)**, not O(total graph) → R2.
- Cross-file edges: owned by the **source** file (the edge's tail), exactly the ownership
  rule RFC-0098 §"Edge ownership" already worked out — carried over verbatim.

### 4.5 Memory & crash safety

- **Memory:** bounded by the OS page cache. No `--max-rss` knob needed for correctness; an
  optional advisory cap can hint `madvise`. RFC-0099's streaming-index spill becomes
  unnecessary — the indexer writes straight into redb in batched txns, so index-time peak
  is also bounded.
- **Crash safety:** redb's copy-on-write commit is atomic; a crash mid-write leaves the
  last committed state intact. This **deletes** RFC-0098's hand-rolled append+fsync+CRC
  tail-truncation logic — we inherit a tested implementation instead.

### 4.6 Time-travel (bonus, future)

redb read transactions are MVCC snapshots. Periodic named savepoints give cheap historical
reads — the original Charter "time-travel queries free" goal becomes reachable without HAMT
gymnastics. Out of scope for v1; noted as the upside of this foundation.

---

## 5. Migration & Compatibility

- Existing on-disk index is a single `rmp-serde` snapshot (e.g. `index.rmp`). A one-time
  **importer** reads it and bulk-loads a new `index.redb` in batched write txns.
- `Store::load` detects the format by magic/extension: `.redb` → open directly; legacy
  `.rmp` → import once, then use redb. Legacy read support kept for **one minor release**,
  then removed.
- `Store::save` semantics change: there is no longer a "write the whole thing" call in the
  hot path; explicit `save` becomes a `commit`/checkpoint. The public method name may stay
  for compatibility (calls `commit`).
- **New CLI surface:** `mycelium migrate` (rmp → redb). Per the **Three-Surface Rule**
  (RFC-0090) this ships as CLI + a byte-identical MCP `migrate` tool + coverage in a
  category Skill. (The only new surface this RFC introduces.)

---

## 6. Detailed rollout (phased, each shippable, default unchanged until flip)

1. **Phase 1 — backend seam.** Introduce a `StorageBackend` trait; current behaviour
   becomes the `InMemory` impl. Add `redb` dependency (`cargo deny` + `cargo audit` pass);
   implement a `Redb` impl behind a `redb-backend` cargo feature (off by default). TDD:
   the existing store test-suite runs green against **both** backends.
2. **Phase 2 — equivalence.** Property/parity tests: for a corpus of repos, building via
   `InMemory` and via `Redb` yields semantically-equal graphs and identical query results
   (the R-series equivalence contract). CI matrix runs both.
3. **Phase 3 — flip default + migrate.** Default backend → `Redb`. Ship `mycelium migrate`
   + legacy `.rmp` auto-import. Watch mode now does per-file write txns (R2 live).
4. **Phase 4 — retire snapshot path.** Remove the full-graph `save`/`load` rmp snapshot
   code (rmp-serde remains as the in-value encoder). Close out RFC-0098/0099 mechanisms.

`#356` (`Store::heap_size_estimate` + RSS-curve tests) is **kept** — repurposed as the
**before/after baseline** that proves redb actually bounds resident memory (Phase 2 gate).

---

## 7. Acceptance Criteria

- [ ] **ADR** in `docs/adr/` (schema, value encoding, migration, crash-recovery) — merged
      before any Phase 3 code.
- [ ] Charter §3 amended (this RFC's §2 text) with founder sign-off recorded.
- [ ] `redb` added; `cargo deny check` + `cargo audit` green; license clean.
- [ ] Phase 1: `StorageBackend` trait + `InMemory` + `Redb` impls; full store test-suite
      green against both (TDD, RED first for new trait tests).
- [ ] Phase 2: parity test proves `Redb` graph ≡ `InMemory` graph (nodes, edges, query
      results) across a multi-repo corpus; CI runs both backends.
- [ ] Phase 2: `#356` baseline shows resident RSS under `Redb` stays bounded while indexing
      a graph that OOMs the `InMemory` backend.
- [ ] Phase 3: per-file write txn is **O(changed file)** (bench: edit one file in a large
      repo, measure write I/O ≪ full-snapshot).
- [ ] Phase 3: `mycelium migrate` on CLI **and** byte-identical MCP tool **and** in a
      category Skill (Three-Surface Rule).
- [ ] Phase 3: crash-injection test — kill mid-commit, reopen, graph is last-committed and
      consistent.
- [ ] Phase 4: legacy snapshot path removed; CHANGELOG documents the format change.

---

## 8. Risks & Mitigations

| Risk | Severity | Mitigation |
|---|---|---|
| New dependency surface / supply chain | Medium | redb is pure Rust, single well-known author, MIT/Apache; `cargo deny`+`audit` in CI; pin version |
| redb maturity vs LMDB | Medium | redb is 2.x stable, widely used; parity tests + crash-injection tests gate the flip; legacy path kept one release as fallback |
| Migration data loss | High | One-time importer is read-only on the old file; verify node/edge counts match before deleting `.rmp`; keep `.rmp` until first successful redb open |
| Query latency regression on cold pages | Medium | mmap cold-fault p99 measured in Phase 2; hot trie cache optional; warm-up scan on open if needed |
| On-disk format churn between redb versions | Medium | Pin redb; treat redb-format bumps as a migration event with the same importer pattern |
| Scope creep (rewrites everything) | Medium | Phased behind a feature flag; default unchanged until Phase 3; each phase independently revertible |

---

## 9. What this closes

- **RFC-0098** (incremental persistence) → **Superseded.** Its edge-ownership rule and
  per-file unit are absorbed into §4.4; its hand-rolled journal/fsync/CRC are replaced by
  redb ACID.
- **RFC-0099** (bounded resident memory) → **Superseded.** Its measurement phase lives on
  as `#356`; its streaming-index + LRU-eviction mechanisms are made unnecessary by mmap.
- **Issues #343, #344** → both addressed by this single layer; they remain open as the
  tracking issues for the phased delivery.

---

## 10. References

- redb — https://github.com/cberner/redb (pure-Rust mmap B-tree KV store)
- RFC-0006 Persistence, RFC-0098 Incremental Persistence, RFC-0099 Bounded Resident Memory
- RFC-0008 Watch Mode (the per-file write trigger), RFC-0090 Three-Surface Rule
- Charter §3 (Tech Stack, locked) — amended by this RFC
- Founder decision 2026-05-31: "允许引入 redb（方案 A）" — best performance + flexibility,
  accept relaxing the "own-engine" rule.

*One layer to make writes incremental and memory bounded — by standing on a tested
memory-mapped B-tree instead of hand-building two mechanisms that approximate it.*
