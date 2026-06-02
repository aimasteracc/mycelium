# RFC-0100 Execution Plan — redb Storage Layer

> Hive expert-team workflow (7 agents: 3 design -> 3 adversarial review -> 1 Lead synthesis), 2026-05-31.
> **Planning only - no code.** Implementation gated on founder signing [ADR-0008](../adr/0008-redb-storage-engine.md).

Companion to [RFC-0100](../../rfcs/0100-unified-storage-redb.md). Unifies R2 (#343) + R3 (#344).

> **Founder decision overrides (2026-05-31) — see [ADR-0008 §Founder Decisions](../adr/0008-redb-storage-engine.md).**
> Mycelium is **not yet launched**, so two tasks are simplified ("稳中求创新"):
> - **Migration tasks:** build only a **minimal full-RAM importer**; **no streaming importer**. Pre-redb `.rmp` migration is best-effort dev-continuity, not a supported user path. `--reindex` (bounded rebuild from source) is the primary escape for large repos.
> - **Phase 4 / legacy reader:** **auto-migrate on first open, then drop the legacy `.rmp` reader entirely** after the Phase-3 flip — no multi-release retention window.
> - **Cold-start SLA:** the cold-open number is **measured by the T1 spike**, then written into Charter §2 — not guessed.
> All other tasks stand as generated.

## Lead go/no-go

**Decision: `go-with-conditions`**

The design is sound and the reviews are convergent and source-grounded — the logical model is unchanged (NodeId is a content hash), redb is the right tool, and the three reviewers independently confirmed the same critical facts. Every CRITICAL finding now has a concrete, locked resolution in the ADR rather than a deferred open question: (1) the `>`/0x3E path-key defect is resolved by a NUL-separator key encoding with a provable boundary test; (2) NodeId non-transparency is resolved by `#[serde(transparent)]` plus a migration-aware legacy decode; (3) EdgeKind `#[non_exhaustive]` is resolved by an explicit append-only u16 tag table; (4) the bidirectional edge cleanup AND the synapse value-MERGE-not-overwrite correctness bugs (the most dangerous findings — silent edge loss on re-index) are now explicit steps in the per-file transaction; (5) the three-way schema conflict is resolved to one canonical 8-table schema; (6) the object-safety bug in the trait is resolved by dropping the associated Write type. This is go-with-conditions, NOT unconditional go, because: the ADR is not yet signed; #356 does not exist in the tree and is now a hard Phase-1 prerequisite (not concurrent work); Dev!=QA must be enforced structurally (the test-author authors all equivalence/crash/perf/mutation suites BEFORE the implementer, reversing the earlier 'implementer writes own unit tests' proposal); and the batch≠incremental stub-resolution gap must be explicitly scoped out of the incremental-equivalence assertion or it will produce false failures. With these conditions met, implementation can start safely after sign-off. It is not no-go because no finding invalidates the core approach — all are addressable in design before code, which is exactly what this planning phase produced.

## Decisions the founder must make first

- [ ] Sign the ADR: do you accept the canonical 8-table separate-table schema (trunk_id_to_path, trunk_path_to_id, kind_map, span_map, synapse_fwd, synapse_rev, file_index, meta) and reject the consolidated nodes-blob alternative? This is the load-bearing choice the three designs disagreed on.
- [ ] Adjacency semantics change: do you approve storing synapse values as SORTED Vec<u64> (currently insertion-order)? This makes BFS/path-finding deterministic and cross-backend identical, but is an observable behavior change for any consumer relying on current ordering.
- [ ] Migration RAM policy: is it acceptable that `mycelium migrate` (rmp-load path) requires peak RAM ~= full graph size, with `--reindex` (rebuild from source) as the recommended escape for repos too large to load? Or do you require a streaming importer (significantly more work) before Phase 3?
- [ ] Cold-start SLA: do you accept annotating Charter §2 as warm-cache steady-state and adding a separate first-query/cold-open budget (proposed <100ms) with a startup warm-up scan? mmap cannot guarantee 3-hop <1ms on a cold page cache.
- [ ] redb version policy: approve pinning redb to an exact version (=2.x.y) with bumps treated as a tracked migration event? A redb major-version on-disk format change otherwise breaks existing indexes.
- [ ] Skill placement: confirm `migrate` belongs in skills/index-management/SKILL.md (the lifecycle home), and that the INDEX.md + SKILL.md rows land in the SAME PR as the CLI/MCP registration (required to keep the parity gate green).
- [ ] Phase 4 deprecation window: confirm the legacy rmp READER is removed exactly one minor release after the Phase-3 default flip — accepting that users who never ran migrate will need to re-index on a too-old binary.
- [ ] Dev!=QA confirmation: approve that the test-author role authors ALL test suites (unit, equivalence, crash, perf, mutation) RED-first BEFORE the rust-implementer writes production code, rather than the implementer writing their own unit tests.

## Three non-negotiables (enforced)

- **Dev != QA** - every implementer task has a separate qa-validator task.
- **QA on real repos** - equivalence/perf/memory validated on real vendored repos, not fixtures.
- **Lead owns go/no-go** - Opus Lead gates each phase.

## Model tiering

- **Opus 4.8**: architecture spikes, ADR, all QA/validation, phase gates.
- **Sonnet 4.6**: Rust implementation.
- **Haiku 4.5**: mechanical cleanup.

## Task breakdown (26 tasks)

### Phase 1 - Backend seam + de-risk spikes + ADR

#### `P1-T01` Pin redb exact version + add redb-backend feature flag (OFF by default)
- **Tier / role:** sonnet / rust-implementer
- **Depends on:** -
- **Files:** `Cargo.toml`, `crates/mycelium-core/Cargo.toml`, `Cargo.lock`
- **Test (TDD, RED first):** cargo build --features redb-backend and default build both green; cargo deny check passes with exact pin (=2.x.y).
- **Acceptance:** redb pinned to exact version; feature `redb-backend` exists, default feature set excludes it; both build modes compile.

#### `P1-T02` Author RED tests: NodeId serde-transparency + EdgeKind stable tag invariants
- **Tier / role:** opus / test-author
- **Depends on:** P1-T01
- **Files:** `crates/mycelium-core/tests/encoding_invariants.rs`
- **Test (TDD, RED first):** RED-first: assert rmp_serde::to_vec(&NodeId(42))==to_vec(&42u64); assert EdgeKind::<each>.as_tag() equals a hardcoded constant; compile-time count assert tags==variants. Tests must fail before impl.
- **Acceptance:** Tests committed, confirmed RED (NodeId not transparent yet, as_tag absent).

#### `P1-T03` Add #[serde(transparent)] to NodeId + EdgeKind as_tag(u16) wire-format table
- **Tier / role:** sonnet / rust-implementer
- **Depends on:** P1-T02
- **Files:** `crates/mycelium-core/src/types.rs`, `crates/mycelium-core/src/store/redb_tags.rs`
- **Test (TDD, RED first):** Make P1-T02 GREEN. redb_tags.rs marked WIRE FORMAT append-only; explicit per-name constants, never discriminant order.
- **Acceptance:** P1-T02 tests pass; cargo test --all green (confirm no rmp snapshot regression elsewhere); legacy reader path still decodes pre-existing fixtures.

#### `P1-T04` Author RED tests: path-key NUL-separator ordering boundary
- **Tier / role:** opus / test-author
- **Depends on:** P1-T01
- **Files:** `crates/mycelium-core/tests/path_key_order.rs`
- **Test (TDD, RED first):** RED-first: encode paths a>b, aZ>c, a>b>d; assert prefix scan for a> yields exactly {a>b, a>b>d} and never aZ>c, using the NUL-substituted key codec (not yet implemented).
- **Acceptance:** Tests committed, confirmed RED.

#### `P1-T05` Implement key codecs: u64 BE, (u16 kind ++ u64 node) BE, NUL-separator path key
- **Tier / role:** sonnet / rust-implementer
- **Depends on:** P1-T03, P1-T04
- **Files:** `crates/mycelium-core/src/store/redb_keys.rs`
- **Test (TDD, RED first):** Make P1-T04 GREEN; unit tests for round-trip and ordering of each codec.
- **Acceptance:** P1-T04 green; codecs are pure functions with property tests for order preservation.

#### `P1-T06` Define StorageBackend trait (sync, object-safe) + StorageError (thiserror) + AsyncBackendHandle
- **Tier / role:** opus / architect
- **Depends on:** P1-T01
- **Files:** `crates/mycelium-core/src/store/backend.rs`, `crates/mycelium-core/src/store/error.rs`
- **Test (TDD, RED first):** Compile-only: assert Box<dyn StorageBackend> constructs (object-safe, no associated Write type; begin_write returns concrete WriteHandle). AsyncBackendHandle exposes only spawn_blocking-wrapped async fns.
- **Acceptance:** Trait compiles as a trait object; StorageError has SchemaVersion/NotFound/Encode/Decode/Backend variants; raw RedbBackend will be crate-private.

#### `P1-T07` Implement InMemoryBackend over existing Trunk/Synapse/maps (the oracle)
- **Tier / role:** sonnet / rust-implementer
- **Depends on:** P1-T06
- **Files:** `crates/mycelium-core/src/store/in_memory_backend.rs`
- **Test (TDD, RED first):** Author (test-author) RED unit tests per trait method against today's behavior FIRST; implementer makes GREEN. Adjacency normalized to sorted Vec<u64>.
- **Acceptance:** InMemoryBackend implements every trait method; existing Store behavior preserved; sorted-adjacency determinism enforced.

#### `P1-T08` Implement #356 prerequisite: Store::heap_size_estimate + RSS sampler
- **Tier / role:** sonnet / rust-implementer
- **Depends on:** P1-T06
- **Files:** `crates/mycelium-core/src/store/mem_metrics.rs`
- **Test (TDD, RED first):** test-author writes RED tests asserting heap_size_estimate grows monotonically with node inserts; RSS sampler reads /proc/self/statm (linux), mach task_info (macos), GetProcessMemoryInfo (windows).
- **Acceptance:** #356 lands as a Phase-1 prerequisite; cross-platform sampler returns plausible RSS; documented as the R3 baseline instrument.

#### `P1-T09` Implement RedbBackend read path + 8-table schema + meta version guard
- **Tier / role:** sonnet / rust-implementer
- **Depends on:** P1-T05, P1-T07
- **Files:** `crates/mycelium-core/src/store/redb_backend.rs`, `crates/mycelium-core/src/store/schema.rs`
- **Test (TDD, RED first):** test-author writes RED tests for each read method + SchemaVersion error on future/foreign-version meta; implementer GREEN. RedbBackend is crate-private; access via AsyncBackendHandle.
- **Acceptance:** All read methods implemented; opening a future-schema or foreign redb-major DB returns named SchemaVersion error, never raw redb error.

#### `P1-T10` Format detection in Store::load (redb probe → rmp fallback → UnknownFormat)
- **Tier / role:** sonnet / rust-implementer
- **Depends on:** P1-T09
- **Files:** `crates/mycelium-core/src/store/mod.rs`
- **Test (TDD, RED first):** test-author RED: feed redb file, legacy rmp file, garbage → assert correct dispatch / UnknownFormat; deprecation warn! on legacy load. All behind cfg(feature=redb-backend); default-off = zero behavior change.
- **Acceptance:** With feature off, byte-identical behavior to today; with feature on, reader dispatches correctly; full lane green on both.

### Phase 2 - Equivalence, memory, crash safety (QA on real repos)

#### `P2-T01` Equivalence harness: Backend trait + backend_matrix! macro + 3-layer comparator
- **Tier / role:** opus / test-author
- **Depends on:** P1-T09, P1-T07
- **Files:** `crates/mycelium-qa/src/equiv.rs`, `crates/mycelium-qa/src/lib.rs`
- **Test (TDD, RED first):** Single fn body<B:Backend> run twice via macro. Layer1 NodeId-set identity; Layer2 sorted Trunk+Synapse+side-map equality; Layer3 corpus-derived query workload, order-insensitive EXCEPT documented order-sensitive queries (find_call_path). Authored independently of impl (Dev!=QA).
- **Acceptance:** Harness compiles; minimized-diff on mismatch; runs RED against any unimplemented RedbBackend method.

#### `P2-T02` Synthetic micro-corpus (~500 nodes) for PR fast lane (<10s)
- **Tier / role:** haiku / test-author
- **Depends on:** P2-T01
- **Files:** `crates/mycelium-qa/fixtures/micro/`, `crates/mycelium-qa/src/corpus_synth.rs`
- **Test (TDD, RED first):** Deterministic generated graph covering each language pack + cross-file calls; index twice, assert self-consistent before cross-backend compare.
- **Acceptance:** Fast-lane equivalence runs <10s; keeps Charter §5.6 <5min gate.

#### `P2-T03` Real-repo corpus.lock (SHA-pinned) + fetch/cache script
- **Tier / role:** sonnet / test-author
- **Depends on:** P2-T01
- **Files:** `crates/mycelium-qa/corpus.lock`, `crates/mycelium-qa/scripts/fetch_corpus.sh`, `crates/mycelium-qa/README.md`
- **Test (TDD, RED first):** Pin ripgrep/flask (PR full lane), tokio/express (full), kubernetes/rust/vscode (nightly). Match slate to actually-shipped language packs. Cache keyed on corpus.lock hash; never commit corpus; document licenses + tarball fallback.
- **Acceptance:** Pinned reproducible fetch; full lane uses tiny+small, nightly uses medium; license file present.

#### `P2-T04` Implement RedbBackend write path: begin_write/WriteHandle/commit + upsert/remove
- **Tier / role:** sonnet / rust-implementer
- **Depends on:** P1-T09, P2-T01
- **Files:** `crates/mycelium-core/src/store/redb_backend.rs`
- **Test (TDD, RED first):** Make P2-T01 equivalence GREEN for write+round-trip+reopen. RED-first via the matrix macro (RED on redb, GREEN on in-memory).
- **Acceptance:** Equivalence layers 1-3 green on synthetic corpus for batch build + reopen-from-disk.

#### `P2-T05` Implement replace_file: bidirectional delete + synapse value-MERGE + file_index
- **Tier / role:** sonnet / rust-implementer
- **Depends on:** P2-T04
- **Files:** `crates/mycelium-core/src/store/redb_backend.rs`
- **Test (TDD, RED first):** test-author RED: (a) re-index file removes only its edges fwd+rev incl external incoming; (b) two files share same src node → re-indexing one does NOT drop the other's edges (value-merge); (c) unrelated files untouched; (d) incremental≡batch scoped to same-file edges (cross-file resolved-stub edges excluded, documented).
- **Acceptance:** All replace_file tests green; bidirectional + merge correctness proven; O(edges-of-file) verified on a high-fan-in corpus file.

#### `P2-T06` Crash-injection harness: in-process commit-boundary aborts (PR CI)
- **Tier / role:** opus / test-author
- **Depends on:** P2-T04
- **Files:** `crates/mycelium-qa/src/crash.rs`
- **Test (TDD, RED first):** CrashPoint hook aborts before/around/after commit; reopen asserts graph ∈ {pre,post}-commit, never torn; referential-integrity (no dangling adjacency); reopen idempotent. Authored by QA, not implementer.
- **Acceptance:** Atomic-boundary + integrity invariants green in PR CI; SIGKILL-fuzz variant scheduled nightly.

#### `P2-T07` Bounded-memory (R3) OOM-divergence gate using #356 sampler
- **Tier / role:** opus / test-author
- **Depends on:** P1-T08, P2-T03, P2-T04
- **Files:** `crates/mycelium-qa/src/mem_bounds.rs`, `.github/workflows/ci.yml`
- **Test (TDD, RED first):** Linux: cgroup v2 memory.max cap; control arm asserts InMemory OOMs/crosses cap (#[should_panic]); subject arm asserts redb completes, RSS<cap, heap_size_estimate>>cap; emit RSS-vs-nodes curve, assert redb slope sub-linear. macOS/Windows: soft threshold via #356 sampler (informational). Authored by QA.
- **Acceptance:** Linux gate hard-blocks; control arm keeps OOM premise honest; macOS/Windows soft checks catch gross regressions; curve artifact published.

#### `P2-T08` Perf gates: Charter §2 SLA at 1K/10K/100K + warm/cold p50/p99 + msgpack-decode isolation
- **Tier / role:** opus / test-author
- **Depends on:** P2-T03, P2-T04
- **Files:** `crates/mycelium-qa/benches/sla.rs`, `.github/workflows/ci.yml`
- **Test (TDD, RED first):** criterion benches at all THREE heavy tiers (1K<2s,10K<10s,100K<30s); single-node<5ms, 3-hop<1ms warm, reactive<10ms; measure cold-open p99 (separate <100ms budget); isolate msgpack-decode cost. Two-sided gate: absolute SLA ceiling + relative regression vs committed baseline. PR=fast tiers, pre-release=100K.
- **Acceptance:** All three tiers green; warm SLA met; cold budget documented+met; baseline committed; regression band enforced.

#### `P2-T09` Mutation + branch coverage gates on storage code
- **Tier / role:** opus / test-author
- **Depends on:** P2-T05, P2-T06
- **Files:** `.github/workflows/ci.yml`, `crates/mycelium-core/src/store/`
- **Test (TDD, RED first):** cargo-llvm-cov >=90% lines AND >=80% branch on store/* new code; cargo-mutants >=70% kill on StorageBackend impls + importer. Crash/perf harnesses excluded from line target but their targets (commit/recovery path) covered by crash injection.
- **Acceptance:** Coverage and mutation gates green per Charter §2/§5.4.

#### `P2-T10` mycelium migrate: CLI + byte-identical MCP + Skill/INDEX rows (Three-Surface) + warm-up
- **Tier / role:** sonnet / rust-implementer
- **Depends on:** P2-T04
- **Files:** `crates/mycelium-cli/src/migrate.rs`, `crates/mycelium-mcp/src/tools/migrate.rs`, `skills/index-management/SKILL.md`, `skills/INDEX.md`, `crates/mycelium-core/src/store/import.rs`
- **Test (TDD, RED first):** test-author RED: importer count+checksum verify, tmp-then-atomic-rename, original-untouched on abort, rename-failure re-runnable, TOCTOU exclusive-create, --reindex path, --dry-run/--force/--keep-legacy, file_index reconstruction (edge→source-file). CLI↔MCP byte-identical parity test extended (feature-aware skip when off). Startup warm-up scan.
- **Acceptance:** migrate parity green; INDEX.md + SKILL.md rows present in SAME PR; check_skill_parity.py --strict green; importer safety tests green; migrate runs on a real ripgrep legacy fixture.

### Phase 3 - Flip default + migration (Three-Surface)

#### `P3-T01` Flip redb-backend default ON; new stores create .redb; legacy rmp readable+deprecated
- **Tier / role:** sonnet / rust-implementer
- **Depends on:** P2-T04, P2-T05, P2-T07, P2-T08, P2-T09, P2-T10
- **Files:** `Cargo.toml`, `crates/mycelium-core/Cargo.toml`, `CHANGELOG.md`
- **Test (TDD, RED first):** Full matrix (linux/macos/windows) green with default-on; legacy rmp still reads with warn!. Single-line revertible default change.
- **Acceptance:** Default-on across matrix; CHANGELOG Unreleased updated; RFC-0100 acceptance criteria ticked; revert path documented.

#### `P3-T02` Founder go/no-go gate: 6-point checklist sign-off (Lead-owned)
- **Tier / role:** opus / orchestrator
- **Depends on:** P3-T01
- **Files:** `docs/adr/0008-redb-storage-engine.md`, `.hive/memory/decisions.jsonl`
- **Test (TDD, RED first):** Verify: (1) equivalence green full corpus, (2) OOM-divergence green, (3) crash suite green, (4) all 3 SLA tiers green, (5) >=90% line/>=80% branch/>=70% mutation, (6) ADR signed by founder. No specialist self-merge.
- **Acceptance:** All six checked; decisions.jsonl entry recorded; PR to develop opened (NOT auto-merged) per no-auto-merge memory.

#### `P3-T03` Document batch≠incremental stub-resolution gap + cold-start budget for users
- **Tier / role:** haiku / doc-updater
- **Depends on:** P3-T01
- **Files:** `docs/`, `CHANGELOG.md`, `rfcs/0100-unified-storage-redb.md`
- **Test (TDD, RED first):** Doc-only; cross-check claims against ADR §Consequences.
- **Acceptance:** Watch-mode stub-resolution limitation, cold-start budget, and migrate --reindex recommendation documented; RFC-0100 boxes ticked at correct phase.

### Phase 4 - Retire legacy path

#### `P4-T01` Remove rmp WRITER; redb is sole writer
- **Tier / role:** sonnet / rust-implementer
- **Depends on:** P3-T02
- **Files:** `crates/mycelium-core/src/store/mod.rs`
- **Test (TDD, RED first):** test-author updates tests to expect redb-only writes; legacy reader retained one minor release. Full matrix green.
- **Acceptance:** No rmp write path; legacy read still works with deprecation; one full release after Phase 3.

#### `P4-T02` Remove rmp READER + redb-backend feature gate (redb unconditional)
- **Tier / role:** sonnet / rust-implementer
- **Depends on:** P4-T01
- **Files:** `crates/mycelium-core/Cargo.toml`, `Cargo.toml`, `crates/mycelium-core/src/store/`
- **Test (TDD, RED first):** migrate errors helpfully on unreadable very-old files; feature gate removed; matrix green. Separate ADR/RFC acceptance tick.
- **Acceptance:** redb unconditional; legacy reader gone after one-minor window; migrate gives helpful error for too-old files.

#### `P4-T03` Mechanical cleanup: dead rmp helpers, cfg(feature) stubs, doc references
- **Tier / role:** haiku / mechanical-cleanup
- **Depends on:** P4-T02
- **Files:** `crates/mycelium-core/src/`, `docs/`
- **Test (TDD, RED first):** cargo clippy --all-features -D warnings; no orphaned cfg blocks; cargo test --all green.
- **Acceptance:** No dead code, no stale feature gates, clippy clean.

---
*Each task is build-ready; implementation starts only after ADR-0008 is signed.*