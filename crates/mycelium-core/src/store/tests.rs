//! Store integration tests — written before implementation per Charter §5.1.
//!
//! Each test maps to an acceptance criterion from RFC-0001 §Public API sketch
//! or §Testing strategy.

use super::Store;
use crate::trunk::TrunkPath;
use crate::types::{EdgeKind, NodeId, NodeKind, SourceSpan};

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

// ── RFC-0017: Store::find_call_path ──────────────────────────────────

#[test]
fn store_find_call_path_direct() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let b = store.upsert_node(path("b.rs>B"));
    store.upsert_edge(EdgeKind::Calls, a, b);

    let result = store.find_call_path(a, b, 10);
    assert_eq!(result, Some(vec![a, b]), "direct call must return [A, B]");
}

#[test]
fn store_find_call_path_transitive() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let b = store.upsert_node(path("b.rs>B"));
    let c = store.upsert_node(path("c.rs>C"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);

    let result = store.find_call_path(a, c, 10);
    assert_eq!(
        result,
        Some(vec![a, b, c]),
        "transitive call must return [A, B, C]"
    );
}

#[test]
fn store_find_call_path_no_path() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let b = store.upsert_node(path("b.rs>B"));
    // No edge A → B

    let result = store.find_call_path(a, b, 10);
    assert_eq!(result, None, "no path should return None");
}

#[test]
fn store_find_call_path_cycle_safe() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let b = store.upsert_node(path("b.rs>B"));
    let c = store.upsert_node(path("c.rs>C"));
    // Cycle: A → B → A, but C is unreachable
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, a);

    let result = store.find_call_path(a, c, 10);
    assert_eq!(result, None, "cycle must not cause infinite loop");
}

#[test]
fn store_find_call_path_respects_max_depth() {
    // Chain: A → B → C → D; max_depth=1 cannot reach D
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let b = store.upsert_node(path("b.rs>B"));
    let c = store.upsert_node(path("c.rs>C"));
    let d = store.upsert_node(path("d.rs>D"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    store.upsert_edge(EdgeKind::Calls, c, d);

    assert_eq!(
        store.find_call_path(a, d, 2),
        None,
        "depth 2 should not reach D (needs 3 hops)"
    );
    assert_eq!(
        store.find_call_path(a, d, 3),
        Some(vec![a, b, c, d]),
        "depth 3 should reach D"
    );
}

#[test]
fn store_find_call_path_same_node() {
    // From and to are the same — trivially reachable in 0 hops
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let result = store.find_call_path(a, a, 10);
    assert_eq!(result, Some(vec![a]), "same node should return [A]");
}

// ── RFC-0027: Store::find_import_path ────────────────────────────────

#[test]
fn store_find_import_path_direct() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs"));
    let b = store.upsert_node(path("b.rs"));
    store.upsert_edge(EdgeKind::Imports, a, b);
    assert_eq!(
        store.find_import_path(a, b, 10),
        Some(vec![a, b]),
        "direct import must return [a, b]"
    );
}

#[test]
fn store_find_import_path_transitive() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs"));
    let b = store.upsert_node(path("b.rs"));
    let c = store.upsert_node(path("c.rs"));
    store.upsert_edge(EdgeKind::Imports, a, b);
    store.upsert_edge(EdgeKind::Imports, b, c);
    assert_eq!(
        store.find_import_path(a, c, 10),
        Some(vec![a, b, c]),
        "transitive path must include intermediary"
    );
}

#[test]
fn store_find_import_path_no_path() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs"));
    let b = store.upsert_node(path("b.rs"));
    assert_eq!(
        store.find_import_path(a, b, 10),
        None,
        "no path returns None"
    );
}

#[test]
fn store_find_import_path_cycle_safe() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs"));
    let b = store.upsert_node(path("b.rs"));
    let c = store.upsert_node(path("c.rs"));
    store.upsert_edge(EdgeKind::Imports, a, b);
    store.upsert_edge(EdgeKind::Imports, b, a); // cycle
    assert_eq!(
        store.find_import_path(a, c, 10),
        None,
        "cycle must not loop"
    );
}

#[test]
fn store_find_import_path_respects_max_depth() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs"));
    let b = store.upsert_node(path("b.rs"));
    let c = store.upsert_node(path("c.rs"));
    let d = store.upsert_node(path("d.rs"));
    store.upsert_edge(EdgeKind::Imports, a, b);
    store.upsert_edge(EdgeKind::Imports, b, c);
    store.upsert_edge(EdgeKind::Imports, c, d);
    assert_eq!(
        store.find_import_path(a, d, 2),
        None,
        "depth 2 cannot reach d (needs 3 hops)"
    );
    assert_eq!(
        store.find_import_path(a, d, 3),
        Some(vec![a, b, c, d]),
        "depth 3 reaches d"
    );
}

