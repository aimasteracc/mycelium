//! Reactive file-watching engine (RFC-0105 / extends RFC-0008).
//!
//! The reactive watch loop used to live only inside `MyceliumServer::start_watch`
//! (in `mycelium-mcp`). This module is the surface-agnostic extraction: both the
//! MCP server and the CLI `mycelium watch` command drive [`WatchEngine`] with
//! their own [`FileReindexer`] implementation, so reactive behavior — debounce,
//! ignore rules, per-file re-extract, cross-file stub resolution — is **byte-
//! identical across surfaces by construction** (the shared-core pattern proven
//! by `mycelium_core::context` and `mycelium_core::budget`).
//!
//! Three-Surface (Charter §5.13): RFC-0105 declares an `EXCEPTION:` — a
//! foreground CLI `watch` vs the server's background `start_watch`/
//! `stop_watch`/`watch_status` is a genuine lifecycle mismatch. The parity
//! bridge is this shared engine plus a byte-identical `watch --status` JSON.
//!
//! The [`WatchEngine::run`] callback `on_batch(&WatchEvent)` is the deliberate
//! emit seam for the next two roadmap steps: PUSH (RFC-0106) and SUBSCRIBE
//! (RFC-0107) bolt onto it without re-touching the loop.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::RwLock;
use tokio::time::{Instant, timeout_at};

use crate::cortex::Cortex;
use crate::store::Store;

// ── public types ─────────────────────────────────────────────────────────────

/// Tunables for [`WatchEngine::run`].
#[derive(Debug, Clone)]
pub struct WatchConfig {
    /// Directory to watch recursively.
    pub root: PathBuf,
    /// Window during which subsequent file-system events are merged into the
    /// current batch. Default 5 ms — matches the Charter §2 reactive
    /// `< 10 ms` SLA and the original MCP value.
    pub debounce: Duration,
}

impl WatchConfig {
    /// New config with the default 5 ms debounce.
    #[must_use]
    pub const fn new(root: PathBuf) -> Self {
        Self {
            root,
            debounce: Duration::from_millis(5),
        }
    }
}

/// One committed batch of file-system changes after debounce + filtering.
///
/// Emitted via the `on_batch` callback **after** the store has been mutated
/// and the write-lock dropped — so handlers can observe the new state.
#[derive(Debug, Clone)]
pub struct WatchEvent {
    /// The root passed to [`WatchConfig`].
    pub root: PathBuf,
    /// Repository-relative paths whose store contents were re-indexed in this
    /// batch. Sorted, deduped, `/`-normalized.
    pub changed_files: Vec<String>,
    /// Monotonic batch counter, starting at 1.
    pub batch_seq: u64,
}

/// Surface-supplied per-file re-extraction. The MCP server and the CLI each
/// implement this with their own grammar/QUERIES table; the watch engine itself
/// is grammar-agnostic.
pub trait FileReindexer: Send + Sync {
    /// Re-extract `src` (`ext` is the file extension without the dot) into the
    /// supplied `store`. The store has already had `remove_file(rel)` called
    /// before this method runs.
    fn reindex(&self, rel: &str, src: &[u8], ext: &str, store: &mut Store);
}

/// A cooperative cancellation flag for [`WatchEngine::run`].
///
/// Signal `cancel()` from any thread (Ctrl-C handler, MCP `stop_watch`, etc.);
/// the engine drains the in-flight batch, persists it via `on_batch`, then
/// returns.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// Create a fresh, un-signalled token.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Signal cancellation. Idempotent.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// `true` once [`Self::cancel`] has been called.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

// ── extension filter ─────────────────────────────────────────────────────────

/// Source-file extensions the watch engine re-extracts.
///
/// Single source of truth for both surfaces (previously duplicated in
/// `mycelium-mcp::source_extension`). Add a new extension here only when a
/// language pack has been wired into the surface reindexers.
#[must_use]
pub fn source_extension(path: &Path) -> Option<&str> {
    let ext = path.extension().and_then(|e| e.to_str())?;
    matches!(
        ext,
        "js" | "jsx"
            | "py"
            | "pyi"
            | "ts"
            | "tsx"
            | "rs"
            | "go"
            | "java"
            | "c"
            | "h"
            | "rb"
            | "cpp"
            | "cc"
            | "cxx"
            | "hpp"
            | "cs"
    )
    .then_some(ext)
}

