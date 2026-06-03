# RFC-0105: Shared watch engine + CLI watch surface

- **RFC**: 0105
- **Title**: Extract the watch loop into `mycelium_core::watch` and add a `mycelium watch` CLI command
- **Status**: Partially Implemented (core engine + MCP rewire + CLI `watch` command landed; `--status` subcommand and CLI integration test deferred to follow-up). **EXCEPTION ratified by founder 2026-06-03** (Charter §5.13 / RFC-0090) — the foreground CLI watch vs the server's background `start_watch`/`stop_watch`/`watch_status` is a documented lifecycle mismatch; both drive the same `WatchEngine` so reactive behavior is byte-identical by construction.
- **Author**: .hive team (design workflow) + rust-implementer
- **Created**: 2026-06-03
- **Tracking**: reactive-completion roadmap, step 1 of 4 (watch → push → subscribe → salsa)
- **Decision gate**: Three-Surface Rule (Charter §5.13) — carries an `EXCEPTION:` line (below)
- **Supersedes / amends**: extends [RFC-0008 Watch Mode](0008-watch-mode.md) (today MCP-only)

> **`EXCEPTION:` (Charter §5.13 / RFC-0090).** Watch is the one capability where CLI
> and MCP are *not* byte-identical tools, because their lifecycles genuinely differ:
> MCP is a long-lived server with `start_watch` / `stop_watch` / `watch_status`
> (background, server-managed); the CLI is a one-shot process, so `mycelium watch`
> is a **foreground blocking** command that exits on Ctrl-C. This is a documented
> surface-shaped variant, not fakeable parity. The parity bridge is twofold:
> (1) **both surfaces drive the identical `WatchEngine`**, so reactive *behavior*
> (debounce, ignore rules, re-extract, redb persist) is byte-identical by
> construction; (2) `mycelium watch --status` emits the **byte-identical**
> `watch_status` JSON, locking a 1:1 introspection anchor.

---

## 1. Summary

The reactive watch loop (RFC-0008) exists only inside `MyceliumServer::start_watch`
(`crates/mycelium-mcp/src/lib.rs`). Extract it into a surface-agnostic
**`mycelium_core::watch::WatchEngine`** that both the MCP server and a new
**`mycelium watch` CLI command** call. This (a) gives the reactive feature its
missing CLI face, (b) makes reactive behavior identical across surfaces *by
construction* (the shared-core pattern that worked for `context` / `OutputBudget`),
and (c) creates the deliberate **`on_batch` emit seam** that PUSH (RFC-0106) and
SUBSCRIBE (RFC-0107) bolt onto without re-touching the loop.

## 2. Design

### 2.1 New `mycelium_core::watch` module

```rust
pub struct WatchConfig { pub root: PathBuf, pub debounce: Duration } // default 5ms (Charter §2 reactive <10ms)

pub struct WatchEvent {
    pub root: PathBuf,
    pub changed_files: Vec<String>,  // sorted, deduped, rel, '/'-normalized
    pub removed_files: Vec<String>,
    pub batch_seq: u64,
}

/// Re-extract one file into the Store. Default impl owns the Extractor + all
/// language grammars (relocated from the MCP `reindex_file` body).
pub trait FileReindexer { fn reindex(&self, rel: &str, src: &[u8], ext: &str, store: &mut Store); }

impl WatchEngine {
    /// Owns notify::RecommendedWatcher + mpsc + debounce + GitignoreBuilder
    /// (.gitignore/.myceliumignore) + the hardcoded target/.mycelium skip +
    /// per-file remove_file → (cortex? apply_to_store) → reindexer.reindex →
    /// resolve_bare_call_stubs. Calls `on_batch(&WatchEvent)` AFTER the store
    /// mutation + drop(store_w). Selects against `cancel` so Ctrl-C drains the
    /// in-flight batch then returns.
    pub async fn run<F: FnMut(&WatchEvent) + Send>(
        cfg: WatchConfig,
        store: Arc<RwLock<Store>>,
        reindexer: &dyn FileReindexer,
        cortex: Option<Arc<tokio::sync::Mutex<Cortex>>>,
        on_batch: F,
        cancel: CancellationToken,
    ) -> anyhow::Result<()>;
}
```

