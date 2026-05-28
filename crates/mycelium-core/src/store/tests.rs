//! Store integration tests — written before implementation per Charter §5.1.
//!
//! Each test maps to an acceptance criterion from RFC-0001 §Public API sketch
//! or §Testing strategy.

use super::Store;
use crate::trunk::TrunkPath;
use crate::types::{EdgeKind, NodeId};

#[cfg(test)]
extern crate tempfile;

fn path(s: &str) -> TrunkPath {
    TrunkPath::parse(s).unwrap()
}

// ──────────────────────────────────────────────────────────────────────
// Node upsert + lookup
// ──────────────────────────────────────────────────────────────────────

#[test]
fn store_upsert_node_returns_stable_id() {
    let mut store = Store::new();
    let id1 = store.upsert_node(path("src/lib.rs>Foo"));
    let id2 = store.upsert_node(path("src/lib.rs>Foo")); // idempotent
    assert_eq!(id1, id2);
    assert_eq!(store.lookup("src/lib.rs>Foo"), Some(id1));
}

#[test]
fn store_lookup_returns_none_for_unknown_path() {
    let store = Store::new();
    assert_eq!(store.lookup("nonexistent"), None);
}

#[test]
fn store_lookup_is_exact_match_only() {
    let mut store = Store::new();
    store.upsert_node(path("src/auth.rs>AuthService>login"));
    assert_eq!(store.lookup("src/auth.rs"), None);
    assert_eq!(store.lookup("src/auth.rs>AuthService"), None);
}

// ──────────────────────────────────────────────────────────────────────
// Edge upsert + query
// ──────────────────────────────────────────────────────────────────────

#[test]
fn store_upsert_edge_connects_nodes_bidirectionally() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>A"));
    let b = store.upsert_node(path("src/b.rs>B"));
    store.upsert_edge(EdgeKind::Calls, a, b);

    assert_eq!(store.outgoing(a, EdgeKind::Calls), &[b]);
    assert_eq!(store.incoming(b, EdgeKind::Calls), &[a]);
}

#[test]
fn store_outgoing_returns_empty_for_unknown_node() {
    let store = Store::new();
    assert!(store.outgoing(NodeId(0x100), EdgeKind::Calls).is_empty());
}

// ──────────────────────────────────────────────────────────────────────
// remove_node — non-cascading, clears both stores
// ──────────────────────────────────────────────────────────────────────

#[test]
fn store_remove_node_clears_trunk_and_synapse() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>A"));
    let b = store.upsert_node(path("src/b.rs>B"));
    store.upsert_edge(EdgeKind::Calls, a, b);

    store.remove_node(a);

    assert_eq!(
        store.lookup("src/a.rs>A"),
        None,
        "trunk should forget removed node"
    );
    assert!(
        store.outgoing(a, EdgeKind::Calls).is_empty(),
        "synapse forward should be cleared"
    );
    assert!(
        store.incoming(b, EdgeKind::Calls).is_empty(),
        "synapse reverse should be cleared"
    );
    // b is unaffected
    assert_eq!(store.lookup("src/b.rs>B"), Some(b));
}

#[test]
fn store_remove_node_is_noncascading() {
    // Removing a parent must not remove its children.
    let mut store = Store::new();
    let cls = store.upsert_node(path("src/auth.rs>AuthService"));
    let method = store.upsert_node(path("src/auth.rs>AuthService>login"));

    store.remove_node(cls);

    assert_eq!(
        store.lookup("src/auth.rs>AuthService"),
        None,
        "parent removed"
    );
    assert_eq!(
        store.lookup("src/auth.rs>AuthService>login"),
        Some(method),
        "child must survive non-cascading remove"
    );
}

// ──────────────────────────────────────────────────────────────────────
// remove_file — cascading subtree removal
// ──────────────────────────────────────────────────────────────────────

#[test]
fn store_remove_file_clears_all_file_nodes_and_edges() {
    let mut store = Store::new();
    store.upsert_node(path("src/auth.rs"));
    store.upsert_node(path("src/auth.rs>AuthService"));
    let method = store.upsert_node(path("src/auth.rs>AuthService>login"));
    let other = store.upsert_node(path("src/other.rs>Other"));
    store.upsert_edge(EdgeKind::Calls, method, other);

    store.remove_file("src/auth.rs");

    assert_eq!(store.lookup("src/auth.rs"), None);
    assert_eq!(store.lookup("src/auth.rs>AuthService"), None);
    assert_eq!(store.lookup("src/auth.rs>AuthService>login"), None);
    // Sibling file is untouched.
    assert_eq!(store.lookup("src/other.rs>Other"), Some(other));
    // The edge to the sibling node is gone.
    assert!(store.incoming(other, EdgeKind::Calls).is_empty());
}

