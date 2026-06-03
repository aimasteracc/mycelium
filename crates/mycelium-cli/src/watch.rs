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
use mycelium_mcp::subscription::{self, QuerySpec};
use tokio::sync::RwLock;

use crate::index::{Extractors, index_path_parallel};

/// Parse a CLI `--subscribe <SPEC>` string into an [`subscription::Interest`].
///
/// SPEC grammar (RFC-0107 §4.3 + RFC-0108 §4.3):
/// - `files:<glob1>,<glob2>,...`
/// - `symbols:<glob1>,<glob2>,...`
/// - `selector:<hyphae source>`  (everything after the first `:` is the source)
/// - `query:selector:<hyphae>`
/// - `query:callers:<path>[,hops=N]`
/// - `query:callees:<path>[,hops=N]`
/// - `query:impact:<path>[,max_paths=N]`
/// - `query:context:<task>,focus=p1+p2+...,max_tokens=N`
///
/// For `query:` variants, an optional comma-separated `key=value` tail follows
/// the path. `focus` is `+`-separated (not comma — comma is the outer separator).
///
/// Returns an `Interest::Query { query, min_interval_seconds: min_interval_secs }`
/// when the spec starts with `query:`. The caller supplies the
/// `min_interval_secs` from the `--subscribe-min-interval` flag.
fn parse_subscribe_spec(
    spec: &str,
    min_interval_secs: Option<u64>,
) -> Result<subscription::Interest> {
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
        "query" => {
            let query = parse_query_subspec(rest)?;
            Ok(subscription::Interest::Query {
                query,
                min_interval_seconds: min_interval_secs,
            })
        }
        other => Err(anyhow!(
            "--subscribe SPEC kind must be `files|symbols|selector|query`, got `{other}`"
        )),
    }
}

/// Parse the tail after `query:` — `<kind>:<args>` per RFC-0108 §4.3.
fn parse_query_subspec(rest: &str) -> Result<QuerySpec> {
    let (qkind, qrest) = rest
        .split_once(':')
        .ok_or_else(|| anyhow!("query: missing sub-kind (expected `query:<kind>:<args>`)"))?;
    match qkind {
        "selector" => Ok(QuerySpec::Selector {
            hyphae: qrest.to_owned(),
        }),
        "callers" => {
            let (path, kv) = split_path_and_kv(qrest);
            let hops = kv.get("hops").map(|s| s.parse::<u32>()).transpose()?;
            Ok(QuerySpec::Callers { path, hops })
        }
        "callees" => {
            let (path, kv) = split_path_and_kv(qrest);
            let hops = kv.get("hops").map(|s| s.parse::<u32>()).transpose()?;
            Ok(QuerySpec::Callees { path, hops })
        }
        "impact" => {
            let (path, kv) = split_path_and_kv(qrest);
            let max_paths = kv.get("max_paths").map(|s| s.parse::<u32>()).transpose()?;
            Ok(QuerySpec::Impact { path, max_paths })
        }
        "context" => {
            let (task, kv) = split_path_and_kv(qrest);
            let focus: Vec<String> = kv
                .get("focus")
                .map(|s| s.split('+').map(str::to_owned).collect())
                .unwrap_or_default();
            let max_tokens = kv.get("max_tokens").map(|s| s.parse::<u32>()).transpose()?;
            Ok(QuerySpec::Context {
                task,
                focus,
                max_tokens,
            })
        }
        other => Err(anyhow!(
            "query: sub-kind must be `selector|callers|callees|impact|context`, got `{other}`"
        )),
    }
}

/// Split a `<path>[,key1=v1,key2=v2,...]` string into `(path, kv_map)`.
/// The path is everything before the first `,`; remaining `,`-separated
/// segments are parsed as `key=value` pairs.
fn split_path_and_kv(s: &str) -> (String, std::collections::BTreeMap<String, String>) {
    let mut parts = s.split(',');
    let path = parts.next().unwrap_or("").to_owned();
    let mut kv = std::collections::BTreeMap::new();
    for p in parts {
        if let Some((k, v)) = p.split_once('=') {
            kv.insert(k.trim().to_owned(), v.trim().to_owned());
        }
    }
    (path, kv)
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
    subscribe_min_interval: Option<u64>,
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
            let interest = parse_subscribe_spec(spec, subscribe_min_interval)
                .context("parsing --subscribe SPEC")?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_files_spec() {
        let i = parse_subscribe_spec("files:src/*.rs,src/auth/*", None).unwrap();
        match i {
            subscription::Interest::Files { paths } => {
                assert_eq!(paths, vec!["src/*.rs", "src/auth/*"]);
            }
            _ => panic!("expected Files"),
        }
    }

    #[test]
    fn parse_query_callers_with_hops() {
        let i = parse_subscribe_spec("query:callers:src/a.rs>fn:b,hops=2", Some(10)).unwrap();
        match i {
            subscription::Interest::Query {
                query: QuerySpec::Callers { path, hops },
                min_interval_seconds,
            } => {
                assert_eq!(path, "src/a.rs>fn:b");
                assert_eq!(hops, Some(2));
                assert_eq!(min_interval_seconds, Some(10));
            }
            other => panic!("expected Query::Callers, got {other:?}"),
        }
    }

    #[test]
    fn parse_query_impact_with_max_paths() {
        let i = parse_subscribe_spec("query:impact:src/a.rs>fn:b,max_paths=42", None).unwrap();
        match i {
            subscription::Interest::Query {
                query: QuerySpec::Impact { path, max_paths },
                ..
            } => {
                assert_eq!(path, "src/a.rs>fn:b");
                assert_eq!(max_paths, Some(42));
            }
            _ => panic!("expected Query::Impact"),
        }
    }

    #[test]
    fn parse_query_context_with_focus_and_tokens() {
        let i = parse_subscribe_spec(
            "query:context:auth,focus=src/a.rs+src/b.rs,max_tokens=4000",
            None,
        )
        .unwrap();
        match i {
            subscription::Interest::Query {
                query:
                    QuerySpec::Context {
                        task,
                        focus,
                        max_tokens,
                    },
                ..
            } => {
                assert_eq!(task, "auth");
                assert_eq!(focus, vec!["src/a.rs", "src/b.rs"]);
                assert_eq!(max_tokens, Some(4000));
            }
            _ => panic!("expected Query::Context"),
        }
    }

    #[test]
    fn parse_query_selector_takes_rest_verbatim() {
        let i = parse_subscribe_spec("query:selector:fn[name=\"login\"]", None).unwrap();
        match i {
            subscription::Interest::Query {
                query: QuerySpec::Selector { hyphae },
                ..
            } => {
                assert_eq!(hyphae, "fn[name=\"login\"]");
            }
            _ => panic!("expected Query::Selector"),
        }
    }

    #[test]
    fn parse_rejects_unknown_kind() {
        assert!(parse_subscribe_spec("nope:x", None).is_err());
        assert!(parse_subscribe_spec("query:nope:x", None).is_err());
    }
}