#[test]
fn store_find_import_path_same_node() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs"));
    assert_eq!(
        store.find_import_path(a, a, 10),
        Some(vec![a]),
        "same node trivially reachable"
    );
}

// ── RFC-0014: Store::resolve_bare_call_stubs ──────────────────────────

#[test]
fn store_resolve_bare_stubs_resolves_unambiguous_stub() {
    // foo in a.py calls bar; bar is defined in b.py.
    // After resolving stubs, the Calls edge must point to b.py>bar.
    let mut store = Store::new();
    let foo = store.upsert_node(path("a.py>foo"));
    let stub = store.upsert_node(TrunkPath::parse("bar").unwrap());
    let def = store.upsert_node(path("b.py>bar"));
    store.upsert_edge(EdgeKind::Calls, foo, stub);

    let resolved = store.resolve_bare_call_stubs();

    assert_eq!(resolved, 1, "one stub should be resolved");
    assert_eq!(
        store.lookup("bar"),
        None,
        "bare stub node must be removed after resolution"
    );
    assert!(
        store.outgoing(foo, EdgeKind::Calls).contains(&def),
        "Calls edge must point to definition node"
    );
    assert!(
        !store.outgoing(foo, EdgeKind::Calls).contains(&stub),
        "Calls edge must not still point to stub"
    );
}

#[test]
fn store_resolve_bare_stubs_reverse_edge_rewired() {
    // Callers of b.py>bar must include a.py>foo after resolution.
    let mut store = Store::new();
    let foo = store.upsert_node(path("a.py>foo"));
    let stub = store.upsert_node(TrunkPath::parse("bar").unwrap());
    let def = store.upsert_node(path("b.py>bar"));
    store.upsert_edge(EdgeKind::Calls, foo, stub);
    let _ = stub; // suppress warning

    store.resolve_bare_call_stubs();

    assert!(
        store.incoming(def, EdgeKind::Calls).contains(&foo),
        "reverse Calls edge must point to a.py>foo after resolution"
    );
}

#[test]
fn store_resolve_bare_stubs_ambiguous_left_unchanged() {
    // Two definitions with the same simple name — stub must stay.
    let mut store = Store::new();
    let foo = store.upsert_node(path("a.py>foo"));
    let stub = store.upsert_node(TrunkPath::parse("bar").unwrap());
    store.upsert_node(path("b.py>bar"));
    store.upsert_node(path("c.py>bar"));
    store.upsert_edge(EdgeKind::Calls, foo, stub);

    let resolved = store.resolve_bare_call_stubs();

    assert_eq!(resolved, 0, "ambiguous stub must not be resolved");
    assert!(
        store.lookup("bar").is_some(),
        "ambiguous stub node must remain in store"
    );
}

