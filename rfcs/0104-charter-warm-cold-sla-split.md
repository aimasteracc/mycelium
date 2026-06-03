# RFC-0104: Charter §2 warm/cold SLA split for the redb mmap path

- **Status**: draft — awaiting BDFL decision
- **Author(s)**: orchestrator (Hive AI agent, PM role)
- **Created**: 2026-06-02
- **Last updated**: 2026-06-02
- **RFC type**: `meta` — amends Charter §2 (requires founder approval, Charter §9)
- **Tracking issue**: Issue #426 AC#4
- **Affected paths**:
  - `CHARTER.md` §2 SLA table
  - `crates/mycelium-core/tests/redb_sla.rs`
  - `docs/adr/0008-redb-storage-engine.md` (Decision-4 cross-ref)

---

## Summary

Charter §2 currently has a single SLA column that mixes two physically distinct
operating modes: **warm** (pages already in the OS page cache — the steady-state
interactive case) and **cold** (first open after a process restart, when mmap pages
must be paged in from disk).

ADR-0008 Decision-4 (founder-authorized 2026-05-31) explicitly required this split:

> "Cold-start SLA → split Charter §2 into warm (steady-state, existing numbers)
> + a separate cold-open budget; do a warm-up scan on open; the exact cold number
> is set from the T1 mmap cold-page spike, NOT guessed."

This RFC formalizes that decision as a Charter amendment. Without it,
`redb_sla.rs` tests assert `< 1 ms` for 3-hop traversal on a path that is
**physically impossible cold on mmap** (each page fault is a kernel-scheduled disk
read; a 100K-node graph's adjacency spans many pages). The claim "CI gates them"
is false for the redb cold path until this is resolved.

This RFC is the prerequisite for:
- Issue #426 AC#2 (RSS-cap CI gate — needs a correct latency baseline)
- Issue #426 AC#5 (flip redb to the default backend)

---

## Motivation

### The physical constraint

mmap-backed storage (which redb uses) maps the database file into virtual address
space. A read that touches a page not yet in the OS page cache generates a **major
page fault**: the kernel suspends the thread, reads the page from disk, and resumes.
On an NVMe SSD a single 4 KiB page read is 50–200 µs. A 3-hop traversal from a
cold graph requires reading:

1. The forward-adjacency row for the root node (one page fault)
2. Each of its ~2 direct callees (up to 2 more page faults)
3. Each second-hop callee (up to 4 more page faults)

That is 7 sequential page faults at minimum → **350–1400 µs** before any Rust
logic runs. The Charter §2 target of `< 1 ms` is unreachable cold on any disk.

The warm path is fine: once the working set fits in the page cache, traversal
stays in L3/DRAM → sub-millisecond is achievable and the existing SLA is correct.

### Current state creates a silent lie

`redb_sla.rs::redb_three_hop_sla_10k_under_1ms` opens a database that was just
seeded in the same test run. The OS page cache is warm because the kernel already
loaded those pages during the seed writes. This test is labelled "SLA" but it
measures the warm path while the Charter says "3-hop graph traversal (callers,
depth 3) < 1 ms" with no qualifier. A production deployment that restarts the
process and immediately issues a 3-hop query on a 100K graph will exceed this
target — by design, not by bug.

### The right fix

Split the table. Keep the warm numbers (they are correct and achievable). Add a
cold column derived from measurement, not assumption.

---

## Detailed design

### Charter §2 amendment

Replace the current §2 SLA table with a two-column table:

