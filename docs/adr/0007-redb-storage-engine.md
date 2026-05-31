# ADR-0007: redb as the Unified On-Disk Storage Engine

## Status

Accepted — founder decisions recorded 2026-05-31 (see **Founder Decisions** below);
implementation may begin per the execution plan. Supersedes the hand-rolled single-blob
MessagePack snapshot for the on-disk *graph* and retires the durability scheme of
RFC-0098. Governs RFC-0100; resolves #343 (incremental writes / R2) and #344
(bounded memory / R3). Depends on #356 (`Store::heap_size_estimate` + RSS-curve
harness) as a prerequisite for the R3 acceptance gate.

Builds on ADR-0005 (MessagePack wire format): MessagePack remains the value
encoder *inside* redb. This ADR changes the *container* and *residency*, not the
logical model.

## Founder Decisions (2026-05-31)

Resolving the 8 open questions. Guiding principle from the founder: **Mycelium is
not yet officially launched — there are no production indexes to protect — so we
"稳中求创新" (innovate steadily): keep engineering rigor, but shed conservative
backward-compat baggage we don't owe anyone yet.**

1. **Schema → the canonical 8-table separate-table schema.** Rejected the
   consolidated nodes-blob. Rationale: the hot path (3-hop traversal) touches only
   adjacency; separate tables keep span/kind data out of the traversal's mmap
   working set, protecting the Charter §2 `<1ms` 3-hop SLA.
2. **Adjacency values → SORTED `Vec<u64>`.** Accepted the behavior change.
   Determinism is *required* by the cross-backend equivalence contract; sorted
   lists also delta+varint-compress → smaller values → fewer page faults. (Edge
   order is already non-deterministic post-R1, so nothing real is lost.)
3. **Migration RAM → minimal full-RAM importer + `--reindex` as the primary
   escape; NO streaming importer.** *Adjusted for not-launched:* migration of any
   pre-redb `.rmp` is **best-effort dev-continuity only**, not a supported user
   path. Full-RAM import costs the same as today's `load` (no regression);
   genuinely-too-large repos use `--reindex` (bounded rebuild from source).
4. **Cold-start SLA → split Charter §2 into warm (steady-state, existing numbers)
   + a separate cold-open budget; do a warm-up scan on open; the exact cold number
   is set from the T1 mmap cold-page spike, NOT guessed.** mmap cold pages are disk
   reads — `<1ms` 3-hop is physically impossible cold. We measure, then commit the
   number to Charter. This is the one place "毫无压力 on 100K files" must be earned
   with data.
5. **redb version → no hard `=` pin.** Cargo.lock pins the tested version;
   Cargo.toml allows patch updates; **every redb bump must pass a CI gate that opens
   an old-format index file.** Hard-pinning would block security patches.
6. **`migrate` Skill → `skills/index-management/SKILL.md`; CLI + MCP + Skill rows
   land in the SAME PR** (RFC-0090 parity gate). No tradeoff; just correct.
7. **Legacy `.rmp` reader → auto-migrate on first open, then DROP the legacy reader
   entirely.** *Adjusted for not-launched:* no multi-release retention window is
   owed. The one-shot importer runs transparently on first open of an old file;
   after the Phase-3 flip the permanent read path is redb-only. Bolder + cleaner,
   safe because there are no real users to strand.
8. **Dev≠QA → approved.** The test-author (QA role) writes ALL suites (unit,
   equivalence, crash, perf, mutation) RED-first, BEFORE the implementer writes
   production code. Founder non-negotiable; also correct TDD.

**Innovation hook (steady):** redb read transactions are MVCC snapshots, so
near-free **time-travel queries** (the original Charter vision) become reachable.
Not in v1 scope, but the schema and `StorageBackend` trait are designed so a later
RFC can add it without a format break. Design the hooks now; build later.

**Stability anchor:** the `StorageBackend` trait + the `InMemory` backend are kept
**not** for backward compat but as (a) a fast unit-test path and (b) the parity
*oracle* for the equivalence tests. This is what lets us innovate on the redb path
without flying blind.