#[test]
fn store_resolve_bare_stubs_no_match_left_unchanged() {
    // Stub with no matching definition (external/stdlib call) — must stay.
    let mut store = Store::new();
    let foo = store.upsert_node(path("a.py>foo"));
    let stub = store.upsert_node(TrunkPath::parse("os").unwrap());
    store.upsert_edge(EdgeKind::Calls, foo, stub);

    let resolved = store.resolve_bare_call_stubs();

    assert_eq!(resolved, 0, "unresolvable stub must not be resolved");
    assert!(
        store.lookup("os").is_some(),
        "unresolvable stub node must remain in store"
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

// ── RFC-0018: Store::all_file_paths ──────────────────────────────────

#[test]
fn store_all_file_paths_returns_only_file_nodes() {
    let mut store = Store::new();
    store.upsert_node(path("src/auth.rs"));
    store.upsert_node(path("src/auth.rs>AuthService"));
    store.upsert_node(path("src/main.rs"));
    store.upsert_node(path("src/main.rs>main"));
    let files = store.all_file_paths();
    // Only file-level paths (no `>`) should be returned.
    assert!(
        files.contains(&"src/auth.rs".to_string()),
        "auth.rs must be listed"
    );
    assert!(
        files.contains(&"src/main.rs".to_string()),
        "main.rs must be listed"
    );
    assert!(
        !files.iter().any(|p| p.contains('>')),
        "symbol-level paths must not appear in file list"
    );
}

#[test]
fn store_all_file_paths_returns_sorted_order() {
    let mut store = Store::new();
    store.upsert_node(path("z.rs"));
    store.upsert_node(path("a.rs"));
    store.upsert_node(path("m.rs"));
    let files = store.all_file_paths();
    let mut sorted = files.clone();
    sorted.sort_unstable();
    assert_eq!(files, sorted, "all_file_paths must return sorted results");
}

#[test]
fn store_all_file_paths_empty_when_only_symbols() {
    let mut store = Store::new();
    // Insert only symbol-level nodes, no file-level nodes.
    store.upsert_node(path("src/lib.rs>Foo"));
    store.upsert_node(path("src/lib.rs>Bar"));
    let files = store.all_file_paths();
    // Bare stubs and symbol nodes must not appear.
    assert!(
        files.is_empty(),
        "no file-level nodes means empty file list"
    );
}

// ── RFC-0019: Store::top_callee_symbols ──────────────────────────────

#[test]
fn store_top_callee_symbols_ranks_by_caller_count() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>a"));
    let b = store.upsert_node(path("b.rs>b"));
    let c = store.upsert_node(path("c.rs>c"));
    let d = store.upsert_node(path("d.rs>d"));
    // c is called by a, b, d (3 callers)
    // b is called by a, d   (2 callers)
    // d is called by a       (1 caller)
    store.upsert_edge(EdgeKind::Calls, a, c);
    store.upsert_edge(EdgeKind::Calls, b, c);
    store.upsert_edge(EdgeKind::Calls, d, c);
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, d, b);
    store.upsert_edge(EdgeKind::Calls, a, d);

    let ranked = store.top_callee_symbols(10);
    assert_eq!(
        ranked[0],
        ("c.rs>c".to_string(), 3),
        "c must be ranked first"
    );
    assert_eq!(
        ranked[1],
        ("b.rs>b".to_string(), 2),
        "b must be ranked second"
    );
    assert_eq!(
        ranked[2],
        ("d.rs>d".to_string(), 1),
        "d must be ranked third"
    );
    // a has no callers, must not appear
    assert!(
        !ranked.iter().any(|(p, _)| p == "a.rs>a"),
        "a has no callers and must be excluded"
    );
}

#[test]
fn store_top_callee_symbols_respects_limit() {
    let mut store = Store::new();
    for i in 0..5usize {
        let src = store.upsert_node(path(&format!("s{i}.rs>s{i}")));
        let dst = store.upsert_node(path(&format!("d{i}.rs>d{i}")));
        store.upsert_edge(EdgeKind::Calls, src, dst);
    }
    let ranked = store.top_callee_symbols(3);
    assert_eq!(ranked.len(), 3, "limit must be respected");
}

#[test]
fn store_top_callee_symbols_empty_when_no_edges() {
    let mut store = Store::new();
    store.upsert_node(path("a.rs>a"));
    let ranked = store.top_callee_symbols(10);
    assert!(ranked.is_empty(), "no call edges means empty ranking");
}

#[test]
fn store_top_callee_symbols_breaks_ties_by_path() {
    let mut store = Store::new();
    let caller = store.upsert_node(path("caller.rs>f"));
    let z = store.upsert_node(path("z.rs>z"));
    let a = store.upsert_node(path("a.rs>a"));
    // Both z and a have exactly 1 caller.
    store.upsert_edge(EdgeKind::Calls, caller, z);
    store.upsert_edge(EdgeKind::Calls, caller, a);

    let ranked = store.top_callee_symbols(10);
    assert_eq!(ranked.len(), 2, "both tied symbols must appear");
    assert_eq!(ranked[0].0, "a.rs>a", "a.rs must sort before z.rs on tie");
    assert_eq!(ranked[1].0, "z.rs>z", "z.rs must sort after a.rs on tie");
}

// ── RFC-0020: Store::callee_tree ─────────────────────────────────────

#[test]
fn store_callee_tree_direct_children() {
    let mut store = Store::new();
    let root = store.upsert_node(path("a.rs>root"));
    let child1 = store.upsert_node(path("b.rs>child1"));
    let child2 = store.upsert_node(path("c.rs>child2"));
    store.upsert_edge(EdgeKind::Calls, root, child1);
    store.upsert_edge(EdgeKind::Calls, root, child2);

    let tree = store.callee_tree(root, 4);
    assert_eq!(tree.id, root);
    assert_eq!(tree.children.len(), 2);
    let child_ids: Vec<NodeId> = tree.children.iter().map(|c| c.id).collect();
    assert!(child_ids.contains(&child1));
    assert!(child_ids.contains(&child2));
}

