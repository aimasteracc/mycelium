# ADR-0008: redb as the Default Storage Backend (Phase 3 Flip)

## Status

Accepted — RFC-0100 Phase 3, PR #448, v0.1.17 (2026-06-02).

> **Note on numbering:** `docs/adr/0007-redb-storage-engine.md` (the original
> redb architecture ADR, covering the schema, key encoding, and trait design)
> was accidentally filed as `0007` while `0007-cli-mcp-skill-parity.md` already
> held that number. The renaming of `0007-redb-storage-engine.md` to the correct
> `0008` is tracked separately. This document (`0008-redb-as-default-backend.md`)
> records the Phase 3 flip decision and is the ADR that satisfies the
> `architect | P1 | ADR-0008: redb as default backend` dispatch requirement.

## Context

RFC-0100 delivered `RedbBackend` behind a `redb-backend` Cargo feature flag over
three phases:

| Phase | What landed | Version |
|---|---|---|
| Phase 1 | `StorageBackend` trait + `InMemoryBackend` + `RedbBackend` skeleton | v0.1.16 |
| Phase 2 | Equivalence tests, crash-safety, per-file R2 incremental writes, O(1) edge-count cache, MCP watch persistence, SLA benchmarks, no-op guards | v0.1.16 |
| Phase 3 | **Flip `redb-backend` to default; deprecate legacy `.rmp` to dev-continuity only** | v0.1.17 |

Before Phase 3, every `mycelium index` / `mycelium serve` invocation materialized
the entire graph in RAM (`HashMap`-backed), rewrote the full blob on every
incremental change, and had no bounded-memory guarantee. This directly violated the
Charter §2 R2/R3 requirements at scale (100 K-node graph).

**Prerequisites satisfied before flipping the default (all from Phase 2 CI):**

1. **Equivalence tests green** — `proptest`-generated random graphs: `InMemoryBackend`
   output ≡ `RedbBackend` output for every operation (upsert, remove, replace_file,
   adjacency, k-hop, kind-of, span-of).
2. **Crash-safety proven** — `SIGKILL` mid-write reopens to either of the two last
   committed states; never a torn third state (redb copy-on-write B-tree guarantee).
3. **Warm latency de-risked (T1 spike, ADR-0007/0008)** — point lookup p99 = 916 ns
   (5,400× SLA headroom); 3-hop p99 = 5.4 µs (185× headroom) at 100 K nodes.
4. **Per-file write latency measured** — 10 K-node replace_file ≈ 9.37 ms after
   read-before-write + no-op guard optimisations; 100 K pending nightly gate.
5. **No-op guard in place** — identical re-index does not grow redb page footprint.
6. **MCP watch persistence wired** — watch batches persist only the changed files
   through `RedbBackend::replace_file_from_store`.

## Decision

In `crates/mycelium-core/Cargo.toml`, change:

```toml
[features]
default = []
redb-backend = ["dep:redb"]
```

to:

```toml
[features]
default = ["redb-backend"]
redb-backend = ["dep:redb"]
```

Effect: every binary built without explicit `--no-default-features` now uses
`RedbBackend`. `InMemoryBackend` remains compiled and available as the test oracle
and for `#[cfg(not(feature = "redb-backend"))]` paths.

Legacy soft migration: `Store::open` probes `redb::Database::open`; if it fails
(file is legacy `.rmp`), it falls back to `Store::load` (legacy rmp-serde decoder),
reads the graph into RAM, and writes it into a fresh `graph.redb` via the redb writer.
The original `.rmp` file is retained (never deleted). A single deprecation `warn!`
is emitted per legacy load. This path is removed after one minor release.

## Rationale

- **All Charter §2 warm SLAs met with large headroom.** There is no reason to keep
  users on the slower, unbounded-RAM path.
- **`InMemory` was always a crutch, not a feature.** The legacy `HashMap`-based
  store was adequate for development but cannot meet the 100 K-node bounded-memory
  requirement. Keeping it as the default would make every new install a known SLA
  violation.
- **Not-launched window.** No production `.rmp` indexes exist in the wild. The
  migration cost is zero for real users; the `.rmp` soft-migration covers developer
  continuity only.
- **Reversible.** A user who hits an unexpected redb issue can `--no-default-features`
  to revert to `InMemoryBackend`. The feature flag is retained; it is not sealed.

## Consequences

### Positive

- Bounded resident memory via mmap paging: large repos no longer OOM. RAM
  proportional to working set, not total graph.
- O(changed-file) incremental writes in watch mode instead of O(graph).
- ACID crash-safety by construction (redb copy-on-write, two-phase commit).
- Deterministic adjacency (sorted `Vec<u64>` values) makes BFS reproducible.
- Time-travel queries (MVCC snapshots) become architecturally reachable in a future
  RFC without a format break.

### Negative / Risks Accepted

- **Cold-start tail latency** — first query after OS page-cache eviction pays mmap
  page-fault cost. Charter §2 SLA is annotated `warm-cache steady-state`; a
  separate cold-start budget is being measured by the `sla_ancestors_100k` nightly
  CI job (RFC-0104). Cold number will be added to Charter §2 once measured (TBD).
- **redb on-disk format churn** — redb major-version bumps change the on-disk
  format. Mitigated by Cargo.lock pin + version guard in `meta` table + friendly
  `StorageError::SchemaVersion` at open time.
- **Legacy import RAM spike** — the soft-migration path materializes the full legacy
  graph in RAM. `--reindex` is documented as the escape hatch for repos that already
  OOM.

### Open

- **RFC-0104 cold SLA numbers**: the `warm/cold` SLA split in Charter §2 carries a
  `TBD` in the cold row until nightly CI produces honest Linux measurements with
  `drop_caches`. This is a follow-up measurement task; it does not block the flip.
- **ADR-0007/0008 numbering conflict**: `0007-redb-storage-engine.md` carries the
  wrong number (conflicts with `0007-cli-mcp-skill-parity.md`). The rename to
  `0009-redb-storage-engine.md` (or similar) is a follow-up docs task.

## Alternatives Considered

- **Keep `redb-backend` opt-in indefinitely.** Rejected: maintains an unbounded-RAM
  default that violates Charter §2 at scale. No technical justification once Phase 2
  equivalence and latency gates pass.
- **Make `InMemoryBackend` the default; ship `RedbBackend` as an opt-in.** Same as
  above — `InMemory` cannot meet the 100 K-node bounded-memory requirement. Opt-in
  would mean most installations silently fail the SLA.
- **Hard-remove `InMemoryBackend`.** Deferred: `InMemory` provides the differential
  oracle for equivalence tests and a fast unit-test path. Removing it now would
  eliminate that safety net before the redb path is fully hardened.