## Context

Mycelium persists its graph (`Trunk` + `Synapse` + `kind_map` + `span_map`) as a
single rmp-serde blob written by `Store::save` (`store/mod.rs:~421`) and read by
`Store::load` (`store/mod.rs:~439`). This has two structural defects the project
has filed as issues:

- **R2 / #343 — no incremental write.** Re-indexing one changed file rewrites the
  entire on-disk blob. Cost is O(graph), not O(changed-file). Watch mode re-saves
  everything on every keystroke-triggered re-index.
- **R3 / #344 — no bounded memory.** The whole graph is materialized in RAM
  (`HashMap`-backed `Trunk.by_id`, `Synapse.by_kind`, `kind_map`, `span_map`).
  Resident set grows linearly with graph size; large repos (100K-node target,
  Charter §2) OOM.

The founder amended Charter §3 to permit `redb` (pure-Rust, mmap, ACID,
copy-on-write B-tree) specifically to unify these. The logical model is unchanged:
`NodeId(u64)` is `BLAKE3(trunk_path)` truncated — a content hash, storage- and
order-independent. Only the on-disk byte layout and residency change.

Three independent reviews (governance, performance-correctness, migration-safety)
converged on the same set of load-bearing facts that MUST shape the design. The
following were re-confirmed against live source and are now design constraints,
not open questions:

1. **Path separator is `>` (U+003E, 0x3E)** (`trunk/path.rs:33`), NOT `::`. It
   sorts *below* ASCII letters (0x41/0x61) but *above* digits and several
   symbols. A naive `range(prefix..)` byte scan on `&str` keys does **not** give
   tight parent/child/sibling boundaries. (See Decision §3.)
2. **`NodeId` is `#[repr(transparent)]` but NOT `#[serde(transparent)]`**
   (`types.rs:20-22`). Under rmp-serde it currently encodes as a 1-element
   sequence, not a bare `u64`. (See Decision §4.)
3. **`EdgeKind` is `#[non_exhaustive]`** (`types.rs:166-167`) with 16 variants.
   Rust discriminants are not stable under reordering. (See Decision §5.)
4. **`AdjacencyList::remove_node` is bidirectional** (`synapse/mod.rs`): it cleans
   both forward and reverse, including edges *owned by other files* that point
   into the removed node. The per-file write transaction MUST replicate both
   halves. (See Decision §6.)
5. **A NodeId may be REFERENCED by edges from many files but is OWNED (in the node
   tables) by exactly one file** — its trunk path is BLAKE3 of the fully-qualified
   path, which embeds the owning file. No node-level ref-counting is needed; edge
   ownership follows RFC-0098's source-owns rule. (See Decision §6.)
6. **`resolve_bare_call_stubs` is a full-graph batch pass** (`cli/index.rs`) not
   run by the incremental watch path (`cortex.rs apply_to_store`). This is a
   pre-existing batch≠incremental gap that the equivalence tests will surface and
   the ADR must scope. (See Consequences.)

## Decision

Adopt `redb` as the single on-disk graph store, behind a `StorageBackend` trait
with two implementations (`InMemoryBackend` = today's behavior + differential
oracle; `RedbBackend` = mmap-resident). Roll out in four cfg-gated phases
(`redb-backend` feature, default OFF until Phase 3). MessagePack (rmp-serde) stays
as the in-value encoder; redb keys use hand-rolled order-preserving byte
encodings.

### 1. Canonical table schema (resolves the 3-way schema conflict)

We adopt the **separate-table** schema (Component 1 shape), explicitly rejecting
the consolidated `nodes`-blob schema (Component 2), because kind-only and
span-only lookups (`symbols_of_kind`) must remain single-value decodes rather than
full-payload decodes. The canonical, ADR-locked set is **8 tables** in one
`graph.redb` file:

| Table | Key | Value | Replaces |
|---|---|---|---|
| `trunk_id_to_path` | `u64` (NodeId.0) | `&str` (path) | `Trunk.by_id` |
| `trunk_path_to_id` | encoded path bytes (see §3) | `u64` (NodeId.0) | `TrieNode` prefix index |
| `kind_map` | `u64` | `&[u8]` = rmp(NodeKind) | `Store.kind_map` |
| `span_map` | `u64` | `&[u8]` = rmp(SourceSpan) | `Store.span_map` |
| `synapse_fwd` | `(u16 kind_tag BE)++(u64 src BE)` | `&[u8]` = rmp(sorted Vec<u64>) | `AdjacencyList.forward` |
| `synapse_rev` | `(u16 kind_tag BE)++(u64 dst BE)` | `&[u8]` = rmp(sorted Vec<u64>) | `AdjacencyList.reverse` |
| `file_index` | `&str` (source file path) | `&[u8]` = rmp(FileEntry) | (new — R2 enabler) |
| `meta` | `&str` | `&[u8]` = rmp(value) | (new — version guard) |

`FileEntry { nodes: Vec<u64>, edges: Vec<(u16 kind_tag, u64 src, u64 dst)> }`.
We store **edge triples** in `file_index` (Component 2 choice), because per-file
delete is then O(edges-of-file) with no per-EdgeKind probing, and because the
synapse value-merge correctness (Decision §6) requires knowing exactly which
edges this file owns.

### 2. Value encoding rule (rmp-in-value; never rmp-in-key)

- **Values** are rmp-serde (MessagePack) byte blobs — reuses existing
  `Serialize`/`Deserialize` derives. This preserves ADR-0005.
- **Keys** are NEVER MessagePack (varint length variance destroys byte order).
  Integer keys use big-endian fixed width; path keys use the §3 encoding.

### 3. Path-key ordering (resolves the `>` / 0x3E defect)

Because `>` (0x3E) does not sort below all identifier bytes, descendant range
scans on a raw `&str` path key are NOT tight. The locked encoding for
`trunk_path_to_id` keys:

- Substitute the separator `>` with the byte **`0x00` (NUL)** in the redb key
  only (display/in-memory representation keeps `>`). NUL sorts before every
  printable byte, so for any path P, all descendants of P sort contiguously after
  `P_encoded ++ 0x00`, and the exclusive upper bound is `P_encoded ++ 0x01`.
  This gives tight, provably-correct parent/child/sibling boundaries independent
  of the identifier character set.
- Path segments are validated to contain no `0x00` byte at the boundary (they
  cannot today — `>`/0x3E was the only structural char). A boundary test asserts:
  index `a>b`, `aZ>c`, `a>b>d`; a prefix scan for `a>` returns exactly `a>b` and
  `a>b>d`, never `aZ>c`. This test is written RED first.

### 4. NodeId serialization (resolves the non-transparent defect)

Add `#[serde(transparent)]` to `NodeId` so it round-trips as a bare `u64` in
MessagePack values (`Vec<NodeId>` payloads halve in size; value-decode matches the
u64 key codec). Because the legacy rmp snapshot was written WITHOUT transparent,
this is a wire-format change for the legacy reader too — handled by the migration
importer reading via the *current* (non-transparent) `Store::load` before the
attribute is added, OR by gating the importer to decode with the pre-change shape.
A regression test asserts
`rmp_serde::to_vec(&NodeId(42)) == rmp_serde::to_vec(&42u64)`.

### 5. EdgeKind → stable tag (resolves the non_exhaustive defect)

Add `const fn as_tag(self) -> u16` with **explicit, hand-assigned constants per
variant name** (NOT discriminant order), living in a single file
(`store/redb_tags.rs`) marked `WIRE FORMAT — append only, never renumber`. `u16`
(not `u8`) for headroom past 16 variants. A compile-time count assertion fails if
a new `EdgeKind` variant lacks a tag. The tag→name table version is written into
`meta` at DB creation. Tests assert each variant's tag is constant.

### 6. Per-file write transaction = the R2 contract (crash-safe, correct)

Re-indexing file `f` happens in ONE redb `WriteTransaction` (atomic, durable on
`commit()`):