#[test]
fn store_callee_tree_transitive() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>a"));
    let b = store.upsert_node(path("b.rs>b"));
    let c = store.upsert_node(path("c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);

    let tree = store.callee_tree(a, 4);
    assert_eq!(tree.children.len(), 1);
    assert_eq!(tree.children[0].id, b);
    assert_eq!(tree.children[0].children.len(), 1);
    assert_eq!(tree.children[0].children[0].id, c);
    assert!(tree.children[0].children[0].children.is_empty());
}

#[test]
fn store_callee_tree_max_depth_respected() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>a"));
    let b = store.upsert_node(path("b.rs>b"));
    let c = store.upsert_node(path("c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);

    // With max_depth=1 only direct children; c must not appear.
    let tree = store.callee_tree(a, 1);
    assert_eq!(tree.children.len(), 1, "a must have b as child");
    assert!(
        tree.children[0].children.is_empty(),
        "b's children must be empty at depth limit"
    );
}

#[test]
fn store_callee_tree_cycle_safe() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>a"));
    let b = store.upsert_node(path("b.rs>b"));
    // a → b → a cycle
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, a);

    // Must not recurse infinitely.  DFS path tracking: a → b → a(leaf).
    let tree = store.callee_tree(a, 10);
    assert_eq!(tree.children.len(), 1, "a must have b as child");
    assert_eq!(tree.children[0].id, b);
    // b → a: a is still in the current DFS path, so a appears as a leaf child of b.
    assert_eq!(
        tree.children[0].children.len(),
        1,
        "b must have a as a leaf child (cycle)"
    );
    assert_eq!(tree.children[0].children[0].id, a);
    assert!(
        tree.children[0].children[0].children.is_empty(),
        "cycle node a must have no further children"
    );
}

#[test]
fn store_callee_tree_leaf_when_no_callees() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>a"));
    let tree = store.callee_tree(a, 4);
    assert_eq!(tree.id, a);
    assert!(tree.children.is_empty(), "leaf node has no children");
}

// ── RFC-0021: Store::caller_tree ─────────────────────────────────────

#[test]
fn store_caller_tree_direct_callers() {
    let mut store = Store::new();
    let root = store.upsert_node(path("b.rs>b"));
    let caller = store.upsert_node(path("a.rs>a"));
    store.upsert_edge(EdgeKind::Calls, caller, root);
    let tree = store.caller_tree(root, 4);
    assert_eq!(tree.id, root);
    assert_eq!(tree.callers.len(), 1);
    assert_eq!(tree.callers[0].id, caller);
    assert!(tree.callers[0].callers.is_empty());
}

