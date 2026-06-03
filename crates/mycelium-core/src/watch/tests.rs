//! Tests for the shared watch engine (RFC-0105). RED-first per Charter §5.1.
//!
//! These exercise the surface-agnostic loop with an in-memory
//! [`RecordingReindexer`] so the file-system semantics (debounce, ignore,
//! cancellation, batch emission) are decoupled from any real language
//! grammar. The MCP + CLI no-regression tests cover the grammar side.
//
// `std::sync::Mutex` is intentionally used here for test-side bookkeeping (a
// purely synchronous recorder); the workspace lint targets async-context use.
#![allow(clippy::disallowed_methods)]

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::RwLock;

use super::{BatchDelta, CancelToken, FileReindexer, WatchConfig, WatchEngine, WatchEvent};
use crate::store::Store;
use crate::trunk::TrunkPath;
use crate::types::NodeKind;

/// A [`FileReindexer`] that records (rel, ext) calls and inserts a fixed
/// `<rel>>marker` symbol into the store, so tests can assert both that the
/// engine drove a reindex AND that the store was mutated.
#[derive(Default)]
struct RecordingReindexer {
    calls: Mutex<Vec<(String, String)>>,
}

impl FileReindexer for RecordingReindexer {
    fn reindex(&self, rel: &str, _src: &[u8], ext: &str, store: &mut Store) {
        self.calls
            .lock()
            .unwrap()
            .push((rel.to_owned(), ext.to_owned()));
        if let Ok(path) = TrunkPath::parse(&format!("{rel}>marker")) {
            let id = store.upsert_node(path);
            store.set_kind(id, NodeKind::Function);
        }
    }
}