/// `true` when `path` is a repository-relative supported source file.
#[must_use]
pub fn is_supported_source_rel(path: &str) -> bool {
    source_extension(Path::new(path)).is_some()
}

// ── engine ───────────────────────────────────────────────────────────────────

/// A live `notify` watcher with its event channel.
///
/// Returned by [`WatchEngine::attach`] so callers can guarantee the OS-level
/// recursive watch is up *before* they spawn the async loop. The MCP server
/// constructs one in the synchronous prologue of `start_watch`, then spawns
/// [`WatchEngine::drive`] in a task — eliminating the race where a file
/// written immediately after `start_watch().await` could be missed.
#[must_use]
pub struct WatchSession {
    watcher: RecommendedWatcher,
    rx: tokio::sync::mpsc::UnboundedReceiver<notify::Event>,
    gitignore: ignore::gitignore::Gitignore,
}

/// Reactive watch engine — debounce + ignore + re-extract loop.
pub struct WatchEngine;

impl WatchEngine {
    /// Synchronously create the `notify` watcher and attach the recursive
    /// watch on `cfg.root`, plus compose the ignore matcher. The returned
    /// [`WatchSession`] must be passed to [`Self::drive`] — typically in a
    /// `tokio::spawn` so the loop runs in the background.
    ///
    /// # Errors
    /// Returns an error if the watcher cannot be created or `cfg.root` cannot
    /// be watched.
    pub fn attach(cfg: &WatchConfig) -> anyhow::Result<WatchSession> {
        use anyhow::Context as _;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<notify::Event>();

        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                if let Ok(ev) = res {
                    tx.send(ev).ok();
                }
            },
            Config::default(),
        )
        .context("creating file system watcher")?;

        watcher
            .watch(&cfg.root, RecursiveMode::Recursive)
            .context("starting recursive watch")?;

        let gitignore = {
            let mut gb = ignore::gitignore::GitignoreBuilder::new(&cfg.root);
            for name in [".gitignore", ".myceliumignore"] {
                let p = cfg.root.join(name);
                if p.exists() {
                    gb.add(p);
                }
            }
            gb.build()
                .unwrap_or_else(|_| ignore::gitignore::Gitignore::empty())
        };

        Ok(WatchSession {
            watcher,
            rx,
            gitignore,
        })
    }
    /// Drive the watch loop until `cancel` fires.
    ///
    /// # Behaviour (byte-identical to the original MCP loop)
    /// 1. Build a `notify::RecommendedWatcher` recursive on `cfg.root`.
    /// 2. Compose a `.gitignore` + `.myceliumignore` matcher rooted at `cfg.root`.
    /// 3. For each batch of file-system events (debounced by `cfg.debounce`):
    ///    - drop paths under `target/` or `.mycelium/`;
    ///    - drop paths matched by the ignore matcher;
    ///    - for each remaining supported source file: `store.remove_file(rel)`,
    ///      optionally update the salsa `Cortex` cache + apply, then call
    ///      `reindexer.reindex(rel, src, ext, &mut store)`.
    /// 4. After the whole batch: `store.resolve_bare_call_stubs()` and **drop
    ///    the write lock**.
    /// 5. Fire `on_batch(&WatchEvent)` so callers can persist / notify.
    ///
    /// # Cancellation
    /// `cancel` is checked between batches. The in-flight batch always
    /// completes its `on_batch` call before `run()` returns.
    ///
    /// # Errors
    /// Returns an error only if the underlying `notify` watcher cannot be
    /// created or attached to `root`. All per-batch / per-file errors are
    /// logged via `tracing::warn` and never abort the loop.
    #[allow(clippy::too_many_lines)] // single coherent loop; splitting fragments state
    pub async fn drive<F>(
        session: WatchSession,
        cfg: WatchConfig,
        store: Arc<RwLock<Store>>,
        reindexer: &dyn FileReindexer,
        cortex: Option<Arc<tokio::sync::Mutex<Cortex>>>,
        mut on_batch: F,
        cancel: CancelToken,
    ) -> anyhow::Result<()>
    where
        F: FnMut(&WatchEvent, &Store) + Send,
    {
        let WatchSession {
            watcher,
            mut rx,
            gitignore,
        } = session;
        let _watcher = watcher; // keep alive for the function lifetime
        let mut batch_seq: u64 = 0;

        loop {
            if cancel.is_cancelled() {
                return Ok(());
            }

            // Wait for the first event of a batch, but stop quickly if cancelled.
            let first = tokio::select! {
                ev = rx.recv() => match ev {
                    Some(ev) => ev,
                    None => return Ok(()),         // channel closed
                },
                () = wait_for_cancel(&cancel) => return Ok(()),
            };

            let mut batch: Vec<PathBuf> = first.paths;

            // Debounce: collect additional events arriving within `cfg.debounce`.
            let deadline = Instant::now() + cfg.debounce;
            while let Ok(Some(ev)) = timeout_at(deadline, rx.recv()).await {
                batch.extend(ev.paths);
            }

            batch.sort_unstable();
            batch.dedup();

            let mut changed_files: Vec<String> = Vec::new();
            {
                let mut store_w = store.write().await;

                for abs_path in &batch {
                    // 1. Skip target/ and .mycelium/ unconditionally.
                    let always_skip = abs_path
                        .strip_prefix(&cfg.root)
                        .ok()
                        .and_then(|rel| rel.components().next())
                        .is_some_and(|first_comp| {
                            matches!(
                                first_comp.as_os_str().to_string_lossy().as_ref(),
                                "target" | ".mycelium"
                            )
                        });
                    if always_skip {
                        continue;
                    }
                    // 2. Honour .gitignore / .myceliumignore.
                    if gitignore.matched(abs_path, abs_path.is_dir()).is_ignore() {
                        continue;
                    }

                    let rel = abs_path
                        .strip_prefix(&cfg.root)
                        .unwrap_or(abs_path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    let Some(ext) = source_extension(abs_path) else {
                        continue;
                    };

                    // Remove old data for this file regardless of event kind.
                    store_w.remove_file(&rel);

                    // Re-extract if the file still exists.
                    if abs_path.is_file() {
                        if let Ok(src) = std::fs::read(abs_path) {
                            // Salsa cache update + apply (mirrors original MCP loop
                            // for callers that supply a Cortex). The reindexer call
                            // below still runs so edge kinds the FileIndex does
                            // not yet propagate (calls, imports, …) are populated.
                            if let Some(cx) = &cortex {
                                let file = cx.lock().await.set_file(abs_path.clone(), src.clone());
                                let idx = cx.lock().await.query_file(file);
                                idx.apply_to_store(&rel, &mut store_w);
                            }
                            reindexer.reindex(&rel, &src, ext, &mut store_w);
                        }
                    }
                    changed_files.push(rel);
                }
                store_w.resolve_bare_call_stubs();
                drop(store_w);
            }

            // After mutation + lock-drop: emit the batch event with a fresh
            // read lock so callers can persist / inspect without doing their
            // own async dance (calling `.blocking_read()` from inside an async
            // runtime deadlocks).
            if changed_files.is_empty() {
                // Pure-noise batch (everything filtered): trace and continue.
                tracing::trace!("watch: empty batch after filtering");
            } else {
                changed_files.sort_unstable();
                changed_files.dedup();
                batch_seq += 1;
                let ev = WatchEvent {
                    root: cfg.root.clone(),
                    changed_files,
                    batch_seq,
                };
                let store_r = store.read().await;
                on_batch(&ev, &store_r);
                drop(store_r);
            }

            if cancel.is_cancelled() {
                return Ok(());
            }
        }
    }

    /// Convenience: [`Self::attach`] then [`Self::drive`] in one call.
    ///
    /// Suitable for callers that drive the engine from the **same** task they
    /// were already running (CLI foreground `watch`, integration tests).
    /// Long-lived server callers that need the OS-level watch to be live
    /// *before* their setup function returns should use `attach` then spawn
    /// `drive` separately.
    ///
    /// # Errors
    /// See [`Self::attach`].
    pub async fn run<F>(
        cfg: WatchConfig,
        store: Arc<RwLock<Store>>,
        reindexer: &dyn FileReindexer,
        cortex: Option<Arc<tokio::sync::Mutex<Cortex>>>,
        on_batch: F,
        cancel: CancelToken,
    ) -> anyhow::Result<()>
    where
        F: FnMut(&WatchEvent, &Store) + Send,
    {
        let session = Self::attach(&cfg)?;
        Self::drive(session, cfg, store, reindexer, cortex, on_batch, cancel).await
    }
}

/// Resolve as soon as `cancel` is signalled. Used inside `tokio::select!` so
/// the engine can wake up promptly on cancellation while idle.
async fn wait_for_cancel(cancel: &CancelToken) {
    while !cancel.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[cfg(test)]
mod tests;