#[test]
fn store_caller_tree_transitive() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>a"));
    let b = store.upsert_node(path("b.rs>b"));
    let c = store.upsert_node(path("c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    let tree = store.caller_tree(c, 4);
    assert_eq!(tree.id, c);
    assert_eq!(tree.callers.len(), 1);
    assert_eq!(tree.callers[0].id, b);
    assert_eq!(tree.callers[0].callers.len(), 1);
    assert_eq!(tree.callers[0].callers[0].id, a);
}

#[test]
fn store_caller_tree_max_depth_respected() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>a"));
    let b = store.upsert_node(path("b.rs>b"));
    let c = store.upsert_node(path("c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    // max_depth=1: only b appears; a is absent
    let tree = store.caller_tree(c, 1);
    assert_eq!(tree.callers.len(), 1);
    assert_eq!(tree.callers[0].id, b);
    assert!(tree.callers[0].callers.is_empty(), "depth limit cuts off a");
}

#[test]
fn store_caller_tree_cycle_safe() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>a"));
    let b = store.upsert_node(path("b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, a); // cycle
    // caller_tree of b: a calls b, a is also called by b (cycle)
    let tree = store.caller_tree(b, 10);
    assert_eq!(tree.id, b);
    assert_eq!(tree.callers.len(), 1);
    assert_eq!(tree.callers[0].id, a);
    // a's callers include b, but b is already in path → leaf
    assert_eq!(tree.callers[0].callers.len(), 1);
    assert_eq!(tree.callers[0].callers[0].id, b);
    assert!(
        tree.callers[0].callers[0].callers.is_empty(),
        "cycle produces leaf"
    );
}

#[test]
fn store_caller_tree_leaf_when_no_callers() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>a"));
    let tree = store.caller_tree(a, 4);
    assert_eq!(tree.id, a);
    assert!(tree.callers.is_empty(), "root caller has no callers");
}

// ── RFC-0022: Store::entry_points ────────────────────────────────────

#[test]
fn store_entry_points_returns_zero_caller_symbols() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>a")); // zero callers
    let b = store.upsert_node(path("b.rs>b")); // called by a
    store.upsert_edge(EdgeKind::Calls, a, b);
    let eps = store.entry_points(None);
    assert!(eps.contains(&"a.rs>a".to_string()), "a has no callers");
    assert!(!eps.contains(&"b.rs>b".to_string()), "b is called by a");
    let _ = a; // used in the edge
}

#[test]
fn store_entry_points_excludes_file_nodes() {
    let mut store = Store::new();
    store.upsert_node(path("a.rs")); // file node, no >
    store.upsert_node(path("a.rs>a")); // symbol node
    let eps = store.entry_points(None);
    assert!(!eps.contains(&"a.rs".to_string()), "file nodes excluded");
    assert!(eps.contains(&"a.rs>a".to_string()), "symbol node included");
}

#[test]
fn store_entry_points_prefix_filter() {
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs>a"));
    store.upsert_node(path("tests/t.rs>test_foo"));
    let eps = store.entry_points(Some("src/"));
    assert!(eps.contains(&"src/a.rs>a".to_string()));
    assert!(!eps.contains(&"tests/t.rs>test_foo".to_string()));
}

#[test]
fn store_entry_points_sorted_lexicographically() {
    let mut store = Store::new();
    store.upsert_node(path("z.rs>z"));
    store.upsert_node(path("a.rs>a"));
    store.upsert_node(path("m.rs>m"));
    let eps = store.entry_points(None);
    let mut sorted = eps.clone();
    sorted.sort_unstable();
    assert_eq!(eps, sorted, "results must be sorted");
}

#[test]
fn store_entry_points_empty_graph() {
    let store = Store::new();
    assert!(store.entry_points(None).is_empty());
}

// ── RFC-0023: Store::imports_of / imported_by ─────────────────────────

#[test]
fn store_imports_of_returns_import_targets() {
    let mut store = Store::new();
    let file = store.upsert_node(path("src/a.rs"));
    let dep = store.upsert_node(path("os"));
    store.upsert_edge(EdgeKind::Imports, file, dep);
    let imports = store.imports_of(file);
    assert_eq!(imports, vec!["os".to_string()]);
}

#[test]
fn store_imported_by_returns_import_sources() {
    let mut store = Store::new();
    let file_a = store.upsert_node(path("src/a.rs"));
    let file_b = store.upsert_node(path("src/b.rs"));
    let dep = store.upsert_node(path("os"));
    store.upsert_edge(EdgeKind::Imports, file_a, dep);
    store.upsert_edge(EdgeKind::Imports, file_b, dep);
    let mut importers = store.imported_by(dep);
    importers.sort_unstable();
    assert_eq!(
        importers,
        vec!["src/a.rs".to_string(), "src/b.rs".to_string()]
    );
}

#[test]
fn store_imports_of_empty_when_no_imports() {
    let mut store = Store::new();
    let file = store.upsert_node(path("src/a.rs"));
    assert!(store.imports_of(file).is_empty());
}

#[test]
fn store_imports_sorted_lexicographically() {
    let mut store = Store::new();
    let file = store.upsert_node(path("src/a.rs"));
    let z_mod = store.upsert_node(path("z_mod"));
    let a_mod = store.upsert_node(path("a_mod"));
    store.upsert_edge(EdgeKind::Imports, file, z_mod);
    store.upsert_edge(EdgeKind::Imports, file, a_mod);
    let imports = store.imports_of(file);
    let mut sorted = imports.clone();
    sorted.sort_unstable();
    assert_eq!(imports, sorted);
}

// ── RFC-0024: Store::import_tree ─────────────────────────────────────

#[test]
fn store_import_tree_direct_imports() {
    let mut store = Store::new();
    let root = store.upsert_node(path("src/a.rs"));
    let dep = store.upsert_node(path("os"));
    store.upsert_edge(EdgeKind::Imports, root, dep);
    let tree = store.import_tree(root, 4);
    assert_eq!(tree.id, root);
    assert_eq!(tree.imports.len(), 1);
    assert_eq!(tree.imports[0].id, dep);
    assert!(tree.imports[0].imports.is_empty());
}

#[test]
fn store_import_tree_transitive() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs"));
    let b = store.upsert_node(path("b.rs"));
    let c = store.upsert_node(path("c.rs"));
    store.upsert_edge(EdgeKind::Imports, a, b);
    store.upsert_edge(EdgeKind::Imports, b, c);
    let tree = store.import_tree(a, 4);
    assert_eq!(tree.imports.len(), 1);
    assert_eq!(tree.imports[0].id, b);
    assert_eq!(tree.imports[0].imports.len(), 1);
    assert_eq!(tree.imports[0].imports[0].id, c);
}

