# RFC-0001: Trunk + Synapse Storage Layer

- **Status**: draft
- **Author(s)**: @aimasteracc (founder), spec-author (drafted)
- **Created**: 2026-05-28
- **Last updated**: 2026-05-28
- **Tracking issue**: TBD (open after merge)
- **Affected source paths**:
  - `crates/mycelium-core/src/trunk/`
  - `crates/mycelium-core/src/synapse/`
  - `crates/mycelium-core/src/store/`

## Summary

Mycelium's storage core consists of two interlocked layers:

- **Trunk** — a Materialized-Path Radix Trie holding the containment tree of code (file → class → method, etc.). Path-encoded node IDs make ancestor/descendant queries O(prefix), not O(graph walk).
- **Synapse** — per-edge-kind Compressed Sparse Row (CSR) adjacency lists holding cross-cutting relationships (calls, extends, implements, …). Forward and reverse adjacency are both materialized; `callers` queries become O(degree) lookups.

Together they replace what SQL-based code graphs do with N JOINs and what graph databases do with full graph engines. The design predicts:

- Cold lookup < 5 ms on 100k nodes (Charter §2 row 1)
- 3-hop traversal < 1 ms on 100k nodes (Charter §2 row 2)

## Motivation

Existing code graphs fall into two camps:

- **SQL-based** (codegraph, Sourcegraph's symbol service): cheap to embed, slow for multi-hop traversal (each hop is a JOIN).
- **Graph-DB-based** (Neo4j, FalkorDB): fast multi-hop traversal, but heavy runtime, network overhead, and large cold-start latency for the simple lookups that dominate AI agent traffic.

AI agents querying a code graph have a specific access pattern:

1. **90% of queries are "find X + look 1–3 hops"**
2. **Cold-start latency matters**, because each query is independent
3. **Updates are frequent and small** (file change → a few nodes change)

Neither incumbent is optimized for this pattern. This RFC proposes a custom storage core that is.

## Detailed design

### Trunk — containment as a radix trie

Code containment is naturally a tree:

```
src/auth.ts                          (file)
├── AuthService                      (class)
│   ├── login(email, password)       (method)
│   │   ├── validate                 (local fn)
│   │   └── generate_token           (local fn)
│   └── logout()                     (method)
└── helper()                         (function)
```

We encode this with **path strings using `>` as separator**:

```
src/auth.ts
src/auth.ts>AuthService
src/auth.ts>AuthService>login(email,password)
src/auth.ts>AuthService>login(email,password)>validate
```

Stored in a **Radix Trie** (prefix-compressed):

```
"src/auth.ts" ─┐
               ├─ ">AuthService" ─┐
                                  ├─ ">login(email,password)" ─┐
                                                               ├─ ">validate"
                                                               └─ ">generate_token"
               └─ ">helper"
```

**Operations and complexity:**

| Op | Complexity |
|---|---|
| Insert | O(L) where L = path length |
| Exact lookup | O(L) |
| All descendants | O(K) where K = output size |
| All ancestors | O(D) where D = depth |
| Rename a subtree | O(K) — single prefix mutation; with HAMT structural sharing, old version free |

### Trunk node payload

Each leaf or interior node in the trie holds:

```rust
pub struct TrunkNode {
    pub id: NodeId,             // u64 — interned from the path
    pub kind: NodeKind,         // function | class | method | ...
    pub language: Language,
    pub span: SourceSpan,       // start_line, end_line, start_col, end_col
    pub attrs_row: AttrRowId,   // pointer into Apache Arrow column store
}
```

Heavy attributes (signature, docstring, decorators, visibility, etc.) live in
the **column store** (Apache Arrow). This keeps the trie node tiny and
cache-friendly, while making "find all `async` methods" a vectorized scan
over a boolean column rather than a row-by-row test.

### Synapse — cross-cutting edges as CSR

UML and code intelligence need a wide vocabulary of relationships:

- `calls` — function/method calls another
- `extends` — class extends class
- `implements` — class implements interface
- `references` — generic name reference
- `type_of` — variable has type
- `returns` — function returns type
- `instantiates` — `new T(...)`
- `overrides` — method overrides parent
- `decorates` — decorator applied
- `uses` (UML) — uses another component
- `aggregates` (UML) — weak ownership
- `composes` (UML) — strong ownership
- `realizes` (UML) — interface realization

Each edge kind is stored as a **separate pair of CSR arrays** (forward + reverse):

```
synapse/
├── calls/
│   ├── forward.csr:
│   │     row_offsets: [0, 2, 3, 3, 5, ...]    // u32
│   │     targets:     [42, 99, 7, 18, 200]    // NodeId
│   │     meta_offsets: [0, 0, 1, 1, 1, 3]     // pointer to per-edge metadata
│   └── reverse.csr:    (same shape, target → sources)
├── extends/
│   ├── forward.csr
│   └── reverse.csr
└── ...
```

**Operations and complexity:**

| Op | Complexity |
|---|---|
| Outgoing `calls` from N | O(degree) — single contiguous array scan |
| Incoming `calls` to N (= callers) | O(degree) — uses reverse.csr |
| K-hop traversal of one edge kind | O(visited × avg-degree) — pure array hopping |
| Mixed-kind traversal | parallel CSR scans, then union |

CSR is the format the graph algorithms research community converges on for
high-performance traversal. It is also what GraphBLAS specifies. Neo4j's
native store is *not* CSR — it interleaves node properties, edge properties,
and topology, which is why CSR-on-attrs-elsewhere wins on pure topology
walks.

### Storage on disk

A single `.myc` file contains:

```
[header:           magic, version, schema fingerprint]
[trunk:            serialized radix trie]
[synapse:          one (forward, reverse) CSR pair per edge kind]
[attrs:            Arrow IPC for each NodeKind's attribute schema]
[id-index:         qname → NodeId, name → NodeId[]]
[WAL tail:         append-only since last snapshot]
```

The WAL holds incremental updates. On clean shutdown or periodic checkpoint,
the WAL is folded into the snapshot.

### NodeId scheme

`NodeId = u64`. Interned from the materialized path via a **monotonic
counter** + **content-hash** for stability:

```
NodeId = ((hash(path) & 0x00FF_FFFF_FFFF_FFFF) << 8) | shard_id
```

Stable across runs (same path → same ID), unique within a database, and
small enough for tight CSR encoding.

### Reactivity (forward reference to a later RFC)

Trunk and Synapse are passive stores. The reactive layer (Salsa, to be
designed in a separate RFC) sits on top, subscribing to mutations and
invalidating dependent queries. This RFC pins the data structures; RFC-0002
will pin the reactivity layer.

### Public API sketch

```rust
pub struct Store {
    trunk: Trunk,
    synapse: SynapseSet,
    attrs: AttrStore,
    name_index: NameIndex,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, StoreError>;
    pub fn create(path: &Path) -> Result<Self, StoreError>;

    pub fn upsert_node(&mut self, node: NodeInsert) -> NodeId;
    pub fn upsert_edge(&mut self, e: EdgeInsert);
    pub fn remove_node(&mut self, id: NodeId);
    pub fn remove_file(&mut self, path: &str);

    pub fn lookup(&self, qpath: &str) -> Option<NodeId>;
    pub fn descendants(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_;
    pub fn ancestors(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_;

    pub fn outgoing<'a>(&'a self, id: NodeId, kind: EdgeKind) -> &'a [NodeId];
    pub fn incoming<'a>(&'a self, id: NodeId, kind: EdgeKind) -> &'a [NodeId];

    pub fn checkpoint(&mut self) -> Result<(), StoreError>;
}
```

## Drawbacks

1. **Single-file binary format**: requires a debugger/inspector tool. We will ship `mycelium inspect`.
2. **Rename cascades touch many trunk nodes**: a class rename rewrites every descendant's path. Mitigated by structural sharing (HAMT) so old snapshots are still O(1) accessible.
3. **CSR is not great for very small graphs**: overhead dominates. We accept this — 100k-node graphs are the design center.
4. **Custom storage means we own all the bugs**. No SQLite or RocksDB to blame. Mitigation: aggressive fuzzing and snapshot testing from day one.
5. **NodeId stability requires care under refactors**: if a file is renamed `src/a.ts` → `src/b.ts`, all descendant NodeIds change. We provide a `mycelium remap` command for explicit cross-rename updates.

## Alternatives

### Alternative A: SQLite + closure table

Used by codegraph. Closure table makes ancestor queries fast (one JOIN), but
inserts are O(D) per node and edges-as-rows means traversal is N JOINs.

**Rejected because**: cold-query latency dominated by SQLite cursor setup
(~50 µs floor), and 3-hop traversal benchmarks 20-50× slower than our CSR
prediction.

### Alternative B: Neo4j embedded

Native graph engine. Excellent for deep traversal. But cold start is in the
hundreds of milliseconds, JVM-based, and ~50× memory of CSR.

**Rejected because**: embedded JVM is a non-starter for our single-binary
goal; runtime weight kills the AI-agent use case.

### Alternative C: DuckDB with experimental graph extension

Columnar SQL with graph extension. Closer to what we want than SQLite, but
graph extension is still maturing, and we'd be a major dependency on
DuckDB's roadmap.

**Rejected because**: dependency risk and graph extension is not yet at the
performance we predict for CSR.

### Alternative D: RocksDB + custom indexing

LSM tree as the persistence, with hand-rolled trunk/synapse structures on
top. Reasonable middle ground.

**Rejected because**: the LSM compaction is unnecessary for our access
pattern (mostly read, infrequent batch writes), and a custom file format
lets us snapshot via mmap for near-zero startup.

## Prior art

- **codegraph** (TypeScript, MIT) — flat SQLite tables, FTS5 search, 2-phase resolution. Our trunk replaces the flat node table; our synapse replaces the flat edge table. We adopt their 2-phase extraction pattern (parse → unresolved refs → resolve).
- **Sourcegraph's SCIP** — protocol for storing code intelligence. We will support SCIP import as a bridge.
- **Apache Arrow** — columnar attribute storage is borrowed wholesale.
- **GraphBLAS / GAP Benchmark Suite** — CSR is the consensus high-performance layout.
- **rust-analyzer** — Salsa for reactivity; we will follow this for RFC-0002.
- **Persistent data structures (HAMT)** — Phil Bagwell's Hash-Array Mapped Trie for structural sharing; used in Clojure, Immer.

## Migration

This is the first storage RFC. No migration from a prior format. The
`.myc` v1 format declared here is the baseline; future migrations will be
explicit via the `schema fingerprint` in the header.

## Testing strategy

### Tests written before implementation

- `trunk::tests::insert_creates_addressable_path`
- `trunk::tests::lookup_distinguishes_exact_match_from_prefix`
- `trunk::tests::descendants_returns_in_dfs_order`
- `trunk::tests::ancestors_returns_in_root_to_leaf_order`
- `trunk::tests::rename_subtree_preserves_descendants`
- `synapse::tests::forward_outgoing_returns_targets`
- `synapse::tests::reverse_incoming_returns_sources`
- `synapse::tests::edge_kinds_isolated`
- `store::tests::roundtrip_open_close`
- `store::tests::wal_replay_after_crash`

### Property tests (proptest)

- For any sequence of upserts and removes, the resulting trunk is well-formed (no orphans, no duplicates).
- For any edge insert, both forward and reverse adjacency reflect it.
- For any sequence of operations, save + reopen yields identical query results (round-trip property).

### Snapshot tests (insta)

- Stable serialization of a 100-node, 200-edge sample graph.

### Benchmarks (criterion)

- `bench_trunk_lookup_100k` — SLA target < 5 ms
- `bench_synapse_3hop_100k` — SLA target < 1 ms
- `bench_store_open_cold` — should not exceed 50 ms cold open

### Fuzz targets

- `fuzz_trunk_path_parser` — invalid paths must not panic
- `fuzz_store_wal_replay` — corrupted WAL must error cleanly, not crash

### E2E

- Index the Mycelium repository itself with a Python pack + TS pack mounted, then run 10 representative queries; results stable via insta.

## Performance impact

| SLA | Current | After this RFC | Δ |
|---|---|---|---|
| Cold small query | n/a (no engine yet) | predicted < 1 ms (trunk lookup) | new baseline |
| 3-hop traversal | n/a | predicted < 0.5 ms (CSR scans) | new baseline |
| Reactive refresh | n/a | RFC-0002 sets this | n/a |
| Token efficiency | n/a | RFC-0003 (Hyphae) sets this | n/a |
| Index size on disk | n/a | predicted ~2× source size for ~10k-symbol codebases | new baseline |
| Memory footprint | n/a | predicted ~150 MB for 100k nodes (CSR + interned strings) | new baseline |

These are predictions to be confirmed by benchmark in implementation phase.

## Open questions

1. **NodeId collision under hash truncation**: at what database size does hash truncation to 56 bits become a concern? (Estimate: > 10^8 nodes. We accept and document the limit.)
2. **CSR rebuild cost on partial invalidation**: full rebuild per file change is too expensive. Need an incremental-update story (segmented CSR? log-structured CSR?). Possibly its own RFC.
3. **Concurrent reads during writes**: read-write lock per Store, or finer? Likely per-edge-kind RwLock plus an immutable snapshot for query side. To be confirmed in implementation.
4. **Snapshot cadence**: time-based, write-count-based, or both? Default 5 minutes + every 10k WAL ops; tunable.
5. **Endianness and portability**: `.myc` files cross-platform? Yes — little-endian only, alignment enforced. Cross-arch tested in CI matrix.

## Future possibilities

- **RFC-0002**: Reactive layer (Salsa-based) on top of this store.
- **RFC-0003**: Hyphae query language (CSS-selector-like) targeting this store.
- **RFC-0004**: AI-native Emmet-style serialization of query results.
- **RFC-0005**: Time-travel queries via HAMT-snapshot chains.
- **RFC-0006**: Distributed multi-machine indexing for monorepos > 10M LOC.

---

*Trunk and Synapse are the bones and ligaments. Cortex and Hyphae will be the nervous system.*