| Metric | Warm target | Cold target |
|---|---|---|
| Single symbol lookup | < 5 ms | < 50 ms |
| 3-hop graph traversal (callers, depth 3) | < 1 ms | < 200 ms |
| Reactive re-query after file change | < 10 ms | *(warm by definition — watch loop runs continuously)* |
| AI token efficiency (Hyphae DSL vs JSON) | ≤ 30% JSON tokens | *(not applicable — no disk I/O)* |
| New language onboarding | ≤ 3 files, 0 core-code lines | *(not applicable)* |
| Public API documentation coverage | 100% pub items have rustdoc | *(not applicable)* |
| Test coverage (line) | ≥ 90% | *(not applicable)* |
| Test coverage (branch) | ≥ 80% | *(not applicable)* |
| Mutation testing kill rate | ≥ 70% | *(not applicable)* |
| Fast-lane CI duration | < 5 min | *(not applicable)* |
| Full-lane CI duration | < 20 min | *(not applicable)* |
| Heavy-graph tools on 1 K-node graph | < 2 s | < 10 s |
| Heavy-graph tools on 10 K-node graph | < 10 s | < 60 s |
| Heavy-graph tools on 100 K-node graph | < 30 s | < 180 s |

**Cold target rationale:**
The proposed numbers (50 ms lookup, 200 ms 3-hop) are conservative estimates
from the disk-access model: NVMe at 100 µs/page fault, 3-hop touching ≤ 7 pages
= ≤ 700 µs with margin. 200 ms gives headroom for slow CI runners and large
adjacency rows. **These are ceiling values, not aspirational targets.** Once the
T1 measurement runs nightly in CI, the actual observed p99 from the nightly
benchmark replaces these placeholders.

### Measurement protocol (how we set the real numbers)

**T1 cold-open test (nightly, Linux only):**

```rust
// In crates/mycelium-core/tests/redb_sla.rs
#[cfg(target_os = "linux")]
#[test]
fn redb_three_hop_cold_open_sla_10k() {
    let (_dir, path, root_symbol) = seed_redb(SLA_NODES); // seed, close redb
    drop_page_cache_for_file(&path); // madvise(MADV_DONTNEED) on the redb file
    let backend = RedbBackend::open_existing(&path).expect("reopen redb cold");
    let root = backend.lookup_path(&root_symbol).expect("root symbol");

    let started = Instant::now();
    let _reached = three_hop_count(&backend, root);
    let elapsed = started.elapsed();

    assert!(
        elapsed < COLD_THREE_HOP_SLA,
        "redb cold 3-hop exceeded SLA: elapsed={elapsed:?}, sla={COLD_THREE_HOP_SLA:?}"
    );
}
```

`drop_page_cache_for_file` uses `madvise(MADV_DONTNEED)` on the mmap'd redb file
region to evict pages without requiring root. This is the correct user-space
mechanism (same technique used by jemalloc and RocksDB tests).

**Process:**
1. Nightly CI runs this test with `MYCELIUM_REDB_BENCH_100K=1`.
2. The p99 cold-open 3-hop time from three nightly runs is committed to Charter
   as the `< X ms` cold target.
3. Future PRs adding redb changes must not regress the p99 by more than 10%.

### Optional warm-up scan on open (ADR-0008 Decision-4)

ADR-0008 Decision-4 mentions "do a warm-up scan on open." This RFC proposes:

```rust
impl RedbBackend {
    /// Pre-reads the forward-adjacency table to populate the OS page cache.
    /// Opt-in; default off. Reduces the latency of the first few queries after
    /// a cold open at the cost of a one-time linear scan (~50 ms for 100K nodes).
    pub fn warm_adjacency_cache(&self) -> anyhow::Result<()> { ... }
}
```

This is NOT a default behavior — it is opt-in for interactive use cases that
want predictable first-query latency. The Charter cold target is measured
**without** warm-up to give an honest worst-case bound.

---

## Drawbacks

- Adds complexity to the §2 table (two columns instead of one).
- The cold targets are initially placeholders; they must be measured before the
  redb backend can be declared production-ready.
- `madvise(MADV_DONTNEED)` is Linux-specific; macOS uses `madvise(MADV_FREE)` with
  slightly different semantics. The cold test is Linux-only (already the pattern in
  this codebase; see `#[cfg(target_os = "linux")]` in `redb_sla.rs`).

---

## Alternatives