#[test]
fn store_import_tree_max_depth_respected() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs"));
    let b = store.upsert_node(path("b.rs"));
    let c = store.upsert_node(path("c.rs"));
    store.upsert_edge(EdgeKind::Imports, a, b);
    store.upsert_edge(EdgeKind::Imports, b, c);
    let tree = store.import_tree(a, 1);
    assert_eq!(tree.imports.len(), 1);
    assert_eq!(tree.imports[0].id, b);
    assert!(tree.imports[0].imports.is_empty(), "depth limit cuts off c");
}

#[test]
fn store_import_tree_cycle_safe() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs"));
    let b = store.upsert_node(path("b.rs"));
    store.upsert_edge(EdgeKind::Imports, a, b);
    store.upsert_edge(EdgeKind::Imports, b, a); // cycle
    let tree = store.import_tree(a, 10);
    assert_eq!(tree.imports.len(), 1);
    assert_eq!(tree.imports[0].id, b);
    // b imports a, but a is in path → leaf
    assert_eq!(tree.imports[0].imports.len(), 1);
    assert_eq!(tree.imports[0].imports[0].id, a);
    assert!(
        tree.imports[0].imports[0].imports.is_empty(),
        "cycle produces leaf"
    );
}

#[test]
fn store_import_tree_leaf_when_no_imports() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs"));
    let tree = store.import_tree(a, 4);
    assert_eq!(tree.id, a);
    assert!(tree.imports.is_empty());
}

// ── RFC-0028: Store::set_kind / kind_of / symbols_of_kind ─────────────

#[test]
fn store_set_and_get_kind() {
    let mut store = Store::new();
    let id = store.upsert_node(path("src/lib.rs>Foo"));
    store.set_kind(id, NodeKind::Class);
    assert_eq!(store.kind_of(id), Some(NodeKind::Class));
}

#[test]
fn store_kind_of_unknown_node_is_none() {
    let store = Store::new();
    assert_eq!(store.kind_of(NodeId(0xdead_beef_0000_0000)), None);
}

#[test]
fn store_symbols_of_kind_returns_matching() {
    let mut store = Store::new();
    let f1 = store.upsert_node(path("src/a.rs>foo"));
    let f2 = store.upsert_node(path("src/b.rs>bar"));
    let c1 = store.upsert_node(path("src/c.rs>Baz"));
    store.set_kind(f1, NodeKind::Function);
    store.set_kind(f2, NodeKind::Function);
    store.set_kind(c1, NodeKind::Class);
    let fns = store.symbols_of_kind(NodeKind::Function, None);
    assert_eq!(fns.len(), 2);
    assert!(fns.contains(&"src/a.rs>foo".to_string()));
    assert!(fns.contains(&"src/b.rs>bar".to_string()));
    let classes = store.symbols_of_kind(NodeKind::Class, None);
    assert_eq!(classes, vec!["src/c.rs>Baz"]);
}

#[test]
fn store_symbols_of_kind_with_prefix_filter() {
    let mut store = Store::new();
    let f1 = store.upsert_node(path("src/auth.rs>login"));
    let f2 = store.upsert_node(path("tests/test_auth.rs>test_login"));
    store.set_kind(f1, NodeKind::Function);
    store.set_kind(f2, NodeKind::Function);
    let src_only = store.symbols_of_kind(NodeKind::Function, Some("src/"));
    assert_eq!(src_only, vec!["src/auth.rs>login"]);
}

#[test]
fn store_symbols_of_kind_empty_when_none_match() {
    let mut store = Store::new();
    let id = store.upsert_node(path("src/lib.rs>Foo"));
    store.set_kind(id, NodeKind::Class);
    let fns = store.symbols_of_kind(NodeKind::Function, None);
    assert!(fns.is_empty());
}

#[test]
fn store_symbols_of_kind_sorted_lexicographically() {
    let mut store = Store::new();
    let b = store.upsert_node(path("b.rs>fn2"));
    let a = store.upsert_node(path("a.rs>fn1"));
    store.set_kind(b, NodeKind::Function);
    store.set_kind(a, NodeKind::Function);
    let result = store.symbols_of_kind(NodeKind::Function, None);
    assert_eq!(result, vec!["a.rs>fn1", "b.rs>fn2"]);
}