#[test]
fn store_remove_file_is_noop_for_unknown_path() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>A"));
    store.remove_file("does/not/exist.rs");
    assert_eq!(store.lookup("src/a.rs>A"), Some(a));
}

// ──────────────────────────────────────────────────────────────────────
// Traversal delegation
// ──────────────────────────────────────────────────────────────────────

// ──────────────────────────────────────────────────────────────────────
// search_symbol
// ──────────────────────────────────────────────────────────────────────

#[test]
fn store_search_symbol_returns_matching_name_segment() {
    let mut store = Store::new();
    store.upsert_node(path("src/auth.rs"));
    store.upsert_node(path("src/auth.rs>AuthService"));
    store.upsert_node(path("src/auth.rs>AuthService>login"));
    store.upsert_node(path("src/utils.rs>Authenticator"));
    store.upsert_node(path("src/other.rs>OtherClass"));

    // "auth" should match: "auth.rs" (filename), "AuthService", "Authenticator"
    // NOT: "login", "OtherClass"
    let results = store.search_symbol("auth", 20);
    assert!(results.contains(&"src/auth.rs".to_string()));
    assert!(results.contains(&"src/auth.rs>AuthService".to_string()));
    assert!(results.contains(&"src/utils.rs>Authenticator".to_string()));
    assert!(!results.contains(&"src/auth.rs>AuthService>login".to_string()));
    assert!(!results.contains(&"src/other.rs>OtherClass".to_string()));
}

#[test]
fn store_search_symbol_is_case_insensitive() {
    let mut store = Store::new();
    store.upsert_node(path("src/lib.rs>MyStruct"));
    let results = store.search_symbol("mystruct", 10);
    assert!(results.contains(&"src/lib.rs>MyStruct".to_string()));
}

#[test]
fn store_search_symbol_respects_limit() {
    let mut store = Store::new();
    for i in 0..10_usize {
        store.upsert_node(TrunkPath::parse(&format!("src/f{i}.rs>foo{i}")).unwrap());
    }
    let results = store.search_symbol("foo", 3);
    assert_eq!(results.len(), 3);
}

#[test]
fn store_search_symbol_returns_sorted_results() {
    let mut store = Store::new();
    store.upsert_node(path("src/z.rs>zap"));
    store.upsert_node(path("src/a.rs>apple"));
    store.upsert_node(path("src/m.rs>mango"));
    let results = store.search_symbol("a", 10);
    // All three contain "a" in their name segment
    let mut expected = results.clone();
    expected.sort_unstable();
    assert_eq!(
        results, expected,
        "results must be lexicographically sorted"
    );
}

// ──────────────────────────────────────────────────────────────────────
// ancestors_of_path
// ──────────────────────────────────────────────────────────────────────

#[test]
fn store_ancestors_of_path_returns_chain_in_child_to_root_order() {
    let mut store = Store::new();
    store.upsert_node(path("src/auth.rs"));
    store.upsert_node(path("src/auth.rs>AuthService"));
    store.upsert_node(path("src/auth.rs>AuthService>login"));

    let ancestors = store
        .ancestors_of_path("src/auth.rs>AuthService>login")
        .expect("path should be materialized");
    assert_eq!(ancestors[0], "src/auth.rs>AuthService");
    assert_eq!(ancestors[1], "src/auth.rs");
}

#[test]
fn store_ancestors_of_path_returns_none_for_unknown_path() {
    let store = Store::new();
    assert!(store.ancestors_of_path("nonexistent>path").is_none());
}

#[test]
fn store_ancestors_of_path_returns_empty_vec_for_root_node() {
    let mut store = Store::new();
    store.upsert_node(path("src/lib.rs"));
    let ancestors = store
        .ancestors_of_path("src/lib.rs")
        .expect("root node is materialized");
    assert!(ancestors.is_empty(), "a root node has no ancestors");
}

// ──────────────────────────────────────────────────────────────────────
// descendants_of_path
// ──────────────────────────────────────────────────────────────────────

#[test]
fn store_descendants_of_path_returns_all_nested_symbols() {
    let mut store = Store::new();
    store.upsert_node(path("src/lib.rs"));
    store.upsert_node(path("src/lib.rs>Foo"));
    store.upsert_node(path("src/lib.rs>Foo>bar"));
    store.upsert_node(path("src/other.rs>Baz"));

    let mut desc = store
        .descendants_of_path("src/lib.rs")
        .expect("path is materialized");
    desc.sort_unstable();
    assert_eq!(
        desc,
        vec![
            "src/lib.rs>Foo".to_string(),
            "src/lib.rs>Foo>bar".to_string()
        ]
    );
}

#[test]
fn store_descendants_of_path_returns_none_for_unknown_path() {
    let store = Store::new();
    assert!(store.descendants_of_path("no/such>path").is_none());
}