1. Read `file_index[f]` → old `FileEntry`. Absent = first index.
2. **Bidirectional edge delete** (replicating `AdjacencyList::remove_node`): for
   each old edge `(k, src, dst)` owned by `f`: remove `dst` from
   `synapse_fwd[(k,src)]` AND remove `src` from `synapse_rev[(k,dst)]`. For each
   old node `n` of `f`, also scan `synapse_rev[(k,n)]` to find *external* sources
   pointing into `n` and strip `n` from their `synapse_fwd` rows, and vice-versa.
   Work is O(edges touching f's nodes in either direction).
3. **Synapse value-merge, not overwrite.** When rewriting `synapse_fwd[(k,src)]`
   for a `src` that `f` owns, the new value = (existing value − f's OLD dsts for
   that key) ∪ (f's NEW dsts). This prevents dropping edges from the same `src`
   owned by another file. Values are stored **sorted by u64** so both backends are
   order-deterministic (also fixes order-sensitive query equivalence and makes BFS
   deterministic).
4. Delete old nodes exclusively owned by `f` from `trunk_id_to_path`,
   `trunk_path_to_id`, `kind_map`, `span_map`. (Ownership is 1:1 with trunk path;
   no ref-counting.)
5. Insert new nodes/edges; write the new `FileEntry` to `file_index[f]`; bump
   `meta` counts.
6. `commit()` — the single durability point.

### 7. Crash-recovery contract (retires RFC-0098's hand-rolled scheme)

redb's copy-on-write B-tree with two-phase commit (write pages → fsync → flip
committed root → fsync) subsumes RFC-0098's append-log + CRC + manual fsync. A
crash leaves the previous committed root intact: the reopened graph equals EITHER
the last-committed state OR the prior-committed state — never a torn third state.
No recovery scan, no CRC trailer. RFC-0098's *edge-ownership rule* (source owns the
edge) is RETAINED as the correctness basis for step 6.2/6.3. Invariants asserted on
every reopen: file opens clean; NodeId set ∈ {last, prev} committed; no dangling
adjacency (every referenced NodeId exists in `trunk_id_to_path`); reopen is
idempotent.

### 8. Migration protocol (`mycelium migrate`, Three-Surface)

- Importer reuses the existing tested `Store::load` legacy decoder → in-memory
  `Store` → writes 8 tables in redb write txn(s) → reconstructs `file_index` by
  assigning each edge to its source node's owning file (first path segment before
  the first `>`), since the legacy format carries NO per-file ownership.
- Build into sibling `graph.redb.tmp`; reopen read-only and verify node/edge
  counts + content checksum equal the pre-import in-memory model; on mismatch
  abort and delete tmp (original rmp untouched). fsync tmp, then atomic rename.
  Retain legacy `.rmp` (never delete). Rename-failure path deletes partial tmp,
  reports both paths, and is safely re-runnable without `--force`. Use an
  exclusive `Database::create` on the tmp/final path to close the TOCTOU race
  between concurrent `migrate` runs.
- **RAM honesty:** the importer materializes the full legacy graph in RAM, which a
  graph that already OOMs cannot afford. `mycelium migrate` therefore offers
  `--reindex` (rebuild from source via the redb writer, bypassing the rmp load —
  the RECOMMENDED path for large repos) and, for the in-RAM path, fails fast with
  a clear message if estimated heap exceeds available RAM. Documented in `--help`.
- **Format detection** in `Store::load`: probe `redb::Database::open` (redb has its
  own magic) → on redb invalid-file fall back to legacy rmp → else
  `StoreError::UnknownFormat`. Legacy read kept for exactly one minor release past
  Phase 3, emitting a deprecation `warn!` per legacy load.