// ── RFC-0029: Store::set_span / span_of ──────────────────────────────

fn span(sl: u32, sc: u32, el: u32, ec: u32, sb: u32, eb: u32) -> SourceSpan {
    SourceSpan {
        start_line: sl,
        start_col: sc,
        end_line: el,
        end_col: ec,
        start_byte: sb,
        end_byte: eb,
    }
}

#[test]
fn store_set_span_stores_and_retrieves() {
    let mut store = Store::new();
    let id = store.upsert_node(path("src/lib.rs>login"));
    let s = span(10, 0, 20, 1, 100, 250);
    store.set_span(id, s);
    assert_eq!(store.span_of(id), Some(s));
}

#[test]
fn store_span_of_returns_none_when_unset() {
    let mut store = Store::new();
    let id = store.upsert_node(path("src/lib.rs>login"));
    assert_eq!(store.span_of(id), None);
}

#[test]
fn store_set_span_overwrites_previous() {
    let mut store = Store::new();
    let id = store.upsert_node(path("src/lib.rs>Foo"));
    store.set_span(id, span(1, 0, 5, 1, 0, 50));
    store.set_span(id, span(2, 0, 8, 1, 50, 120));
    assert_eq!(store.span_of(id), Some(span(2, 0, 8, 1, 50, 120)));
}

#[test]
fn store_remove_node_clears_span() {
    let mut store = Store::new();
    let id = store.upsert_node(path("src/lib.rs>Foo"));
    store.set_span(id, span(1, 0, 5, 1, 0, 50));
    store.remove_node(id);
    assert_eq!(store.span_of(id), None);
}

#[test]
fn store_remove_file_clears_spans() {
    let mut store = Store::new();
    let file_id = store.upsert_node(path("src/auth.rs"));
    let sym_id = store.upsert_node(path("src/auth.rs>login"));
    store.set_span(file_id, span(1, 0, 40, 0, 0, 800));
    store.set_span(sym_id, span(5, 0, 10, 1, 80, 200));
    store.remove_file("src/auth.rs");
    assert_eq!(store.span_of(file_id), None);
    assert_eq!(store.span_of(sym_id), None);
}

#[test]
fn store_span_of_unknown_id_returns_none() {
    let store = Store::new();
    assert_eq!(store.span_of(NodeId(999_999)), None);
}

// ── RFC-0030: Store::find_extends_path ───────────────────────────────

#[test]
fn store_find_extends_path_self_returns_single_element() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>A"));
    assert_eq!(store.find_extends_path(a, a, 5), Some(vec![a]));
}

#[test]
fn store_find_extends_path_direct_hop() {
    let mut store = Store::new();
    let base = store.upsert_node(path("src/base.rs>Base"));
    let child = store.upsert_node(path("src/child.rs>Child"));
    store.upsert_edge(EdgeKind::Extends, child, base);
    assert_eq!(
        store.find_extends_path(child, base, 5),
        Some(vec![child, base])
    );
}

#[test]
fn store_find_extends_path_transitive() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let b = store.upsert_node(path("b.rs>B"));
    let c = store.upsert_node(path("c.rs>C"));
    store.upsert_edge(EdgeKind::Extends, a, b);
    store.upsert_edge(EdgeKind::Extends, b, c);
    let result = store.find_extends_path(a, c, 5);
    assert_eq!(result, Some(vec![a, b, c]));
}

#[test]
fn store_find_extends_path_unreachable_returns_none() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let b = store.upsert_node(path("b.rs>B"));
    assert_eq!(store.find_extends_path(a, b, 5), None);
}

#[test]
fn store_find_extends_path_max_depth_limits_hops() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let b = store.upsert_node(path("b.rs>B"));
    let c = store.upsert_node(path("c.rs>C"));
    store.upsert_edge(EdgeKind::Extends, a, b);
    store.upsert_edge(EdgeKind::Extends, b, c);
    // max_depth=1 means at most 1 hop — A→B is reachable, A→C is not
    assert!(store.find_extends_path(a, b, 1).is_some());
    assert!(store.find_extends_path(a, c, 1).is_none());
}

// ── RFC-0031: Store::extends_tree ────────────────────────────────────

#[test]
fn store_extends_tree_leaf_at_max_depth_zero() {
    let mut store = Store::new();
    let id = store.upsert_node(path("src/a.rs>A"));
    let tree = store.extends_tree(id, 0);
    assert_eq!(tree.id, id);
    assert!(tree.parents.is_empty());
}

