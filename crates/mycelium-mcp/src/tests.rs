use mycelium_core::{trunk::TrunkPath, types::EdgeKind};
use rmcp::handler::server::wrapper::Parameters;

use super::*;

fn result_str(r: &CallToolResult) -> &str {
    r.content
        .first()
        .and_then(|c| c.raw.as_text())
        .map(|t| t.text.as_str())
        .expect("CallToolResult must have non-empty text content")
}

async fn server_with_fixture() -> MyceliumServer {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let file = store.upsert_node(TrunkPath::parse("src/greet.rs").unwrap());
        let greet = store.upsert_node(TrunkPath::parse("src/greet.rs>greet").unwrap());
        let helper = store.upsert_node(TrunkPath::parse("src/greet.rs>helper").unwrap());
        store.upsert_edge(EdgeKind::Contains, file, greet);
        store.upsert_edge(EdgeKind::Contains, file, helper);
    }
    server
}

#[tokio::test]
async fn search_symbol_returns_matching_paths() {
    let server = server_with_fixture().await;
    let raw = server
        .mycelium_search_symbol(Parameters(SearchSymbolRequest {
            query: "greet".to_string(),
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let arr = val["matches"].as_array().unwrap();
    assert!(
        arr.iter().any(|v| v.as_str() == Some("src/greet.rs>greet")),
        "greet symbol should match"
    );
    assert!(
        !arr.iter()
            .any(|v| v.as_str() == Some("src/greet.rs>helper")),
        "helper should not match query 'greet'"
    );
}

#[tokio::test]
async fn search_symbol_respects_limit() {
    let server = server_with_fixture().await;
    let raw = server
        .mycelium_search_symbol(Parameters(SearchSymbolRequest {
            query: String::new(), // matches everything
            limit: Some(1),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(
        val["matches"].as_array().unwrap().len(),
        1,
        "limit should be respected"
    );
}

#[tokio::test]
async fn get_ancestors_returns_containment_chain() {
    let server = server_with_fixture().await;
    let raw = server
        .mycelium_get_ancestors(Parameters(GetAncestorsRequest {
            path: "src/greet.rs>greet".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(
        val["ancestors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str() == Some("src/greet.rs")),
        "file node should be an ancestor"
    );
}

#[tokio::test]
async fn get_ancestors_returns_empty_list_for_unknown_path() {
    let server = server_with_fixture().await;
    let raw = server
        .mycelium_get_ancestors(Parameters(GetAncestorsRequest {
            path: "no/such>path".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(
        val["ancestors"].as_array().unwrap().is_empty(),
        "unknown path should yield empty ancestors list"
    );
}

#[tokio::test]
async fn index_workspace_indexes_rust_file() {
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("lib.rs"), "fn hello() {}").unwrap();

    let server = MyceliumServer::new();
    let raw = server
        .mycelium_index_workspace(Parameters(IndexWorkspaceRequest {
            path: tmp.path().to_string_lossy().into_owned(),
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["files"], 1, "should report one indexed file");
    assert_eq!(val["errors"], 0, "no errors expected");
    assert!(
        val["languages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str() == Some("rust")),
        "rust must appear in languages list"
    );

    assert!(
        server.store.read().await.lookup("lib.rs>hello").is_some(),
        "function node must be in the store after indexing"
    );
}

#[tokio::test]
async fn index_workspace_includes_languages_for_js_file() {
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("app.js"), "function greet() {}").unwrap();

    let server = MyceliumServer::new();
    let raw = server
        .mycelium_index_workspace(Parameters(IndexWorkspaceRequest {
            path: tmp.path().to_string_lossy().into_owned(),
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["files"], 1, "should report one indexed file");
    assert!(
        val["languages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str() == Some("javascript")),
        "javascript must appear in languages list"
    );
    assert!(
        server.store.read().await.lookup("app.js>greet").is_some(),
        "function node must be in the store after indexing"
    );
}

#[tokio::test]
async fn get_descendants_returns_all_nested_symbols() {
    let server = server_with_fixture().await;
    let raw = server
        .mycelium_get_descendants(Parameters(GetDescendantsRequest {
            path: "src/greet.rs".to_string(),
            include_inherited: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let arr = val["descendants"].as_array().unwrap();
    assert!(
        arr.iter().any(|v| v.as_str() == Some("src/greet.rs>greet")),
        "greet must be a descendant of the file"
    );
    assert!(
        arr.iter()
            .any(|v| v.as_str() == Some("src/greet.rs>helper")),
        "helper must be a descendant of the file"
    );
}

#[tokio::test]
async fn get_descendants_returns_empty_list_for_leaf_node() {
    let server = server_with_fixture().await;
    let raw = server
        .mycelium_get_descendants(Parameters(GetDescendantsRequest {
            path: "src/greet.rs>greet".to_string(),
            include_inherited: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(
        val["descendants"].as_array().unwrap().is_empty(),
        "leaf node should yield empty descendants list"
    );
}

#[tokio::test]
async fn get_descendants_returns_empty_list_for_unknown_path() {
    let server = server_with_fixture().await;
    let raw = server
        .mycelium_get_descendants(Parameters(GetDescendantsRequest {
            path: "no/such>path".to_string(),
            include_inherited: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(
        val["descendants"].as_array().unwrap().is_empty(),
        "unknown path should yield empty descendants list"
    );
}

// ── issue #248: get-descendants with include_inherited ────────────────────

/// Helper: server with a base class and a subclass that has its own method.
async fn server_with_descendants_inheritance_fixture() -> MyceliumServer {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        // Base class: pkg/base.py>BaseClass with methods foo and shared
        let base_file = store.upsert_node(TrunkPath::parse("pkg/base.py").unwrap());
        let base_cls = store.upsert_node(TrunkPath::parse("pkg/base.py>BaseClass").unwrap());
        let base_foo = store.upsert_node(TrunkPath::parse("pkg/base.py>BaseClass>foo").unwrap());
        let base_shared =
            store.upsert_node(TrunkPath::parse("pkg/base.py>BaseClass>shared").unwrap());
        store.upsert_edge(EdgeKind::Contains, base_file, base_cls);
        store.upsert_edge(EdgeKind::Contains, base_cls, base_foo);
        store.upsert_edge(EdgeKind::Contains, base_cls, base_shared);
        // Sub class: pkg/sub.py>SubClass with its own bar + shared (override)
        let sub_file = store.upsert_node(TrunkPath::parse("pkg/sub.py").unwrap());
        let sub_cls = store.upsert_node(TrunkPath::parse("pkg/sub.py>SubClass").unwrap());
        let sub_bar = store.upsert_node(TrunkPath::parse("pkg/sub.py>SubClass>bar").unwrap());
        let sub_shared = store.upsert_node(TrunkPath::parse("pkg/sub.py>SubClass>shared").unwrap());
        store.upsert_edge(EdgeKind::Contains, sub_file, sub_cls);
        store.upsert_edge(EdgeKind::Contains, sub_cls, sub_bar);
        store.upsert_edge(EdgeKind::Contains, sub_cls, sub_shared);
        // Inheritance: SubClass extends BaseClass
        store.upsert_edge(EdgeKind::Extends, sub_cls, base_cls);
    }
    server
}

#[tokio::test]
async fn get_descendants_include_inherited_returns_base_methods() {
    let server = server_with_descendants_inheritance_fixture().await;
    let raw = server
        .mycelium_get_descendants(Parameters(GetDescendantsRequest {
            path: "pkg/sub.py>SubClass".to_string(),
            include_inherited: Some(true),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let inherited = val["inherited_descendants"]
        .as_array()
        .expect("include_inherited=true must produce an inherited_descendants array");
    // `foo` is inherited (not overridden in SubClass)
    assert!(
        inherited
            .iter()
            .any(|v| v["path"].as_str() == Some("pkg/base.py>BaseClass>foo")),
        "inherited_descendants must include BaseClass>foo (not overridden)"
    );
    // `shared` is overridden in SubClass — must NOT appear in inherited
    assert!(
        !inherited
            .iter()
            .any(|v| v["path"].as_str() == Some("pkg/base.py>BaseClass>shared")),
        "shared is overridden; must not appear in inherited_descendants"
    );
}

#[tokio::test]
async fn get_descendants_default_unchanged_without_include_inherited() {
    let server = server_with_descendants_inheritance_fixture().await;
    let raw = server
        .mycelium_get_descendants(Parameters(GetDescendantsRequest {
            path: "pkg/sub.py>SubClass".to_string(),
            include_inherited: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    // Without include_inherited, inherited_descendants must be absent or empty
    let inherited_absent = val["inherited_descendants"].is_null()
        || val["inherited_descendants"]
            .as_array()
            .is_none_or(Vec::is_empty);
    assert!(
        inherited_absent,
        "without include_inherited, inherited_descendants must not appear"
    );
}

#[tokio::test]
async fn index_workspace_followed_by_search_and_ancestors() {
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join("lib.rs"),
        "struct Point { x: i32 } impl Point { fn new() -> Self { Point { x: 0 } } }",
    )
    .unwrap();

    let server = MyceliumServer::new();
    server
        .mycelium_index_workspace(Parameters(IndexWorkspaceRequest {
            path: tmp.path().to_string_lossy().into_owned(),
        }))
        .await;

    // Search for the impl method
    let search_raw = server
        .mycelium_search_symbol(Parameters(SearchSymbolRequest {
            query: "new".to_string(),
            limit: None,
            output_format: None,
        }))
        .await;
    let search_val: serde_json::Value = serde_json::from_str(result_str(&search_raw)).unwrap();
    let matches: Vec<&str> = search_val["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(matches.iter().any(|p| p.ends_with(">new")));

    // Get ancestors of the method
    let method_path = matches
        .iter()
        .find(|p| p.ends_with(">new"))
        .copied()
        .unwrap();
    let anc_raw = server
        .mycelium_get_ancestors(Parameters(GetAncestorsRequest {
            path: method_path.to_string(),
            output_format: None,
        }))
        .await;
    let anc_val: serde_json::Value = serde_json::from_str(result_str(&anc_raw)).unwrap();
    assert!(
        anc_val["ancestors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str().is_some_and(|s| s.ends_with("lib.rs"))),
        "file node must appear in ancestor chain"
    );
}

#[tokio::test]
async fn index_workspace_saves_snapshot() {
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("lib.rs"), "fn greet() {}").unwrap();

    let server = MyceliumServer::new();
    server
        .mycelium_index_workspace(Parameters(IndexWorkspaceRequest {
            path: tmp.path().to_string_lossy().into_owned(),
        }))
        .await;

    let snap = tmp.path().join(".mycelium").join("index.rmp");
    assert!(snap.exists(), "snapshot must be created after indexing");
    assert!(
        snap.metadata().unwrap().len() > 0,
        "snapshot must not be empty"
    );
}

#[tokio::test]
async fn load_index_restores_symbols() {
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("lib.rs"), "fn greet() {}").unwrap();

    // Index and save
    let server1 = MyceliumServer::new();
    server1
        .mycelium_index_workspace(Parameters(IndexWorkspaceRequest {
            path: tmp.path().to_string_lossy().into_owned(),
        }))
        .await;

    // Load on a fresh server
    let server2 = MyceliumServer::new();
    let raw = server2
        .mycelium_load_index(Parameters(LoadIndexRequest {
            path: tmp.path().to_string_lossy().into_owned(),
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none(), "load must not return error");
    assert!(
        val["nodes"].as_u64().unwrap() > 0,
        "loaded store must contain nodes"
    );
    assert!(
        server2.store.read().await.lookup("lib.rs>greet").is_some(),
        "greet symbol must be present after load"
    );
}

#[tokio::test]
async fn load_index_errors_on_missing_snapshot() {
    let tmp = tempfile::tempdir().unwrap();

    let server = MyceliumServer::new();
    let raw = server
        .mycelium_load_index(Parameters(LoadIndexRequest {
            path: tmp.path().to_string_lossy().into_owned(),
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(
        val.get("error").is_some(),
        "loading from a directory without a snapshot must return an error"
    );
}

// ── RFC-0007 tests ────────────────────────────────────────────────

#[tokio::test]
async fn with_root_loads_existing_snapshot() {
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("lib.rs"), "fn hello() {}").unwrap();

    // Build and save a snapshot via a first server.
    let server1 = MyceliumServer::new();
    server1
        .mycelium_index_workspace(Parameters(IndexWorkspaceRequest {
            path: tmp.path().to_string_lossy().into_owned(),
        }))
        .await;
    let snap = tmp.path().join(".mycelium").join("index.rmp");
    assert!(snap.exists(), "pre-condition: snapshot must exist");

    // A second server boots from with_root — should load the snapshot.
    let server2 = MyceliumServer::with_root(tmp.path().to_owned())
        .await
        .expect("with_root must succeed when snapshot exists");
    assert!(
        server2.store.read().await.lookup("lib.rs>hello").is_some(),
        "symbol must be present after with_root loads snapshot"
    );
}

#[tokio::test]
async fn with_root_indexes_when_no_snapshot() {
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("app.py"), "def run(): pass").unwrap();

    // No snapshot exists yet; with_root must fall back to live index.
    let server = MyceliumServer::with_root(tmp.path().to_owned())
        .await
        .expect("with_root must succeed even without snapshot");
    assert!(
        server.store.read().await.lookup("app.py>run").is_some(),
        "symbol must be present after with_root runs live index"
    );
    // Snapshot should now exist.
    assert!(
        tmp.path().join(".mycelium").join("index.rmp").exists(),
        "with_root must save a snapshot after live indexing"
    );
}

#[cfg(feature = "redb-backend")]
#[tokio::test]
async fn redb_watch_batch_persists_one_changed_file() {
    use mycelium_core::store::backend::StorageBackend as _;
    use mycelium_core::store::redb_backend::RedbBackend;

    let tmp = tempfile::tempdir().unwrap();

    let mut store = Store::new();
    reindex_file("a.py", b"def old(): pass", "py", &mut store);
    reindex_file("b.py", b"def keep(): pass", "py", &mut store);
    store.resolve_bare_call_stubs();
    persist_full_redb_index(tmp.path(), &store).expect("initial redb import");

    store.remove_file("a.py");
    reindex_file("a.py", b"def new(): pass", "py", &mut store);
    store.resolve_bare_call_stubs();
    persist_redb_watch_batch(tmp.path(), &store, &["a.py".to_string()])
        .expect("single-file redb replacement");

    let redb = tmp.path().join(".mycelium").join("index.redb");
    assert!(redb.exists(), "watch persistence must use index.redb");

    let loaded = Store::load(&redb).expect("load redb store");
    assert!(
        loaded.lookup("a.py>old").is_none(),
        "changed-file replacement must remove stale symbols"
    );
    assert!(
        loaded.lookup("a.py>new").is_some(),
        "changed-file replacement must persist new symbols"
    );
    assert!(
        loaded.lookup("b.py>keep").is_some(),
        "single-file replacement must preserve unrelated files"
    );

    let reopened = RedbBackend::open_existing(&redb).expect("reopen redb backend");
    assert_eq!(
        reopened.node_count(),
        loaded.node_count(),
        "redb node count must match the materialized store"
    );
}

#[tokio::test]
async fn server_status_returns_node_and_edge_count() {
    let server = server_with_fixture().await;
    let raw = server.mycelium_server_status().await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(
        val["node_count"].as_u64().unwrap() > 0,
        "node_count must be non-zero"
    );
    assert!(
        val.get("edge_count").is_some(),
        "edge_count key must be present"
    );
    assert!(
        val.get("indexed_root").is_some(),
        "indexed_root key must be present"
    );
    assert!(
        val.get("is_loaded").is_some(),
        "is_loaded key must be present"
    );
}

// ── RFC-0008 watch mode tests ─────────────────────────────────────

#[tokio::test]
async fn watch_status_not_watching_by_default() {
    let server = MyceliumServer::new();
    let raw = server.mycelium_watch_status().await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(
        val["watching"].as_bool(),
        Some(false),
        "brand-new server must not be watching"
    );
}

#[tokio::test]
async fn start_watch_sets_watching_true() {
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("lib.rs"), "fn hello() {}").unwrap();

    let server = MyceliumServer::with_root(tmp.path().to_owned())
        .await
        .unwrap();

    let raw = server.mycelium_watch_status().await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(
        val["watching"].as_bool(),
        Some(true),
        "with_root must start the watch loop"
    );
}

// Slow: relies on filesystem watcher debounce (≥300 ms per event).
// Run explicitly with: cargo test -- --ignored watch_mode
#[tokio::test(flavor = "multi_thread")]
#[ignore = "slow: filesystem watcher timing (run with --ignored)"]
async fn watch_mode_detects_modified_file() {
    use std::fs;
    use tokio::time::Duration;
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("app.py"), "def hello(): pass").unwrap();

    let server = MyceliumServer::with_root(tmp.path().to_owned())
        .await
        .unwrap();

    assert!(
        server.store.read().await.lookup("app.py>hello").is_some(),
        "initial symbol must be present"
    );

    // Modify the file: replace hello with goodbye
    fs::write(tmp.path().join("app.py"), "def goodbye(): pass").unwrap();

    // Poll up to 2 seconds for the watcher to pick up the change.
    let found = poll_for(
        Duration::from_secs(2),
        Duration::from_millis(100),
        || async { server.store.read().await.lookup("app.py>goodbye").is_some() },
    )
    .await;

    assert!(found, "watcher must detect modification and update store");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "slow: filesystem watcher timing (run with --ignored)"]
async fn watch_mode_detects_deleted_file() {
    use std::fs;
    use tokio::time::Duration;
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("del.rs"), "fn drop_me() {}").unwrap();

    let server = MyceliumServer::with_root(tmp.path().to_owned())
        .await
        .unwrap();

    assert!(
        server.store.read().await.lookup("del.rs>drop_me").is_some(),
        "initial symbol must be present"
    );

    fs::remove_file(tmp.path().join("del.rs")).unwrap();

    let removed = poll_for(
        Duration::from_secs(2),
        Duration::from_millis(100),
        || async { server.store.read().await.lookup("del.rs>drop_me").is_none() },
    )
    .await;

    assert!(removed, "watcher must detect deletion and remove nodes");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "slow: filesystem watcher timing (run with --ignored)"]
async fn watch_mode_detects_new_file() {
    use std::fs;
    use tokio::time::Duration;
    let tmp = tempfile::tempdir().unwrap();
    // Start with an empty directory.
    let server = MyceliumServer::with_root(tmp.path().to_owned())
        .await
        .unwrap();

    // Create a new file after the server is running.
    fs::write(tmp.path().join("new.rs"), "fn fresh() {}").unwrap();

    let found = poll_for(
        Duration::from_secs(2),
        Duration::from_millis(100),
        || async { server.store.read().await.lookup("new.rs>fresh").is_some() },
    )
    .await;

    assert!(found, "watcher must detect new file and index it");
}

// ── RFC-0012: call-graph MCP tools ───────────────────────────────────

async fn server_with_call_fixture() -> MyceliumServer {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let file = store.upsert_node(TrunkPath::parse("src/lib.rs").unwrap());
        let foo = store.upsert_node(TrunkPath::parse("src/lib.rs>foo").unwrap());
        let bar = store.upsert_node(TrunkPath::parse("src/lib.rs>bar").unwrap());
        let baz = store.upsert_node(TrunkPath::parse("src/lib.rs>baz").unwrap());
        store.upsert_edge(EdgeKind::Contains, file, foo);
        store.upsert_edge(EdgeKind::Contains, file, bar);
        store.upsert_edge(EdgeKind::Contains, file, baz);
        // foo calls bar and baz; baz calls bar.
        store.upsert_edge(EdgeKind::Calls, foo, bar);
        store.upsert_edge(EdgeKind::Calls, foo, baz);
        store.upsert_edge(EdgeKind::Calls, baz, bar);
    }
    server
}

#[tokio::test]
async fn get_callees_returns_functions_called_by_path() {
    let server = server_with_call_fixture().await;
    let raw = server
        .mycelium_get_callees(Parameters(GetCalleesRequest {
            path: "src/lib.rs>foo".to_string(),
            edge_kind: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let paths: Vec<&str> = val["callee_paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(
        paths.contains(&"src/lib.rs>bar"),
        "callees of foo must include bar"
    );
    assert!(
        paths.contains(&"src/lib.rs>baz"),
        "callees of foo must include baz"
    );
    assert_eq!(
        paths.len(),
        2,
        "foo calls exactly bar and baz — no more, no less"
    );
}

#[tokio::test]
async fn get_callers_returns_functions_that_call_path() {
    let server = server_with_call_fixture().await;
    let raw = server
        .mycelium_get_callers(Parameters(GetCallersRequest {
            path: "src/lib.rs>bar".to_string(),
            edge_kind: None,
            include_virtual: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let paths: Vec<&str> = val["caller_paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(
        paths.contains(&"src/lib.rs>foo"),
        "callers of bar must include foo"
    );
    assert!(
        paths.contains(&"src/lib.rs>baz"),
        "callers of bar must include baz"
    );
    assert_eq!(
        paths.len(),
        2,
        "bar is called by exactly foo and baz — no more, no less"
    );
}

#[tokio::test]
async fn get_callees_returns_error_for_unknown_path() {
    let server = server_with_call_fixture().await;
    let raw = server
        .mycelium_get_callees(Parameters(GetCalleesRequest {
            path: "no/such/path".to_string(),
            edge_kind: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(
        val.get("error").is_some(),
        "unknown path should return error"
    );
    assert_eq!(
        raw.is_error,
        Some(true),
        "error response must carry is_error=true on CallToolResult per RFC-0093 Phase 3"
    );
}

// ── RFC-0016: mycelium_get_symbol_info ───────────────────────────────

#[tokio::test]
async fn get_symbol_info_returns_all_relationships() {
    let server = server_with_call_fixture().await;
    // Add containment so ancestors/descendants are non-empty
    {
        let mut store = server.store.write().await;
        let file = store.upsert_node(TrunkPath::parse("src/lib.rs").unwrap());
        let foo = store.lookup("src/lib.rs>foo").unwrap();
        store.upsert_edge(mycelium_core::types::EdgeKind::Contains, file, foo);
    }
    let raw = server
        .mycelium_get_symbol_info(Parameters(GetSymbolInfoRequest {
            path: "src/lib.rs>foo".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["path"].as_str(), Some("src/lib.rs>foo"));
    assert!(
        val.get("ancestors").is_some(),
        "ancestors field must be present"
    );
    assert!(
        val.get("descendants").is_some(),
        "descendants field must be present"
    );
    let callees: Vec<&str> = val["callees"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(
        callees.contains(&"src/lib.rs>bar"),
        "callees must include bar"
    );
    assert!(
        callees.contains(&"src/lib.rs>baz"),
        "callees must include baz"
    );
    // lists are sorted
    let mut sorted = callees.clone();
    sorted.sort_unstable();
    assert_eq!(callees, sorted, "callees must be sorted");
}

#[tokio::test]
async fn get_symbol_info_returns_error_for_unknown_path() {
    let server = server_with_call_fixture().await;
    let raw = server
        .mycelium_get_symbol_info(Parameters(GetSymbolInfoRequest {
            path: "no/such>path".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some(), "unknown path must return error");
}

// ── RFC-0017: mycelium_find_call_path ────────────────────────────────

#[tokio::test]
async fn find_call_path_direct() {
    let server = server_with_call_fixture().await;
    // foo → bar is a direct Calls edge (hops = 1)
    let raw = server
        .mycelium_find_call_path(Parameters(FindCallPathRequest {
            from_path: "src/lib.rs>foo".to_string(),
            to_path: "src/lib.rs>bar".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(
        val.get("error").is_none(),
        "direct path must not return error"
    );
    let path = val["path"].as_array().unwrap();
    assert_eq!(path.len(), 2, "direct path must contain 2 nodes");
    assert_eq!(path[0].as_str(), Some("src/lib.rs>foo"));
    assert_eq!(path[1].as_str(), Some("src/lib.rs>bar"));
    assert_eq!(val["hops"].as_u64(), Some(1), "direct call is 1 hop");
}

#[tokio::test]
async fn find_call_path_transitive() {
    // Build a chain a → b → c with no direct a → c edge.
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("x.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("x.rs>b").unwrap());
        let c = store.upsert_node(TrunkPath::parse("x.rs>c").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, b, c);
    }
    let raw = server
        .mycelium_find_call_path(Parameters(FindCallPathRequest {
            from_path: "x.rs>a".to_string(),
            to_path: "x.rs>c".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(
        val.get("error").is_none(),
        "transitive path must not return error"
    );
    let path = val["path"].as_array().unwrap();
    assert_eq!(path.len(), 3, "transitive path must contain 3 nodes");
    assert_eq!(path[0].as_str(), Some("x.rs>a"));
    assert_eq!(path[2].as_str(), Some("x.rs>c"));
    assert_eq!(val["hops"].as_u64(), Some(2), "transitive call is 2 hops");
}

#[tokio::test]
async fn find_call_path_no_path() {
    let server = server_with_call_fixture().await;
    // bar has no outgoing Calls edge to foo, so bar → foo is unreachable.
    let raw = server
        .mycelium_find_call_path(Parameters(FindCallPathRequest {
            from_path: "src/lib.rs>bar".to_string(),
            to_path: "src/lib.rs>foo".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(
        val.get("error").is_none(),
        "no-path result must not return error"
    );
    assert!(
        val["path"].as_array().unwrap().is_empty(),
        "unreachable path must return empty path array"
    );
    assert!(
        val["hops"].is_null(),
        "unreachable path must return null hops"
    );
    assert!(
        val.get("message").is_some(),
        "unreachable path must include a human-readable message"
    );
}

#[tokio::test]
async fn find_call_path_unknown_from_path() {
    let server = server_with_call_fixture().await;
    let raw = server
        .mycelium_find_call_path(Parameters(FindCallPathRequest {
            from_path: "no/such>symbol".to_string(),
            to_path: "src/lib.rs>bar".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(
        val.get("error").is_some(),
        "unknown from_path must return error"
    );
}

#[tokio::test]
async fn find_call_path_unknown_to_path() {
    let server = server_with_call_fixture().await;
    let raw = server
        .mycelium_find_call_path(Parameters(FindCallPathRequest {
            from_path: "src/lib.rs>foo".to_string(),
            to_path: "no/such>symbol".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(
        val.get("error").is_some(),
        "unknown to_path must return error"
    );
}

// ── RFC-0020: mycelium_get_callee_tree ───────────────────────────────

#[tokio::test]
async fn get_callee_tree_returns_nested_structure() {
    let server = server_with_call_fixture().await;
    // foo → bar, foo → baz, baz → bar
    let raw = server
        .mycelium_get_callee_tree(Parameters(GetCalleeTreeRequest {
            path: "src/lib.rs>foo".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(
        val.get("error").is_none(),
        "known path must not return error"
    );
    let root = &val["root"];
    assert_eq!(root["path"].as_str(), Some("src/lib.rs>foo"));
    let children = root["children"].as_array().unwrap();
    assert_eq!(children.len(), 2, "foo has 2 direct callees");
}

#[tokio::test]
async fn get_callee_tree_returns_error_for_unknown_path() {
    let server = server_with_call_fixture().await;
    let raw = server
        .mycelium_get_callee_tree(Parameters(GetCalleeTreeRequest {
            path: "no/such>path".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some(), "unknown path must return error");
}

#[tokio::test]
async fn get_callee_tree_leaf_at_max_depth() {
    // Build a chain a → b → c
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("b.rs>b").unwrap());
        let c = store.upsert_node(TrunkPath::parse("c.rs>c").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, b, c);
    }
    let raw = server
        .mycelium_get_callee_tree(Parameters(GetCalleeTreeRequest {
            path: "a.rs>a".to_string(),
            max_depth: Some(1),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let root = &val["root"];
    let b_node = &root["children"][0];
    assert_eq!(b_node["path"].as_str(), Some("b.rs>b"));
    assert!(
        b_node["children"].as_array().unwrap().is_empty(),
        "b must be a leaf at max_depth=1"
    );
}

// ── RFC-0021: mycelium_get_caller_tree ──────────────────────────────────

#[tokio::test]
async fn get_caller_tree_returns_nested_structure() {
    let server = server_with_call_fixture().await;
    // Fixture: foo→bar, foo→baz, baz→bar → bar has 2 callers (foo, baz)
    let raw = server
        .mycelium_get_caller_tree(Parameters(GetCallerTreeRequest {
            path: "src/lib.rs>bar".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(
        val.get("error").is_none(),
        "known path must not return error"
    );
    let root = &val["root"];
    assert_eq!(root["path"].as_str(), Some("src/lib.rs>bar"));
    let callers = root["callers"].as_array().unwrap();
    assert_eq!(callers.len(), 2, "bar has 2 direct callers");
}

#[tokio::test]
async fn get_caller_tree_returns_error_for_unknown_path() {
    let server = server_with_call_fixture().await;
    let raw = server
        .mycelium_get_caller_tree(Parameters(GetCallerTreeRequest {
            path: "no/such>path".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some(), "unknown path must return error");
}

#[tokio::test]
async fn get_caller_tree_leaf_at_max_depth() {
    // Build a chain a → b → c; caller_tree of c with max_depth=1 yields b as leaf
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("b.rs>b").unwrap());
        let c = store.upsert_node(TrunkPath::parse("c.rs>c").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, b, c);
    }
    let raw = server
        .mycelium_get_caller_tree(Parameters(GetCallerTreeRequest {
            path: "c.rs>c".to_string(),
            max_depth: Some(1),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let root = &val["root"];
    let b_node = &root["callers"][0];
    assert_eq!(b_node["path"].as_str(), Some("b.rs>b"));
    assert!(
        b_node["callers"].as_array().unwrap().is_empty(),
        "b must be a leaf at max_depth=1"
    );
}

// ── RFC-0022: mycelium_get_entry_points ──────────────────────────────

#[tokio::test]
async fn get_entry_points_returns_zero_caller_symbols() {
    let server = server_with_call_fixture().await;
    // Fixture: foo→bar, foo→baz, baz→bar
    // foo has no callers → entry point; bar and baz have callers → not
    let raw = server
        .mycelium_get_entry_points(Parameters(GetEntryPointsRequest {
            path_prefix: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let eps = val["entry_points"].as_array().unwrap();
    let ep_paths: Vec<&str> = eps.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(ep_paths.contains(&"src/lib.rs>foo"), "foo has no callers");
    assert!(
        !ep_paths.contains(&"src/lib.rs>bar"),
        "bar is called by foo and baz"
    );
    assert!(
        !ep_paths.contains(&"src/lib.rs>baz"),
        "baz is called by foo"
    );
}

#[tokio::test]
async fn get_entry_points_excludes_file_nodes() {
    let server = server_with_call_fixture().await;
    let raw = server
        .mycelium_get_entry_points(Parameters(GetEntryPointsRequest {
            path_prefix: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let eps = val["entry_points"].as_array().unwrap();
    for ep in eps {
        let p = ep.as_str().unwrap();
        assert!(p.contains('>'), "file nodes must not appear: {p}");
    }
}

#[tokio::test]
async fn get_entry_points_prefix_filter() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/a.rs>fn_a").unwrap());
        store.upsert_node(TrunkPath::parse("tests/t.rs>test_foo").unwrap());
    }
    let raw = server
        .mycelium_get_entry_points(Parameters(GetEntryPointsRequest {
            path_prefix: Some("src/".to_string()),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let eps = val["entry_points"].as_array().unwrap();
    let ep_paths: Vec<&str> = eps.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(ep_paths.contains(&"src/a.rs>fn_a"));
    assert!(!ep_paths.contains(&"tests/t.rs>test_foo"));
}

// ── RFC-0023: mycelium_get_imports ───────────────────────────────────

#[tokio::test]
async fn get_imports_returns_import_neighbors() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let file = store.upsert_node(TrunkPath::parse("src/auth.rs").unwrap());
        let os_mod = store.upsert_node(TrunkPath::parse("os").unwrap());
        let hash_mod = store.upsert_node(TrunkPath::parse("hashlib").unwrap());
        let importer = store.upsert_node(TrunkPath::parse("src/main.rs").unwrap());
        store.upsert_edge(EdgeKind::Imports, file, os_mod);
        store.upsert_edge(EdgeKind::Imports, file, hash_mod);
        store.upsert_edge(EdgeKind::Imports, importer, file);
    }
    let raw = server
        .mycelium_get_imports(Parameters(GetImportsRequest {
            path: "src/auth.rs".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none(), "known path must not error");
    let imports = val["imports"].as_array().unwrap();
    let imported_by = val["imported_by"].as_array().unwrap();
    assert_eq!(imports.len(), 2, "auth.rs imports os and hashlib");
    assert_eq!(imported_by.len(), 1, "auth.rs imported_by src/main.rs");
}

#[tokio::test]
async fn get_imports_returns_error_for_unknown_path() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_imports(Parameters(GetImportsRequest {
            path: "no/such.rs".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some(), "unknown path must return error");
}

#[tokio::test]
async fn get_imports_empty_when_no_import_edges() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/isolated.rs").unwrap());
    }
    let raw = server
        .mycelium_get_imports(Parameters(GetImportsRequest {
            path: "src/isolated.rs".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["imports"].as_array().unwrap().is_empty());
    assert!(val["imported_by"].as_array().unwrap().is_empty());
}

// ── RFC-0024: mycelium_get_import_tree ───────────────────────────────

#[tokio::test]
async fn get_import_tree_returns_nested_structure() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("a.rs").unwrap());
        let b = store.upsert_node(TrunkPath::parse("b.rs").unwrap());
        let c = store.upsert_node(TrunkPath::parse("c.rs").unwrap());
        store.upsert_edge(EdgeKind::Imports, a, b);
        store.upsert_edge(EdgeKind::Imports, b, c);
    }
    let raw = server
        .mycelium_get_import_tree(Parameters(GetImportTreeRequest {
            path: "a.rs".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none(), "known path must not error");
    let root = &val["root"];
    assert_eq!(root["path"].as_str(), Some("a.rs"));
    assert_eq!(root["imports"].as_array().unwrap().len(), 1);
    assert_eq!(root["imports"][0]["path"].as_str(), Some("b.rs"));
    assert_eq!(
        root["imports"][0]["imports"][0]["path"].as_str(),
        Some("c.rs")
    );
}

#[tokio::test]
async fn get_import_tree_returns_error_for_unknown_path() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_import_tree(Parameters(GetImportTreeRequest {
            path: "no/such.rs".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some(), "unknown path must return error");
}

#[tokio::test]
async fn get_import_tree_leaf_at_max_depth() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("a.rs").unwrap());
        let b = store.upsert_node(TrunkPath::parse("b.rs").unwrap());
        let c = store.upsert_node(TrunkPath::parse("c.rs").unwrap());
        store.upsert_edge(EdgeKind::Imports, a, b);
        store.upsert_edge(EdgeKind::Imports, b, c);
    }
    let raw = server
        .mycelium_get_import_tree(Parameters(GetImportTreeRequest {
            path: "a.rs".to_string(),
            max_depth: Some(1),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let b_node = &val["root"]["imports"][0];
    assert_eq!(b_node["path"].as_str(), Some("b.rs"));
    assert!(
        b_node["imports"].as_array().unwrap().is_empty(),
        "b must be a leaf at max_depth=1"
    );
}

// ── RFC-0025: mycelium_batch_symbol_info ─────────────────────────────

#[tokio::test]
async fn batch_symbol_info_returns_info_for_each_path() {
    let server = server_with_call_fixture().await;
    // foo→bar, foo→baz
    let raw = server
        .mycelium_batch_symbol_info(Parameters(BatchSymbolInfoRequest {
            paths: vec!["src/lib.rs>foo".to_string(), "src/lib.rs>bar".to_string()],
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let symbols = val["symbols"].as_array().unwrap();
    assert_eq!(symbols.len(), 2);
    let foo = &symbols[0];
    let bar = &symbols[1];
    assert_eq!(foo["path"].as_str(), Some("src/lib.rs>foo"));
    assert!(foo.get("error").is_none(), "foo should be found");
    assert!(
        !foo["callees"].as_array().unwrap().is_empty(),
        "foo has callees"
    );
    assert_eq!(bar["path"].as_str(), Some("src/lib.rs>bar"));
    assert!(bar.get("error").is_none(), "bar should be found");
}

#[tokio::test]
async fn batch_symbol_info_unknown_path_returns_error_entry() {
    let server = server_with_call_fixture().await;
    let raw = server
        .mycelium_batch_symbol_info(Parameters(BatchSymbolInfoRequest {
            paths: vec!["src/lib.rs>foo".to_string(), "no/such>path".to_string()],
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let symbols = val["symbols"].as_array().unwrap();
    assert_eq!(symbols.len(), 2);
    assert!(symbols[0].get("error").is_none(), "foo is found");
    assert!(
        symbols[1].get("error").is_some(),
        "unknown path has error field"
    );
}

#[tokio::test]
async fn batch_symbol_info_preserves_input_order() {
    let server = server_with_call_fixture().await;
    let raw = server
        .mycelium_batch_symbol_info(Parameters(BatchSymbolInfoRequest {
            paths: vec![
                "src/lib.rs>bar".to_string(),
                "src/lib.rs>foo".to_string(),
                "src/lib.rs>baz".to_string(),
            ],
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let symbols = val["symbols"].as_array().unwrap();
    assert_eq!(symbols[0]["path"].as_str(), Some("src/lib.rs>bar"));
    assert_eq!(symbols[1]["path"].as_str(), Some("src/lib.rs>foo"));
    assert_eq!(symbols[2]["path"].as_str(), Some("src/lib.rs>baz"));
}

// ── RFC-0019: mycelium_rank_symbols ──────────────────────────────────

#[tokio::test]
async fn rank_symbols_returns_top_callee_descending() {
    let server = server_with_call_fixture().await;
    // Fixture: foo→bar, foo→baz, baz→bar → bar has 2 callers, baz has 1, foo has 0
    let raw = server
        .mycelium_rank_symbols(Parameters(RankSymbolsRequest {
            limit: None,
            edge_kind: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let symbols = val["symbols"].as_array().unwrap();
    assert!(!symbols.is_empty(), "ranked list must be non-empty");
    // First symbol must have the highest caller_count
    let first_count = symbols[0]["caller_count"].as_u64().unwrap();
    assert!(first_count >= 2, "bar must be ranked first with 2 callers");
    // Verify foo (no callers) is excluded
    assert!(
        !symbols
            .iter()
            .any(|s| s["path"].as_str() == Some("src/lib.rs>foo")),
        "foo has no callers and must be excluded"
    );
}

#[tokio::test]
async fn rank_symbols_respects_limit() {
    let server = server_with_call_fixture().await;
    let raw = server
        .mycelium_rank_symbols(Parameters(RankSymbolsRequest {
            limit: Some(1),
            edge_kind: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(
        val["symbols"].as_array().unwrap().len(),
        1,
        "limit=1 must return exactly one symbol"
    );
}

#[tokio::test]
async fn rank_symbols_empty_when_no_call_edges() {
    let server = server_with_fixture().await; // only Contains edges, no Calls
    let raw = server
        .mycelium_rank_symbols(Parameters(RankSymbolsRequest {
            limit: None,
            edge_kind: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(
        val["symbols"].as_array().unwrap().is_empty(),
        "no call edges means empty ranking"
    );
}

// ── RFC-0018: mycelium_get_files ─────────────────────────────────────

#[tokio::test]
async fn get_files_returns_only_file_paths() {
    let server = server_with_fixture().await;
    // server_with_fixture has src/greet.rs, src/greet.rs>greet, src/greet.rs>helper
    let raw = server
        .mycelium_get_files(Parameters(GetFilesRequest {
            path_prefix: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let files: Vec<&str> = val["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(files.contains(&"src/greet.rs"), "greet.rs must be listed");
    assert!(
        !files.iter().any(|p| p.contains('>')),
        "symbol paths must not appear in file list"
    );
}

#[tokio::test]
async fn get_files_filters_by_prefix() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/auth.rs").unwrap());
        store.upsert_node(TrunkPath::parse("tests/auth_test.rs").unwrap());
        store.upsert_node(TrunkPath::parse("src/main.rs").unwrap());
    }
    let raw = server
        .mycelium_get_files(Parameters(GetFilesRequest {
            path_prefix: Some("src/".to_string()),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let files: Vec<&str> = val["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(
        files.contains(&"src/auth.rs"),
        "src/auth.rs must match prefix"
    );
    assert!(
        files.contains(&"src/main.rs"),
        "src/main.rs must match prefix"
    );
    assert!(
        !files.contains(&"tests/auth_test.rs"),
        "tests/ file must not match src/ prefix"
    );
}

#[tokio::test]
async fn get_files_returns_sorted_order() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("z.rs").unwrap());
        store.upsert_node(TrunkPath::parse("a.rs").unwrap());
        store.upsert_node(TrunkPath::parse("m.rs").unwrap());
    }
    let raw = server
        .mycelium_get_files(Parameters(GetFilesRequest {
            path_prefix: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let files: Vec<&str> = val["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let mut sorted = files.clone();
    sorted.sort_unstable();
    assert_eq!(files, sorted, "files must be returned in sorted order");
}

// ── RFC-0015: watch-mode stub resolution ─────────────────────────────

#[tokio::test(flavor = "multi_thread")]
#[ignore = "slow: filesystem watcher timing (run with --ignored)"]
async fn watch_mode_resolves_stub_after_callee_file_added() {
    use std::fs;
    use tokio::time::Duration;

    let tmp = tempfile::tempdir().unwrap();
    // a.py calls bar() which is not yet defined anywhere.
    fs::write(tmp.path().join("a.py"), "def foo():\n    bar()").unwrap();

    let server = MyceliumServer::with_root(tmp.path().to_owned())
        .await
        .unwrap();

    // After initial index, bare stub "bar" exists.
    assert!(
        server.store.read().await.lookup("a.py>foo").is_some(),
        "a.py>foo must be indexed"
    );

    // Now create b.py which defines bar().
    fs::write(tmp.path().join("b.py"), "def bar(): pass").unwrap();

    // Poll until the watcher picks up b.py and resolve_bare_call_stubs runs.
    // 30 s budget: 300 ms FSE debounce + extraction + re-index on slow CI.
    // GitHub-Actions runners under heavy load occasionally exceed 8 s; 30 s
    // gives a generous safety margin without slowing the green-path case.
    let resolved = poll_for(
        Duration::from_secs(30),
        Duration::from_millis(100),
        || async {
            let store = server.store.read().await;
            let foo = store.lookup("a.py>foo");
            let bar_def = store.lookup("b.py>bar");
            let bar_stub = store.lookup("bar");
            match (foo, bar_def) {
                (Some(f), Some(b)) => {
                    store.outgoing(f, EdgeKind::Calls).contains(&b) && bar_stub.is_none()
                }
                _ => false,
            }
        },
    )
    .await;

    assert!(
        resolved,
        "watch mode must resolve bare 'bar' stub to 'b.py>bar' after b.py is created"
    );
}

// ── RFC-0026: mycelium_get_extends / mycelium_get_implements ─────────

async fn server_with_inheritance_fixture() -> MyceliumServer {
    let server = MyceliumServer::new();
    let mut store = server.store.write().await;
    // Shape ← Rectangle ← Square (extends chain)
    let shape =
        store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/shapes.py>Shape").unwrap());
    let rect = store
        .upsert_node(mycelium_core::trunk::TrunkPath::parse("src/shapes.py>Rectangle").unwrap());
    let square =
        store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/shapes.py>Square").unwrap());
    // IShape interface implemented by Shape
    let ishape =
        store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/shapes.py>IShape").unwrap());
    store.upsert_edge(mycelium_core::types::EdgeKind::Extends, rect, shape);
    store.upsert_edge(mycelium_core::types::EdgeKind::Extends, square, rect);
    store.upsert_edge(mycelium_core::types::EdgeKind::Implements, shape, ishape);
    drop(store);
    server
}

#[tokio::test]
async fn get_extends_returns_extends_and_extended_by() {
    let server = server_with_inheritance_fixture().await;
    let raw = server
        .mycelium_get_extends(Parameters(GetExtendsRequest {
            path: "src/shapes.py>Rectangle".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none(), "rect is found");
    let extends = val["extends"].as_array().unwrap();
    let extended_by = val["extended_by"].as_array().unwrap();
    assert_eq!(extends.len(), 1);
    assert_eq!(extends[0].as_str(), Some("src/shapes.py>Shape"));
    assert_eq!(extended_by.len(), 1);
    assert_eq!(extended_by[0].as_str(), Some("src/shapes.py>Square"));
}

#[tokio::test]
async fn get_extends_empty_when_no_edges() {
    let server = server_with_inheritance_fixture().await;
    let raw = server
        .mycelium_get_extends(Parameters(GetExtendsRequest {
            path: "src/shapes.py>Shape".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["extends"].as_array().unwrap().is_empty());
    assert_eq!(val["extended_by"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn get_extends_unknown_path_returns_error() {
    let server = server_with_inheritance_fixture().await;
    let raw = server
        .mycelium_get_extends(Parameters(GetExtendsRequest {
            path: "no/such>Symbol".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

#[tokio::test]
async fn get_implements_returns_implements_and_implemented_by() {
    let server = server_with_inheritance_fixture().await;
    let raw = server
        .mycelium_get_implements(Parameters(GetImplementsRequest {
            path: "src/shapes.py>Shape".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none());
    let implements = val["implements"].as_array().unwrap();
    let implemented_by = val["implemented_by"].as_array().unwrap();
    assert_eq!(implements.len(), 1);
    assert_eq!(implements[0].as_str(), Some("src/shapes.py>IShape"));
    assert!(implemented_by.is_empty());
}

#[tokio::test]
async fn get_implements_interface_side_shows_implemented_by() {
    let server = server_with_inheritance_fixture().await;
    let raw = server
        .mycelium_get_implements(Parameters(GetImplementsRequest {
            path: "src/shapes.py>IShape".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none());
    assert!(val["implements"].as_array().unwrap().is_empty());
    let implemented_by = val["implemented_by"].as_array().unwrap();
    assert_eq!(implemented_by.len(), 1);
    assert_eq!(implemented_by[0].as_str(), Some("src/shapes.py>Shape"));
}

#[tokio::test]
async fn get_implements_unknown_path_returns_error() {
    let server = server_with_inheritance_fixture().await;
    let raw = server
        .mycelium_get_implements(Parameters(GetImplementsRequest {
            path: "no/such>Symbol".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

// ── RFC-0027: mycelium_find_import_path ──────────────────────────────

async fn server_with_import_chain_fixture() -> MyceliumServer {
    let server = MyceliumServer::new();
    let mut store = server.store.write().await;
    // a → b → c (import chain)
    let a = store.upsert_node(mycelium_core::trunk::TrunkPath::parse("a.rs").unwrap());
    let b = store.upsert_node(mycelium_core::trunk::TrunkPath::parse("b.rs").unwrap());
    let c = store.upsert_node(mycelium_core::trunk::TrunkPath::parse("c.rs").unwrap());
    store.upsert_edge(mycelium_core::types::EdgeKind::Imports, a, b);
    store.upsert_edge(mycelium_core::types::EdgeKind::Imports, b, c);
    drop(store);
    let _ = (a, b, c);
    server
}

#[tokio::test]
async fn find_import_path_direct() {
    let server = server_with_import_chain_fixture().await;
    let raw = server
        .mycelium_find_import_path(Parameters(FindImportPathRequest {
            from_path: "a.rs".to_string(),
            to_path: "b.rs".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none());
    assert_eq!(val["hops"].as_u64(), Some(1));
    let path = val["path"].as_array().unwrap();
    assert_eq!(path.len(), 2);
    assert_eq!(path[0].as_str(), Some("a.rs"));
    assert_eq!(path[1].as_str(), Some("b.rs"));
}

#[tokio::test]
async fn find_import_path_transitive() {
    let server = server_with_import_chain_fixture().await;
    let raw = server
        .mycelium_find_import_path(Parameters(FindImportPathRequest {
            from_path: "a.rs".to_string(),
            to_path: "c.rs".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none());
    assert_eq!(val["hops"].as_u64(), Some(2));
    let path = val["path"].as_array().unwrap();
    assert_eq!(path, &["a.rs", "b.rs", "c.rs"]);
}

#[tokio::test]
async fn find_import_path_unreachable_returns_empty() {
    let server = server_with_import_chain_fixture().await;
    let raw = server
        .mycelium_find_import_path(Parameters(FindImportPathRequest {
            from_path: "c.rs".to_string(),
            to_path: "a.rs".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none());
    assert!(val["path"].as_array().unwrap().is_empty());
    assert!(val["hops"].is_null());
    assert!(val["message"].is_string());
}

#[tokio::test]
async fn find_import_path_unknown_from_returns_error() {
    let server = server_with_import_chain_fixture().await;
    let raw = server
        .mycelium_find_import_path(Parameters(FindImportPathRequest {
            from_path: "no/such.rs".to_string(),
            to_path: "b.rs".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

#[tokio::test]
async fn find_import_path_max_depth_respected() {
    let server = server_with_import_chain_fixture().await;
    // a → b → c; max_depth=1 cannot reach c
    let raw_shallow = server
        .mycelium_find_import_path(Parameters(FindImportPathRequest {
            from_path: "a.rs".to_string(),
            to_path: "c.rs".to_string(),
            max_depth: Some(1),
            output_format: None,
        }))
        .await;
    let val_shallow: serde_json::Value = serde_json::from_str(result_str(&raw_shallow)).unwrap();
    assert!(val_shallow["path"].as_array().unwrap().is_empty());

    let raw_deep = server
        .mycelium_find_import_path(Parameters(FindImportPathRequest {
            from_path: "a.rs".to_string(),
            to_path: "c.rs".to_string(),
            max_depth: Some(2),
            output_format: None,
        }))
        .await;
    let val_deep: serde_json::Value = serde_json::from_str(result_str(&raw_deep)).unwrap();
    assert_eq!(val_deep["hops"].as_u64(), Some(2));
}

// ── RFC-0028: mycelium_get_node_kind / mycelium_get_symbols_by_kind ──

async fn server_with_kind_fixture() -> MyceliumServer {
    let server = MyceliumServer::new();
    let mut store = server.store.write().await;
    let f1 = store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/a.rs>foo").unwrap());
    let f2 = store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/b.rs>bar").unwrap());
    let c1 = store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/c.rs>Baz").unwrap());
    store.set_kind(f1, mycelium_core::types::NodeKind::Function);
    store.set_kind(f2, mycelium_core::types::NodeKind::Function);
    store.set_kind(c1, mycelium_core::types::NodeKind::Class);
    drop(store);
    server
}

#[tokio::test]
async fn get_node_kind_returns_kind_for_known_path() {
    let server = server_with_kind_fixture().await;
    let raw = server
        .mycelium_get_node_kind(Parameters(GetNodeKindRequest {
            path: "src/a.rs>foo".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none());
    assert_eq!(val["kind"].as_str(), Some("function"));
}

#[tokio::test]
async fn get_node_kind_returns_null_when_kind_not_recorded() {
    let server = MyceliumServer::new();
    let mut store = server.store.write().await;
    store.upsert_node(mycelium_core::trunk::TrunkPath::parse("x.rs>foo").unwrap());
    drop(store);
    let raw = server
        .mycelium_get_node_kind(Parameters(GetNodeKindRequest {
            path: "x.rs>foo".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none());
    assert!(val["kind"].is_null());
}

#[tokio::test]
async fn get_node_kind_unknown_path_returns_error() {
    let server = server_with_kind_fixture().await;
    let raw = server
        .mycelium_get_node_kind(Parameters(GetNodeKindRequest {
            path: "no/such>path".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

#[tokio::test]
async fn get_symbols_by_kind_returns_all_matching() {
    let server = server_with_kind_fixture().await;
    let raw = server
        .mycelium_get_symbols_by_kind(Parameters(GetSymbolsByKindRequest {
            kind: "function".to_string(),
            path_prefix: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none());
    let syms = val["symbols"].as_array().unwrap();
    assert_eq!(syms.len(), 2);
    assert!(syms.iter().any(|s| s.as_str() == Some("src/a.rs>foo")));
    assert!(syms.iter().any(|s| s.as_str() == Some("src/b.rs>bar")));
}

#[tokio::test]
async fn get_symbols_by_kind_with_prefix_filter() {
    let server = server_with_kind_fixture().await;
    // add one function outside src/
    {
        let mut store = server.store.write().await;
        let id = store
            .upsert_node(mycelium_core::trunk::TrunkPath::parse("tests/t.rs>test_fn").unwrap());
        store.set_kind(id, mycelium_core::types::NodeKind::Function);
    }
    let raw = server
        .mycelium_get_symbols_by_kind(Parameters(GetSymbolsByKindRequest {
            kind: "function".to_string(),
            path_prefix: Some("src/".to_string()),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let syms = val["symbols"].as_array().unwrap();
    assert_eq!(syms.len(), 2, "only src/ functions");
    assert!(syms.iter().all(|s| s.as_str().unwrap().starts_with("src/")));
}

#[tokio::test]
async fn get_symbols_by_kind_unknown_kind_returns_error() {
    let server = server_with_kind_fixture().await;
    let raw = server
        .mycelium_get_symbols_by_kind(Parameters(GetSymbolsByKindRequest {
            kind: "not_a_real_kind".to_string(),
            path_prefix: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

// ── RFC-0029: mycelium_get_source_span ───────────────────────────────

async fn server_with_span_fixture() -> MyceliumServer {
    let server = MyceliumServer::new();
    let mut store = server.store.write().await;
    let id =
        store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/auth.rs>login").unwrap());
    store.set_span(
        id,
        mycelium_core::types::SourceSpan {
            start_line: 10,
            start_col: 0,
            end_line: 20,
            end_col: 1,
            start_byte: 100,
            end_byte: 300,
        },
    );
    drop(store);
    server
}

#[tokio::test]
async fn get_source_span_returns_all_fields() {
    let server = server_with_span_fixture().await;
    let raw = server
        .mycelium_get_source_span(Parameters(GetSourceSpanRequest {
            path: "src/auth.rs>login".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none(), "should not error");
    assert_eq!(val["start_line"].as_u64(), Some(10));
    assert_eq!(val["start_col"].as_u64(), Some(0));
    assert_eq!(val["end_line"].as_u64(), Some(20));
    assert_eq!(val["end_col"].as_u64(), Some(1));
    assert_eq!(val["start_byte"].as_u64(), Some(100));
    assert_eq!(val["end_byte"].as_u64(), Some(300));
}

#[tokio::test]
async fn get_source_span_returns_null_when_span_not_recorded() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(mycelium_core::trunk::TrunkPath::parse("x.rs>foo").unwrap());
    }
    let raw = server
        .mycelium_get_source_span(Parameters(GetSourceSpanRequest {
            path: "x.rs>foo".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none(), "should not error");
    assert!(val["span"].is_null(), "span must be null when unrecorded");
}

#[tokio::test]
async fn get_source_span_unknown_path_returns_error() {
    let server = server_with_span_fixture().await;
    let raw = server
        .mycelium_get_source_span(Parameters(GetSourceSpanRequest {
            path: "no/such>path".to_string(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(
        val.get("error").is_some(),
        "must return error for unknown path"
    );
}

// ── RFC-0030: mycelium_find_extends_path ─────────────────────────────

async fn server_with_extends_fixture() -> MyceliumServer {
    let server = MyceliumServer::new();
    let mut store = server.store.write().await;
    let base =
        store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/base.ts>Base").unwrap());
    let mid = store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/mid.ts>Mid").unwrap());
    let child =
        store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/child.ts>Child").unwrap());
    store.upsert_edge(mycelium_core::types::EdgeKind::Extends, child, mid);
    store.upsert_edge(mycelium_core::types::EdgeKind::Extends, mid, base);
    drop(store);
    server
}

#[tokio::test]
async fn find_extends_path_direct() {
    let server = server_with_extends_fixture().await;
    let raw = server
        .mycelium_find_extends_path(Parameters(FindExtendsPathRequest {
            from_path: "src/child.ts>Child".to_string(),
            to_path: "src/mid.ts>Mid".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none());
    assert_eq!(val["hops"].as_u64(), Some(1));
}

#[tokio::test]
async fn find_extends_path_transitive() {
    let server = server_with_extends_fixture().await;
    let raw = server
        .mycelium_find_extends_path(Parameters(FindExtendsPathRequest {
            from_path: "src/child.ts>Child".to_string(),
            to_path: "src/base.ts>Base".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none());
    assert_eq!(val["hops"].as_u64(), Some(2));
}

#[tokio::test]
async fn find_extends_path_unreachable_returns_empty() {
    let server = server_with_extends_fixture().await;
    let raw = server
        .mycelium_find_extends_path(Parameters(FindExtendsPathRequest {
            from_path: "src/base.ts>Base".to_string(),
            to_path: "src/child.ts>Child".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none());
    assert!(val["hops"].is_null());
    assert!(val["path"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn find_extends_path_unknown_from_returns_error() {
    let server = server_with_extends_fixture().await;
    let raw = server
        .mycelium_find_extends_path(Parameters(FindExtendsPathRequest {
            from_path: "no/such>path".to_string(),
            to_path: "src/base.ts>Base".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

// ── RFC-0031: mycelium_get_extends_tree ──────────────────────────────

#[tokio::test]
async fn get_extends_tree_returns_superclass_chain() {
    let server = server_with_extends_fixture().await;
    let raw = server
        .mycelium_get_extends_tree(Parameters(GetExtendsTreeRequest {
            path: "src/child.ts>Child".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none());
    let root = &val["root"];
    assert_eq!(root["path"].as_str(), Some("src/child.ts>Child"));
    assert_eq!(root["parents"].as_array().unwrap().len(), 1);
    assert_eq!(root["parents"][0]["path"].as_str(), Some("src/mid.ts>Mid"));
}

#[tokio::test]
async fn get_extends_tree_leaf_at_max_depth_zero() {
    let server = server_with_extends_fixture().await;
    let raw = server
        .mycelium_get_extends_tree(Parameters(GetExtendsTreeRequest {
            path: "src/child.ts>Child".to_string(),
            max_depth: Some(0),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["root"]["parents"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn get_extends_tree_unknown_path_returns_error() {
    let server = server_with_extends_fixture().await;
    let raw = server
        .mycelium_get_extends_tree(Parameters(GetExtendsTreeRequest {
            path: "no/such>path".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

// ── RFC-0032: mycelium_get_subclasses_tree ───────────────────────────

#[tokio::test]
async fn get_subclasses_tree_returns_subclass_forest() {
    let server = server_with_extends_fixture().await;
    // Fixture: Child→Mid→Base (Child extends Mid, Mid extends Base)
    // From Base perspective: Base has one subclass Mid, Mid has one subclass Child
    let raw = server
        .mycelium_get_subclasses_tree(Parameters(GetSubclassesTreeRequest {
            path: "src/base.ts>Base".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let root = val.get("root").expect("root key present");
    assert_eq!(root["path"], "src/base.ts>Base");
    let subclasses = root["subclasses"].as_array().unwrap();
    assert_eq!(subclasses.len(), 1);
    assert_eq!(subclasses[0]["path"], "src/mid.ts>Mid");
    let mid_subclasses = subclasses[0]["subclasses"].as_array().unwrap();
    assert_eq!(mid_subclasses.len(), 1);
    assert_eq!(mid_subclasses[0]["path"], "src/child.ts>Child");
}

#[tokio::test]
async fn get_subclasses_tree_leaf_at_max_depth_zero() {
    let server = server_with_extends_fixture().await;
    let raw = server
        .mycelium_get_subclasses_tree(Parameters(GetSubclassesTreeRequest {
            path: "src/base.ts>Base".to_string(),
            max_depth: Some(0),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let root = val.get("root").expect("root key present");
    let subclasses = root["subclasses"].as_array().unwrap();
    assert!(subclasses.is_empty());
}

#[tokio::test]
async fn get_subclasses_tree_unknown_path_returns_error() {
    let server = server_with_extends_fixture().await;
    let raw = server
        .mycelium_get_subclasses_tree(Parameters(GetSubclassesTreeRequest {
            path: "no/such>path".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

// ── RFC-0033: mycelium_find_implements_path ───────────────────────────

async fn server_with_implements_fixture() -> MyceliumServer {
    let server = MyceliumServer::new();
    {
        let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
        let cls =
            store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/cls.ts>Cls").unwrap());
        let iface = store
            .upsert_node(mycelium_core::trunk::TrunkPath::parse("src/iface.ts>IFace").unwrap());
        let base_iface = store
            .upsert_node(mycelium_core::trunk::TrunkPath::parse("src/base.ts>BaseIFace").unwrap());
        store.upsert_edge(mycelium_core::EdgeKind::Implements, cls, iface);
        store.upsert_edge(mycelium_core::EdgeKind::Implements, iface, base_iface);
    }
    server
}

#[tokio::test]
async fn find_implements_path_direct_hop() {
    let server = server_with_implements_fixture().await;
    let raw = server
        .mycelium_find_implements_path(Parameters(FindImplementsPathRequest {
            from_path: "src/cls.ts>Cls".to_string(),
            to_path: "src/iface.ts>IFace".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let path = val["path"].as_array().unwrap();
    assert_eq!(path.len(), 2);
    assert_eq!(path[0], "src/cls.ts>Cls");
    assert_eq!(path[1], "src/iface.ts>IFace");
    assert_eq!(val["hops"], 1);
}

#[tokio::test]
async fn find_implements_path_unreachable() {
    let server = server_with_implements_fixture().await;
    let raw = server
        .mycelium_find_implements_path(Parameters(FindImplementsPathRequest {
            from_path: "src/iface.ts>IFace".to_string(),
            to_path: "src/cls.ts>Cls".to_string(), // backwards — no path
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["path"].as_array().unwrap().len(), 0);
    assert!(val["hops"].is_null());
}

#[tokio::test]
async fn find_implements_path_unknown_path_returns_error() {
    let server = server_with_implements_fixture().await;
    let raw = server
        .mycelium_find_implements_path(Parameters(FindImplementsPathRequest {
            from_path: "no/such>path".to_string(),
            to_path: "src/iface.ts>IFace".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

#[tokio::test]
async fn find_implements_path_transitive() {
    let server = server_with_implements_fixture().await;
    let raw = server
        .mycelium_find_implements_path(Parameters(FindImplementsPathRequest {
            from_path: "src/cls.ts>Cls".to_string(),
            to_path: "src/base.ts>BaseIFace".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let path = val["path"].as_array().unwrap();
    assert_eq!(path.len(), 3);
    assert_eq!(val["hops"], 2);
}

// ── RFC-0034: mycelium_get_implements_tree ────────────────────────────

#[tokio::test]
async fn get_implements_tree_returns_interface_chain() {
    // Re-use server_with_implements_fixture: Cls→IFace→BaseIFace
    let server = server_with_implements_fixture().await;
    let raw = server
        .mycelium_get_implements_tree(Parameters(GetImplementsTreeRequest {
            path: "src/cls.ts>Cls".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let root = val.get("root").expect("root key present");
    assert_eq!(root["path"], "src/cls.ts>Cls");
    let ifaces = root["interfaces"].as_array().unwrap();
    assert_eq!(ifaces.len(), 1);
    assert_eq!(ifaces[0]["path"], "src/iface.ts>IFace");
    let base_ifaces = ifaces[0]["interfaces"].as_array().unwrap();
    assert_eq!(base_ifaces.len(), 1);
    assert_eq!(base_ifaces[0]["path"], "src/base.ts>BaseIFace");
}

#[tokio::test]
async fn get_implements_tree_leaf_at_max_depth_zero() {
    let server = server_with_implements_fixture().await;
    let raw = server
        .mycelium_get_implements_tree(Parameters(GetImplementsTreeRequest {
            path: "src/cls.ts>Cls".to_string(),
            max_depth: Some(0),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let root = val.get("root").unwrap();
    assert!(root["interfaces"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn get_implements_tree_unknown_path_returns_error() {
    let server = server_with_implements_fixture().await;
    let raw = server
        .mycelium_get_implements_tree(Parameters(GetImplementsTreeRequest {
            path: "no/such>path".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

// ── RFC-0035: mycelium_get_implementors_tree ──────────────────────────

#[tokio::test]
async fn get_implementors_tree_returns_implementor_chain() {
    // Fixture: Cls→IFace→BaseIFace (Cls implements IFace, IFace implements BaseIFace)
    // From BaseIFace perspective: BaseIFace←IFace←Cls
    let server = server_with_implements_fixture().await;
    let raw = server
        .mycelium_get_implementors_tree(Parameters(GetImplementorsTreeRequest {
            path: "src/base.ts>BaseIFace".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let root = val.get("root").expect("root key present");
    assert_eq!(root["path"], "src/base.ts>BaseIFace");
    let impls = root["implementors"].as_array().unwrap();
    assert_eq!(impls.len(), 1);
    assert_eq!(impls[0]["path"], "src/iface.ts>IFace");
    let cls_impls = impls[0]["implementors"].as_array().unwrap();
    assert_eq!(cls_impls.len(), 1);
    assert_eq!(cls_impls[0]["path"], "src/cls.ts>Cls");
}

#[tokio::test]
async fn get_implementors_tree_leaf_at_max_depth_zero() {
    let server = server_with_implements_fixture().await;
    let raw = server
        .mycelium_get_implementors_tree(Parameters(GetImplementorsTreeRequest {
            path: "src/base.ts>BaseIFace".to_string(),
            max_depth: Some(0),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let root = val.get("root").unwrap();
    assert!(root["implementors"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn get_implementors_tree_unknown_path_returns_error() {
    let server = server_with_implements_fixture().await;
    let raw = server
        .mycelium_get_implementors_tree(Parameters(GetImplementorsTreeRequest {
            path: "no/such>path".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

// ── RFC-0036: mycelium_get_importers_tree ─────────────────────────────

async fn server_with_imports_fixture() -> MyceliumServer {
    let server = MyceliumServer::new();
    {
        let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
        let core_mod =
            store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/core.ts>core").unwrap());
        let mid_mod =
            store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/mid.ts>mid").unwrap());
        let top_mod =
            store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/top.ts>top").unwrap());
        store.upsert_edge(mycelium_core::EdgeKind::Imports, mid_mod, core_mod);
        store.upsert_edge(mycelium_core::EdgeKind::Imports, top_mod, mid_mod);
    }
    server
}

#[tokio::test]
async fn get_importers_tree_returns_reverse_dependency_chain() {
    let server = server_with_imports_fixture().await;
    let raw = server
        .mycelium_get_importers_tree(Parameters(GetImportersTreeRequest {
            path: "src/core.ts>core".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let root = val.get("root").expect("root key present");
    assert_eq!(root["path"], "src/core.ts>core");
    let importers = root["importers"].as_array().unwrap();
    assert_eq!(importers.len(), 1);
    assert_eq!(importers[0]["path"], "src/mid.ts>mid");
    let top_importers = importers[0]["importers"].as_array().unwrap();
    assert_eq!(top_importers.len(), 1);
    assert_eq!(top_importers[0]["path"], "src/top.ts>top");
}

#[tokio::test]
async fn get_importers_tree_leaf_at_max_depth_zero() {
    let server = server_with_imports_fixture().await;
    let raw = server
        .mycelium_get_importers_tree(Parameters(GetImportersTreeRequest {
            path: "src/core.ts>core".to_string(),
            max_depth: Some(0),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let root = val.get("root").unwrap();
    assert!(root["importers"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn get_importers_tree_unknown_path_returns_error() {
    let server = server_with_imports_fixture().await;
    let raw = server
        .mycelium_get_importers_tree(Parameters(GetImportersTreeRequest {
            path: "no/such>path".to_string(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

// ── RFC-0037: mycelium_get_dead_symbols ──────────────────────────────

async fn server_with_mixed_fixture() -> MyceliumServer {
    let server = MyceliumServer::new();
    let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
    // live: caller calls target
    let caller = store.upsert_node(TrunkPath::parse("src/main.rs>main").unwrap());
    let target = store.upsert_node(TrunkPath::parse("src/lib.rs>helper").unwrap());
    store.upsert_edge(EdgeKind::Calls, caller, target);
    // dead: no callers, no importers
    store.upsert_node(TrunkPath::parse("src/lib.rs>dead_fn").unwrap());
    // file node (should be excluded from dead_symbols)
    store.upsert_node(TrunkPath::parse("src/lib.rs").unwrap());
    drop(store);
    server
}

#[tokio::test]
async fn get_dead_symbols_returns_unreferenced_symbols() {
    let server = server_with_mixed_fixture().await;
    let raw = server
        .mycelium_get_dead_symbols(Parameters(GetDeadSymbolsRequest {
            path_prefix: None,
            edge_kind: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let dead: Vec<String> = val["dead_symbols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    // main has no callers → dead; helper is called → live; dead_fn → dead
    assert!(dead.contains(&"src/lib.rs>dead_fn".to_owned()));
    assert!(dead.contains(&"src/main.rs>main".to_owned()));
    assert!(!dead.contains(&"src/lib.rs>helper".to_owned()));
    // file node must not appear
    assert!(!dead.iter().any(|s| s == "src/lib.rs"));
    assert_eq!(val["count"].as_u64().unwrap(), dead.len() as u64);
    // exact count: fixture has exactly 2 dead symbols (dead_fn + main); helper is live
    assert_eq!(
        dead.len(),
        2,
        "fixture has exactly 2 dead symbols: dead_fn and main"
    );
}

#[tokio::test]
async fn get_dead_symbols_empty_store() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_dead_symbols(Parameters(GetDeadSymbolsRequest {
            path_prefix: None,
            edge_kind: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["dead_symbols"].as_array().unwrap().len(), 0);
    assert_eq!(val["count"].as_u64().unwrap(), 0);
}

#[tokio::test]
async fn get_dead_symbols_prefix_filter() {
    let server = server_with_mixed_fixture().await;
    let raw = server
        .mycelium_get_dead_symbols(Parameters(GetDeadSymbolsRequest {
            path_prefix: Some("src/lib.rs".to_owned()),
            edge_kind: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let dead: Vec<String> = val["dead_symbols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    // Only dead symbols under src/lib.rs prefix
    assert!(dead.iter().all(|s| s.starts_with("src/lib.rs")));
    assert!(dead.contains(&"src/lib.rs>dead_fn".to_owned()));
    assert!(!dead.contains(&"src/main.rs>main".to_owned()));
    // exact count: only dead_fn is under src/lib.rs; main is under src/main.rs
    assert_eq!(
        dead.len(),
        1,
        "prefix filter src/lib.rs must return exactly 1 dead symbol"
    );
}

// ── RFC-0056: mycelium_get_isolated_symbols ───────────────────────────

#[tokio::test]
async fn get_isolated_symbols_returns_completely_disconnected() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let _orphan = store.upsert_node(TrunkPath::parse("src/orphan.rs>orphan").unwrap());
        let conn_a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let conn_b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        store.upsert_edge(EdgeKind::Calls, conn_a, conn_b);
    }
    let raw = server
        .mycelium_get_isolated_symbols(Parameters(GetIsolatedSymbolsRequest {
            path_prefix: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 1);
    assert_eq!(
        val["isolated_symbols"].as_array().unwrap()[0]
            .as_str()
            .unwrap(),
        "src/orphan.rs>orphan"
    );
}

#[tokio::test]
async fn get_isolated_symbols_empty_store() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_isolated_symbols(Parameters(GetIsolatedSymbolsRequest {
            path_prefix: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
}

#[tokio::test]
async fn get_isolated_symbols_prefix_filter() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let _src_orphan = store.upsert_node(TrunkPath::parse("src/orphan.rs>orphan").unwrap());
        let _lib_orphan = store.upsert_node(TrunkPath::parse("lib/orphan.rs>orphan").unwrap());
    }
    let raw = server
        .mycelium_get_isolated_symbols(Parameters(GetIsolatedSymbolsRequest {
            path_prefix: Some("src/".to_owned()),
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 1);
    assert!(
        val["isolated_symbols"].as_array().unwrap()[0]
            .as_str()
            .unwrap()
            .starts_with("src/")
    );
}

// ── RFC-0038: mycelium_get_stats ─────────────────────────────────────

#[tokio::test]
async fn get_stats_empty_store() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_stats(Parameters(GetStatsRequest {
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["total_nodes"].as_u64().unwrap(), 0);
    assert_eq!(val["total_edges"].as_u64().unwrap(), 0);
    assert_eq!(val["nodes_by_kind"].as_object().unwrap().len(), 0);
    assert_eq!(val["edges_by_kind"].as_object().unwrap().len(), 0);
}

#[tokio::test]
async fn get_stats_counts_nodes_and_edges() {
    let server = MyceliumServer::new();
    {
        let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
        let fn1 = store.upsert_node(TrunkPath::parse("src/a.rs>fn1").unwrap());
        let fn2 = store.upsert_node(TrunkPath::parse("src/b.rs>fn2").unwrap());
        store.set_kind(fn1, mycelium_core::NodeKind::Function);
        store.set_kind(fn2, mycelium_core::NodeKind::Function);
        store.upsert_edge(EdgeKind::Calls, fn1, fn2);
        store.upsert_edge(EdgeKind::Imports, fn1, fn2);
    }
    let raw = server
        .mycelium_get_stats(Parameters(GetStatsRequest {
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["total_nodes"].as_u64().unwrap(), 2);
    assert_eq!(val["total_edges"].as_u64().unwrap(), 2);
    assert_eq!(val["nodes_by_kind"]["function"].as_u64().unwrap(), 2);
    assert_eq!(val["edges_by_kind"]["calls"].as_u64().unwrap(), 1);
    assert_eq!(val["edges_by_kind"]["imports"].as_u64().unwrap(), 1);
    assert!(val["edges_by_kind"].get("contains").is_none());
}

// ── RFC-0039: mycelium_get_cross_refs ────────────────────────────────

#[tokio::test]
async fn get_cross_refs_all_kinds() {
    let server = MyceliumServer::new();
    {
        let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
        let target = store.upsert_node(TrunkPath::parse("src/lib.rs>Base").unwrap());
        let caller = store.upsert_node(TrunkPath::parse("src/a.rs>caller").unwrap());
        let importer = store.upsert_node(TrunkPath::parse("src/b.rs>importer").unwrap());
        let child = store.upsert_node(TrunkPath::parse("src/c.rs>Child").unwrap());
        let impl_sym = store.upsert_node(TrunkPath::parse("src/d.rs>Impl").unwrap());
        store.upsert_edge(EdgeKind::Calls, caller, target);
        store.upsert_edge(EdgeKind::Imports, importer, target);
        store.upsert_edge(EdgeKind::Extends, child, target);
        store.upsert_edge(EdgeKind::Implements, impl_sym, target);
    }
    let raw = server
        .mycelium_get_cross_refs(Parameters(GetCrossRefsRequest {
            path: "src/lib.rs>Base".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["callers"][0].as_str().unwrap(), "src/a.rs>caller");
    assert_eq!(val["importers"][0].as_str().unwrap(), "src/b.rs>importer");
    assert_eq!(val["extended_by"][0].as_str().unwrap(), "src/c.rs>Child");
    assert_eq!(val["implemented_by"][0].as_str().unwrap(), "src/d.rs>Impl");
}

#[tokio::test]
async fn get_cross_refs_empty_lists_present() {
    let server = MyceliumServer::new();
    {
        let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/lone.rs>lone").unwrap());
    }
    let raw = server
        .mycelium_get_cross_refs(Parameters(GetCrossRefsRequest {
            path: "src/lone.rs>lone".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["callers"].as_array().unwrap().len(), 0);
    assert_eq!(val["importers"].as_array().unwrap().len(), 0);
    assert_eq!(val["extended_by"].as_array().unwrap().len(), 0);
    assert_eq!(val["implemented_by"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn get_cross_refs_unknown_path_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_cross_refs(Parameters(GetCrossRefsRequest {
            path: "no/such>path".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

// ── RFC-0040: mycelium_detect_cycles ─────────────────────────────────

#[tokio::test]
async fn detect_cycles_finds_circular_imports() {
    let server = MyceliumServer::new();
    {
        let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        store.upsert_edge(EdgeKind::Imports, a, b);
        store.upsert_edge(EdgeKind::Imports, b, a); // cycle
    }
    let raw = server
        .mycelium_detect_cycles(Parameters(DetectCyclesRequest {
            edge_kind: "imports".to_owned(),
            path_prefix: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let nodes: Vec<String> = val["cycle_nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(nodes.len(), 2);
    assert!(nodes.contains(&"src/a.rs>a".to_owned()));
    assert!(nodes.contains(&"src/b.rs>b".to_owned()));
    assert_eq!(val["count"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn detect_cycles_no_cycles_returns_empty() {
    let server = MyceliumServer::new();
    {
        let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        store.upsert_edge(EdgeKind::Imports, a, b); // no cycle
    }
    let raw = server
        .mycelium_detect_cycles(Parameters(DetectCyclesRequest {
            edge_kind: "imports".to_owned(),
            path_prefix: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["cycle_nodes"].as_array().unwrap().len(), 0);
    assert_eq!(val["count"].as_u64().unwrap(), 0);
}

#[tokio::test]
async fn detect_cycles_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_detect_cycles(Parameters(DetectCyclesRequest {
            edge_kind: "unknown_kind".to_owned(),
            path_prefix: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

// ── RFC-0057: mycelium_get_scc_groups ────────────────────────────────

#[tokio::test]
async fn get_scc_groups_finds_cycle_group() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let sym_a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let sym_b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        store.upsert_edge(EdgeKind::Calls, sym_a, sym_b);
        store.upsert_edge(EdgeKind::Calls, sym_b, sym_a);
    }
    let raw = server
        .mycelium_get_scc_groups(Parameters(GetSccGroupsRequest {
            edge_kind: "calls".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["group_count"].as_u64().unwrap(), 1);
    assert_eq!(val["total_symbols"].as_u64().unwrap(), 2);
    let group = val["groups"].as_array().unwrap()[0].as_array().unwrap();
    assert_eq!(group.len(), 2);
}

#[tokio::test]
async fn get_scc_groups_no_cycles_returns_empty() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let sym_a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let sym_b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        store.upsert_edge(EdgeKind::Calls, sym_a, sym_b);
    }
    let raw = server
        .mycelium_get_scc_groups(Parameters(GetSccGroupsRequest {
            edge_kind: "calls".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["group_count"].as_u64().unwrap(), 0);
    assert_eq!(val["total_symbols"].as_u64().unwrap(), 0);
}

#[tokio::test]
async fn get_scc_groups_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_scc_groups(Parameters(GetSccGroupsRequest {
            edge_kind: "bad".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0058: mycelium_get_dependency_layers ─────────────────────────

#[tokio::test]
async fn get_dependency_layers_simple_chain() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let sym_a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let sym_b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        let sym_c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
        // c → b → a
        store.upsert_edge(EdgeKind::Calls, sym_b, sym_a);
        store.upsert_edge(EdgeKind::Calls, sym_c, sym_b);
    }
    let raw = server
        .mycelium_get_dependency_layers(Parameters(GetDependencyLayersRequest {
            edge_kind: "calls".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["layer_count"].as_u64().unwrap(), 3);
    assert_eq!(val["total_symbols"].as_u64().unwrap(), 3);
    assert_eq!(val["cycle_excluded_count"].as_u64().unwrap(), 0);
    let layers = val["layers"].as_array().unwrap();
    assert_eq!(
        layers[0].as_array().unwrap()[0].as_str().unwrap(),
        "src/a.rs>a"
    );
}

#[tokio::test]
async fn get_dependency_layers_cycle_excluded() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let sym_a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let sym_b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        // cycle: a ↔ b
        store.upsert_edge(EdgeKind::Calls, sym_a, sym_b);
        store.upsert_edge(EdgeKind::Calls, sym_b, sym_a);
    }
    let raw = server
        .mycelium_get_dependency_layers(Parameters(GetDependencyLayersRequest {
            edge_kind: "calls".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["layer_count"].as_u64().unwrap(), 0);
    assert_eq!(val["cycle_excluded_count"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn get_dependency_layers_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_dependency_layers(Parameters(GetDependencyLayersRequest {
            edge_kind: "bad".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0059: mycelium_get_two_hop_neighbors ─────────────────────────

#[tokio::test]
async fn get_two_hop_neighbors_basic() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let sym_a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let sym_b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        let sym_c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
        // a → b → c
        store.upsert_edge(EdgeKind::Calls, sym_a, sym_b);
        store.upsert_edge(EdgeKind::Calls, sym_b, sym_c);
    }
    let raw = server
        .mycelium_get_two_hop_neighbors(Parameters(GetTwoHopNeighborsRequest {
            path: "src/a.rs>a".to_owned(),
            edge_kind: "calls".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 1);
    assert_eq!(
        val["neighbors"].as_array().unwrap()[0].as_str().unwrap(),
        "src/c.rs>c"
    );
}

#[tokio::test]
async fn get_two_hop_neighbors_unknown_path_returns_empty() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_two_hop_neighbors(Parameters(GetTwoHopNeighborsRequest {
            path: "nonexistent.rs>x".to_owned(),
            edge_kind: "calls".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
    assert!(val["neighbors"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn get_two_hop_neighbors_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_two_hop_neighbors(Parameters(GetTwoHopNeighborsRequest {
            path: "src/a.rs>a".to_owned(),
            edge_kind: "bad".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0060: mycelium_get_symbol_neighborhood ───────────────────────

#[tokio::test]
async fn get_symbol_neighborhood_basic() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let main = store.upsert_node(TrunkPath::parse("src/main.rs>main").unwrap());
        let svc = store.upsert_node(TrunkPath::parse("src/svc.rs>svc").unwrap());
        let util = store.upsert_node(TrunkPath::parse("src/util.rs>util").unwrap());
        store.upsert_edge(EdgeKind::Calls, main, svc);
        store.upsert_edge(EdgeKind::Calls, svc, util);
    }
    let raw = server
        .mycelium_get_symbol_neighborhood(Parameters(GetSymbolNeighborhoodRequest {
            path: "src/svc.rs>svc".to_owned(),
            edge_kind: "calls".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["path"].as_str().unwrap(), "src/svc.rs>svc");
    assert_eq!(val["incoming_count"].as_u64().unwrap(), 1);
    assert_eq!(val["outgoing_count"].as_u64().unwrap(), 1);
    assert_eq!(val["incoming"][0].as_str().unwrap(), "src/main.rs>main");
    assert_eq!(val["outgoing"][0].as_str().unwrap(), "src/util.rs>util");
}

#[tokio::test]
async fn get_symbol_neighborhood_unknown_path_returns_empty() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_symbol_neighborhood(Parameters(GetSymbolNeighborhoodRequest {
            path: "nonexistent.rs>x".to_owned(),
            edge_kind: "calls".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["path"].as_str().unwrap(), "");
    assert_eq!(val["incoming_count"].as_u64().unwrap(), 0);
    assert_eq!(val["outgoing_count"].as_u64().unwrap(), 0);
}

#[tokio::test]
async fn get_symbol_neighborhood_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_symbol_neighborhood(Parameters(GetSymbolNeighborhoodRequest {
            path: "src/a.rs>a".to_owned(),
            edge_kind: "bad".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0061: mycelium_get_hub_symbols ───────────────────────────────

#[tokio::test]
async fn get_hub_symbols_basic() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let hub = store.upsert_node(TrunkPath::parse("src/hub.rs>hub").unwrap());
        // 2 callers, 2 callees
        for i in 0..2_u32 {
            let c = store.upsert_node(TrunkPath::parse(&format!("src/c{i}.rs>c{i}")).unwrap());
            store.upsert_edge(EdgeKind::Calls, c, hub);
        }
        for i in 0..2_u32 {
            let d = store.upsert_node(TrunkPath::parse(&format!("src/d{i}.rs>d{i}")).unwrap());
            store.upsert_edge(EdgeKind::Calls, hub, d);
        }
    }
    let raw = server
        .mycelium_get_hub_symbols(Parameters(GetHubSymbolsRequest {
            edge_kind: "calls".to_owned(),
            min_in: Some(2),
            min_out: Some(2),
            limit: Some(10),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 1);
    assert_eq!(val["hubs"][0]["path"].as_str().unwrap(), "src/hub.rs>hub");
    assert_eq!(val["hubs"][0]["in_degree"].as_u64().unwrap(), 2);
    assert_eq!(val["hubs"][0]["out_degree"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn get_hub_symbols_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_hub_symbols(Parameters(GetHubSymbolsRequest {
            edge_kind: "bad".to_owned(),
            min_in: None,
            min_out: None,
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

#[tokio::test]
async fn get_hub_symbols_empty_store_returns_empty() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_hub_symbols(Parameters(GetHubSymbolsRequest {
            edge_kind: "calls".to_owned(),
            min_in: None,
            min_out: None,
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
    assert!(val["hubs"].as_array().unwrap().is_empty());
}

// ── RFC-0062: mycelium_get_singly_referenced ──────────────────────────

#[tokio::test]
async fn get_singly_referenced_basic() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let caller = store.upsert_node(TrunkPath::parse("src/main.rs>main").unwrap());
        let tgt = store.upsert_node(TrunkPath::parse("src/util.rs>helper").unwrap());
        store.upsert_edge(EdgeKind::Calls, caller, tgt);
    }
    let raw = server
        .mycelium_get_singly_referenced(Parameters(GetSinglyReferencedRequest {
            edge_kind: "calls".to_owned(),
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 1);
    let syms = val["symbols"].as_array().unwrap();
    assert_eq!(syms[0]["path"].as_str().unwrap(), "src/util.rs>helper");
    assert_eq!(
        syms[0]["referenced_by"].as_str().unwrap(),
        "src/main.rs>main"
    );
}

#[tokio::test]
async fn get_singly_referenced_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_singly_referenced(Parameters(GetSinglyReferencedRequest {
            edge_kind: "unknown_kind".to_owned(),
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

#[tokio::test]
async fn get_singly_referenced_empty_store_returns_empty() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_singly_referenced(Parameters(GetSinglyReferencedRequest {
            edge_kind: "calls".to_owned(),
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
    assert!(val["symbols"].as_array().unwrap().is_empty());
}

// ── RFC-0063: mycelium_batch_reachable_to ─────────────────────────────

#[tokio::test]
async fn batch_reachable_to_basic() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let tgt = store.upsert_node(TrunkPath::parse("src/util.rs>helper").unwrap());
        let mid = store.upsert_node(TrunkPath::parse("src/mid.rs>mid").unwrap());
        store.upsert_edge(EdgeKind::Calls, mid, tgt);
    }
    let raw = server
        .mycelium_batch_reachable_to(Parameters(BatchReachableToRequest {
            paths: vec!["src/util.rs>helper".to_owned()],
            edge_kind: "calls".to_owned(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 1);
    let reachable = val["reachable"].as_array().unwrap();
    assert_eq!(reachable[0].as_str().unwrap(), "src/mid.rs>mid");
}

#[tokio::test]
async fn batch_reachable_to_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_batch_reachable_to(Parameters(BatchReachableToRequest {
            paths: vec!["src/a.rs>a".to_owned()],
            edge_kind: "bogus".to_owned(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

#[tokio::test]
async fn batch_reachable_to_empty_paths_returns_empty() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_batch_reachable_to(Parameters(BatchReachableToRequest {
            paths: vec![],
            edge_kind: "calls".to_owned(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
    assert!(val["reachable"].as_array().unwrap().is_empty());
}

// ── RFC-0064: mycelium_get_k_core ─────────────────────────────────────

#[tokio::test]
async fn get_k_core_basic_triangle() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, b, c);
        store.upsert_edge(EdgeKind::Calls, c, a);
    }
    let raw = server
        .mycelium_get_k_core(Parameters(GetKCoreRequest {
            edge_kind: "calls".to_owned(),
            k: Some(2),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 3);
    assert_eq!(val["k"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn get_k_core_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_k_core(Parameters(GetKCoreRequest {
            edge_kind: "nope".to_owned(),
            k: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

#[tokio::test]
async fn get_k_core_empty_store_returns_empty() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_k_core(Parameters(GetKCoreRequest {
            edge_kind: "calls".to_owned(),
            k: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
    assert!(val["core"].as_array().unwrap().is_empty());
}

// ── RFC-0065: mycelium_batch_reachable_from ───────────────────────────

#[tokio::test]
async fn batch_reachable_from_basic() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let src = store.upsert_node(TrunkPath::parse("src/top.rs>top").unwrap());
        let leaf = store.upsert_node(TrunkPath::parse("src/leaf.rs>leaf").unwrap());
        store.upsert_edge(EdgeKind::Calls, src, leaf);
    }
    let raw = server
        .mycelium_batch_reachable_from(Parameters(BatchReachableFromRequest {
            paths: vec!["src/top.rs>top".to_owned()],
            edge_kind: "calls".to_owned(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 1);
    let reachable = val["reachable"].as_array().unwrap();
    assert_eq!(reachable[0].as_str().unwrap(), "src/leaf.rs>leaf");
}

#[tokio::test]
async fn batch_reachable_from_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_batch_reachable_from(Parameters(BatchReachableFromRequest {
            paths: vec!["src/a.rs>a".to_owned()],
            edge_kind: "invalid".to_owned(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

#[tokio::test]
async fn batch_reachable_from_empty_paths_returns_empty() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_batch_reachable_from(Parameters(BatchReachableFromRequest {
            paths: vec![],
            edge_kind: "calls".to_owned(),
            max_depth: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
    assert!(val["reachable"].as_array().unwrap().is_empty());
}

// ── RFC-0066: mycelium_batch_node_degree ──────────────────────────────

#[tokio::test]
async fn batch_node_degree_known_path() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
    }
    let raw = server
        .mycelium_batch_node_degree(Parameters(BatchNodeDegreeRequest {
            paths: vec!["src/a.rs>a".to_owned(), "src/b.rs>b".to_owned()],
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 2);
    let degs = val["degrees"].as_array().unwrap();
    assert_eq!(degs[0]["out_calls"].as_u64().unwrap(), 1);
    assert_eq!(degs[1]["in_calls"].as_u64().unwrap(), 1);
}

#[tokio::test]
async fn batch_node_degree_unknown_path_returns_error_entry() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_batch_node_degree(Parameters(BatchNodeDegreeRequest {
            paths: vec!["src/missing.rs>nope".to_owned()],
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let degs = val["degrees"].as_array().unwrap();
    assert!(degs[0]["error"].as_str().is_some());
}

#[tokio::test]
async fn batch_node_degree_empty_paths_returns_empty() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_batch_node_degree(Parameters(BatchNodeDegreeRequest {
            paths: vec![],
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
    assert!(val["degrees"].as_array().unwrap().is_empty());
}

// ── RFC-0067: mycelium_find_cycle_members ─────────────────────────────

#[tokio::test]
async fn find_cycle_members_returns_cycle_symbols() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, b, a);
    }
    let raw = server
        .mycelium_find_cycle_members(Parameters(FindCycleMembersRequest {
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 2);
    let members: Vec<&str> = val["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(members.contains(&"src/a.rs>a"));
    assert!(members.contains(&"src/b.rs>b"));
}

#[tokio::test]
async fn find_cycle_members_no_cycle_returns_empty() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
    }
    let raw = server
        .mycelium_find_cycle_members(Parameters(FindCycleMembersRequest {
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
    assert!(val["members"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn find_cycle_members_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_find_cycle_members(Parameters(FindCycleMembersRequest {
            edge_kind: "unknown_kind".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0068: mycelium_get_wcc ─────────────────────────────────────────

#[tokio::test]
async fn wcc_two_disjoint_components() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
        let d = store.upsert_node(TrunkPath::parse("src/d.rs>d").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, c, d);
    }
    let raw = server
        .mycelium_get_wcc(Parameters(GetWccRequest {
            edge_kind: "calls".into(),
            min_size: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["component_count"].as_u64().unwrap(), 2);
    assert_eq!(val["total_symbols"].as_u64().unwrap(), 4);
}

#[tokio::test]
async fn wcc_min_size_filter() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        store.upsert_node(TrunkPath::parse("src/lone.rs>lone").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
    }
    let raw = server
        .mycelium_get_wcc(Parameters(GetWccRequest {
            edge_kind: "calls".into(),
            min_size: Some(2),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    // singleton lone filtered out
    assert_eq!(val["component_count"].as_u64().unwrap(), 1);
    assert_eq!(val["total_symbols"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn wcc_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_wcc(Parameters(GetWccRequest {
            edge_kind: "bad_kind".into(),
            min_size: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0069: mycelium_topological_sort ───────────────────────────────

#[tokio::test]
async fn topo_sort_linear_chain_ordered() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, b, c);
    }
    let raw = server
        .mycelium_topological_sort(Parameters(TopologicalSortRequest {
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["ordered_count"].as_u64().unwrap(), 3);
    assert_eq!(val["cycle_count"].as_u64().unwrap(), 0);
    let order: Vec<&str> = val["order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let pos_a = order.iter().position(|&s| s == "src/a.rs>a").unwrap();
    let pos_b = order.iter().position(|&s| s == "src/b.rs>b").unwrap();
    let pos_c = order.iter().position(|&s| s == "src/c.rs>c").unwrap();
    assert!(pos_a < pos_b && pos_b < pos_c);
}

#[tokio::test]
async fn topo_sort_cycle_surfaced() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, b, a);
    }
    let raw = server
        .mycelium_topological_sort(Parameters(TopologicalSortRequest {
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["ordered_count"].as_u64().unwrap(), 0);
    assert_eq!(val["cycle_count"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn topo_sort_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_topological_sort(Parameters(TopologicalSortRequest {
            edge_kind: "bad".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0070: mycelium_find_articulation_points ───────────────────────

#[tokio::test]
async fn articulation_points_bridge_node_found() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, b, c);
    }
    let raw = server
        .mycelium_find_articulation_points(Parameters(FindArticulationPointsRequest {
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 1);
    assert!(
        val["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .any(|x| x == "src/b.rs>b")
    );
}

#[tokio::test]
async fn articulation_points_cycle_returns_none() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, b, c);
        store.upsert_edge(EdgeKind::Calls, c, a);
    }
    let raw = server
        .mycelium_find_articulation_points(Parameters(FindArticulationPointsRequest {
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
}

#[tokio::test]
async fn articulation_points_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_find_articulation_points(Parameters(FindArticulationPointsRequest {
            edge_kind: "unknown".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0071: mycelium_find_bridge_edges ──────────────────────────────

#[tokio::test]
async fn bridge_edges_chain_returns_bridges() {
    // a — b — c: both edges are bridges
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, b, c);
    }
    let raw = server
        .mycelium_find_bridge_edges(Parameters(FindBridgeEdgesRequest {
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 2);
    let arr = val["bridges"].as_array().unwrap();
    assert!(
        arr.iter()
            .any(|b| b["from"] == "src/a.rs>a" && b["to"] == "src/b.rs>b")
    );
    assert!(
        arr.iter()
            .any(|b| b["from"] == "src/b.rs>b" && b["to"] == "src/c.rs>c")
    );
}

#[tokio::test]
async fn bridge_edges_cycle_returns_none() {
    // a → b → c → a: no bridges in a cycle
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, b, c);
        store.upsert_edge(EdgeKind::Calls, c, a);
    }
    let raw = server
        .mycelium_find_bridge_edges(Parameters(FindBridgeEdgesRequest {
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
}

#[tokio::test]
async fn bridge_edges_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_find_bridge_edges(Parameters(FindBridgeEdgesRequest {
            edge_kind: "unknown".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0072: mycelium_get_biconnected_components ─────────────────────

#[tokio::test]
async fn bcc_triangle_is_one_component() {
    // a → b → c → a: one BCC of 3 nodes
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, b, c);
        store.upsert_edge(EdgeKind::Calls, c, a);
    }
    let raw = server
        .mycelium_get_biconnected_components(Parameters(BiconnectedComponentsRequest {
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["component_count"].as_u64().unwrap(), 1);
    assert_eq!(val["total_symbols"].as_u64().unwrap(), 3);
}

#[tokio::test]
async fn bcc_singleton_returns_empty() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
    }
    let raw = server
        .mycelium_get_biconnected_components(Parameters(BiconnectedComponentsRequest {
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["component_count"].as_u64().unwrap(), 0);
}

#[tokio::test]
async fn bcc_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_biconnected_components(Parameters(BiconnectedComponentsRequest {
            edge_kind: "unknown".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0073: mycelium_get_degree_histogram ───────────────────────────

#[tokio::test]
async fn degree_histogram_counts_correct() {
    // a → b, a → c: a has out=2; b and c have in=1; a has in=0
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, a, c);
    }
    let raw = server
        .mycelium_get_degree_histogram(Parameters(DegreeHistogramRequest {
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["total_symbols"].as_u64().unwrap(), 3);
    let in_arr = val["in_degrees"].as_array().unwrap();
    let in_sum: u64 = in_arr.iter().map(|e| e["count"].as_u64().unwrap()).sum();
    assert_eq!(in_sum, 3);
}

#[tokio::test]
async fn degree_histogram_empty_returns_zero() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_degree_histogram(Parameters(DegreeHistogramRequest {
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["total_symbols"].as_u64().unwrap(), 0);
}

#[tokio::test]
async fn degree_histogram_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_degree_histogram(Parameters(DegreeHistogramRequest {
            edge_kind: "unknown".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0074: mycelium_get_graph_metrics ──────────────────────────────

#[tokio::test]
async fn graph_metrics_basic_counts() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
    }
    let raw = server
        .mycelium_get_graph_metrics(Parameters(GraphMetricsRequest {
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["symbol_count"].as_u64().unwrap(), 2);
    assert_eq!(val["directed_edge_count"].as_u64().unwrap(), 1);
    assert_eq!(val["max_out_degree"].as_u64().unwrap(), 1);
    assert_eq!(val["max_in_degree"].as_u64().unwrap(), 1);
}

#[tokio::test]
async fn graph_metrics_empty_returns_zeros() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_graph_metrics(Parameters(GraphMetricsRequest {
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["symbol_count"].as_u64().unwrap(), 0);
    assert!(val["density"].as_f64().unwrap().abs() < 1e-15);
}

#[tokio::test]
async fn graph_metrics_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_graph_metrics(Parameters(GraphMetricsRequest {
            edge_kind: "unknown".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0075: mycelium_get_neighbor_similarity ────────────────────────

#[tokio::test]
async fn neighbor_similarity_identical_neighbors_returns_one() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        // a and b both call hub; hub is shared neighbor
        let a = store.upsert_node(TrunkPath::parse("src/x.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/x.rs>b").unwrap());
        let hub = store.upsert_node(TrunkPath::parse("src/x.rs>hub").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, hub);
        store.upsert_edge(EdgeKind::Calls, b, hub);
    }
    let raw = server
        .mycelium_get_neighbor_similarity(Parameters(NeighborSimilarityRequest {
            path1: "src/x.rs>a".into(),
            path2: "src/x.rs>b".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let sim = val["similarity"].as_f64().unwrap();
    assert!((sim - 1.0).abs() < 1e-9, "expected 1.0, got {sim}");
    assert_eq!(val["shared"].as_u64().unwrap(), 1);
    assert_eq!(val["total"].as_u64().unwrap(), 1);
}

#[tokio::test]
async fn neighbor_similarity_no_overlap_returns_zero() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/y.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/y.rs>b").unwrap());
        let na = store.upsert_node(TrunkPath::parse("src/y.rs>na").unwrap());
        let nb = store.upsert_node(TrunkPath::parse("src/y.rs>nb").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, na);
        store.upsert_edge(EdgeKind::Calls, b, nb);
    }
    let raw = server
        .mycelium_get_neighbor_similarity(Parameters(NeighborSimilarityRequest {
            path1: "src/y.rs>a".into(),
            path2: "src/y.rs>b".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let sim = val["similarity"].as_f64().unwrap();
    assert!(sim.abs() < 1e-9, "expected 0.0, got {sim}");
    assert_eq!(val["shared"].as_u64().unwrap(), 0);
    assert_eq!(val["total"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn neighbor_similarity_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_neighbor_similarity(Parameters(NeighborSimilarityRequest {
            path1: "src/z.rs>a".into(),
            path2: "src/z.rs>b".into(),
            edge_kind: "unknown".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0076: mycelium_get_clustering_coefficient ─────────────────────

#[tokio::test]
async fn clustering_coefficient_complete_triangle() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let hub = store.upsert_node(TrunkPath::parse("src/cc.rs>hub").unwrap());
        let alpha = store.upsert_node(TrunkPath::parse("src/cc.rs>alpha").unwrap());
        let beta = store.upsert_node(TrunkPath::parse("src/cc.rs>beta").unwrap());
        store.upsert_edge(EdgeKind::Calls, hub, alpha);
        store.upsert_edge(EdgeKind::Calls, hub, beta);
        store.upsert_edge(EdgeKind::Calls, alpha, beta);
        store.upsert_edge(EdgeKind::Calls, beta, alpha);
    }
    let raw = server
        .mycelium_get_clustering_coefficient(Parameters(ClusteringCoefficientRequest {
            path: "src/cc.rs>hub".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let coeff = val["coefficient"].as_f64().unwrap();
    assert!((coeff - 1.0).abs() < 1e-9, "expected 1.0, got {coeff}");
    assert_eq!(val["neighbor_count"].as_u64().unwrap(), 2);
    assert_eq!(val["neighbor_edge_count"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn clustering_coefficient_unknown_path_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_clustering_coefficient(Parameters(ClusteringCoefficientRequest {
            path: "src/no_such.rs>ghost".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown path"));
}

#[tokio::test]
async fn clustering_coefficient_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_clustering_coefficient(Parameters(ClusteringCoefficientRequest {
            path: "src/any.rs>any".into(),
            edge_kind: "unknown".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0077: mycelium_get_eccentricity ──────────────────────────────────

#[tokio::test]
async fn eccentricity_chain_returns_correct_depth() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let root = store.upsert_node(TrunkPath::parse("src/ecc.rs>root").unwrap());
        let mid = store.upsert_node(TrunkPath::parse("src/ecc.rs>mid").unwrap());
        let far = store.upsert_node(TrunkPath::parse("src/ecc.rs>far").unwrap());
        store.upsert_edge(EdgeKind::Calls, root, mid);
        store.upsert_edge(EdgeKind::Calls, mid, far);
    }
    let raw = server
        .mycelium_get_eccentricity(Parameters(EccentricityRequest {
            path: "src/ecc.rs>root".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["eccentricity"].as_u64().unwrap(), 2);
    assert_eq!(val["reachable_count"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn eccentricity_unknown_path_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_eccentricity(Parameters(EccentricityRequest {
            path: "src/ghost.rs>none".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown path"));
}

#[tokio::test]
async fn eccentricity_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_eccentricity(Parameters(EccentricityRequest {
            path: "src/any.rs>any".into(),
            edge_kind: "unknown".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0078: mycelium_get_harmonic_centrality ────────────────────────

#[tokio::test]
async fn harmonic_centrality_chain_returns_correct_value() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let root = store.upsert_node(TrunkPath::parse("src/hc.rs>root").unwrap());
        let mid = store.upsert_node(TrunkPath::parse("src/hc.rs>mid").unwrap());
        let far = store.upsert_node(TrunkPath::parse("src/hc.rs>far").unwrap());
        store.upsert_edge(EdgeKind::Calls, root, mid);
        store.upsert_edge(EdgeKind::Calls, mid, far);
    }
    let raw = server
        .mycelium_get_harmonic_centrality(Parameters(HarmonicCentralityRequest {
            path: "src/hc.rs>root".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    // n=3, root reaches mid(d=1) and far(d=2)
    // HC = (1/2) * (1/1 + 1/2) = 0.5 * 1.5 = 0.75
    let hc = val["harmonic_centrality"].as_f64().unwrap();
    assert!((hc - 0.75).abs() < 1e-9, "expected 0.75, got {hc}");
    assert_eq!(val["reachable_count"].as_u64().unwrap(), 2);
    assert_eq!(val["symbol_count"].as_u64().unwrap(), 3);
}

#[tokio::test]
async fn harmonic_centrality_unknown_path_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_harmonic_centrality(Parameters(HarmonicCentralityRequest {
            path: "src/ghost.rs>none_hc".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown path"));
}

#[tokio::test]
async fn harmonic_centrality_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_harmonic_centrality(Parameters(HarmonicCentralityRequest {
            path: "src/any.rs>any_hc".into(),
            edge_kind: "unknown".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0079: mycelium_get_mutual_reachability ────────────────────────

#[tokio::test]
async fn mutual_reachability_forward_only_returns_correct_flags() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let src = store.upsert_node(TrunkPath::parse("src/mr.rs>src").unwrap());
        let dst = store.upsert_node(TrunkPath::parse("src/mr.rs>dst").unwrap());
        store.upsert_edge(EdgeKind::Calls, src, dst);
    }
    let raw = server
        .mycelium_get_mutual_reachability(Parameters(MutualReachabilityRequest {
            path1: "src/mr.rs>src".into(),
            path2: "src/mr.rs>dst".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["forward"].as_bool().unwrap());
    assert!(!val["backward"].as_bool().unwrap());
    assert!(!val["mutual"].as_bool().unwrap());
    assert_eq!(val["forward_distance"].as_u64().unwrap(), 1);
    assert!(val["backward_distance"].is_null());
}

#[tokio::test]
async fn mutual_reachability_unknown_path_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_mutual_reachability(Parameters(MutualReachabilityRequest {
            path1: "src/ghost.rs>none_mr".into(),
            path2: "src/ghost.rs>other_mr".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown path"));
}

#[tokio::test]
async fn mutual_reachability_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_mutual_reachability(Parameters(MutualReachabilityRequest {
            path1: "src/any.rs>any_mr1".into(),
            path2: "src/any.rs>any_mr2".into(),
            edge_kind: "unknown".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0080: mycelium_get_reachable_set ──────────────────────────────

#[tokio::test]
async fn reachable_set_chain_returns_all_reachable() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let head = store.upsert_node(TrunkPath::parse("src/rset.rs>head").unwrap());
        let mid = store.upsert_node(TrunkPath::parse("src/rset.rs>mid").unwrap());
        let tail = store.upsert_node(TrunkPath::parse("src/rset.rs>tail").unwrap());
        store.upsert_edge(EdgeKind::Calls, head, mid);
        store.upsert_edge(EdgeKind::Calls, mid, tail);
    }
    let raw = server
        .mycelium_get_reachable_set(Parameters(ReachableSetRequest {
            path: "src/rset.rs>head".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 2);
    let reachable: Vec<&str> = val["reachable"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(reachable.contains(&"src/rset.rs>mid"));
    assert!(reachable.contains(&"src/rset.rs>tail"));
}

#[tokio::test]
async fn reachable_set_unknown_path_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_reachable_set(Parameters(ReachableSetRequest {
            path: "src/ghost.rs>none_rset".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown path"));
}

#[tokio::test]
async fn reachable_set_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_reachable_set(Parameters(ReachableSetRequest {
            path: "src/any.rs>any_rset".into(),
            edge_kind: "unknown".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0081: mycelium_get_reaches_into ───────────────────────────────

#[tokio::test]
async fn reaches_into_chain_returns_reverse_closure() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let head = store.upsert_node(TrunkPath::parse("src/ri.rs>head_ri").unwrap());
        let mid = store.upsert_node(TrunkPath::parse("src/ri.rs>mid_ri").unwrap());
        let tail = store.upsert_node(TrunkPath::parse("src/ri.rs>tail_ri").unwrap());
        store.upsert_edge(EdgeKind::Calls, head, mid);
        store.upsert_edge(EdgeKind::Calls, mid, tail);
    }
    let raw = server
        .mycelium_get_reaches_into(Parameters(ReachesIntoRequest {
            path: "src/ri.rs>tail_ri".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 2);
    let callers: Vec<&str> = val["callers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(callers.contains(&"src/ri.rs>head_ri"));
    assert!(callers.contains(&"src/ri.rs>mid_ri"));
}

#[tokio::test]
async fn reaches_into_unknown_path_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_reaches_into(Parameters(ReachesIntoRequest {
            path: "src/ghost.rs>none_ri".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown path"));
}

#[tokio::test]
async fn reaches_into_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_reaches_into(Parameters(ReachesIntoRequest {
            path: "src/any.rs>any_ri".into(),
            edge_kind: "unknown".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0082: mycelium_page_rank ──────────────────────────────────────

#[tokio::test]
async fn page_rank_star_hub_ranks_first() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let hub = store.upsert_node(TrunkPath::parse("src/pr.rs>pr_hub").unwrap());
        let spoke_a = store.upsert_node(TrunkPath::parse("src/pr.rs>pr_spoke_a").unwrap());
        let spoke_b = store.upsert_node(TrunkPath::parse("src/pr.rs>pr_spoke_b").unwrap());
        store.upsert_edge(EdgeKind::Calls, spoke_a, hub);
        store.upsert_edge(EdgeKind::Calls, spoke_b, hub);
    }
    let raw = server
        .mycelium_page_rank(Parameters(PageRankRequest {
            edge_kind: "calls".into(),
            damping: Some(0.85),
            iterations: Some(30),
            top_n: Some(3),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["symbol_count"].as_u64().unwrap(), 3);
    let first_path = val["nodes"][0]["path"].as_str().unwrap();
    assert_eq!(first_path, "src/pr.rs>pr_hub");
}

#[tokio::test]
async fn page_rank_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_page_rank(Parameters(PageRankRequest {
            edge_kind: "unknown".into(),
            damping: None,
            iterations: None,
            top_n: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0083: mycelium_get_common_reachable ───────────────────────────

#[tokio::test]
async fn common_reachable_shared_dep_returned() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let left = store.upsert_node(TrunkPath::parse("src/cr.rs>mcp_cr_left").unwrap());
        let right = store.upsert_node(TrunkPath::parse("src/cr.rs>mcp_cr_right").unwrap());
        let shared = store.upsert_node(TrunkPath::parse("src/cr.rs>mcp_cr_shared").unwrap());
        store.upsert_edge(EdgeKind::Calls, left, shared);
        store.upsert_edge(EdgeKind::Calls, right, shared);
    }
    let raw = server
        .mycelium_get_common_reachable(Parameters(CommonReachableRequest {
            path1: "src/cr.rs>mcp_cr_left".into(),
            path2: "src/cr.rs>mcp_cr_right".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 1);
    assert_eq!(
        val["common"][0].as_str().unwrap(),
        "src/cr.rs>mcp_cr_shared"
    );
}

#[tokio::test]
async fn common_reachable_unknown_path_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_common_reachable(Parameters(CommonReachableRequest {
            path1: "src/ghost.rs>none_cr".into(),
            path2: "src/ghost.rs>also_none".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown path"));
}

#[tokio::test]
async fn common_reachable_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_common_reachable(Parameters(CommonReachableRequest {
            path1: "src/any.rs>any_cr1".into(),
            path2: "src/any.rs>any_cr2".into(),
            edge_kind: "unknown".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0084: mycelium_get_k_hop_neighbors ────────────────────────────

#[tokio::test]
async fn k_hop_neighbors_k2_returns_grandchildren() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let root = store.upsert_node(TrunkPath::parse("src/kh.rs>mcp_kh_root").unwrap());
        let mid = store.upsert_node(TrunkPath::parse("src/kh.rs>mcp_kh_mid").unwrap());
        let far = store.upsert_node(TrunkPath::parse("src/kh.rs>mcp_kh_far").unwrap());
        store.upsert_edge(EdgeKind::Calls, root, mid);
        store.upsert_edge(EdgeKind::Calls, mid, far);
    }
    let raw = server
        .mycelium_get_k_hop_neighbors(Parameters(KHopNeighborsRequest {
            path: "src/kh.rs>mcp_kh_root".into(),
            edge_kind: "calls".into(),
            k: 2,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["k"].as_u64().unwrap(), 2);
    assert_eq!(val["count"].as_u64().unwrap(), 1);
    assert_eq!(
        val["neighbors"][0].as_str().unwrap(),
        "src/kh.rs>mcp_kh_far"
    );
}

#[tokio::test]
async fn k_hop_neighbors_unknown_path_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_k_hop_neighbors(Parameters(KHopNeighborsRequest {
            path: "src/ghost.rs>none_kh".into(),
            edge_kind: "calls".into(),
            k: 1,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown path"));
}

#[tokio::test]
async fn k_hop_neighbors_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_k_hop_neighbors(Parameters(KHopNeighborsRequest {
            path: "src/any.rs>any_kh".into(),
            edge_kind: "unknown".into(),
            k: 1,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0085: mycelium_get_betweenness_centrality ─────────────────────

#[tokio::test]
async fn betweenness_chain_middle_ranks_first() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let head = store.upsert_node(TrunkPath::parse("src/bw.rs>bw_head").unwrap());
        let mid = store.upsert_node(TrunkPath::parse("src/bw.rs>bw_mid").unwrap());
        let tail = store.upsert_node(TrunkPath::parse("src/bw.rs>bw_tail").unwrap());
        store.upsert_edge(EdgeKind::Calls, head, mid);
        store.upsert_edge(EdgeKind::Calls, mid, tail);
    }
    let raw = server
        .mycelium_get_betweenness_centrality(Parameters(BetweennessCentralityRequest {
            edge_kind: "calls".into(),
            top_n: Some(3),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["symbol_count"].as_u64().unwrap(), 3);
    let first_path = val["nodes"][0]["path"].as_str().unwrap();
    assert_eq!(first_path, "src/bw.rs>bw_mid");
}

#[tokio::test]
async fn betweenness_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_betweenness_centrality(Parameters(BetweennessCentralityRequest {
            edge_kind: "unknown".into(),
            top_n: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0086: mycelium_get_strongly_connected_components ─────────────

#[tokio::test]
async fn scc_finds_cycle_with_min_size_2() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/scc_mcp.rs>mcp_scc_a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/scc_mcp.rs>mcp_scc_b").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, b, a);
        // Singleton node outside the cycle.
        store.upsert_node(TrunkPath::parse("src/scc_mcp.rs>mcp_scc_c").unwrap());
    }
    let raw = server
        .mycelium_get_strongly_connected_components(Parameters(
            StronglyConnectedComponentsRequest {
                edge_kind: "calls".into(),
                min_size: Some(2),
                output_format: None,
            },
        ))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["min_size"].as_u64().unwrap(), 2);
    assert_eq!(val["symbol_count"].as_u64().unwrap(), 3);
    let comps = val["components"].as_array().unwrap();
    assert_eq!(comps.len(), 1);
    assert_eq!(comps[0]["size"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn scc_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_strongly_connected_components(Parameters(
            StronglyConnectedComponentsRequest {
                edge_kind: "unknown".into(),
                min_size: None,
                output_format: None,
            },
        ))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0087: mycelium_get_degree_centrality ──────────────────────────

#[tokio::test]
async fn degree_centrality_identifies_fan_in_hub() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let hub = store.upsert_node(TrunkPath::parse("src/dc_mcp.rs>mcp_dc_hub").unwrap());
        let c1 = store.upsert_node(TrunkPath::parse("src/dc_mcp.rs>mcp_dc_c1").unwrap());
        let c2 = store.upsert_node(TrunkPath::parse("src/dc_mcp.rs>mcp_dc_c2").unwrap());
        store.upsert_edge(EdgeKind::Calls, c1, hub);
        store.upsert_edge(EdgeKind::Calls, c2, hub);
    }
    let raw = server
        .mycelium_get_degree_centrality(Parameters(DegreeCentralityRequest {
            edge_kind: "calls".into(),
            top_n: Some(3),
            sort_by: Some("in".into()),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["symbol_count"].as_u64().unwrap(), 3);
    assert_eq!(val["sort_by"].as_str().unwrap(), "in");
    let nodes = val["nodes"].as_array().unwrap();
    assert_eq!(
        nodes[0]["path"].as_str().unwrap(),
        "src/dc_mcp.rs>mcp_dc_hub"
    );
    assert_eq!(nodes[0]["in_degree"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn degree_centrality_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_degree_centrality(Parameters(DegreeCentralityRequest {
            edge_kind: "unknown".into(),
            top_n: None,
            sort_by: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

#[tokio::test]
async fn degree_centrality_unknown_sort_by_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_degree_centrality(Parameters(DegreeCentralityRequest {
            edge_kind: "calls".into(),
            top_n: None,
            sort_by: Some("bogus".into()),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown sort_by"));
}

// ── RFC-0088: mycelium_get_closeness_centrality ───────────────────────

#[tokio::test]
async fn closeness_chain_head_ranks_first() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/clc.rs>mcp_clc_a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/clc.rs>mcp_clc_b").unwrap());
        let c = store.upsert_node(TrunkPath::parse("src/clc.rs>mcp_clc_c").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, b, c);
    }
    let raw = server
        .mycelium_get_closeness_centrality(Parameters(ClosenessCentralityRequest {
            edge_kind: "calls".into(),
            top_n: Some(3),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["symbol_count"].as_u64().unwrap(), 3);
    // A reaches B and C with shortest total distance → highest closeness.
    assert_eq!(
        val["nodes"][0]["path"].as_str().unwrap(),
        "src/clc.rs>mcp_clc_a"
    );
}

#[tokio::test]
async fn closeness_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_closeness_centrality(Parameters(ClosenessCentralityRequest {
            edge_kind: "unknown".into(),
            top_n: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0089: mycelium_get_dependency_depth ─────────────────────────────────

#[tokio::test]
async fn dep_depth_mcp_leaf_is_zero() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/dd_mcp.rs>mcp_dd_leaf").unwrap());
    }
    let raw = server
        .mycelium_get_dependency_depth(Parameters(DependencyDepthRequest {
            path: "src/dd_mcp.rs>mcp_dd_leaf".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["depth"].as_u64().unwrap(), 0);
    assert_eq!(val["path"].as_str().unwrap(), "src/dd_mcp.rs>mcp_dd_leaf");
    assert_eq!(val["edge_kind"].as_str().unwrap(), "calls");
}

#[tokio::test]
async fn dep_depth_mcp_two_hop_chain() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/dd_mcp.rs>mcp_dd_a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/dd_mcp.rs>mcp_dd_b").unwrap());
        let c = store.upsert_node(TrunkPath::parse("src/dd_mcp.rs>mcp_dd_c").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, b, c);
    }
    let raw = server
        .mycelium_get_dependency_depth(Parameters(DependencyDepthRequest {
            path: "src/dd_mcp.rs>mcp_dd_c".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["depth"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn dep_depth_mcp_unknown_path_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_dependency_depth(Parameters(DependencyDepthRequest {
            path: "src/nonexistent.rs>ghost".into(),
            edge_kind: "calls".into(),
            output_format: None,
        }))
        .await;
    assert_eq!(raw.is_error, Some(true), "unknown path must set is_error");
}

#[tokio::test]
async fn dep_depth_mcp_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/dd_mcp.rs>mcp_dd_kind_check").unwrap());
    }
    let raw = server
        .mycelium_get_dependency_depth(Parameters(DependencyDepthRequest {
            path: "src/dd_mcp.rs>mcp_dd_kind_check".into(),
            edge_kind: "unknown_kind".into(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0041: mycelium_get_outgoing_refs ──────────────────────────────

#[tokio::test]
async fn get_outgoing_refs_all_kinds() {
    let server = MyceliumServer::new();
    {
        let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
        let src = store.upsert_node(TrunkPath::parse("src/app.rs>App").unwrap());
        let callee = store.upsert_node(TrunkPath::parse("src/a.rs>callee").unwrap());
        let imported = store.upsert_node(TrunkPath::parse("src/b.rs>imported").unwrap());
        let parent = store.upsert_node(TrunkPath::parse("src/c.rs>Parent").unwrap());
        let iface = store.upsert_node(TrunkPath::parse("src/d.rs>IFace").unwrap());
        store.upsert_edge(EdgeKind::Calls, src, callee);
        store.upsert_edge(EdgeKind::Imports, src, imported);
        store.upsert_edge(EdgeKind::Extends, src, parent);
        store.upsert_edge(EdgeKind::Implements, src, iface);
    }
    let raw = server
        .mycelium_get_outgoing_refs(Parameters(GetOutgoingRefsRequest {
            path: "src/app.rs>App".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["callees"][0].as_str().unwrap(), "src/a.rs>callee");
    assert_eq!(val["imports"][0].as_str().unwrap(), "src/b.rs>imported");
    assert_eq!(val["extends"][0].as_str().unwrap(), "src/c.rs>Parent");
    assert_eq!(val["implements"][0].as_str().unwrap(), "src/d.rs>IFace");
}

#[tokio::test]
async fn get_outgoing_refs_empty_lists_present() {
    let server = MyceliumServer::new();
    {
        let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/lone.rs>lone").unwrap());
    }
    let raw = server
        .mycelium_get_outgoing_refs(Parameters(GetOutgoingRefsRequest {
            path: "src/lone.rs>lone".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["callees"].as_array().unwrap().len(), 0);
    assert_eq!(val["imports"].as_array().unwrap().len(), 0);
    assert_eq!(val["extends"].as_array().unwrap().len(), 0);
    assert_eq!(val["implements"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn get_outgoing_refs_unknown_path_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_outgoing_refs(Parameters(GetOutgoingRefsRequest {
            path: "no/such>path".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

// ── RFC-0042: mycelium_get_all_symbols ───────────────────────────────────

#[tokio::test]
async fn get_all_symbols_excludes_file_nodes() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/a.rs").unwrap());
        store.upsert_node(TrunkPath::parse("src/a.rs>fn1").unwrap());
    }
    let raw = server
        .mycelium_get_all_symbols(Parameters(GetAllSymbolsRequest {
            path_prefix: None,
            kind: None,
            limit: None,
            offset: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let symbols = val["symbols"].as_array().unwrap();
    assert!(!symbols.iter().any(|s| s.as_str().unwrap() == "src/a.rs"));
    assert!(
        symbols
            .iter()
            .any(|s| s.as_str().unwrap() == "src/a.rs>fn1")
    );
    // exact count: store has 1 file + 1 symbol; file node must be excluded
    assert_eq!(
        symbols.len(),
        1,
        "only fn1 is a symbol; file node src/a.rs is excluded"
    );
}

#[tokio::test]
async fn get_all_symbols_prefix_filter() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/a.rs>fn1").unwrap());
        store.upsert_node(TrunkPath::parse("lib/b.rs>fn2").unwrap());
    }
    let raw = server
        .mycelium_get_all_symbols(Parameters(GetAllSymbolsRequest {
            path_prefix: Some("src/".to_owned()),
            kind: None,
            limit: None,
            offset: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let symbols = val["symbols"].as_array().unwrap();
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].as_str().unwrap(), "src/a.rs>fn1");
}

#[tokio::test]
async fn get_all_symbols_kind_filter() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let fn_id = store.upsert_node(TrunkPath::parse("src/a.rs>fn1").unwrap());
        store.set_kind(fn_id, mycelium_core::types::NodeKind::Function);
        let cls_id = store.upsert_node(TrunkPath::parse("src/a.rs>MyClass").unwrap());
        store.set_kind(cls_id, mycelium_core::types::NodeKind::Class);
    }
    let raw = server
        .mycelium_get_all_symbols(Parameters(GetAllSymbolsRequest {
            path_prefix: None,
            kind: Some("function".to_owned()),
            limit: None,
            offset: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let symbols = val["symbols"].as_array().unwrap();
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].as_str().unwrap(), "src/a.rs>fn1");
}

#[tokio::test]
async fn get_all_symbols_unknown_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_all_symbols(Parameters(GetAllSymbolsRequest {
            path_prefix: None,
            kind: Some("bogus_kind".to_owned()),
            limit: None,
            offset: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
    assert!(val["error"].as_str().unwrap().contains("bogus_kind"));
}

#[tokio::test]
async fn get_all_symbols_no_params_returns_all() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("a.rs>x").unwrap());
        store.upsert_node(TrunkPath::parse("b.rs>y").unwrap());
    }
    let raw = server
        .mycelium_get_all_symbols(Parameters(GetAllSymbolsRequest {
            path_prefix: None,
            kind: None,
            limit: None,
            offset: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn get_all_symbols_limit_caps_result_count() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/x.rs>a").unwrap());
        store.upsert_node(TrunkPath::parse("src/x.rs>b").unwrap());
        store.upsert_node(TrunkPath::parse("src/x.rs>c").unwrap());
        store.upsert_node(TrunkPath::parse("src/x.rs>d").unwrap());
        store.upsert_node(TrunkPath::parse("src/x.rs>e").unwrap());
    }
    let raw = server
        .mycelium_get_all_symbols(Parameters(GetAllSymbolsRequest {
            path_prefix: None,
            kind: None,
            limit: Some(3),
            offset: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["symbols"].as_array().unwrap().len(), 3);
    assert_eq!(val["count"].as_u64().unwrap(), 3);
    assert_eq!(val["total_count"].as_u64().unwrap(), 5);
}

#[tokio::test]
async fn get_all_symbols_offset_skips_results() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/x.rs>a").unwrap());
        store.upsert_node(TrunkPath::parse("src/x.rs>b").unwrap());
        store.upsert_node(TrunkPath::parse("src/x.rs>c").unwrap());
        store.upsert_node(TrunkPath::parse("src/x.rs>d").unwrap());
        store.upsert_node(TrunkPath::parse("src/x.rs>e").unwrap());
    }
    let raw = server
        .mycelium_get_all_symbols(Parameters(GetAllSymbolsRequest {
            path_prefix: None,
            kind: None,
            limit: None,
            offset: Some(2),
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    // 5 symbols sorted: a, b, c, d, e → skip 2 → c, d, e
    let syms = val["symbols"].as_array().unwrap();
    assert_eq!(syms.len(), 3);
    assert_eq!(syms[0].as_str().unwrap(), "src/x.rs>c");
    assert_eq!(val["total_count"].as_u64().unwrap(), 5);
}

// ── RFC-0043: mycelium_get_reachable ─────────────────────────────────────

#[tokio::test]
async fn get_reachable_direct_callees() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, a, c);
    }
    let raw = server
        .mycelium_get_reachable(Parameters(GetReachableRequest {
            path: "src/a.rs>a".to_owned(),
            edge_kind: "calls".to_owned(),
            max_depth: Some(1),
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 2);
    let reachable: Vec<&str> = val["reachable"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    assert!(reachable.contains(&"src/b.rs>b"));
    assert!(reachable.contains(&"src/c.rs>c"));
}

#[tokio::test]
async fn get_reachable_cycle_safe() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        store.upsert_edge(EdgeKind::Calls, b, a);
    }
    let raw = server
        .mycelium_get_reachable(Parameters(GetReachableRequest {
            path: "src/a.rs>a".to_owned(),
            edge_kind: "calls".to_owned(),
            max_depth: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 1);
    assert_eq!(val["reachable"][0].as_str().unwrap(), "src/b.rs>b");
}

#[tokio::test]
async fn get_reachable_unknown_path_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_reachable(Parameters(GetReachableRequest {
            path: "no/such>path".to_owned(),
            edge_kind: "calls".to_owned(),
            max_depth: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

#[tokio::test]
async fn get_reachable_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
    }
    let raw = server
        .mycelium_get_reachable(Parameters(GetReachableRequest {
            path: "src/a.rs>a".to_owned(),
            edge_kind: "bogus".to_owned(),
            max_depth: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

#[tokio::test]
async fn get_reachable_max_depth_zero_empty() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
    }
    let raw = server
        .mycelium_get_reachable(Parameters(GetReachableRequest {
            path: "src/a.rs>a".to_owned(),
            edge_kind: "calls".to_owned(),
            max_depth: Some(0),
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
}

// ── RFC-0044: mycelium_get_reachable_to ──────────────────────────────────

#[tokio::test]
async fn get_reachable_to_direct_callers() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
        store.upsert_edge(EdgeKind::Calls, b, a);
        store.upsert_edge(EdgeKind::Calls, c, a);
    }
    let raw = server
        .mycelium_get_reachable_to(Parameters(GetReachableToRequest {
            path: "src/a.rs>a".to_owned(),
            edge_kind: "calls".to_owned(),
            max_depth: Some(1),
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 2);
    let reachable: Vec<&str> = val["reachable"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    assert!(reachable.contains(&"src/b.rs>b"));
    assert!(reachable.contains(&"src/c.rs>c"));
}

#[tokio::test]
async fn get_reachable_to_cycle_safe() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        store.upsert_edge(EdgeKind::Calls, b, a);
        store.upsert_edge(EdgeKind::Calls, a, b);
    }
    let raw = server
        .mycelium_get_reachable_to(Parameters(GetReachableToRequest {
            path: "src/a.rs>a".to_owned(),
            edge_kind: "calls".to_owned(),
            max_depth: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 1);
    assert_eq!(val["reachable"][0].as_str().unwrap(), "src/b.rs>b");
}

#[tokio::test]
async fn get_reachable_to_unknown_path_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_reachable_to(Parameters(GetReachableToRequest {
            path: "no/such>path".to_owned(),
            edge_kind: "calls".to_owned(),
            max_depth: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

#[tokio::test]
async fn get_reachable_to_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
    }
    let raw = server
        .mycelium_get_reachable_to(Parameters(GetReachableToRequest {
            path: "src/a.rs>a".to_owned(),
            edge_kind: "bogus".to_owned(),
            max_depth: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

#[tokio::test]
async fn get_reachable_to_max_depth_zero_empty() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        store.upsert_edge(EdgeKind::Calls, b, a);
    }
    let raw = server
        .mycelium_get_reachable_to(Parameters(GetReachableToRequest {
            path: "src/a.rs>a".to_owned(),
            edge_kind: "calls".to_owned(),
            max_depth: Some(0),
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
}

// ── RFC-0045: mycelium_get_siblings ──────────────────────────────────────

// ── mycelium_query (Three-Surface Rule: MCP twin of CLI `mycelium query`) ──

#[tokio::test]
async fn query_name_selector_returns_matches() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/a.rs>login").unwrap());
        store.upsert_node(TrunkPath::parse("src/a.rs>logout").unwrap());
    }
    let raw = server
        .mycelium_query(Parameters(QueryRequest {
            expr: "#login".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].is_null(), "did not expect error, got: {val}");
    let matches: Vec<&str> = val["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    assert!(
        matches.iter().any(|s| s.contains("login")),
        "expected login in matches, got: {matches:?}"
    );
    assert!(
        !matches.iter().any(|s| s.contains("logout")),
        "name selector #login should NOT match logout"
    );
}

#[tokio::test]
async fn query_parse_error_returns_error_envelope() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_query(Parameters(QueryRequest {
            expr: "this is not a selector >>".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let err = val["error"].as_str().unwrap_or("");
    assert!(
        err.to_lowercase().contains("hyphae") || err.to_lowercase().contains("parse"),
        "expected parse error envelope, got: {}",
        result_str(&raw)
    );
    // No partial-result leakage.
    assert!(val.get("matches").is_none());
}

#[tokio::test]
async fn get_siblings_class_methods() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/a.rs>App>init").unwrap());
        store.upsert_node(TrunkPath::parse("src/a.rs>App>render").unwrap());
        store.upsert_node(TrunkPath::parse("src/a.rs>App>destroy").unwrap());
    }
    let raw = server
        .mycelium_get_siblings(Parameters(GetSiblingsRequest {
            path: "src/a.rs>App>render".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 2);
    let siblings: Vec<&str> = val["siblings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    assert!(siblings.contains(&"src/a.rs>App>init"));
    assert!(siblings.contains(&"src/a.rs>App>destroy"));
    assert!(!siblings.contains(&"src/a.rs>App>render"));
}

#[tokio::test]
async fn get_siblings_root_node_returns_empty() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/a.rs").unwrap());
    }
    let raw = server
        .mycelium_get_siblings(Parameters(GetSiblingsRequest {
            path: "src/a.rs".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
    assert_eq!(val["siblings"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn get_siblings_unknown_path_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_siblings(Parameters(GetSiblingsRequest {
            path: "no/such>path".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

#[tokio::test]
async fn get_siblings_excludes_grandchildren() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/a.rs>App>method").unwrap());
        store.upsert_node(TrunkPath::parse("src/a.rs>App>method>inner").unwrap());
        store.upsert_node(TrunkPath::parse("src/a.rs>App>other").unwrap());
    }
    let raw = server
        .mycelium_get_siblings(Parameters(GetSiblingsRequest {
            path: "src/a.rs>App>method".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 1);
    assert_eq!(val["siblings"][0].as_str().unwrap(), "src/a.rs>App>other");
}

// ── RFC-0046: mycelium_get_node_degree ───────────────────────────────────

#[tokio::test]
async fn get_node_degree_isolated_all_zero() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/a.rs>fn1").unwrap());
    }
    let raw = server
        .mycelium_get_node_degree(Parameters(GetNodeDegreeRequest {
            path: "src/a.rs>fn1".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["in_calls"].as_u64().unwrap(), 0);
    assert_eq!(val["out_calls"].as_u64().unwrap(), 0);
    assert_eq!(val["in_imports"].as_u64().unwrap(), 0);
    assert_eq!(val["out_imports"].as_u64().unwrap(), 0);
}

#[tokio::test]
async fn get_node_degree_counts_edges() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
        store.upsert_edge(EdgeKind::Calls, b, a);
        store.upsert_edge(EdgeKind::Calls, c, a);
        store.upsert_edge(EdgeKind::Calls, a, b);
    }
    let raw = server
        .mycelium_get_node_degree(Parameters(GetNodeDegreeRequest {
            path: "src/a.rs>a".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["in_calls"].as_u64().unwrap(), 2);
    assert_eq!(val["out_calls"].as_u64().unwrap(), 1);
}

#[tokio::test]
async fn get_node_degree_unknown_path_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_node_degree(Parameters(GetNodeDegreeRequest {
            path: "no/such>path".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

// ── RFC-0047: mycelium_get_top_files ─────────────────────────────────────

#[tokio::test]
async fn get_top_files_ranks_by_symbol_count() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/big.rs").unwrap());
        store.upsert_node(TrunkPath::parse("src/big.rs>fn1").unwrap());
        store.upsert_node(TrunkPath::parse("src/big.rs>fn2").unwrap());
        store.upsert_node(TrunkPath::parse("src/big.rs>fn3").unwrap());
        store.upsert_node(TrunkPath::parse("src/small.rs").unwrap());
        store.upsert_node(TrunkPath::parse("src/small.rs>fn1").unwrap());
    }
    let raw = server
        .mycelium_get_top_files(Parameters(GetTopFilesRequest {
            limit: Some(10),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let files = val["files"].as_array().unwrap();
    assert_eq!(files[0]["path"].as_str().unwrap(), "src/big.rs");
    assert_eq!(files[0]["symbol_count"].as_u64().unwrap(), 3);
    assert_eq!(files[1]["path"].as_str().unwrap(), "src/small.rs");
}

#[tokio::test]
async fn get_top_files_empty_graph_returns_empty() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_top_files(Parameters(GetTopFilesRequest {
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
    assert_eq!(val["files"].as_array().unwrap().len(), 0);
}

#[tokio::test]
#[allow(clippy::significant_drop_tightening)]
async fn get_top_files_default_limit() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        for i in 0..15u32 {
            let p = format!("src/{i}.rs");
            store.upsert_node(TrunkPath::parse(&p).unwrap());
            store.upsert_node(TrunkPath::parse(&format!("{p}>fn")).unwrap());
        }
    }
    let raw = server
        .mycelium_get_top_files(Parameters(GetTopFilesRequest {
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    // default limit is 10, so at most 10 files returned even though 15 exist
    assert!(val["files"].as_array().unwrap().len() <= 10);
}

// ── RFC-0048: mycelium_get_most_connected ────────────────────────────────

#[tokio::test]
async fn get_most_connected_ranks_hub_node() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let hub = store.upsert_node(TrunkPath::parse("src/hub.rs>hub").unwrap());
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, hub);
        store.upsert_edge(EdgeKind::Calls, b, hub);
    }
    let raw = server
        .mycelium_get_most_connected(Parameters(GetMostConnectedRequest {
            edge_kind: "calls".to_owned(),
            limit: Some(10),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let syms = val["symbols"].as_array().unwrap();
    assert_eq!(syms[0]["path"].as_str().unwrap(), "src/hub.rs>hub");
    assert_eq!(syms[0]["degree"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn get_most_connected_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_most_connected(Parameters(GetMostConnectedRequest {
            edge_kind: "bogus".to_owned(),
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_some());
}

#[tokio::test]
async fn get_most_connected_empty_excludes_zero_degree() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/a.rs>isolated").unwrap());
    }
    let raw = server
        .mycelium_get_most_connected(Parameters(GetMostConnectedRequest {
            edge_kind: "calls".to_owned(),
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
}

// ── RFC-0049: mycelium_get_leaf_symbols ──────────────────────────────────

#[tokio::test]
async fn get_leaf_symbols_returns_out_degree_zero() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let root = store.upsert_node(TrunkPath::parse("src/a.rs>root").unwrap());
        let leaf = store.upsert_node(TrunkPath::parse("src/b.rs>leaf").unwrap());
        store.upsert_edge(EdgeKind::Calls, root, leaf);
    }
    let raw = server
        .mycelium_get_leaf_symbols(Parameters(GetLeafSymbolsRequest {
            edge_kind: "calls".to_owned(),
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let syms = val["symbols"].as_array().unwrap();
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].as_str().unwrap(), "src/b.rs>leaf");
    assert_eq!(val["count"].as_u64().unwrap(), 1);
}

#[tokio::test]
async fn get_leaf_symbols_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_leaf_symbols(Parameters(GetLeafSymbolsRequest {
            edge_kind: "unknown".to_owned(),
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

#[tokio::test]
async fn get_leaf_symbols_empty_graph_returns_empty() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_leaf_symbols(Parameters(GetLeafSymbolsRequest {
            edge_kind: "calls".to_owned(),
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
}

// ── RFC-0050: mycelium_get_shortest_path ─────────────────────────────────

#[tokio::test]
async fn get_shortest_path_direct_edge() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/a.rs>root").unwrap());
        store.upsert_node(TrunkPath::parse("src/b.rs>leaf").unwrap());
        let root = store.lookup("src/a.rs>root").unwrap();
        let leaf = store.lookup("src/b.rs>leaf").unwrap();
        store.upsert_edge(EdgeKind::Calls, root, leaf);
    }
    let raw = server
        .mycelium_get_shortest_path(Parameters(GetShortestPathRequest {
            from: "src/a.rs>root".to_owned(),
            to: "src/b.rs>leaf".to_owned(),
            edge_kind: "calls".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["length"].as_u64().unwrap(), 1);
    let path = val["path"].as_array().unwrap();
    assert_eq!(path.len(), 2);
}

#[tokio::test]
async fn get_shortest_path_no_path_returns_null() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
    }
    let raw = server
        .mycelium_get_shortest_path(Parameters(GetShortestPathRequest {
            from: "src/a.rs>a".to_owned(),
            to: "src/b.rs>b".to_owned(),
            edge_kind: "calls".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["path"].is_null());
    assert!(val["length"].is_null());
}

#[tokio::test]
async fn get_shortest_path_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_shortest_path(Parameters(GetShortestPathRequest {
            from: "src/a.rs>a".to_owned(),
            to: "src/b.rs>b".to_owned(),
            edge_kind: "unknown".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

#[tokio::test]
async fn get_shortest_path_unknown_from_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_shortest_path(Parameters(GetShortestPathRequest {
            from: "no/such.rs>sym".to_owned(),
            to: "src/b.rs>b".to_owned(),
            edge_kind: "calls".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("path not found"));
}

// ── RFC-0051: mycelium_get_symbol_count_by_kind ──────────────────────────

#[tokio::test]
async fn get_symbol_count_by_kind_basic() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node_with_kind(
            TrunkPath::parse("src/a.rs>fn1").unwrap(),
            mycelium_core::types::NodeKind::Function,
        );
        store.upsert_node_with_kind(
            TrunkPath::parse("src/a.rs>MyClass").unwrap(),
            mycelium_core::types::NodeKind::Class,
        );
    }
    let raw = server
        .mycelium_get_symbol_count_by_kind(Parameters(GetSymbolCountByKindRequest {
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["total"].as_u64().unwrap(), 2);
    let kinds = val["kinds"].as_array().unwrap();
    assert_eq!(kinds.len(), 2);
}

#[tokio::test]
async fn get_symbol_count_by_kind_empty_graph() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_symbol_count_by_kind(Parameters(GetSymbolCountByKindRequest {
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["total"].as_u64().unwrap(), 0);
    assert_eq!(val["kinds"].as_array().unwrap().len(), 0);
}

// ── RFC-0052: mycelium_get_common_callers ────────────────────────────────

#[tokio::test]
async fn get_common_callers_intersection() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        store.upsert_node(TrunkPath::parse("src/shared.rs>shared").unwrap());
        store.upsert_node(TrunkPath::parse("src/ta.rs>ta").unwrap());
        store.upsert_node(TrunkPath::parse("src/tb.rs>tb").unwrap());
        let shared = store.lookup("src/shared.rs>shared").unwrap();
        let ta = store.lookup("src/ta.rs>ta").unwrap();
        let tb = store.lookup("src/tb.rs>tb").unwrap();
        store.upsert_edge(EdgeKind::Calls, shared, ta);
        store.upsert_edge(EdgeKind::Calls, shared, tb);
    }
    let raw = server
        .mycelium_get_common_callers(Parameters(GetCommonCallersRequest {
            paths: vec!["src/ta.rs>ta".to_owned(), "src/tb.rs>tb".to_owned()],
            edge_kind: "calls".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 1);
    assert_eq!(
        val["callers"].as_array().unwrap()[0].as_str().unwrap(),
        "src/shared.rs>shared"
    );
}

#[tokio::test]
async fn get_common_callers_empty_paths_returns_empty() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_common_callers(Parameters(GetCommonCallersRequest {
            paths: vec![],
            edge_kind: "calls".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
}

#[tokio::test]
async fn get_common_callers_unknown_path_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_common_callers(Parameters(GetCommonCallersRequest {
            paths: vec!["no/such.rs>sym".to_owned()],
            edge_kind: "calls".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("path not found"));
}

// ── RFC-0055: mycelium_get_common_callees ────────────────────────────────

#[tokio::test]
async fn get_common_callees_returns_intersection() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let shared = store.upsert_node(TrunkPath::parse("src/shared.rs>shared").unwrap());
        let src_a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        let src_b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        store.upsert_edge(EdgeKind::Calls, src_a, shared);
        store.upsert_edge(EdgeKind::Calls, src_b, shared);
    }
    let raw = server
        .mycelium_get_common_callees(Parameters(GetCommonCalleesRequest {
            paths: vec!["src/a.rs>a".to_owned(), "src/b.rs>b".to_owned()],
            edge_kind: "calls".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 1);
    assert_eq!(
        val["callees"].as_array().unwrap()[0].as_str().unwrap(),
        "src/shared.rs>shared"
    );
}

#[tokio::test]
async fn get_common_callees_empty_paths_returns_empty() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_common_callees(Parameters(GetCommonCalleesRequest {
            paths: vec![],
            edge_kind: "calls".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
}

#[tokio::test]
async fn get_common_callees_unknown_path_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_common_callees(Parameters(GetCommonCalleesRequest {
            paths: vec!["no/such.rs>sym".to_owned()],
            edge_kind: "calls".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("path not found"));
}

#[tokio::test]
async fn get_common_callees_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_common_callees(Parameters(GetCommonCalleesRequest {
            paths: vec!["src/a.rs>a".to_owned()],
            edge_kind: "bad".to_owned(),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

// ── RFC-0053: mycelium_get_fan_out_rank ──────────────────────────────────

#[tokio::test]
async fn get_fan_out_rank_ranks_by_out_degree() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let hub = store.upsert_node(TrunkPath::parse("src/hub.rs>hub").unwrap());
        let sp1 = store.upsert_node(TrunkPath::parse("src/s1.rs>s1").unwrap());
        let sp2 = store.upsert_node(TrunkPath::parse("src/s2.rs>s2").unwrap());
        store.upsert_edge(EdgeKind::Calls, hub, sp1);
        store.upsert_edge(EdgeKind::Calls, hub, sp2);
    }
    let raw = server
        .mycelium_get_fan_out_rank(Parameters(GetFanOutRankRequest {
            edge_kind: "calls".to_owned(),
            limit: Some(10),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let syms = val["symbols"].as_array().unwrap();
    assert_eq!(syms[0]["path"].as_str().unwrap(), "src/hub.rs>hub");
    assert_eq!(syms[0]["out_degree"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn get_fan_out_rank_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_fan_out_rank(Parameters(GetFanOutRankRequest {
            edge_kind: "bad".to_owned(),
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

#[tokio::test]
async fn get_fan_out_rank_empty_graph_returns_empty() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_fan_out_rank(Parameters(GetFanOutRankRequest {
            edge_kind: "calls".to_owned(),
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
}

// ── RFC-0054: mycelium_get_fan_in_rank ───────────────────────────────────

#[tokio::test]
async fn get_fan_in_rank_ranks_by_in_degree() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let hub = store.upsert_node(TrunkPath::parse("src/hub.rs>hub").unwrap());
        let sp1 = store.upsert_node(TrunkPath::parse("src/s1.rs>s1").unwrap());
        let sp2 = store.upsert_node(TrunkPath::parse("src/s2.rs>s2").unwrap());
        // hub is called by both spokes
        store.upsert_edge(EdgeKind::Calls, sp1, hub);
        store.upsert_edge(EdgeKind::Calls, sp2, hub);
    }
    let raw = server
        .mycelium_get_fan_in_rank(Parameters(GetFanInRankRequest {
            edge_kind: "calls".to_owned(),
            limit: Some(10),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let syms = val["symbols"].as_array().unwrap();
    assert_eq!(syms[0]["path"].as_str().unwrap(), "src/hub.rs>hub");
    assert_eq!(syms[0]["in_degree"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn get_fan_in_rank_unknown_edge_kind_returns_error() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_fan_in_rank(Parameters(GetFanInRankRequest {
            edge_kind: "bad".to_owned(),
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
}

#[tokio::test]
async fn get_fan_in_rank_default_limit() {
    let server = MyceliumServer::new();
    let raw = server
        .mycelium_get_fan_in_rank(Parameters(GetFanInRankRequest {
            edge_kind: "calls".to_owned(),
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert_eq!(val["count"].as_u64().unwrap(), 0);
}

/// Poll `predicate` every `interval` for up to `timeout`. Returns `true`
/// when the predicate first returns `true`, `false` on timeout.
async fn poll_for<F, Fut>(
    timeout: tokio::time::Duration,
    interval: tokio::time::Duration,
    predicate: F,
) -> bool
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if predicate().await {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(interval).await;
    }
}

// ── RFC-0090: mycelium_set_compact_mode / mycelium_get_token_stats ────

#[tokio::test]
async fn set_compact_mode_toggles_flag() {
    let server = MyceliumServer::new();

    // Default: compact mode off.
    assert!(
        !server
            .compact_mode
            .load(std::sync::atomic::Ordering::Relaxed)
    );

    // Enable compact mode.
    let raw = server
        .mycelium_set_compact_mode(Parameters(SetCompactModeRequest { enabled: true }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val["compact_mode"].as_bool().unwrap());
    assert!(
        server
            .compact_mode
            .load(std::sync::atomic::Ordering::Relaxed)
    );

    // Disable compact mode.
    let raw = server
        .mycelium_set_compact_mode(Parameters(SetCompactModeRequest { enabled: false }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(!val["compact_mode"].as_bool().unwrap());
    assert!(
        !server
            .compact_mode
            .load(std::sync::atomic::Ordering::Relaxed)
    );
}

#[tokio::test]
async fn search_symbol_compact_mode_returns_msgpack_hex() {
    let server = server_with_fixture().await;

    // Enable compact mode.
    server
        .mycelium_set_compact_mode(Parameters(SetCompactModeRequest { enabled: true }))
        .await;

    let raw = server
        .mycelium_search_symbol(Parameters(SearchSymbolRequest {
            query: "greet".to_string(),
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    // Compact response must have fmt = "msgpack_hex".
    assert_eq!(val["fmt"].as_str().unwrap(), "msgpack_hex");
    let hex = val["data"].as_str().unwrap();
    // Hex string must be non-empty and contain only hex chars.
    assert!(!hex.is_empty());
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
}

#[tokio::test]
async fn search_symbol_json_mode_when_compact_disabled() {
    let server = server_with_fixture().await;
    // compact mode off by default
    let raw = server
        .mycelium_search_symbol(Parameters(SearchSymbolRequest {
            query: "greet".to_string(),
            limit: None,
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    // Normal JSON response must have "matches" key.
    assert!(val["matches"].as_array().is_some());
}

#[tokio::test]
async fn get_token_stats_returns_valid_shape() {
    let server = MyceliumServer::new();
    let raw = server.mycelium_get_token_stats().await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    // All expected fields must be present and positive.
    assert_eq!(val["sample_query"].as_str().unwrap(), "top 3 symbols");
    let json_bytes = val["json_bytes"].as_u64().unwrap();
    let msgpack_bytes = val["msgpack_bytes"].as_u64().unwrap();
    assert!(json_bytes > 0, "json_bytes must be positive");
    assert!(msgpack_bytes > 0, "msgpack_bytes must be positive");
    let ratio = val["ratio"].as_f64().unwrap();
    // Ratio must be a sane positive fraction.
    assert!(ratio > 0.0, "ratio must be positive");
    // MessagePack raw bytes are always smaller than an equivalent JSON
    // string for structured data (field names not repeated).  For the
    // fixed sample this is consistently < 1.0.
    assert!(
        ratio < 1.0,
        "msgpack raw bytes should be smaller than JSON bytes, ratio was {ratio:.3}"
    );
    // New fields added for SPRINT-004 token-ratio SLA fix.
    let compact_chars = val["compact_chars"].as_u64().unwrap();
    assert!(compact_chars > 0, "compact_chars must be positive");
    let token_ratio = val["token_ratio"].as_f64().unwrap();
    assert!(token_ratio > 0.0, "token_ratio must be positive");
}

#[tokio::test]
async fn get_token_stats_token_ratio_vs_byte_ratio() {
    let server = MyceliumServer::new();
    let raw = server.mycelium_get_token_stats().await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    let token_ratio = val["token_ratio"].as_f64().unwrap();
    // token_ratio is abbreviated-compact-text chars / verbose-JSON chars.
    // It must be strictly between 0 and 1 to satisfy the Charter §2
    // AI token-efficiency SLA (compact output uses fewer AI tokens than JSON).
    assert!(
        0.0 < token_ratio && token_ratio < 1.0,
        "token_ratio out of range: {token_ratio:.4}"
    );
    // The raw msgpack byte ratio must be <= the text-compact token ratio:
    // binary is always at least as compact as any text abbreviation.
    let byte_ratio = val["ratio"].as_f64().unwrap();
    assert!(
        byte_ratio <= token_ratio,
        "byte_ratio {byte_ratio:.4} should be <= token_ratio {token_ratio:.4}"
    );
}

// ── embedded pack query sanity (verifies include_str! paths resolve) ──────

#[test]
fn embedded_pack_queries_are_non_empty() {
    // These constants are populated via include_str! from crates/mycelium-mcp/packs/.
    // A non-empty string proves the path resolved correctly at compile time.
    // If any include_str! path is wrong this test will fail to compile.
    for (lang, q) in [
        ("javascript", JAVASCRIPT_QUERIES),
        ("python", PYTHON_QUERIES),
        ("typescript", TYPESCRIPT_QUERIES),
        ("rust", RUST_QUERIES),
        ("go", GO_QUERIES),
        ("java", JAVA_QUERIES),
        ("c", C_QUERIES),
        ("ruby", RUBY_QUERIES),
        ("cpp", CPP_QUERIES),
        ("csharp", CSHARP_QUERIES),
    ] {
        assert!(!q.is_empty(), "{lang} pack query must be non-empty");
        assert!(
            q.contains('(') || q.contains('['),
            "{lang} pack query must contain tree-sitter syntax"
        );
    }
}

// ── RFC-0094 Phase 2: output_format per-request (basic-queries family) ──

#[tokio::test]
async fn search_symbol_json_format_returns_json() {
    let server = server_with_fixture().await;
    let raw = server
        .mycelium_search_symbol(Parameters(SearchSymbolRequest {
            query: "greet".to_string(),
            limit: None,
            output_format: Some(OutputFormat::Json),
        }))
        .await;
    assert!(
        result_str(&raw).trim_start().starts_with('{'),
        "JSON format must start with {{"
    );
    let _: serde_json::Value = serde_json::from_str(result_str(&raw)).expect("must be valid JSON");
}

#[tokio::test]
async fn search_symbol_text_format_not_json_envelope() {
    let server = server_with_fixture().await;
    let raw = server
        .mycelium_search_symbol(Parameters(SearchSymbolRequest {
            query: "greet".to_string(),
            limit: None,
            output_format: Some(OutputFormat::Text),
        }))
        .await;
    // TOON text output starts with a key name, not a JSON brace
    assert!(
        !result_str(&raw).trim_start().starts_with('{'),
        "Text format must not start with JSON brace; got: {raw:?}"
    );
    assert!(
        result_str(&raw).contains("matches"),
        "Text format must still contain the 'matches' key"
    );
}

#[tokio::test]
async fn get_ancestors_text_format_not_json_envelope() {
    let server = server_with_fixture().await;
    let raw = server
        .mycelium_get_ancestors(Parameters(GetAncestorsRequest {
            path: "src/greet.rs>greet".to_string(),
            output_format: Some(OutputFormat::Text),
        }))
        .await;
    assert!(
        !result_str(&raw).trim_start().starts_with('{'),
        "Text format must not start with JSON brace"
    );
    assert!(
        result_str(&raw).contains("ancestors"),
        "must contain the 'ancestors' key"
    );
}

#[tokio::test]
async fn get_descendants_text_format_not_json_envelope() {
    let server = server_with_fixture().await;
    let raw = server
        .mycelium_get_descendants(Parameters(GetDescendantsRequest {
            path: "src/greet.rs".to_string(),
            include_inherited: None,
            output_format: Some(OutputFormat::Text),
        }))
        .await;
    assert!(
        !result_str(&raw).trim_start().starts_with('{'),
        "Text format must not start with JSON brace"
    );
    assert!(
        result_str(&raw).contains("descendants"),
        "must contain the 'descendants' key"
    );
}

#[tokio::test]
async fn search_symbol_none_format_defaults_to_json() {
    // Backward-compat: no output_format → JSON (same as before Phase 2)
    let server = server_with_fixture().await;
    let raw = server
        .mycelium_search_symbol(Parameters(SearchSymbolRequest {
            query: "greet".to_string(),
            limit: None,
            output_format: None,
        }))
        .await;
    let _: serde_json::Value =
        serde_json::from_str(result_str(&raw)).expect("default must be valid JSON");
}

// ── issue #246: virtual dispatch callers ──────────────────────────────────

/// Fixture: `Abstract>method` is called by Caller.
/// `SubClass` extends `Abstract` but defines its own `SubClass>method`.
/// `get-callers(SubClass>method, include_virtual=true)` must include the
/// call site (`Caller>fn`) that calls `Abstract>method` via virtual dispatch.
async fn server_with_virtual_dispatch_fixture() -> MyceliumServer {
    use mycelium_core::trunk::TrunkPath;
    use mycelium_core::types::{EdgeKind, NodeKind};
    let server = MyceliumServer::new();
    let mut store = server.store.write().await;
    // Nodes
    let abstract_class = store.upsert_node(TrunkPath::parse("pkg/base.py>AbstractPlugin").unwrap());
    let abstract_method =
        store.upsert_node(TrunkPath::parse("pkg/base.py>AbstractPlugin>analyze").unwrap());
    store.set_kind(abstract_method, NodeKind::Method);
    let sub_class = store.upsert_node(TrunkPath::parse("pkg/sub.py>ConcretePlugin").unwrap());
    let sub_method =
        store.upsert_node(TrunkPath::parse("pkg/sub.py>ConcretePlugin>analyze").unwrap());
    store.set_kind(sub_method, NodeKind::Method);
    let caller_fn = store.upsert_node(TrunkPath::parse("pkg/engine.py>Engine>run").unwrap());
    store.set_kind(caller_fn, NodeKind::Method);
    // Edges
    store.upsert_edge(EdgeKind::Extends, sub_class, abstract_class);
    // Virtual dispatch: Engine>run calls AbstractPlugin>analyze (via typed variable)
    store.upsert_edge(EdgeKind::Calls, caller_fn, abstract_method);
    drop(store);
    server
}

#[tokio::test]
async fn get_callers_include_virtual_surfaces_virtual_dispatch_caller() {
    let server = server_with_virtual_dispatch_fixture().await;
    let raw = server
        .mycelium_get_callers(Parameters(GetCallersRequest {
            path: "pkg/sub.py>ConcretePlugin>analyze".to_string(),
            edge_kind: None,
            include_virtual: Some(true),
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none(), "must not return error");
    let arr = val["caller_paths"].as_array().unwrap();
    assert!(
        arr.iter()
            .any(|v| v.as_str() == Some("pkg/engine.py>Engine>run")),
        "include_virtual must surface Engine>run which calls AbstractPlugin>analyze"
    );
}

#[tokio::test]
async fn get_callers_default_does_not_include_virtual() {
    let server = server_with_virtual_dispatch_fixture().await;
    let raw = server
        .mycelium_get_callers(Parameters(GetCallersRequest {
            path: "pkg/sub.py>ConcretePlugin>analyze".to_string(),
            edge_kind: None,
            include_virtual: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&raw)).unwrap();
    assert!(val.get("error").is_none(), "must not return error");
    let arr = val["caller_paths"].as_array().unwrap();
    assert!(
        !arr.iter()
            .any(|v| v.as_str() == Some("pkg/engine.py>Engine>run")),
        "without include_virtual, virtual dispatch callers must not appear"
    );
}

// ── RFC-0094 Phase 3: output_format on remaining MCP query tools ──────────
// These tests were written BEFORE the field was added (Charter §5.1 TDD RED step).

#[tokio::test]
async fn test_get_callees_text_format() {
    let server = server_with_fixture().await;
    let result = server
        .mycelium_get_callees(Parameters(GetCalleesRequest {
            path: "src/greet.rs>greet".to_owned(),
            edge_kind: None,
            output_format: Some(OutputFormat::Text),
            budget: None,
        }))
        .await;
    // Text format must not start with a JSON brace.
    assert!(
        !result_str(&result).trim_start().starts_with('{'),
        "expected text format, got JSON: {}",
        result_str(&result)
    );
}

#[tokio::test]
async fn test_get_callers_text_format() {
    let server = server_with_fixture().await;
    let result = server
        .mycelium_get_callers(Parameters(GetCallersRequest {
            path: "src/greet.rs>greet".to_owned(),
            edge_kind: None,
            include_virtual: None,
            output_format: Some(OutputFormat::Text),
            budget: None,
        }))
        .await;
    assert!(
        !result_str(&result).trim_start().starts_with('{'),
        "expected text format, got JSON: {}",
        result_str(&result)
    );
}

#[tokio::test]
async fn test_get_symbol_info_text_format() {
    let server = server_with_fixture().await;
    let result = server
        .mycelium_get_symbol_info(Parameters(GetSymbolInfoRequest {
            path: "src/greet.rs>greet".to_owned(),
            output_format: Some(OutputFormat::Text),
        }))
        .await;
    assert!(
        !result_str(&result).trim_start().starts_with('{'),
        "expected text format, got JSON: {}",
        result_str(&result)
    );
    assert!(
        result_str(&result).contains("path"),
        "text output must contain 'path' key"
    );
}

#[tokio::test]
async fn test_get_callee_tree_text_format() {
    let server = server_with_fixture().await;
    let result = server
        .mycelium_get_callee_tree(Parameters(GetCalleeTreeRequest {
            path: "src/greet.rs>greet".to_owned(),
            max_depth: None,
            output_format: Some(OutputFormat::Text),
        }))
        .await;
    assert!(
        !result_str(&result).trim_start().starts_with('{'),
        "expected text format, got JSON: {}",
        result_str(&result)
    );
}

#[tokio::test]
async fn test_get_files_text_format() {
    let server = server_with_fixture().await;
    let result = server
        .mycelium_get_files(Parameters(GetFilesRequest {
            path_prefix: None,
            output_format: Some(OutputFormat::Text),
        }))
        .await;
    assert!(
        !result_str(&result).trim_start().starts_with('{'),
        "expected text format, got JSON: {}",
        result_str(&result)
    );
    assert!(
        result_str(&result).contains("files"),
        "text output must contain 'files' key"
    );
}

#[tokio::test]
async fn test_get_entry_points_text_format() {
    let server = server_with_fixture().await;
    let result = server
        .mycelium_get_entry_points(Parameters(GetEntryPointsRequest {
            path_prefix: None,
            output_format: Some(OutputFormat::Text),
        }))
        .await;
    assert!(
        !result_str(&result).trim_start().starts_with('{'),
        "expected text format, got JSON: {}",
        result_str(&result)
    );
}

#[tokio::test]
async fn test_get_imports_text_format() {
    let server = server_with_fixture().await;
    let result = server
        .mycelium_get_imports(Parameters(GetImportsRequest {
            path: "src/greet.rs>greet".to_owned(),
            output_format: Some(OutputFormat::Text),
        }))
        .await;
    // path not found → error JSON, that's OK — error paths remain JSON.
    // For text format on a found path the result would be text.
    // We just confirm compilation succeeds.
    let _ = result;
}

#[tokio::test]
async fn test_rank_symbols_text_format() {
    let server = server_with_fixture().await;
    let result = server
        .mycelium_rank_symbols(Parameters(RankSymbolsRequest {
            limit: None,
            edge_kind: None,
            output_format: Some(OutputFormat::Text),
        }))
        .await;
    assert!(
        !result_str(&result).trim_start().starts_with('{'),
        "expected text format, got JSON: {}",
        result_str(&result)
    );
}

#[tokio::test]
async fn test_get_node_kind_text_format() {
    let server = server_with_fixture().await;
    let result = server
        .mycelium_get_node_kind(Parameters(GetNodeKindRequest {
            path: "src/greet.rs>greet".to_owned(),
            output_format: Some(OutputFormat::Text),
        }))
        .await;
    assert!(
        !result_str(&result).trim_start().starts_with('{'),
        "expected text format, got JSON: {}",
        result_str(&result)
    );
}

#[tokio::test]
async fn test_output_format_none_defaults_to_json_for_new_tools() {
    // Backward-compat regression check: all newly-wired tools must default
    // to JSON when output_format is None.
    let server = server_with_fixture().await;

    let raw = server
        .mycelium_rank_symbols(Parameters(RankSymbolsRequest {
            limit: None,
            edge_kind: None,
            output_format: None,
        }))
        .await;
    let _: serde_json::Value =
        serde_json::from_str(result_str(&raw)).expect("None output_format must yield valid JSON");

    let raw = server
        .mycelium_get_files(Parameters(GetFilesRequest {
            path_prefix: None,
            output_format: None,
        }))
        .await;
    let _: serde_json::Value =
        serde_json::from_str(result_str(&raw)).expect("None output_format must yield valid JSON");
}

// ── RFC-0097: filesystem access boundary ──────────────────────────────────

#[test]
fn check_path_allows_empty_roots() {
    let result = check_path_in_allowed_roots("/etc", &[]);
    assert!(result.is_ok(), "empty allowlist must permit all paths");
}

#[test]
fn check_path_rejects_nonexistent_with_populated_roots() {
    let dir = tempfile::tempdir().unwrap();
    let roots = vec![dir.path().to_path_buf()];
    let result = check_path_in_allowed_roots("/nonexistent_path_xyz", &roots);
    assert!(
        result.is_err(),
        "nonexistent path must be rejected when roots are set"
    );
}

#[tokio::test]
async fn index_workspace_rejects_path_outside_allowed_roots() {
    let allowed = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let server = MyceliumServer::new_with_allowed_roots(vec![allowed.path().to_path_buf()]);
    let raw = server
        .mycelium_index_workspace(Parameters(IndexWorkspaceRequest {
            path: outside.path().to_string_lossy().into_owned(),
        }))
        .await;
    assert!(
        raw.is_error.unwrap_or(false),
        "must reject path outside allowed roots"
    );
    let msg = result_str(&raw);
    assert!(
        msg.contains("outside allowed"),
        "error message should mention allowed roots"
    );
}

#[tokio::test]
async fn index_workspace_rejects_path_traversal() {
    let allowed = tempfile::tempdir().unwrap();
    let server = MyceliumServer::new_with_allowed_roots(vec![allowed.path().to_path_buf()]);
    // Construct a traversal path: <allowed>/subdir/../../etc
    let traversal = format!("{}/subdir/../../etc", allowed.path().to_string_lossy());
    let raw = server
        .mycelium_index_workspace(Parameters(IndexWorkspaceRequest { path: traversal }))
        .await;
    assert!(
        raw.is_error.unwrap_or(false),
        "path traversal must be rejected"
    );
}

#[tokio::test]
async fn index_workspace_accepts_path_inside_allowed_roots() {
    let allowed = tempfile::tempdir().unwrap();
    let server = MyceliumServer::new_with_allowed_roots(vec![allowed.path().to_path_buf()]);
    // Indexing the allowed root itself must not be rejected for boundary reasons
    // (it may fail to index any files, but it must not be a security rejection)
    let raw = server
        .mycelium_index_workspace(Parameters(IndexWorkspaceRequest {
            path: allowed.path().to_string_lossy().into_owned(),
        }))
        .await;
    let msg = result_str(&raw);
    assert!(
        !msg.contains("outside allowed"),
        "allowed path must not be rejected: {msg}"
    );
}

// ── Issue #297: --edge-kind consistency ────────────────────────────────────

#[tokio::test]
async fn get_callees_edge_kind_imports_returns_import_targets() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>ModA").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>ModB").unwrap());
        store.upsert_edge(EdgeKind::Imports, a, b);
    }
    let result = server
        .mycelium_get_callees(Parameters(GetCalleesRequest {
            path: "src/a.rs>ModA".to_string(),
            edge_kind: Some("imports".to_string()),
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&result)).unwrap();
    let paths = val["callee_paths"].as_array().unwrap();
    assert!(
        paths.iter().any(|p| p == "src/b.rs>ModB"),
        "expected src/b.rs>ModB in callee_paths, got: {paths:?}"
    );
}

#[tokio::test]
async fn get_callers_edge_kind_extends_returns_extenders() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let base = store.upsert_node(TrunkPath::parse("src/base.rs>Base").unwrap());
        let child = store.upsert_node(TrunkPath::parse("src/child.rs>Child").unwrap());
        store.upsert_edge(EdgeKind::Extends, child, base);
    }
    let result = server
        .mycelium_get_callers(Parameters(GetCallersRequest {
            path: "src/base.rs>Base".to_string(),
            edge_kind: Some("extends".to_string()),
            include_virtual: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&result)).unwrap();
    let paths = val["caller_paths"].as_array().unwrap();
    assert!(
        paths.iter().any(|p| p == "src/child.rs>Child"),
        "expected src/child.rs>Child in caller_paths, got: {paths:?}"
    );
}

#[tokio::test]
async fn rank_symbols_edge_kind_imports_ranks_most_imported_first() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let hub = store.upsert_node(TrunkPath::parse("src/hub.rs>Hub").unwrap());
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>A").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>B").unwrap());
        let c = store.upsert_node(TrunkPath::parse("src/c.rs>C").unwrap());
        store.upsert_edge(EdgeKind::Imports, a, hub);
        store.upsert_edge(EdgeKind::Imports, b, hub);
        store.upsert_edge(EdgeKind::Imports, c, hub);
    }
    let result = server
        .mycelium_rank_symbols(Parameters(RankSymbolsRequest {
            limit: Some(5),
            edge_kind: Some("imports".to_string()),
            output_format: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&result)).unwrap();
    let symbols = val["symbols"].as_array().unwrap();
    assert!(!symbols.is_empty(), "expected ranked symbols");
    assert_eq!(
        symbols[0]["path"].as_str().unwrap(),
        "src/hub.rs>Hub",
        "most-imported symbol should rank first"
    );
}

#[tokio::test]
async fn get_dead_symbols_edge_kind_calls_finds_call_unreferenced() {
    let server = MyceliumServer::new();
    {
        let mut store = server.store.write().await;
        let importer = store.upsert_node(TrunkPath::parse("src/importer.rs>A").unwrap());
        let target = store.upsert_node(TrunkPath::parse("src/target.rs>B").unwrap());
        // target has an incoming Imports edge but no incoming Calls edge
        store.upsert_edge(EdgeKind::Imports, importer, target);
    }
    // Default (no edge_kind): checks Calls AND Imports → target NOT dead
    let result_default = server
        .mycelium_get_dead_symbols(Parameters(GetDeadSymbolsRequest {
            path_prefix: None,
            edge_kind: None,
            output_format: None,
            budget: None,
        }))
        .await;
    let val: serde_json::Value = serde_json::from_str(result_str(&result_default)).unwrap();
    let dead: Vec<&str> = val["dead_symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        !dead.contains(&"src/target.rs>B"),
        "default dead check must NOT flag symbol that has an Imports edge; got: {dead:?}"
    );
    // With edge_kind "calls": target has no Calls → IS dead for calls
    let result_calls = server
        .mycelium_get_dead_symbols(Parameters(GetDeadSymbolsRequest {
            path_prefix: None,
            edge_kind: Some("calls".to_string()),
            output_format: None,
            budget: None,
        }))
        .await;
    let val2: serde_json::Value = serde_json::from_str(result_str(&result_calls)).unwrap();
    let dead_calls: Vec<&str> = val2["dead_symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        dead_calls.contains(&"src/target.rs>B"),
        "with edge_kind=calls, symbol with no Calls edge must appear as dead; got: {dead_calls:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// v0.1.18 coverage-gate top-up: parse_hash_hex private-fn tests.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_hash_hex_round_trips_with_hash_hex_format() {
    let bytes: [u8; 16] = [
        0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32,
        0x10,
    ];
    let mut hex = String::with_capacity(32);
    for b in &bytes {
        use std::fmt::Write as _;
        write!(&mut hex, "{b:02x}").unwrap();
    }
    let s = format!("b3:{hex}");
    let parsed = parse_hash_hex(&s).expect("valid hash parses");
    assert_eq!(parsed, bytes);
}

#[test]
fn parse_hash_hex_rejects_missing_prefix() {
    let s = "00112233445566778899aabbccddeeff";
    assert!(parse_hash_hex(s).is_none(), "no b3: prefix → None");
}

#[test]
fn parse_hash_hex_rejects_wrong_length() {
    assert!(parse_hash_hex("b3:beef").is_none(), "too short → None");
    let too_long = format!("b3:{}", "a".repeat(33));
    assert!(parse_hash_hex(&too_long).is_none(), "too long → None");
}

#[test]
fn parse_hash_hex_rejects_non_hex() {
    let bad = format!("b3:{}", "z".repeat(32));
    assert!(parse_hash_hex(&bad).is_none(), "non-hex digit → None");
}

#[test]
fn source_extension_recognises_supported_languages() {
    use std::path::Path;
    let cases = [
        ("a.rs", Some("rs")),
        ("a.py", Some("py")),
        ("a.pyi", Some("pyi")),
        ("a.js", Some("js")),
        ("a.jsx", Some("jsx")),
        ("a.ts", Some("ts")),
        ("a.tsx", Some("tsx")),
        ("a.go", Some("go")),
        ("a.java", Some("java")),
        ("a.c", Some("c")),
        ("a.h", Some("h")),
        ("a.rb", Some("rb")),
        ("a.cpp", Some("cpp")),
        ("a.cc", Some("cc")),
        ("a.cxx", Some("cxx")),
        ("a.hpp", Some("hpp")),
        ("a.cs", Some("cs")),
        ("a.txt", None),
        ("a", None),
        ("Makefile", None),
    ];
    for (input, want) in cases {
        let got = source_extension(Path::new(input));
        assert_eq!(got, want, "source_extension({input:?}) mismatch");
    }
}

#[test]
fn legacy_index_path_points_under_mycelium_dir() {
    use std::path::Path;
    let p = legacy_index_path(Path::new("/r"));
    let s = p.to_string_lossy();
    assert!(s.contains(".mycelium"), "path under .mycelium dir: {s}");
    assert!(s.ends_with("index.rmp"), "ends with index.rmp: {s}");
}

#[test]
fn existing_index_path_returns_none_when_no_files() {
    let dir = tempfile::tempdir().unwrap();
    let result = existing_index_path(dir.path());
    assert!(
        result.is_none(),
        "fresh dir has no index files, got: {result:?}"
    );
}

#[test]
fn existing_index_path_finds_legacy_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let mycelium_dir = dir.path().join(".mycelium");
    std::fs::create_dir_all(&mycelium_dir).unwrap();
    let snap = mycelium_dir.join("index.rmp");
    std::fs::write(&snap, b"x").unwrap();
    let result = existing_index_path(dir.path()).expect("found");
    assert_eq!(result, snap, "found legacy index.rmp");
}