/// Wait until `pred()` is true or `timeout` elapses. Polling-based — never
/// depends on stdout ordering. Returns `true` if the predicate became true.
async fn wait_until<F: Fn() -> bool>(timeout: Duration, pred: F) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if pred() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    pred()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn engine_reindexes_changed_file_and_emits_event() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let store = Arc::new(RwLock::new(Store::new()));
    let reindexer = Arc::new(RecordingReindexer::default());
    let events: Arc<Mutex<Vec<WatchEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let cancel = CancelToken::new();

    // Spawn the engine.
    let store_h = Arc::clone(&store);
    let reindexer_h = Arc::clone(&reindexer);
    let events_h = Arc::clone(&events);
    let cancel_h = cancel.clone();
    let root_h = root.clone();
    let task = tokio::spawn(async move {
        let cfg = WatchConfig::new(root_h);
        let reindexer_ref: &dyn FileReindexer = reindexer_h.as_ref();
        WatchEngine::run(
            cfg,
            store_h,
            reindexer_ref,
            None,
            move |ev, _delta, _store| events_h.lock().unwrap().push(ev.clone()),
            cancel_h,
        )
        .await
        .unwrap();
    });

    // Give the watcher a moment to attach before we write.
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Create a supported source file.
    let target = root.join("lib.rs");
    std::fs::write(&target, b"fn main() {}\n").unwrap();

    // Wait for the reindexer to record the call and an event to fire.
    let saw_call = wait_until(Duration::from_secs(3), || {
        !reindexer.calls.lock().unwrap().is_empty() && !events.lock().unwrap().is_empty()
    })
    .await;

    cancel.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(2), task).await;

    assert!(
        saw_call,
        "engine should reindex the file and emit a batch within 3s"
    );

    // RecordingReindexer mutated the store.
    let found = store.read().await.lookup("lib.rs>marker").is_some();
    assert!(
        found,
        "store should contain the symbol the reindexer inserted"
    );

    // WatchEvent has the relative path.
    let first = events
        .lock()
        .unwrap()
        .first()
        .expect("at least one batch event")
        .clone();
    assert_eq!(first.batch_seq, 1);
    assert!(
        first.changed_files.iter().any(|p| p == "lib.rs"),
        "WatchEvent should carry the rel path"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_drains_final_batch_then_stops() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let store = Arc::new(RwLock::new(Store::new()));
    let reindexer = Arc::new(RecordingReindexer::default());
    let events: Arc<Mutex<Vec<WatchEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let cancel = CancelToken::new();

    let store_h = Arc::clone(&store);
    let reindexer_h = Arc::clone(&reindexer);
    let events_h = Arc::clone(&events);
    let cancel_h = cancel.clone();
    let root_h = root.clone();
    let task = tokio::spawn(async move {
        let cfg = WatchConfig::new(root_h);
        let reindexer_ref: &dyn FileReindexer = reindexer_h.as_ref();
        WatchEngine::run(
            cfg,
            store_h,
            reindexer_ref,
            None,
            move |ev, _delta, _store| events_h.lock().unwrap().push(ev.clone()),
            cancel_h,
        )
        .await
        .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(150)).await;
    std::fs::write(root.join("a.rs"), b"x\n").unwrap();

    // Wait until the in-flight batch has been observed.
    let drained = wait_until(Duration::from_secs(3), || {
        !events.lock().unwrap().is_empty()
    })
    .await;
    assert!(
        drained,
        "in-flight batch should be drained before we cancel"
    );

    cancel.cancel();
    let result = tokio::time::timeout(Duration::from_secs(2), task).await;
    assert!(result.is_ok(), "run() should return shortly after cancel()");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ignore_rules_skip_target_and_gitignored() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    // gitignore one path, then write to target/ + the gitignored path; neither
    // should produce a WatchEvent.
    std::fs::write(root.join(".gitignore"), b"vendored.rs\n").unwrap();
    std::fs::create_dir_all(root.join("target")).unwrap();

    let store = Arc::new(RwLock::new(Store::new()));
    let reindexer = Arc::new(RecordingReindexer::default());
    let events: Arc<Mutex<Vec<WatchEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let cancel = CancelToken::new();

    let store_h = Arc::clone(&store);
    let reindexer_h = Arc::clone(&reindexer);
    let events_h = Arc::clone(&events);
    let cancel_h = cancel.clone();
    let root_h = root.clone();
    let task = tokio::spawn(async move {
        let cfg = WatchConfig::new(root_h);
        let reindexer_ref: &dyn FileReindexer = reindexer_h.as_ref();
        WatchEngine::run(
            cfg,
            store_h,
            reindexer_ref,
            None,
            move |ev, _delta, _store| events_h.lock().unwrap().push(ev.clone()),
            cancel_h,
        )
        .await
        .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(150)).await;

    // Trigger writes that all the filters should swallow.
    std::fs::write(root.join("target").join("ignored.rs"), b"x\n").unwrap();
    std::fs::write(root.join("vendored.rs"), b"x\n").unwrap();

    // Wait for the debounce + filter pass to settle.
    tokio::time::sleep(Duration::from_millis(500)).await;

    cancel.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(2), task).await;

    let n_events = events.lock().unwrap().len();
    let n_calls = reindexer.calls.lock().unwrap().len();
    assert_eq!(
        n_events, 0,
        "filtered-only batch must not emit a WatchEvent: {n_events} events"
    );
    assert_eq!(
        n_calls, 0,
        "filtered files must not reach the reindexer: {n_calls} calls"
    );

    // sanity: the rel-path helpers were imported and work.
    assert!(super::is_supported_source_rel("a.rs"));
    assert!(!super::is_supported_source_rel("README.md"));
    assert_eq!(super::source_extension(Path::new("a.rs")), Some("rs"));
}

/// A [`FileReindexer`] that replaces the file's symbols with a fixed NEW
/// set on each invocation, so tests can deterministically observe the OLD
/// → NEW diff the engine emits inside `BatchDelta`.
struct ProgrammableReindexer {
    new_paths: Mutex<Vec<String>>,
}

impl FileReindexer for ProgrammableReindexer {
    fn reindex(&self, _rel: &str, _src: &[u8], _ext: &str, store: &mut Store) {
        for p in self.new_paths.lock().unwrap().iter() {
            if let Ok(parsed) = TrunkPath::parse(p) {
                let id = store.upsert_node(parsed);
                store.set_kind(id, NodeKind::Function);
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn batch_delta_classifies_added_modified_removed_per_file() {
    // RFC-0107 §6 test 2 — canary for the §5 lock discipline: OLD set is
    // captured BEFORE remove_file, NEW set AFTER reindex, and the diff is
    // classified into added / modified / removed.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let store = Arc::new(RwLock::new(Store::new()));
    // OLD: pre-populate the store with the file node + {fn:a, fn:b}.
    {
        let mut w = store.write().await;
        w.upsert_node(TrunkPath::parse("lib.rs").unwrap());
        w.upsert_node(TrunkPath::parse("lib.rs>fn:a").unwrap());
        w.upsert_node(TrunkPath::parse("lib.rs>fn:b").unwrap());
    }
    // NEW (after reindex): {fn:a, fn:c}. Expected: added=[fn:c],
    // modified=[fn:a], removed=[fn:b].
    let reindexer = Arc::new(ProgrammableReindexer {
        new_paths: Mutex::new(vec![
            "lib.rs".to_owned(),
            "lib.rs>fn:a".to_owned(),
            "lib.rs>fn:c".to_owned(),
        ]),
    });
    let deltas: Arc<Mutex<Vec<BatchDelta>>> = Arc::new(Mutex::new(Vec::new()));
    let cancel = CancelToken::new();

    let store_h = Arc::clone(&store);
    let reindexer_h = Arc::clone(&reindexer);
    let deltas_h = Arc::clone(&deltas);
    let cancel_h = cancel.clone();
    let root_h = root.clone();
    let task = tokio::spawn(async move {
        let cfg = WatchConfig::new(root_h);
        let reindexer_ref: &dyn FileReindexer = reindexer_h.as_ref();
        WatchEngine::run(
            cfg,
            store_h,
            reindexer_ref,
            None,
            move |_ev, delta, _store| deltas_h.lock().unwrap().push(delta.clone()),
            cancel_h,
        )
        .await
        .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(150)).await;
    std::fs::write(root.join("lib.rs"), b"// new contents\n").unwrap();

    let saw = wait_until(Duration::from_secs(3), || {
        !deltas.lock().unwrap().is_empty()
    })
    .await;
    cancel.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
    assert!(saw, "engine should emit a BatchDelta");

    let delta = deltas.lock().unwrap().first().expect("one batch").clone();
    let pf = delta
        .per_file
        .iter()
        .find(|f| f.file == "lib.rs")
        .expect("per-file entry for lib.rs");
    assert_eq!(
        pf.added,
        vec!["lib.rs>fn:c"],
        "fn:c present in NEW but not OLD"
    );
    assert_eq!(
        pf.modified,
        vec!["lib.rs>fn:a"],
        "fn:a in both OLD and NEW (survivor)"
    );
    assert_eq!(
        pf.removed,
        vec!["lib.rs>fn:b"],
        "fn:b present in OLD but not NEW"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn on_batch_signature_carries_batch_delta() {
    // RFC-0107 §6 test 3 — observable that the `on_batch` widened third arg
    // is non-empty for non-empty batches.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let store = Arc::new(RwLock::new(Store::new()));
    let reindexer = Arc::new(RecordingReindexer::default());
    let deltas: Arc<Mutex<Vec<BatchDelta>>> = Arc::new(Mutex::new(Vec::new()));
    let cancel = CancelToken::new();

    let store_h = Arc::clone(&store);
    let reindexer_h = Arc::clone(&reindexer);
    let deltas_h = Arc::clone(&deltas);
    let cancel_h = cancel.clone();
    let root_h = root.clone();
    let task = tokio::spawn(async move {
        let cfg = WatchConfig::new(root_h);
        let reindexer_ref: &dyn FileReindexer = reindexer_h.as_ref();
        WatchEngine::run(
            cfg,
            store_h,
            reindexer_ref,
            None,
            move |_ev, delta, _store| deltas_h.lock().unwrap().push(delta.clone()),
            cancel_h,
        )
        .await
        .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(150)).await;
    std::fs::write(root.join("a.rs"), b"x\n").unwrap();

    let saw = wait_until(Duration::from_secs(3), || {
        !deltas.lock().unwrap().is_empty()
    })
    .await;
    cancel.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(2), task).await;

    assert!(saw, "engine should emit a batch");
    let d = deltas.lock().unwrap().first().expect("delta").clone();
    assert!(
        !d.per_file.is_empty(),
        "non-empty batch should carry non-empty per_file BatchDelta"
    );
    assert!(d.per_file.iter().any(|f| f.file == "a.rs"));
}
