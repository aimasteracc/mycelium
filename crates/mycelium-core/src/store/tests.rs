//! Store integration tests — written before implementation per Charter §5.1.
//!
//! Each test maps to an acceptance criterion from RFC-0001 §Public API sketch
//! or §Testing strategy.

use super::Store;
use crate::trunk::TrunkPath;
use crate::types::{EdgeKind, NodeId};

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