- Adds `notify` + `tokio-util` (CancellationToken) to `crates/mycelium-core/Cargo.toml`.
- Moves `persist_redb_watch_batch` and `source_extension` into core so **CLI and
  MCP share one persist impl** → byte-identical redb writes by construction.
- The loop body is lifted **verbatim** from the current MCP loop
  (`lib.rs` ~1643–1738) — no behavior change.

### 2.2 MCP rewire

`start_watch` shrinks to: build `WatchConfig`, define
`on_batch = |ev| { watch_state.batches_processed.fetch_add(1, ..); persist_watch_batch(&ev.root, &store_r, &ev.changed_files) }`,
spawn `WatchEngine::run(.., Some(cortex), on_batch, ..)`, stash the abort handle.
`mycelium_watch_status` is **unchanged**. The full existing MCP watch test suite
is the no-regression gate.

### 2.3 CLI command

`Cmd::Watch { root: PathBuf, --debounce-ms=5, --status }`. `main` stays sync; the
dispatch arm builds a local multi-thread tokio runtime and `block_on`s a foreground
watch: seed via `index_path_parallel`, initial redb persist, then
`select! { WatchEngine::run(.., None /*CLI cortex opt-in is Salsa Phase 2*/, ..), ctrl_c => cancel }`.
`mycelium watch --status` prints the byte-identical `watch_status` JSON.

### 2.4 Three-Surface / Skill

Surface-shaped variant per the `EXCEPTION:` above. Skill home =
**`skills/index-management/SKILL.md`** (the only existing Skill referencing watch);
add `mycelium watch` + the watch trio to its `allowed-tools`.

## 3. Acceptance criteria (RED-first)

- [x] `core::watch::tests::engine_reindexes_changed_file_and_emits_event` — change a file → Store gains the symbol AND `on_batch` fired a `WatchEvent` containing the rel path.
- [x] `core::watch::tests::cancellation_drains_final_batch_then_stops` — fire a change, cancel → engine returns after the in-flight batch.
- [x] `core::watch::tests::ignore_rules_skip_target_and_gitignored` — writes under `target/` and a gitignored path emit no `WatchEvent`.
- [x] **No-regression**: the existing MCP `watch` test suite passes unchanged after `start_watch` is refactored to drive `WatchEngine` (verified locally; the 4 timing-sensitive `#[ignore]`'d tests have the same pass/fail behavior as on develop).
- [ ] `cli` integration test: `mycelium watch <tmp>` child via `assert_cmd`, write a file, poll the index until the symbol appears, SIGINT, exit 0. **(Deferred — process-spawning test is inherently flaky; follow-up.)**
- [ ] `mycelium watch --status` subcommand emitting the byte-identical `watch_status` JSON. **(Deferred — needs an MCP-style WatchState persisted to disk; follow-up.)**
- [x] Quality gate green (fmt, clippy `-D warnings` clean, `cargo test --workspace` 0 failures).

## 4. Why this is step 1

PUSH (RFC-0106) and SUBSCRIBE (RFC-0107) both attach to the `on_batch` /
`on_committed` seam this creates; the shared-core extraction is what makes
byte-identical reactive behavior across surfaces possible by construction rather
than hand-synced. SALSA Phase 2 is independent and may run on a parallel branch.

## 5. Risks

- **Regression to the working MCP watch** — mitigated: the MCP-rewrite step is
  gated on the full existing MCP watch suite passing unchanged; `WatchState` and
  `mycelium_watch_status` are untouched; the loop body moves verbatim.
- **New core deps** (`notify`, `tokio-util`) — both already vetted workspace deps;
  flag for `cargo-deny`, no new crate enters the tree.
- **`reindex_file` relocation** pulls the grammar `QUERIES` consts into core — core
  already owns `Extractor` + the grammars; the `QUERIES` move with it.