#[test]
fn store_extends_tree_single_parent() {
    let mut store = Store::new();
    let child = store.upsert_node(path("src/child.rs>Child"));
    let parent = store.upsert_node(path("src/parent.rs>Parent"));
    store.upsert_edge(EdgeKind::Extends, child, parent);
    let tree = store.extends_tree(child, 4);
    assert_eq!(tree.id, child);
    assert_eq!(tree.parents.len(), 1);
    assert_eq!(tree.parents[0].id, parent);
    assert!(tree.parents[0].parents.is_empty());
}

#[test]
fn store_extends_tree_transitive_chain() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let b = store.upsert_node(path("b.rs>B"));
    let c = store.upsert_node(path("c.rs>C"));
    store.upsert_edge(EdgeKind::Extends, a, b);
    store.upsert_edge(EdgeKind::Extends, b, c);
    let tree = store.extends_tree(a, 4);
    assert_eq!(tree.parents.len(), 1);
    assert_eq!(tree.parents[0].id, b);
    assert_eq!(tree.parents[0].parents.len(), 1);
    assert_eq!(tree.parents[0].parents[0].id, c);
}

#[test]
fn store_extends_tree_cycle_safe() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let b = store.upsert_node(path("b.rs>B"));
    store.upsert_edge(EdgeKind::Extends, a, b);
    store.upsert_edge(EdgeKind::Extends, b, a); // cycle
    let tree = store.extends_tree(a, 10);
    // A → B → A(leaf) — second visit to A is cut as a leaf
    assert_eq!(tree.parents.len(), 1);
    assert_eq!(tree.parents[0].id, b);
    // b's parent is a again, but a is already in the path so it becomes a leaf
    assert_eq!(tree.parents[0].parents.len(), 1);
    assert_eq!(tree.parents[0].parents[0].id, a);
    assert!(tree.parents[0].parents[0].parents.is_empty()); // cycle cut here
}

// ── RFC-0032: Store::subclasses_tree ────────────────────────────────────

#[test]
fn store_subclasses_tree_leaf_at_max_depth_zero() {
    let mut store = Store::new();
    let id = store.upsert_node(path("src/a.rs>A"));
    let tree = store.subclasses_tree(id, 0);
    assert_eq!(tree.id, id);
    assert!(tree.subclasses.is_empty());
}

#[test]
fn store_subclasses_tree_single_child() {
    let mut store = Store::new();
    let base = store.upsert_node(path("src/base.rs>Base"));
    let child = store.upsert_node(path("src/child.rs>Child"));
    store.upsert_edge(EdgeKind::Extends, child, base); // child extends base
    let tree = store.subclasses_tree(base, 4);
    assert_eq!(tree.id, base);
    assert_eq!(tree.subclasses.len(), 1);
    assert_eq!(tree.subclasses[0].id, child);
    assert!(tree.subclasses[0].subclasses.is_empty());
}

#[test]
fn store_subclasses_tree_transitive_chain() {
    let mut store = Store::new();
    let base = store.upsert_node(path("base.rs>Base"));
    let mid = store.upsert_node(path("mid.rs>Mid"));
    let leaf = store.upsert_node(path("leaf.rs>Leaf"));
    store.upsert_edge(EdgeKind::Extends, mid, base); // mid extends base
    store.upsert_edge(EdgeKind::Extends, leaf, mid); // leaf extends mid
    let tree = store.subclasses_tree(base, 4);
    assert_eq!(tree.subclasses.len(), 1);
    assert_eq!(tree.subclasses[0].id, mid);
    assert_eq!(tree.subclasses[0].subclasses.len(), 1);
    assert_eq!(tree.subclasses[0].subclasses[0].id, leaf);
}

#[test]
fn store_subclasses_tree_cycle_safe() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let b = store.upsert_node(path("b.rs>B"));
    store.upsert_edge(EdgeKind::Extends, b, a); // b extends a
    store.upsert_edge(EdgeKind::Extends, a, b); // a extends b (cycle)
    let tree = store.subclasses_tree(a, 10);
    // A ← B ← A(leaf) — second visit to A is cut
    assert_eq!(tree.subclasses.len(), 1);
    assert_eq!(tree.subclasses[0].id, b);
    assert_eq!(tree.subclasses[0].subclasses.len(), 1);
    assert_eq!(tree.subclasses[0].subclasses[0].id, a);
    assert!(tree.subclasses[0].subclasses[0].subclasses.is_empty());
}