- **Version guard:** `meta` stores (a) Mycelium schema version and (b) the redb
  library major version. On open, a mismatch yields a named
  `StorageError::SchemaVersion { found, expected }` mapped at the binary boundary
  to: "Index was written by mycelium vX.Y / redb vN; run `mycelium migrate
  --force` or install a matching mycelium." Never surface a raw redb error.
  `redb` is pinned to an exact version (`=2.x.y`) in Cargo.toml; bumps are a
  tracked, lessons.jsonl-logged migration event.
- **Three-Surface (RFC-0090):** `migrate` ships as CLI + byte-identical MCP tool +
  a row in `skills/index-management/SKILL.md` `allowed-tools` AND `skills/INDEX.md`
  — all in the SAME PR (the parity check is a required gate; a one-PR orphan blocks
  all merges). `check_skill_parity.py` is made feature-aware so `migrate` is not
  flagged as an orphan while `redb-backend` is OFF in Phases 1–2.

### 9. Trait shape & dispatch (resolves object-safety bug)

`StorageBackend` is **synchronous**; async callers use
`tokio::task::spawn_blocking`. To keep `Box<dyn StorageBackend>` object-safe, there
is NO associated `Write` type: `begin_write` returns a concrete type-erased
`WriteHandle` (enum over the two backends, or `Box<dyn WriteTransaction>`). The
public async surface is an `AsyncBackendHandle` newtype exposing only
`spawn_blocking`-wrapped `async fn`s; the raw sync `RedbBackend` is crate-private so
the reactor cannot be blocked accidentally. `StorageError` via `thiserror` at the
lib boundary; `anyhow` only at the binary boundary.

## Consequences

**Positive**
- O(changed-file) incremental writes (#343); bounded resident memory via mmap
  paging (#344); ACID durability replaces hand-rolled fsync/CRC; deterministic
  sorted adjacency makes BFS/path queries reproducible across backends.

**Negative / risks accepted**
- mmap cold-page faults create a first-query / p99 tail. Charter §2 SLA is
  annotated as **warm-cache steady-state**; a separate cold-start budget
  (first-query < 100ms) is added, and a startup warm-up (sequential scan of
  `synapse_fwd`/`synapse_rev`) is provided. Phase 2 CI measures cold p99.
- Migration peak RAM equals legacy graph size; `--reindex` is the escape hatch.
- `redb` major-version on-disk format churn is a supply-chain risk; mitigated by
  exact pin + version guard + friendly error.
- **Batch≠incremental gap:** `resolve_bare_call_stubs` runs only in batch index,
  not in the watch/incremental path. This pre-existing gap is documented, not
  fixed here: watch mode will not resolve NEW cross-file call stubs until the next
  full re-index. The equivalence test's "incremental ≡ batch" assertion is scoped
  to same-file edges; cross-file resolved edges are excluded from the incremental
  comparison.
- A widely-imported file (many incoming cross-file edges) has write amplification
  > O(changed-file) in redb reads during bidirectional cleanup; measured in
  Phase 2 benches.

## Alternatives Considered

- **Keep the rmp single-blob.** Rejected: cannot meet R2 or R3; the whole reason
  for this ADR.
- **SQLite / an embedded graph DB.** Forbidden by Charter §3.
- **Consolidated `nodes` blob table (path+kind+span as one value).** Rejected:
  regresses kind-only/span-only scans to full-payload decodes.
- **Row-per-edge `(kind,src,dst)→()` instead of `Vec<dst>` values.** Deferred as
  an escape hatch for pathological fan-out; chosen `Vec<dst>` value-merge makes the
  rewrite unit match the re-parse unit and keeps reads to one decode.
- **Async `StorageBackend` trait.** Rejected: lies about redb's sync mmap/CPU cost
  model, infects the graph layer with `async`, risks lock-across-await.
- **Generic `Store<B>` instead of `Box<dyn>`.** Kept as an escape hatch for hot
  paths if benchmarks demand monomorphization; default is `dyn` for runtime
  backend selection.
- **`u8` EdgeKind tag.** Rejected: only 256 slots for a `#[non_exhaustive]` enum;
  `u16` chosen.
- **Raw `>` path keys.** Rejected: 0x3E ordering is not tight; NUL-separator key
  encoding chosen.