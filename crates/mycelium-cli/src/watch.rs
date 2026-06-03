//! `mycelium watch` — foreground reactive watch mode (RFC-0105).
//!
//! Drives `mycelium_core::watch::WatchEngine` from the CLI. Seed the store
//! via the existing `index_path_parallel` (matching `mycelium index`), then
//! attach the OS-level watcher synchronously, then drive the async loop on
//! a local multi-thread tokio runtime until SIGINT.
//!
//! The reactive *behavior* (debounce, ignore matching, per-file re-extract)
//! is byte-identical to the MCP server by construction — both surfaces drive
//! the same `WatchEngine`.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use mycelium_core::store::Store;
use mycelium_core::watch::{
    BatchDelta, CancelToken, FileReindexer, WatchConfig, WatchEngine, WatchEvent,
};
use tokio::sync::RwLock;

use crate::index::{Extractors, index_path_parallel};

/// Bridge to the CLI's existing per-extension extractors so the watch loop
/// re-indexes a changed file the same way `mycelium index` would.
struct CliReindexer {
    extractors: Extractors,
}

impl FileReindexer for CliReindexer {
    fn reindex(&self, rel: &str, src: &[u8], ext: &str, store: &mut Store) {
        let extractor = match ext {
            "js" | "jsx" => &self.extractors.js,
            "py" | "pyi" => &self.extractors.python,
            "ts" => &self.extractors.ts,
            "tsx" => &self.extractors.tsx,
            "rs" => &self.extractors.rs,
            "go" => &self.extractors.go,
            "java" => &self.extractors.java,
            "c" | "h" => &self.extractors.c,
            "rb" => &self.extractors.ruby,
            "cpp" | "cc" | "cxx" | "hpp" => &self.extractors.cpp,
            "cs" => &self.extractors.csharp,
            _ => return,
        };
        let _ = extractor.extract(rel, src, store);
    }
}

/// Foreground watch entry point. Blocks until SIGINT (Ctrl-C).
#[allow(clippy::redundant_pub_crate)]
pub(super) fn run_foreground(root: &Path, debounce_ms: u64) -> Result<()> {
    // Build a multi-thread runtime so the engine's async loop, the
    // notify callback (sync), and the Ctrl-C handler can all make progress.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("building tokio runtime for watch")?;

    runtime.block_on(async move {
        // 1. Seed the store with an initial parallel index.
        eprintln!(
            "mycelium watch: seeding initial index of {} ...",
            root.display()
        );
        let (store, stats) = index_path_parallel(root, None).context("initial index for watch")?;
        eprintln!(
            "mycelium watch: indexed {} files ({} errors); watching for changes (Ctrl-C to stop)",
            stats.files, stats.errors
        );

        let store = Arc::new(RwLock::new(store));

        // 2. Build the reindexer.
        let extractors = Extractors::build().context("building reindexer extractors")?;
        let reindexer = CliReindexer { extractors };

        // 3. Attach the watcher synchronously (race-free; matches MCP).
        let cfg = WatchConfig {
            root: root.to_path_buf(),
            debounce: Duration::from_millis(debounce_ms),
        };
        let session = WatchEngine::attach(&cfg).context("attaching watch session")?;

        let cancel = CancelToken::new();
        let cancel_signal = cancel.clone();

        // 4. Drive the engine in a task; race against ctrl_c.
        let store_drive = Arc::clone(&store);
        let drive = tokio::spawn(async move {
            let on_batch = move |ev: &WatchEvent, _delta: &BatchDelta, _store: &Store| {
                eprintln!(
                    "mycelium watch: batch {} — reindexed {} file(s): {}",
                    ev.batch_seq,
                    ev.changed_files.len(),
                    ev.changed_files.join(", "),
                );
            };
            WatchEngine::drive(
                session,
                cfg,
                store_drive,
                &reindexer,
                None, // CLI doesn't keep a Salsa Cortex (Phase-2 opt-in later)
                on_batch,
                cancel,
            )
            .await
        });

        // 5. Wait for SIGINT, then signal cancel and drain.
        let _ = tokio::signal::ctrl_c().await;
        eprintln!("mycelium watch: stop signal received, draining ...");
        cancel_signal.cancel();
        let _ = drive.await;
        eprintln!("mycelium watch: stopped");

        Ok::<(), anyhow::Error>(())
    })
}