**Alternative A — Keep single column, change the 3-hop target from `< 1ms` to
`< 200ms`.**
- Pros: simpler table.
- Cons: the `< 200ms` target is meaningless for the warm path (already `< 1ms`).
  Users see a degraded promise. Rejected.

**Alternative B — Add a footnote to §2 saying "these are warm targets".**
- Pros: minimal change.
- Cons: footnotes are easy to miss; CI gates are written against the cell values.
  A note doesn't fix the test. Rejected.

**Alternative C — Add a `warm_on_open` default and treat the existing single
column as "post-warmup."**
- Pros: the single column stays, no ambiguity.
- Cons: warm-up is O(graph) latency penalty on every process restart, even when
  the graph is already warm from the prior run. Adds 50–500 ms to startup on large
  repos. Users will notice. Rejected.

---

## Prior art

- **RocksDB block cache** has explicit warm/cold statistics and separate latency
  budgets for cache-miss vs cache-hit paths.
- **rust-analyzer** documents that its first-launch analysis is "cold" and takes
  seconds; subsequent queries are fast. Two-tier expectations are standard in
  language tooling.
- **PostgreSQL** effective_cache_size distinguishes what the planner can assume is
  in cache vs what must be read from disk.

---

## Migration

Non-breaking. This is a Charter amendment only. No code migration; the only code
change is:
1. New cold-path SLA constants in `redb_sla.rs`.
2. New `drop_page_cache_for_file` helper (Linux madvise, ~20 lines).
3. New test function using the cold constants.

---

## Testing strategy

The RFC is considered implemented when:

- [ ] `redb_three_hop_cold_open_sla_10k` passes in CI on Linux (`redb-backend`
      feature, nightly lane).
- [ ] `redb_lookup_cold_open_sla_10k` passes in CI on Linux.
- [ ] Charter §2 table in `CHARTER.md` has been updated with the measured p99
      values replacing the placeholder ceilings.
- [ ] `docs/adr/0008-redb-storage-engine.md` Decision-4 cross-references this RFC.
- [ ] Issue #426 AC#4 is closed.

---

## Performance impact

The Charter §2 warm column is unchanged. The only additions are:

| Metric | Current (Charter) | After this RFC |
|---|---|---|
| Single-symbol lookup | < 5 ms (unqualified) | < 5 ms warm / < 50 ms cold |
| 3-hop traversal | < 1 ms (unqualified) | < 1 ms warm / < 200 ms cold (placeholder) |
| Heavy-graph 100K | < 30 s (unqualified) | < 30 s warm / < 180 s cold |

Placeholder cold values will be replaced with measured p99 numbers from the first
nightly CI run after this RFC merges.

---

## Open questions

1. **Should `warm_adjacency_cache()` be a CLI flag (`mycelium serve --warm-on-open`)?**
   If yes, it needs a CLI ↔ MCP ↔ Skill surface entry per Charter §5.13. The
   current proposal defers this to a follow-up RFC if demand materializes.

2. **Should the cold target be p50, p90, or p99?** Recommended: p99 (worst-case
   CI runner), so that "CI gates them" is an honest claim. Requires 3+ nightly runs
   to compute.

3. **macOS cold measurement:** `madvise(MADV_FREE)` on macOS marks pages as
   reusable but the OS may not immediately evict them. True cold measurement on
   macOS requires `purge(8)` (which is the macOS equivalent of Linux
   `drop_caches`). Should we skip the cold test on macOS (current pattern) or add
   a macOS-specific path?

---

## Future possibilities

- Once the cold column is populated with real data, the redb backend can graduate
  to default (Issue #426 AC#5).
- The warm-up scan API enables a future `mycelium serve --hot` mode optimized for
  interactive use (pre-warm on launch, then sub-millisecond queries).
- MVCC time-travel queries (ADR-0008 innovation hook) benefit from the warm/cold
  distinction: time-travel queries are inherently colder (snapshot pages may have
  been evicted), so the cold column gives them a principled SLA home.

---

*Once accepted, this RFC is the contract. Update via superseding RFC.*
