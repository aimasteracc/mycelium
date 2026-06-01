# RFC-0100 Phase 2 — Build Plan (Expert-Team Refined, Trustworthy)

> **Status:** Ready to build · **Verdict:** GO-WITH-CONDITIONS
> **Source:** Expert-team workflow `wf_21a3635f-0e6` (6 design + 3 adversarial review + 1 lead synthesis, 2026-06-01).
> **This run is trustworthy:** reviewers ran against the **real shipped `redb_backend.rs`** (Phase-1 branch / PR #365) and cite real line numbers — unlike the first aborted run, whose branch lacked the code.
> **Supersedes the Phase 2 ordering in** [`rfc-0100-execution-plan.md`](rfc-0100-execution-plan.md) **§Phase 2.**

## Headline

The shipped `RedbBackend` (Phase 1, behind the `redb-backend` cargo feature, **default OFF — no
released binary is affected**) is functionally correct on the **single-threaded happy path** the
12 Phase-1 tests cover. But three independent adversarial reviews converged: the
**`auto-commit-per-op`** design is *simultaneously* a **crash-atomicity defect** and a
**performance cliff**, and the on-disk adjacency violates the ADR-0007 sorted-Vec contract.

The fix — a **batched single-transaction `WriteBatch` API** (task **T05**) — is therefore **not a
perf nice-to-have; it is the correctness fix** for the CRITICAL crash bugs. The crash-safety suite
(T03) must land **RED first** to prove the bugs, *then* T05 turns it green.

## 🔴 Confirmed findings (real line numbers in `crates/mycelium-core/src/store/redb_backend.rs`)

| # | Severity | Finding | Location |
|---|---|---|---|
| 1 | **CRITICAL** | `upsert_edge` commits forward (`write_fwd`) and reverse (`write_rev`) adjacency in **two separate transactions**, both `Result`s discarded with `let _ =`. Crash/SIGKILL/disk-full between them → one-directional edge: `outgoing()` has it, `incoming()` doesn't. The oracle sets both halves atomically. | `upsert_edge` 446-457; `write_fwd` 244-260; `write_rev` 277-293 |
| 2 | **CRITICAL** | `remove_node_edges` is non-atomic across up to `16×(2+2·degree)` separate committed txns, every write discarded with `let _ =`. Any failure leaves dangling adjacency (a ghost id whose `path_of()` is `None`). | 459-480 (463,467,472,476) |
| 3 | **CRITICAL** | `remove_node` splits edge-removal and trunk/kind/span-removal into **two commit groups**. Crash between → ghost node: in `all_paths`/`lookup_path` but edgeless. | 343-364 |
| 4 | **CRITICAL** | **Silent error swallowing in production**, not just on crash. The same `let _ =` pattern in `upsert_edge`/`remove_node`/`set_kind`/`set_span` means a transient redb/IO error (disk full, mmap error) produces a torn state with **no signal to the caller**. | 408, 428, 450, 455, 346, 463-476 |
| 5 | **HIGH** | Adjacency stored **UNSORTED** (`fwd.push` / `rev.push`) — violates ADR-0007 §2 (sorted `Vec<u64>`). `outgoing()`/`incoming()` sort at read time to compensate, so on-disk bytes are non-canonical and `contains()` can't become a binary search. | 449, 454; read-sort 484, 490 |
| 6 | **HIGH** | `upsert_node` has **no read-before-write** — always fsyncs even when the node already exists unchanged. Re-indexing 100K nodes for a one-line change = ~100K wasted fsyncs. | 320-341 |
| 7 | **HIGH** | **Perf cliff:** each `upsert_edge` = 4 txns (2 fsync). 300K edges ≈ **1.2M fsyncs** vs Charter §2 `< 30 s`. Plus `contains()` duplicate check is O(degree) → O(degree²)/node. | 446-457 |
| 8 | **HIGH** | `flush()` is a no-op but the trait doc promises "finalises the current write transaction". When the batch API lands, `flush()` MUST commit the open batch or callers believe data is durable when it isn't. | 537-539; `backend.rs` 118-122 |
| 9 | **HIGH/MED** | `edge_count()` is an O(E) full scan, called by `heap_size_estimate()`. Oracle is O(1). Fix: cached counter in the `META` table updated inside the edge write txn. | 494-507; 531-535 |
| 10 | **MEDIUM** | `all_edges()` returns B-tree-sorted triples while the oracle returns HashMap order — a **test-design landmine**: any equivalence assertion using `Vec::eq` instead of set/sort will false-fail. (Harness already sorts; enforce for every new edge assertion.) | 509-529 |

**ADR-0007 is currently aspirational, not implemented:** §6 ("ONE redb WriteTransaction per file
re-index") and §7 ("never a torn third state") are **false for shipped code**. Update the ADR's
status/consequences when T05 lands — do not "fix forward" silently; the founder must know the
Phase-1 durability claim didn't hold.

## Task order (dependency-true; ignore the label drift between artifacts)

| Order | Task | What | Why here |
|---|---|---|---|
| 1 | **P2-T01** | Equivalence harness: `run_matrix` + 3-layer set-based comparator (paths / kind+span / `(kind,src_path,dst_path)` triples / per-kind `outgoing`+`incoming`). InMemory oracle vs tempfile Redb. **THIS PR.** | Zero prod deps; runs today; every later task reuses the comparator. |
| 2 | **P2-T03** | Crash-safety suite, **RED-first**. Test seam (`stage_*_uncommitted`, `commit_fwd_only`) gated `#[cfg(any(test, feature="redb-test-hooks"))]` so it never ships. SIGKILL child-process test (Unix, `libc` dev-dep) for true durability — Drop-based abort only proves MVCC logic. | Executable proof of CRITICAL #1-3; must go RED before the fix. |
| 3 | **P2-T05** | **Batched single-transaction `WriteBatch` API** — the keystone correctness fix. Extract private `*_in(&WriteTransaction, …)` helpers shared by auto-commit and batch so there's ONE code path. Fold `upsert_edge` (fwd+rev) and `remove_node` (edges+trunk+kind+span) into one txn each. Also: insert adjacency **sorted**, add `upsert_node` read-before-write, cache `edge_count` in META. | Turns T03 RED→GREEN; fixes #1-9; honest write benchmarks need it. |
| 4 | **P2-T02** | proptest property-based equivalence over the Op model + comparator. | Broader fuzzing of the same divergence class. |
| 5 | **P2-T04** | Memory-ceiling / heap-win proof (the R3 win). Needs a **real allocator stat** (dhat/jemalloc) — the current `nodes*256+edges*24` heuristic measures a formula, not heap. | Needs T01 equivalence guard + T05 batch (else 700K fsyncs = minutes). |
| 6 | **P2-T06** | criterion bench + Linux-gated SLA `#[test]` (`tests/redb_sla.rs`): cold lookup `<5ms`, 3-hop `<1ms`. macOS cold numbers advisory-only (page-cache, per T1 spike). criterion never gates CI. | Needs T05 for a non-pathological write number. |

### P2-T04 progress marker — 2026-06-01

The first P2-T04 implementation slice replaces the `RedbBackend` memory
estimate's in-memory formula with redb allocated-page accounting from
`WriteTransaction::stats()` and adds opt-in redb RSS/page-footprint measurements
to `crates/mycelium-core/tests/sla_memory_curve.rs`. It also adds
`crates/mycelium-core/tests/redb_memory_ceiling.rs`, an ignored Linux-only
child-process RSS comparison scaffold for the later hard cap/OOM gate.

Scope boundaries:

- This is a measurement foundation, not the full #344 closure.
- `redb-backend` remains feature-gated off by default.
- Full 10K/100K/500K proof and the "InMemory crosses cap while redb stays
  under cap" gate remain open until the batched/file-scoped write path avoids
  per-operation fsync costs.

### P2-T05a progress marker — 2026-06-01

The first P2-T05 implementation slice adds the ADR-0007 `file_index` table to
`RedbBackend` and introduces a feature-gated core-only `replace_file` API. The
API reads the persisted file entry, removes the file's previous nodes and owned
edges, strips stale external references to removed nodes, writes the new
nodes/edges, updates `file_index`, and commits all of that in one redb write
transaction.

Scope boundaries:

- This is the core storage primitive for Issue #343, not the full issue
  closure.
- The default backend remains unchanged and `redb-backend` remains feature-gated
  off by default.
- CLI/MCP/Skill surfaces are intentionally untouched until the backend flip or
  migration command, so the Three-Surface Rule is not triggered in this slice.
- Watch-mode wiring and large-repo O(changed-file) benchmarks remain open.

### P2-T05b progress marker — 2026-06-01

The second P2-T05 implementation slice caches `edge_count` in the redb `meta`
table and updates it inside the same transactions as `upsert_edge`,
`remove_node_edges`, `remove_edge`, and `replace_file`-driven stale-reference
cleanup. Existing schema-v2 databases opened through `RedbBackend::open` seed
the key from one compatibility scan; `open_existing` callers fall back to the
scan until a write transaction creates the metadata key.

Scope boundaries:

- This fixes the P2 finding that `edge_count()` and the edge portion of
  `heap_size_estimate()` performed O(E) scans.
- It does not close Issue #343 or Issue #344 by itself; watch-mode wiring,
  write batching, and large-repo SLA/memory proof remain open.
- The default backend remains unchanged and `redb-backend` remains feature-gated
  off by default.

### P2-T05c progress marker — 2026-06-01

The third P2-T05 implementation slice adds
`RedbBackend::replace_file_from_store`, a core-only bridge that converts a
single-file in-memory `Store` into the `FileNode` and `FileEdge` payload expected
by `replace_file`. It carries file-owned nodes, kind/span metadata, and
source-owned edges into the existing one-transaction redb replacement path.

Scope boundaries:

- This is the watch-mode persistence bridge, not the MCP watch-loop flip.
- It advances Issue #343 by removing the manual payload-construction gap between
  extraction and redb `replace_file`.
- CLI/MCP/Skill surfaces are intentionally untouched, so the Three-Surface Rule
  is not triggered in this slice.
- O(changed-file) watch benchmarks and default-backend migration remain open.

### P2-T05d progress marker — 2026-06-01

The fourth P2-T05 implementation slice wires the MCP watch persistence path to
redb behind a new `mycelium-mcp/redb-backend` cargo feature. With that feature
enabled, `with_root` prefers `.mycelium/index.redb` over the legacy
`.mycelium/index.rmp`, imports the initial loaded or live-indexed in-memory
graph into redb, and persists watch batches by calling
`RedbBackend::replace_file_from_store` for each changed source file. The
default build still uses MessagePack snapshots.

Scope boundaries:

- This is an internal cargo-feature storage path, not a new CLI command or MCP
  tool, so the Three-Surface Rule is not triggered.
- `RedbBackend::replace_file_from_store` now reads the file subtree via
  `Store::descendants` rather than scanning every path in the graph.
- Large-repo O(changed-file) benchmarks, default-backend migration, and the
  public `migrate` CLI/MCP/Skill surface remain open.

### P2-T06a progress marker — 2026-06-01

The first P2-T06 measurement slice adds
`crates/mycelium-core/tests/redb_sla.rs` and
`crates/mycelium-core/benches/redb_incremental_persistence.rs`. The SLA test
target is feature-gated on `redb-backend` and asserts Linux timing budgets for
exact lookup and bounded 3-hop traversal on a 10K-symbol redb fixture; non-Linux
platforms keep an advisory smoke path. The criterion benchmark compares legacy
full `.rmp` snapshot rewrites with redb single-file replacement.

Local benchmark sample from this slice on macOS/aarch64:

| Scenario | Scale | Mean |
|---|---:|---:|
| full MessagePack snapshot | 10K symbols | ~1.26 ms |
| full MessagePack snapshot | 100K symbols | ~28.2 ms |
| redb single-file replacement | 10K symbols | ~18.4 ms |

Scope boundaries:

- This is a harness/measurement slice, not the final #343 closure.
- 100K redb replacement is env-gated behind `MYCELIUM_REDB_BENCH_100K=1`
  because the release-grade criterion run is slow enough to be unsuitable for
  routine local checks.
- The current data exposes the next optimization target: redb single-file
  replacement already avoids rewriting a full MessagePack snapshot, but its
  fixed transaction/write cost is still higher than a small 10K legacy snapshot.
  Default-backend migration remains gated on the 100K redb run and follow-up
  write-amplification work.

### P2-T05e progress marker — 2026-06-01

The next replace-file correctness slice fixes the ownership boundary for
external incoming edges. `RedbBackend::replace_file` now distinguishes old node
ids that remain present in the replacement payload from stale old node ids:

- stable old node ids keep external incoming edges owned by other files;
- stale old node ids still have external references stripped so removed or
  renamed symbols do not leave dangling graph edges;
- stable old node metadata is cleared before new metadata is applied, so kind or
  span removals do not leak from the previous version.

This also reduces unnecessary replacement work for unchanged symbols, because
the stale-node edge cleanup scans run only for node ids that actually disappear.
On the local macOS/aarch64 criterion benchmark, the 10K-symbol redb
single-file replacement mean improved from ~18.4 ms to ~9.70 ms after this
change.

### P2-T05f progress marker — 2026-06-01

The next write-amplification slice fixes the P2 finding that `upsert_node`
always wrote both trunk tables even when the same path already existed with the
same deterministic id. `RedbBackend::upsert_node` now checks `trunk_by_path`
and `trunk_by_id` inside the write transaction and returns early when both
indexes are already consistent.

RED-first coverage proves 1,000 repeated upserts of the same node do not grow
the redb allocated page footprint. This directly supports the R2/R3 tracks:
unchanged symbols in a watch replacement no longer churn trunk pages, and the
local macOS/aarch64 criterion benchmark for 10K-symbol redb single-file
replacement improved further from ~9.70 ms to ~9.37 ms.

## First PR (this one) — P2-T01 only

The equivalence harness as RED-first integration tests gated `#[cfg(feature="redb-backend")]`.
Set-based comparator (never `Vec::eq`) to dodge the `all_edges` ordering trap (#10). Ships value
immediately — proves happy-path equivalence of the shipped code, introduces **no production
changes**, and creates the one comparator T02-T04 reuse.

**Result (verified):** all 12 matrix cases — including `bidirectional_removal`, `all_edge_kinds`,
`reverse_insertion_order`, `multi_file`, and `reopen` — **PASS**. This is the honest, expected
outcome and is itself the finding: the shipped `RedbBackend` **is** observably equivalent to the
oracle under normal operation. The CRITICAL bugs are **invisible to this harness by construction** —
they are crash-only (atomicity windows, #1-4), on-disk-byte-only (unsorted adjacency masked by the
read-time sort, #5), or perf-only (#6-9, #7). No `#[ignore]` is needed; exposing them requires
**crash injection (P2-T03)** and **benchmarks (P2-T06)**, exactly as the synthesis predicted. The
harness's value is the reusable comparator + the now-locked-in guarantee that no *normal-operation*
regression slips through.

## Integration risks (carry forward)

- **Real trait signatures, not the spec's fictional ones.** `WriteBatch` must mirror
  `upsert_node(&str)->NodeId`, `upsert_edge(EdgeKind,NodeId,NodeId)`, `set_kind(NodeId,NodeKind)`
  — the design skeleton invented `&Node`/`&Edge` params and a `set_kind` taking `EdgeKind`.
- **One shared `tests/support/` comparator** across T01-T04 — do not let each task grow its own
  (drift = crash assertions inherit gaps).
- **Test seam must not ship** — `#[cfg(any(test, feature="redb-test-hooks"))]`; derive from the
  `*_in` helpers so prod and test share one path. Touches a shipped file → reviewer sign-off.
- **Windows crash fidelity unproven** — and Windows bit the project in the v0.1.4 saga.
- **Three-Surface Rule:** internal storage tasks have no CLI/MCP surface; confirm no `--backend
  redb` flag sneaks in without CLI↔MCP↔Skill parity.

## TDD discipline (Charter §5.1 / Dev≠QA)

test-author writes **all** suites RED-first **before** the implementer touches `redb_backend.rs`.
Each task: confirm RED → minimal GREEN → refactor → quality gate (`fmt --check`, `clippy -D
warnings`, `test --all`, `llvm-cov --fail-under-lines 90`). QA validates on real repos (ripgrep,
requests). **No auto-merge to develop** — every task is a PR for founder review.
