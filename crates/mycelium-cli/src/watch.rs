//! `mycelium watch` — foreground reactive watch mode (RFC-0105 + RFC-0107).
//!
//! Drives `mycelium_core::watch::WatchEngine` from the CLI. Seed the store
//! via the existing `index_path_parallel` (matching `mycelium index`), then
//! attach the OS-level watcher synchronously, then drive the async loop on
//! a local multi-thread tokio runtime until SIGINT.
//!
//! The reactive *behavior* (debounce, ignore matching, per-file re-extract)
//! is byte-identical to the MCP server by construction — both surfaces drive
//! the same `WatchEngine`.
//!
//! With `--subscribe <SPEC>` (RFC-0107) the CLI also registers an in-process
//! SUBSCRIBE interest and prints one `mycelium/subscriptionDelta` payload to
//! stdout per matching batch as NDJSON. The byte-identical contract with the
//! MCP `mycelium_subscribe` tool is asserted by
//! `tests/contract_subscription.rs`.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use mycelium_core::store::Store;
use mycelium_core::watch::{
    BatchDelta, CancelToken, FileReindexer, WatchConfig, WatchEngine, WatchEvent,
};
use mycelium_mcp::subscription;
use tokio::sync::RwLock;

use crate::index::{Extractors, index_path_parallel};

/// Parse a CLI `--subscribe <SPEC>` string into an [`subscription::Interest`].
///
/// SPEC grammar (RFC-0107 §4.3):
/// - `files:<glob1>,<glob2>,...`
/// - `symbols:<glob1>,<glob2>,...`
/// - `selector:<hyphae source>`  (everything after the first `:` is the source)
fn parse_subscribe_spec(spec: &str) -> Result<subscription::Interest> {
    let (kind, rest) = spec
        .split_once(':')
        .ok_or_else(|| anyhow!("--subscribe SPEC must be `<kind>:<rest>`"))?;
    match kind {
        "files" => {
            let paths: Vec<String> = rest
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            if paths.is_empty() {
                return Err(anyhow!("--subscribe files: requires at least one glob"));
            }
            Ok(subscription::Interest::Files { paths })
        }
        "symbols" => {
            let paths: Vec<String> = rest
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            if paths.is_empty() {
                return Err(anyhow!("--subscribe symbols: requires at least one glob"));
            }
            Ok(subscription::Interest::Symbols { paths })
        }
        "selector" => Ok(subscription::Interest::Selector {
            hyphae: rest.to_owned(),
        }),
        other => Err(anyhow!(
            "--subscribe SPEC kind must be `files|symbols|selector`, got `{other}`"
        )),
    }
}

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
///
/// When `subscribe_spec` is `Some`, register the parsed Interest against an
/// in-process subscription store and stream per-batch
/// `mycelium/subscriptionDelta` payloads to stdout as NDJSON. The wire shape
/// is byte-identical to what the MCP server emits for the same Interest
/// (asserted by `tests/contract_subscription.rs`).
#[allow(clippy::redundant_pub_crate)]
pub(super) fn run_foreground(
    root: &Path,
    debounce_ms: u64,
    subscribe_spec: Option<&str>,
    subscribe_id: Option<&str>,
    subscribe_ttl: Option<u64>,
) -> Result<()> {
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

        // 1b. SUBSCRIBE (RFC-0107): if a `--subscribe` spec was supplied,
        // register the Interest with an in-process subscription store now,
        // so the on_batch closure below can fan-out matched payloads.
        let subscriptions = subscription::new_store();
        let registered_sub_id: Option<String> = if let Some(spec) = subscribe_spec {
            let interest = parse_subscribe_spec(spec).context("parsing --subscribe SPEC")?;
            let req = subscription::SubscribeRequest {
                subscription_id: subscribe_id.map(str::to_owned),
                interest,
                ttl_seconds: subscribe_ttl,
                root: None,
            };
            let resp = subscription::subscribe(
                &subscriptions,
                req,
                "cli-foreground".to_owned(),
                root.to_path_buf(),
            )
            .await
            .map_err(|e| anyhow!("subscribe failed: {e}"))?;
            eprintln!(
                "mycelium watch: subscription {} (kind={}, ttl={}s) registered",
                resp.subscription_id, resp.interest_kind, resp.ttl_seconds
            );
            Some(resp.subscription_id)
        } else {
            None
        };

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
        let subscriptions_drive = Arc::clone(&subscriptions);
        let registered_sub_id_drive = registered_sub_id.clone();
        let drive = tokio::spawn(async move {
            let on_batch = move |ev: &WatchEvent, delta: &BatchDelta, store_r: &Store| {
                eprintln!(
                    "mycelium watch: batch {} — reindexed {} file(s): {}",
                    ev.batch_seq,
                    ev.changed_files.len(),
                    ev.changed_files.join(", "),
                );
                // RFC-0107 + RFC-0108 SUBSCRIBE: stream matched payloads as
                // NDJSON on stdout, byte-identical to the MCP
                // `mycelium_subscribe` wire shape (one event per match per
                // batch — `SubscriptionDeltaEvent` for file/symbol/selector,
                // `QueryResultChangedEvent` for query subscriptions).
                if let Some(sub_id) = &registered_sub_id_drive {
                    let r = subscriptions_drive.blocking_read();
                    if let Some(sub) = r.by_id.get(sub_id) {
                        match subscription::match_batch(sub, ev, delta, store_r) {
                            Some(subscription::BatchMatch::Delta(payload)) => {
                                if let Ok(line) = serde_json::to_string(&payload) {
                                    println!("{line}");
                                }
                            }
                            Some(subscription::BatchMatch::QueryDelta(payload)) => {
                                if let Ok(line) = serde_json::to_string(&payload) {
                                    println!("{line}");
                                }
                            }
                            Some(subscription::BatchMatch::PauseQuery { .. }) | None => {}
                        }
                    }
                }
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