#[test]
fn store_descendants_of_path_returns_empty_vec_for_leaf_node() {
    let mut store = Store::new();
    store.upsert_node(path("src/lib.rs"));
    store.upsert_node(path("src/lib.rs>leaf"));

    let desc = store
        .descendants_of_path("src/lib.rs>leaf")
        .expect("leaf node is materialized");
    assert!(desc.is_empty(), "a leaf node has no descendants");
}

// ──────────────────────────────────────────────────────────────────────
// save / load (persistence round-trip)
// ──────────────────────────────────────────────────────────────────────

#[test]
fn store_save_creates_snapshot_file() {
    let tmp = tempfile::tempdir().unwrap();
    let snap = tmp.path().join(".mycelium").join("index.rmp");

    let mut store = Store::new();
    store.upsert_node(path("src/lib.rs"));
    store.upsert_node(path("src/lib.rs>hello"));
    store.save(&snap).expect("save must succeed");

    assert!(snap.exists(), "snapshot file must be created");
    assert!(
        snap.metadata().unwrap().len() > 0,
        "snapshot must not be empty"
    );
}

#[test]
fn store_load_roundtrips_nodes() {
    let tmp = tempfile::tempdir().unwrap();
    let snap = tmp.path().join("index.rmp");

    let mut store = Store::new();
    let file_id = store.upsert_node(path("src/lib.rs"));
    let fn_id = store.upsert_node(path("src/lib.rs>hello"));
    store.upsert_edge(EdgeKind::Contains, file_id, fn_id);
    store.save(&snap).expect("save must succeed");

    let loaded = Store::load(&snap).expect("load must succeed");
    assert_eq!(
        loaded.lookup("src/lib.rs"),
        Some(file_id),
        "file node must survive round-trip"
    );
    assert_eq!(
        loaded.lookup("src/lib.rs>hello"),
        Some(fn_id),
        "function node must survive round-trip"
    );
}

#[test]
fn store_load_roundtrips_edges() {
    let tmp = tempfile::tempdir().unwrap();
    let snap = tmp.path().join("index.rmp");

    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>A"));
    let b = store.upsert_node(path("src/b.rs>B"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.save(&snap).expect("save must succeed");

    let loaded = Store::load(&snap).expect("load must succeed");
    assert_eq!(
        loaded.outgoing(a, EdgeKind::Calls),
        &[b],
        "calls edge must survive round-trip"
    );
    assert_eq!(
        loaded.incoming(b, EdgeKind::Calls),
        &[a],
        "reverse edge must survive round-trip"
    );
}

#[test]
fn store_load_error_on_missing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let no_such = tmp.path().join("does_not_exist.rmp");
    assert!(
        Store::load(&no_such).is_err(),
        "loading missing file must fail"
    );
}

#[test]
fn store_save_creates_parent_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    let nested = tmp
        .path()
        .join("deep")
        .join("nesting")
        .join(".mycelium")
        .join("index.rmp");

    let store = Store::new();
    store.save(&nested).expect("save must create parent dirs");
    assert!(nested.exists());
}

#[test]
fn store_delegates_ancestors_and_descendants() {
    let mut store = Store::new();
    let file = store.upsert_node(path("src/auth.rs"));
    let cls = store.upsert_node(path("src/auth.rs>AuthService"));
    let method = store.upsert_node(path("src/auth.rs>AuthService>login"));

    let anc: Vec<_> = store.ancestors(method).collect();
    assert!(anc.contains(&cls), "ancestors must include parent class");
    assert!(
        anc.contains(&file),
        "ancestors must include grandparent file"
    );

    let desc: Vec<_> = store.descendants(file).collect();
    assert!(
        desc.contains(&cls),
        "descendants of file must include class"
    );
    assert!(
        desc.contains(&method),
        "descendants of file must include method"
    );
}

// ── RFC-0010: Store::edge_count ───────────────────────────────────────

#[test]
fn store_edge_count_empty() {
    assert_eq!(Store::new().edge_count(), 0);
}

#[test]
fn store_edge_count_counts_synapse_edges() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs"));
    let b = store.upsert_node(path("b.rs"));
    let c = store.upsert_node(path("c.rs"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, a, c);
    store.upsert_edge(EdgeKind::Imports, b, c);
    assert_eq!(store.edge_count(), 3);
}

#[test]
fn store_edge_count_excludes_contains_edges() {
    // Contains edges live in Trunk, not Synapse; edge_count() counts Synapse only.
    let mut store = Store::new();
    let _file = store.upsert_node(path("a.rs"));
    let _func = store.upsert_node(path("a.rs>foo"));
    // No explicit synapse edge added — Contains is implicit in Trunk.
    assert_eq!(store.edge_count(), 0);
}
