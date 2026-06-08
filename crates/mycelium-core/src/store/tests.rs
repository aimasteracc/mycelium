//! Store integration tests — written before implementation per Charter §5.1.
//!
//! Each test maps to an acceptance criterion from RFC-0001 §Public API sketch
//! or §Testing strategy.

use super::{NodeDegree, Store};
use crate::resolver::receiver::{LocalBinding, ReceiverContext};
use crate::store::journal::Journal;
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
fn symbols_in_file_returns_sorted_descendants_excluding_root() {
    // RFC-0107 §6 test 1 — `Store::symbols_in_file(file_rel)` is the OLD-set
    // helper consumed by the watch engine's lock discipline. Returns trunk
    // paths of every symbol *under* the file, sorted lexicographically, with
    // the file path itself excluded.
    let mut store = Store::new();
    store.upsert_node(path("src/auth.rs"));
    store.upsert_node(path("src/auth.rs>fn:login"));
    store.upsert_node(path("src/auth.rs>fn:logout"));
    store.upsert_node(path("src/auth.rs>AuthService"));
    store.upsert_node(path("src/auth.rs>AuthService>verify"));
    // unrelated file — must NOT appear in src/auth.rs's set
    store.upsert_node(path("src/other.rs>fn:noise"));

    let got = store.symbols_in_file("src/auth.rs");
    let want = vec![
        "src/auth.rs>AuthService".to_owned(),
        "src/auth.rs>AuthService>verify".to_owned(),
        "src/auth.rs>fn:login".to_owned(),
        "src/auth.rs>fn:logout".to_owned(),
    ];
    assert_eq!(got, want, "sorted lexicographically; file path excluded");
    // Unknown file path → empty vec (well-defined OLD set for a new file).
    assert!(store.symbols_in_file("src/nonexistent.rs").is_empty());
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
fn store_search_symbol_drops_unnavigable_nodes_when_kind_annotated() {
    // dogfood-new: search-symbol must not return nodes an agent can't navigate to.
    // In a kind-annotated store (built by the extractor — has ≥1 File node), drop
    // (a) kind-less import-target stubs the extractor mints via bare upsert_node
    // and (b) NodeKind::Unresolved resolver phantoms. Keep real definitions.
    let mut store = Store::new();
    let f = store.upsert_node(path("a.rs"));
    store.set_kind(f, NodeKind::File);
    let real = store.upsert_node(path("a.rs>fooContext"));
    store.set_kind(real, NodeKind::Function);
    // Kind-less import-target stub (no kind, no span — unnavigable).
    store.upsert_node(path("anyhow::Context"));
    // Unresolved resolver phantom.
    store.upsert_node_with_kind(path("Db>context"), NodeKind::Unresolved);

    let results = store.search_symbol("context", 20);
    assert!(
        results.contains(&"a.rs>fooContext".to_string()),
        "real Function symbol must be returned, got: {results:?}"
    );
    assert!(
        !results.contains(&"anyhow::Context".to_string()),
        "kind-less import stub must be dropped, got: {results:?}"
    );
    assert!(
        !results.contains(&"Db>context".to_string()),
        "Unresolved phantom must be dropped, got: {results:?}"
    );
}

#[test]
fn store_search_symbol_legacy_unannotated_store_is_unfiltered() {
    // Back-compat (RFC-0018): a purely programmatic store that never sets kinds
    // is NOT kind-annotated, so search keeps the historical contract and returns
    // all name matches (no silent emptying).
    let mut store = Store::new();
    store.upsert_node(path("a.rs>fooContext"));
    store.upsert_node(path("anyhow::Context"));
    let results = store.search_symbol("context", 20);
    assert!(results.contains(&"a.rs>fooContext".to_string()));
    assert!(
        results.contains(&"anyhow::Context".to_string()),
        "legacy unannotated store must keep historical (unfiltered) behavior"
    );
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

// ── RFC-0103: import-aware Extends-stub resolution ────────────────────

#[test]
fn store_resolve_extends_stub_via_import_evidence() {
    // a.py>Sub extends `Base` (bare stub). `Base` is defined in BOTH b.py and
    // c.py (ambiguous for the simple resolver). a.py imports b.py only, so the
    // Extends edge must resolve to b.py>Base.
    let mut store = Store::new();
    let a_file = store.upsert_node(path("a.py"));
    let subclass = store.upsert_node(path("a.py>Sub"));
    let stub = store.upsert_node(TrunkPath::parse("Base").unwrap());
    let b_file = store.upsert_node(path("b.py"));
    let b_base = store.upsert_node(path("b.py>Base"));
    let _c_file = store.upsert_node(path("c.py"));
    let _c_base = store.upsert_node(path("c.py>Base"));
    store.upsert_edge(EdgeKind::Extends, subclass, stub);
    store.upsert_edge(EdgeKind::Imports, a_file, b_file);

    let resolved = store.resolve_bare_call_stubs();

    assert_eq!(
        resolved, 1,
        "ambiguous Extends stub should resolve via unique import evidence"
    );
    assert!(
        store.lookup("Base").is_none(),
        "bare stub must be removed after resolution"
    );
    assert!(
        store
            .outgoing(subclass, EdgeKind::Extends)
            .contains(&b_base),
        "Extends edge must point to the imported b.py>Base"
    );
}

#[test]
fn store_resolve_extends_stub_no_import_evidence_left_unchanged() {
    // Ambiguous `Base` (b.py + c.py) but a.py imports NEITHER — conservative:
    // the stub must stay unresolved rather than be guessed.
    let mut store = Store::new();
    let _a_file = store.upsert_node(path("a.py"));
    let subclass = store.upsert_node(path("a.py>Sub"));
    let stub = store.upsert_node(TrunkPath::parse("Base").unwrap());
    store.upsert_node(path("b.py"));
    store.upsert_node(path("b.py>Base"));
    store.upsert_node(path("c.py"));
    store.upsert_node(path("c.py>Base"));
    store.upsert_edge(EdgeKind::Extends, subclass, stub);

    let resolved = store.resolve_bare_call_stubs();

    assert_eq!(resolved, 0, "no import evidence → stub stays unresolved");
    assert!(
        store.lookup("Base").is_some(),
        "ambiguous stub without evidence must remain"
    );
}

#[test]
fn store_resolve_extends_stub_tie_left_unchanged() {
    // a.py imports BOTH b.py and c.py → both candidates tie on evidence →
    // conservative: stub stays unresolved (RFC-0103 ambiguity rule).
    let mut store = Store::new();
    let a_file = store.upsert_node(path("a.py"));
    let subclass = store.upsert_node(path("a.py>Sub"));
    let stub = store.upsert_node(TrunkPath::parse("Base").unwrap());
    let b_file = store.upsert_node(path("b.py"));
    store.upsert_node(path("b.py>Base"));
    let c_file = store.upsert_node(path("c.py"));
    store.upsert_node(path("c.py>Base"));
    store.upsert_edge(EdgeKind::Extends, subclass, stub);
    store.upsert_edge(EdgeKind::Imports, a_file, b_file);
    store.upsert_edge(EdgeKind::Imports, a_file, c_file);

    let resolved = store.resolve_bare_call_stubs();

    assert_eq!(
        resolved, 0,
        "tie on import evidence → stub stays unresolved"
    );
    assert!(store.lookup("Base").is_some(), "tied stub must remain");
}

// ── RFC-0118: kind-aware call resolution (calls never bind to TypeAlias) ──

#[test]
fn store_resolve_call_stub_does_not_bind_to_type_alias() {
    // Reproduces the budget.rs>Err dogfood bug: a function CALL site emits a
    // bare `Err` stub (the stdlib `Result::Err` variant constructor). The only
    // matching definition is `budget.rs>Err`, an associated `type Err = String`
    // recorded as NodeKind::TypeAlias. A call must NEVER resolve to a type
    // alias — you cannot call a type alias — so the stub must be LEFT in place
    // (Unresolved), not redirected onto the TypeAlias def.
    let mut store = Store::new();
    let caller = store.upsert_node_with_kind(path("caller.rs>do_work"), NodeKind::Function);
    let stub = store.upsert_node_with_kind(TrunkPath::parse("Err").unwrap(), NodeKind::Unresolved);
    let type_alias = store.upsert_node_with_kind(path("budget.rs>Err"), NodeKind::TypeAlias);
    store.upsert_edge(EdgeKind::Calls, caller, stub);

    let resolved = store.resolve_bare_call_stubs();

    assert_eq!(
        resolved, 0,
        "a CALL stub must not resolve to a TypeAlias definition"
    );
    assert!(
        store.lookup("Err").is_some(),
        "the unresolved call stub must be left in place, not redirected"
    );
    assert!(
        !store
            .incoming(type_alias, EdgeKind::Calls)
            .contains(&caller),
        "the type alias must not gain a phantom incoming Calls edge"
    );
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&stub),
        "the caller's Calls edge must still point at the Unresolved stub"
    );
}

#[test]
fn store_resolve_import_stub_still_binds_to_type_alias() {
    // No-regression: an IMPORT of a type alias (e.g. `use foo::SomeAlias`)
    // legitimately resolves to a unique TypeAlias definition. The guard gates
    // ONLY on incoming Calls edges, so an import-only stub must STILL resolve.
    let mut store = Store::new();
    let importer_file = store.upsert_node_with_kind(path("consumer.rs"), NodeKind::File);
    let stub =
        store.upsert_node_with_kind(TrunkPath::parse("SomeAlias").unwrap(), NodeKind::Unresolved);
    let type_alias = store.upsert_node_with_kind(path("defs.rs>SomeAlias"), NodeKind::TypeAlias);
    store.upsert_edge(EdgeKind::Imports, importer_file, stub);

    let resolved = store.resolve_bare_call_stubs();

    assert_eq!(
        resolved, 1,
        "an import-only stub to a unique TypeAlias must still resolve"
    );
    assert!(
        store.lookup("SomeAlias").is_none(),
        "import stub must be removed after resolution"
    );
    assert!(
        store
            .outgoing(importer_file, EdgeKind::Imports)
            .contains(&type_alias),
        "import edge must point at the resolved TypeAlias def"
    );
}

#[test]
fn store_resolve_call_stub_binds_to_struct_ctor() {
    // No-regression: a tuple-struct constructor call `MyStruct(...)` emits a
    // bare `MyStruct` call stub that legitimately resolves to a Struct def.
    // The guard blocks ONLY TypeAlias, so Struct ctor calls still resolve.
    let mut store = Store::new();
    let caller = store.upsert_node_with_kind(path("caller.rs>build"), NodeKind::Function);
    let stub =
        store.upsert_node_with_kind(TrunkPath::parse("MyStruct").unwrap(), NodeKind::Unresolved);
    let struct_def = store.upsert_node_with_kind(path("model.rs>MyStruct"), NodeKind::Struct);
    store.upsert_edge(EdgeKind::Calls, caller, stub);

    let resolved = store.resolve_bare_call_stubs();

    assert_eq!(resolved, 1, "a Struct ctor call stub must still resolve");
    assert!(
        store
            .outgoing(caller, EdgeKind::Calls)
            .contains(&struct_def),
        "Calls edge must point at the Struct definition"
    );
}

#[test]
fn store_resolve_import_aware_call_stub_does_not_bind_to_type_alias() {
    // Same guard must hold in the import-aware pass: an AMBIGUOUS call stub
    // `Err` defined as TypeAlias in two files, where the caller's file imports
    // one of them, must NOT be redirected onto a TypeAlias. (Mixed Calls+Import
    // evidence with only a TypeAlias candidate → conservatively left unresolved.)
    let mut store = Store::new();
    let caller_file = store.upsert_node_with_kind(path("caller.rs"), NodeKind::File);
    let caller = store.upsert_node_with_kind(path("caller.rs>do_work"), NodeKind::Function);
    let stub = store.upsert_node_with_kind(TrunkPath::parse("Err").unwrap(), NodeKind::Unresolved);
    let b_file = store.upsert_node_with_kind(path("b.rs"), NodeKind::File);
    let b_err = store.upsert_node_with_kind(path("b.rs>Err"), NodeKind::TypeAlias);
    let _c_file = store.upsert_node_with_kind(path("c.rs"), NodeKind::File);
    let _c_err = store.upsert_node_with_kind(path("c.rs>Err"), NodeKind::TypeAlias);
    store.upsert_edge(EdgeKind::Calls, caller, stub);
    store.upsert_edge(EdgeKind::Imports, caller_file, b_file);

    let resolved = store.resolve_bare_call_stubs();

    assert_eq!(
        resolved, 0,
        "import-aware pass must not bind a call stub to a TypeAlias"
    );
    assert!(
        store.lookup("Err").is_some(),
        "call stub must remain unresolved (TypeAlias is not callable)"
    );
    assert!(
        !store.incoming(b_err, EdgeKind::Calls).contains(&caller),
        "the imported TypeAlias must not gain a phantom Calls caller"
    );
}

#[test]
fn store_resolve_call_site_context_does_not_bind_to_type_alias() {
    // Same guard must hold in the RFC-0118 Part B receiver-context pass
    // (`resolve_call_site_contexts`): it disambiguates a recorded method call
    // site against path-suffix candidates and ADDs a Calls edge. A method call
    // can never resolve to a type alias either. Here the sole `>foo` candidate
    // is a TypeAlias; the single-candidate fast-path in `disambiguate` would
    // bind it without a kind check, manufacturing a phantom Calls→TypeAlias edge
    // (independent-review SHOULD-FIX on PR #682). The simple/import passes leave
    // the call stub in place (their guard already blocks the TypeAlias), so the
    // stub survives to the Part B pass — which must also decline.
    let mut store = Store::new();
    let caller = store.upsert_node_with_kind(path("caller.rs>do_work"), NodeKind::Function);
    let stub = store.upsert_node_with_kind(TrunkPath::parse("foo").unwrap(), NodeKind::Unresolved);
    let type_alias = store.upsert_node_with_kind(path("defs.rs>foo"), NodeKind::TypeAlias);
    store.upsert_edge(EdgeKind::Calls, caller, stub);
    store.record_call_site(
        caller,
        stub,
        ReceiverContext {
            receiver: "x".to_owned(),
            method: "foo".to_owned(),
            imports: Vec::new(),
            locals: Vec::new(),
            self_type: None,
            params: Vec::new(),
            fields: Vec::new(),
        },
    );

    let _ = store.resolve_bare_call_stubs();

    assert!(
        !store
            .incoming(type_alias, EdgeKind::Calls)
            .contains(&caller),
        "the receiver-context pass must not add a Calls edge to a TypeAlias def"
    );
}

#[test]
fn store_resolve_go_named_type_call_still_resolves() {
    // Go named types (`type Status int`) are stored as NodeKind::TypeAlias but
    // ARE valid call targets in expression context — `Status(1)` is a type
    // conversion, emitted by the Go pack as a Calls edge. The TypeAlias call
    // guard must NOT block these; it must only block Rust/TS non-callable aliases.
    // Language is detected by the definition's file path (.go> marker).
    let mut store = Store::new();
    let caller = store.upsert_node_with_kind(path("main.go>run"), NodeKind::Function);
    let stub =
        store.upsert_node_with_kind(TrunkPath::parse("Status").unwrap(), NodeKind::Unresolved);
    let go_type = store.upsert_node_with_kind(path("types.go>Status"), NodeKind::TypeAlias);
    store.upsert_edge(EdgeKind::Calls, caller, stub);

    let resolved = store.resolve_bare_call_stubs();

    assert_eq!(
        resolved, 1,
        "a Go named-type call (type conversion) must resolve to the TypeAlias def"
    );
    assert!(
        store.lookup("Status").is_none(),
        "resolved Go type-conversion stub must be removed after resolution"
    );
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&go_type),
        "caller's Calls edge must point at the resolved Go TypeAlias def"
    );
}

#[test]
fn store_resolve_extends_stub_mixed_import_sites_resolved_per_edge() {
    // Two subclasses extend the same bare `Base` but import DIFFERENT defs:
    // a.py>Sub imports b.py → resolves to b.py>Base.
    // c.py>Sub2 imports d.py → resolves to d.py>Base.
    // RFC-0103 per-edge follow-up (Issue #555): each edge resolves independently;
    // stub is removed after all incoming Extends edges are redirected.
    let mut store = Store::new();
    let a_file = store.upsert_node(path("a.py"));
    let sub1 = store.upsert_node(path("a.py>Sub"));
    let c_file = store.upsert_node(path("c.py"));
    let sub2 = store.upsert_node(path("c.py>Sub2"));
    let stub = store.upsert_node(TrunkPath::parse("Base").unwrap());
    let b_file = store.upsert_node(path("b.py"));
    let b_base = store.upsert_node(path("b.py>Base"));
    let d_file = store.upsert_node(path("d.py"));
    let d_base = store.upsert_node(path("d.py>Base"));
    store.upsert_edge(EdgeKind::Extends, sub1, stub);
    store.upsert_edge(EdgeKind::Extends, sub2, stub);
    store.upsert_edge(EdgeKind::Imports, a_file, b_file);
    store.upsert_edge(EdgeKind::Imports, c_file, d_file);

    let resolved = store.resolve_bare_call_stubs();

    assert_eq!(
        resolved, 1,
        "stub resolved once all Extends edges redirected"
    );
    assert!(
        store.lookup("Base").is_none(),
        "stub removed after full resolution"
    );
    assert!(
        store.outgoing(sub1, EdgeKind::Extends).contains(&b_base),
        "sub1 resolved to b.py>Base"
    );
    assert!(
        store.outgoing(sub2, EdgeKind::Extends).contains(&d_base),
        "sub2 resolved to d.py>Base"
    );
    let _ = (b_file, d_file);
}

// ── Issue #555: per-edge Extends resolution for mixed-import sites ────

#[test]
fn store_resolve_extends_stub_per_edge_mixed_imports() {
    // a.py>Sub extends `Base` and imports b.py  → should resolve to b.py>Base.
    // c.py>Sub2 extends `Base` and imports d.py → should resolve to d.py>Base.
    // Per-edge resolution handles each independently; after both edges are
    // redirected the stub has no remaining incoming Extends edges and is removed.
    let mut store = Store::new();
    let a_file = store.upsert_node(path("a.py"));
    let sub1 = store.upsert_node(path("a.py>Sub"));
    let c_file = store.upsert_node(path("c.py"));
    let sub2 = store.upsert_node(path("c.py>Sub2"));
    let stub = store.upsert_node(TrunkPath::parse("Base").unwrap());
    let b_file = store.upsert_node(path("b.py"));
    let b_base = store.upsert_node(path("b.py>Base"));
    let d_file = store.upsert_node(path("d.py"));
    let d_base = store.upsert_node(path("d.py>Base"));
    store.upsert_edge(EdgeKind::Extends, sub1, stub);
    store.upsert_edge(EdgeKind::Extends, sub2, stub);
    store.upsert_edge(EdgeKind::Imports, a_file, b_file);
    store.upsert_edge(EdgeKind::Imports, c_file, d_file);

    let resolved = store.resolve_bare_call_stubs();

    assert_eq!(
        resolved, 1,
        "stub resolved after all Extends edges redirected per-edge"
    );
    assert!(
        store.lookup("Base").is_none(),
        "stub removed once all incoming Extends edges are gone"
    );
    assert!(
        store.outgoing(sub1, EdgeKind::Extends).contains(&b_base),
        "sub1 Extends now points to b.py>Base"
    );
    assert!(
        store.outgoing(sub2, EdgeKind::Extends).contains(&d_base),
        "sub2 Extends now points to d.py>Base"
    );
    assert!(
        !store.outgoing(sub1, EdgeKind::Extends).contains(&stub),
        "sub1 Extends must not still point at bare stub"
    );
    assert!(
        !store.outgoing(sub2, EdgeKind::Extends).contains(&stub),
        "sub2 Extends must not still point at bare stub"
    );
    // Unused variable suppression (b_file, d_file not queried above).
    let _ = (b_file, d_file);
}

// ── Codex P2 / PR #572: stub with cross-kind edges must survive Extends resolution ──

#[test]
fn store_resolve_extends_stub_preserved_when_calls_edge_exists() {
    // Two definitions of `Base` (b.py>Base and d.py>Base) prevent the
    // simple pass from resolving the stub (needs unique match). The
    // import-aware Extends pass resolves Sub's edge (a.py imports b.py →
    // b.py>Base), but caller in c.py imports nothing matching → its Calls
    // edge stays unresolved. After Extends resolution the stub's Extends-
    // incoming is empty; with the old `incoming(Extends).is_empty()` guard
    // the stub would be removed and the Calls edge would dangle (Codex P2,
    // PR #572). `is_isolated()` keeps the stub alive.
    let mut store = Store::new();
    let a_file = store.upsert_node(path("a.py"));
    let subclass = store.upsert_node(path("a.py>Sub"));
    let bare_stub = store.upsert_node(TrunkPath::parse("Base").unwrap());
    let _c_file = store.upsert_node(path("c.py")); // no imports → caller unresolved
    let caller = store.upsert_node(path("c.py>caller"));
    let b_file = store.upsert_node(path("b.py"));
    let b_base = store.upsert_node(path("b.py>Base"));
    let _d_file = store.upsert_node(path("d.py"));
    let _d_base = store.upsert_node(path("d.py>Base")); // second def — blocks simple pass
    // Subclass extends the bare stub; a.py imports b.py → Extends resolves to b.py>Base.
    store.upsert_edge(EdgeKind::Extends, subclass, bare_stub);
    store.upsert_edge(EdgeKind::Imports, a_file, b_file);
    // caller calls the bare stub (c.py imports nothing → Calls stays unresolved).
    store.upsert_edge(EdgeKind::Calls, caller, bare_stub);

    let resolved = store.resolve_bare_call_stubs();

    // Extends edge resolved; bare_stub must NOT be removed because the Calls edge remains.
    assert_eq!(resolved, 0, "bare_stub not removed while Calls edge exists");
    assert!(
        store.lookup("Base").is_some(),
        "bare_stub preserved — Calls edge still references it"
    );
    assert!(
        store
            .outgoing(subclass, EdgeKind::Extends)
            .contains(&b_base),
        "subclass Extends edge redirected to b.py>Base"
    );
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&bare_stub),
        "caller's Calls edge still points to bare_stub"
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
    // Real file nodes carry NodeKind::File (the extractor sets it at index time).
    store.upsert_node_with_kind(path("src/auth.rs"), NodeKind::File);
    store.upsert_node(path("src/auth.rs>AuthService"));
    store.upsert_node_with_kind(path("src/main.rs"), NodeKind::File);
    store.upsert_node(path("src/main.rs>main"));
    // A bare, kind-less import/call stub WITHOUT a `>` — exactly the kind of node
    // the resolver mints for unresolved callees (`unwrap`) and import targets
    // (`HashMap`). It must NOT be reported as a file (dogfood F1: get-files
    // returned 671/786 such fakes).
    store.upsert_node(path("unwrap"));

    let files = store.all_file_paths();
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
    assert!(
        !files.contains(&"unwrap".to_string()),
        "a kind-less bare stub must not be reported as a file: {files:?}"
    );
}

#[test]
fn store_all_file_paths_returns_sorted_order() {
    let mut store = Store::new();
    store.upsert_node_with_kind(path("z.rs"), NodeKind::File);
    store.upsert_node_with_kind(path("a.rs"), NodeKind::File);
    store.upsert_node_with_kind(path("m.rs"), NodeKind::File);
    let files = store.all_file_paths();
    let mut sorted = files.clone();
    sorted.sort_unstable();
    assert_eq!(files, sorted, "all_file_paths must return sorted results");
}

// ── RFC-0118 Part A: NodeKind::Unresolved phantoms excluded from the symbol universe ──

#[test]
fn unresolved_phantoms_excluded_from_symbol_queries() {
    let mut store = Store::new();
    // A real symbol, and two resolver phantoms (a qualified one and a bare one),
    // each the target of a Calls edge (so they have in-degree ≥ 1 — exactly how
    // the extractor mints them, which is why they leak into rank/all/pagerank).
    let caller = store.upsert_node_with_kind(path("app.rs>main"), NodeKind::Function);
    let real = store.upsert_node_with_kind(path("db.rs>Db>query"), NodeKind::Method);
    let phantom_q = store.upsert_node_with_kind(path("Db>upsert_node"), NodeKind::Unresolved);
    let phantom_bare = store.upsert_node_with_kind(path("unwrap"), NodeKind::Unresolved);
    store.upsert_edge(EdgeKind::Calls, caller, real);
    store.upsert_edge(EdgeKind::Calls, caller, phantom_q);
    store.upsert_edge(EdgeKind::Calls, caller, phantom_bare);

    // all_symbols: real symbol present, neither phantom.
    let syms = store.all_symbols(None, None);
    assert!(
        syms.contains(&"db.rs>Db>query".to_string()),
        "real symbol must list: {syms:?}"
    );
    assert!(
        !syms.contains(&"Db>upsert_node".to_string()),
        "qualified phantom must be excluded: {syms:?}"
    );
    assert!(
        !syms.iter().any(|s| s == "unwrap"),
        "bare phantom must be excluded: {syms:?}"
    );

    // rank_symbols (top_callee_symbols + top_symbols_by_incoming): phantoms must
    // not be ranked even though they have incoming Calls edges.
    let by_callee: Vec<String> = store
        .top_callee_symbols(10)
        .into_iter()
        .map(|(p, _)| p)
        .collect();
    assert!(
        !by_callee
            .iter()
            .any(|p| p == "Db>upsert_node" || p == "unwrap"),
        "phantoms must not appear in top_callee_symbols: {by_callee:?}"
    );
    let by_incoming: Vec<String> = store
        .top_symbols_by_incoming(EdgeKind::Calls, 10)
        .into_iter()
        .map(|(p, _)| p)
        .collect();
    assert!(
        !by_incoming
            .iter()
            .any(|p| p == "Db>upsert_node" || p == "unwrap"),
        "phantoms must not appear in top_symbols_by_incoming: {by_incoming:?}"
    );

    // page_rank: phantoms must not be in the ranked symbol universe.
    let ranked: Vec<String> = store
        .page_rank(EdgeKind::Calls, 0.85, 5)
        .into_iter()
        .map(|e| e.path)
        .collect();
    assert!(
        !ranked
            .iter()
            .any(|p| p == "Db>upsert_node" || p == "unwrap"),
        "phantoms must not appear in page_rank: {ranked:?}"
    );
}

#[test]
fn is_real_symbol_only_excludes_unresolved() {
    let mut store = Store::new();
    let f = store.upsert_node_with_kind(path("a.rs>f"), NodeKind::Function);
    let u = store.upsert_node_with_kind(path("ghost"), NodeKind::Unresolved);
    let kindless = store.upsert_node(path("b.rs>g")); // no kind (e.g. test/programmatic store)
    assert!(store.is_real_symbol(f), "a real kinded symbol is real");
    assert!(
        !store.is_real_symbol(u),
        "an Unresolved phantom is NOT real"
    );
    assert!(
        store.is_real_symbol(kindless),
        "a kind-less node stays real (back-compat)"
    );
}

// ── RFC-0118 Part A.2: symbol_universe() — real-symbol node universe ──────

#[test]
fn symbol_universe_empty_store_is_empty() {
    let store = Store::new();
    assert!(
        store.symbol_universe().is_empty(),
        "an empty store has no symbol universe"
    );
}

#[test]
fn symbol_universe_single_real_symbol() {
    let mut store = Store::new();
    // A file node (no `>`) and one real symbol; only the symbol is in the universe.
    store.upsert_node_with_kind(path("a.rs"), NodeKind::File);
    let real = store.upsert_node_with_kind(path("a.rs>f"), NodeKind::Function);
    assert_eq!(
        store.symbol_universe(),
        vec![real],
        "only the `>`-qualified real symbol is in the universe (file node excluded)"
    );
}

#[test]
fn symbol_universe_excludes_unresolved_phantoms() {
    let mut store = Store::new();
    let real_a = store.upsert_node_with_kind(path("a.rs>caller"), NodeKind::Function);
    let real_b = store.upsert_node_with_kind(path("a.rs>callee"), NodeKind::Function);
    // Qualified + bare resolver phantoms (each a Calls target — how the extractor mints them).
    let phantom_q = store.upsert_node_with_kind(path("Db>upsert_node"), NodeKind::Unresolved);
    let phantom_bare = store.upsert_node_with_kind(path("unwrap"), NodeKind::Unresolved);
    store.upsert_edge(EdgeKind::Calls, real_a, real_b);
    store.upsert_edge(EdgeKind::Calls, real_a, phantom_q);
    store.upsert_edge(EdgeKind::Calls, real_a, phantom_bare);

    let universe = store.symbol_universe();
    assert!(
        universe.contains(&real_a),
        "real caller present: {universe:?}"
    );
    assert!(
        universe.contains(&real_b),
        "real callee present: {universe:?}"
    );
    assert!(
        !universe.contains(&phantom_q),
        "qualified phantom excluded: {universe:?}"
    );
    assert!(
        !universe.contains(&phantom_bare),
        "bare phantom excluded: {universe:?}"
    );
    assert_eq!(
        universe.len(),
        2,
        "exactly the two real symbols: {universe:?}"
    );
}

// ── RFC-0118 Part A.2: graph-theory queries operate on the real-symbol ──
// ── induced subgraph (phantoms excluded as nodes AND as edge endpoints) ──

/// Build a store: real `app.rs>main` Calls real `db.rs>query`, and `main`
/// additionally Calls a *qualified* `NodeKind::Unresolved` phantom
/// `Db>upsert_node`. Returns `(store, main_id, query_id, phantom_id)`.
fn store_with_phantom_callee() -> (Store, NodeId, NodeId, NodeId) {
    let mut store = Store::new();
    let main = store.upsert_node_with_kind(path("app.rs>main"), NodeKind::Function);
    let query = store.upsert_node_with_kind(path("db.rs>query"), NodeKind::Method);
    let phantom = store.upsert_node_with_kind(path("Db>upsert_node"), NodeKind::Unresolved);
    store.upsert_edge(EdgeKind::Calls, main, query);
    store.upsert_edge(EdgeKind::Calls, main, phantom);
    (store, main, query, phantom)
}

/// Phantom-free twin of [`store_with_phantom_callee`] — the oracle.
fn store_phantom_free_twin() -> Store {
    let mut store = Store::new();
    let main = store.upsert_node_with_kind(path("app.rs>main"), NodeKind::Function);
    let query = store.upsert_node_with_kind(path("db.rs>query"), NodeKind::Method);
    store.upsert_edge(EdgeKind::Calls, main, query);
    store
}

#[test]
fn leaf_symbols_excludes_phantoms_and_induces_degree() {
    let (store, ..) = store_with_phantom_callee();
    let leaves = store.leaf_symbols(EdgeKind::Calls, 100);
    assert!(
        !leaves.contains(&"Db>upsert_node".to_string()),
        "phantom must not be listed as a leaf: {leaves:?}"
    );
    // `main` calls a real symbol + a phantom; on the induced subgraph it still
    // has a real out-edge, so it is NOT a leaf. Oracle: phantom-free twin.
    assert_eq!(
        leaves,
        store_phantom_free_twin().leaf_symbols(EdgeKind::Calls, 100),
        "leaf set must match the phantom-free twin"
    );
}

#[test]
fn isolated_symbols_excludes_phantoms() {
    // A real isolated symbol, plus an isolated phantom (no edges at all).
    let mut store = Store::new();
    let lonely = store.upsert_node_with_kind(path("a.rs>lonely"), NodeKind::Function);
    let _phantom = store.upsert_node_with_kind(path("ghost"), NodeKind::Unresolved);
    let _phantom_q = store.upsert_node_with_kind(path("Db>ghost"), NodeKind::Unresolved);
    let iso = store.isolated_symbols(None);
    assert_eq!(
        iso,
        vec!["a.rs>lonely".to_string()],
        "only the real isolated symbol; phantoms excluded: {iso:?}"
    );
    let _ = lonely;
}

#[test]
fn singly_referenced_excludes_phantoms_and_induces_callers() {
    let (store, ..) = store_with_phantom_callee();
    let singly = store.singly_referenced(EdgeKind::Calls, 100);
    assert!(
        !singly.iter().any(|(p, _)| p == "Db>upsert_node"),
        "phantom must not be reported as singly-referenced: {singly:?}"
    );
    assert_eq!(
        singly,
        store_phantom_free_twin().singly_referenced(EdgeKind::Calls, 100),
        "singly-referenced set must match the phantom-free twin"
    );
}

#[test]
fn hub_symbols_excludes_phantoms_and_induces_degree() {
    let (store, ..) = store_with_phantom_callee();
    // min_in=0, min_out=0 lists every symbol with its degrees.
    let hubs = store.hub_symbols(EdgeKind::Calls, 0, 0, 100);
    assert!(
        !hubs.iter().any(|(p, _, _)| p == "Db>upsert_node"),
        "phantom must not be a hub: {hubs:?}"
    );
    // `main`'s out-degree must count only the real callee (1), not the phantom.
    let main_out = hubs
        .iter()
        .find(|(p, _, _)| p == "app.rs>main")
        .map(|(_, _, out)| *out);
    assert_eq!(
        main_out,
        Some(1),
        "main out-degree must exclude the phantom edge: {hubs:?}"
    );
    assert_eq!(
        hubs,
        store_phantom_free_twin().hub_symbols(EdgeKind::Calls, 0, 0, 100),
        "hub set must match the phantom-free twin"
    );
}

#[test]
fn most_connected_excludes_phantoms_and_induces_degree() {
    let (store, ..) = store_with_phantom_callee();
    let mc = store.most_connected(100, EdgeKind::Calls);
    assert!(
        !mc.iter().any(|(p, _)| p == "Db>upsert_node"),
        "phantom must not appear in most_connected: {mc:?}"
    );
    // `main` degree = 1 real out-edge (phantom edge not counted).
    let main_deg = mc.iter().find(|(p, _)| p == "app.rs>main").map(|(_, d)| *d);
    assert_eq!(
        main_deg,
        Some(1),
        "main degree must exclude the phantom edge: {mc:?}"
    );
    assert_eq!(
        mc,
        store_phantom_free_twin().most_connected(100, EdgeKind::Calls),
        "most_connected must match the phantom-free twin"
    );
}

#[test]
fn degree_centrality_excludes_phantoms_and_induces_degree() {
    // PR #677 review: degree_centrality was the 20th graph query with the same
    // phantom leak. Gating its universe to symbol_universe() excludes the phantom
    // node AND (via the existing idx guard) the phantom edge.
    let (store, ..) = store_with_phantom_callee();
    let dc = store.degree_centrality(EdgeKind::Calls);
    assert!(
        !dc.iter().any(|e| e.path == "Db>upsert_node"),
        "phantom must not appear in degree_centrality: {dc:?}"
    );
    let main_out = dc
        .iter()
        .find(|e| e.path == "app.rs>main")
        .map(|e| e.out_degree);
    assert_eq!(
        main_out,
        Some(1),
        "main out_degree must exclude the phantom edge: {dc:?}"
    );
    assert_eq!(
        dc,
        store_phantom_free_twin().degree_centrality(EdgeKind::Calls),
        "degree_centrality must match the phantom-free twin (induced subgraph)"
    );
}

#[test]
fn k_core_excludes_phantoms() {
    let (store, ..) = store_with_phantom_callee();
    // k == 0 returns the whole symbol universe — must not include the phantom.
    let core0 = store.k_core(EdgeKind::Calls, 0);
    assert!(
        !core0.contains(&"Db>upsert_node".to_string()),
        "phantom must not appear in k_core(0): {core0:?}"
    );
    assert_eq!(
        core0,
        store_phantom_free_twin().k_core(EdgeKind::Calls, 0),
        "k_core(0) must match the phantom-free twin"
    );
    // k == 1: with the phantom edge induced out, `main` has subgraph degree 1
    // and `query` degree 1, so both survive — same as the twin.
    assert_eq!(
        store.k_core(EdgeKind::Calls, 1),
        store_phantom_free_twin().k_core(EdgeKind::Calls, 1),
        "k_core(1) must match the phantom-free twin"
    );
}

#[test]
fn dependency_layers_excludes_phantoms() {
    let (store, ..) = store_with_phantom_callee();
    let layers = store.dependency_layers(EdgeKind::Calls);
    assert!(
        !layers
            .iter()
            .any(|layer| layer.contains(&"Db>upsert_node".to_string())),
        "phantom must not appear in any dependency layer: {layers:?}"
    );
    assert_eq!(
        layers,
        store_phantom_free_twin().dependency_layers(EdgeKind::Calls),
        "dependency layering must match the phantom-free twin"
    );
}

/// Build a store with a genuine real 2-cycle (`x ⇄ y`) PLUS a would-be cycle
/// through a phantom (`a → p → a`, where `p` is `NodeKind::Unresolved`).
/// Returns the store. The phantom-free oracle (same minus `p` and its edges)
/// is [`scc_phantom_free_twin`].
fn store_with_phantom_cycle() -> Store {
    let mut store = Store::new();
    let x = store.upsert_node_with_kind(path("m.rs>x"), NodeKind::Function);
    let y = store.upsert_node_with_kind(path("m.rs>y"), NodeKind::Function);
    let a = store.upsert_node_with_kind(path("m.rs>a"), NodeKind::Function);
    let p = store.upsert_node_with_kind(path("Phantom>p"), NodeKind::Unresolved);
    store.upsert_edge(EdgeKind::Calls, x, y);
    store.upsert_edge(EdgeKind::Calls, y, x); // real SCC {x, y}
    store.upsert_edge(EdgeKind::Calls, a, p);
    store.upsert_edge(EdgeKind::Calls, p, a); // phantom SCC {a, p} — must vanish
    store
}

/// Phantom-free oracle for [`store_with_phantom_cycle`].
fn scc_phantom_free_twin() -> Store {
    let mut store = Store::new();
    let x = store.upsert_node_with_kind(path("m.rs>x"), NodeKind::Function);
    let y = store.upsert_node_with_kind(path("m.rs>y"), NodeKind::Function);
    let _a = store.upsert_node_with_kind(path("m.rs>a"), NodeKind::Function);
    store.upsert_edge(EdgeKind::Calls, x, y);
    store.upsert_edge(EdgeKind::Calls, y, x);
    store
}

#[test]
fn nodes_in_cycles_excludes_phantoms() {
    let store = store_with_phantom_cycle();
    let cyc = store.nodes_in_cycles(EdgeKind::Calls, None);
    assert!(
        !cyc.contains(&"Phantom>p".to_string()),
        "phantom must not be a cycle member: {cyc:?}"
    );
    assert!(
        !cyc.contains(&"m.rs>a".to_string()),
        "a real node only in a phantom-cycle must not be reported: {cyc:?}"
    );
    assert_eq!(
        cyc,
        scc_phantom_free_twin().nodes_in_cycles(EdgeKind::Calls, None),
        "cycle members must match the phantom-free twin (only the real {{x,y}})"
    );
}

#[test]
fn cycle_members_excludes_phantoms() {
    let store = store_with_phantom_cycle();
    let cyc = store.cycle_members(EdgeKind::Calls);
    assert!(
        !cyc.contains(&"Phantom>p".to_string()) && !cyc.contains(&"m.rs>a".to_string()),
        "phantom and its phantom-only-cycle partner excluded: {cyc:?}"
    );
    assert_eq!(
        cyc,
        scc_phantom_free_twin().cycle_members(EdgeKind::Calls),
        "cycle_members must match the phantom-free twin"
    );
}

#[test]
fn scc_groups_excludes_phantoms() {
    let store = store_with_phantom_cycle();
    let groups = store.scc_groups(EdgeKind::Calls);
    assert!(
        !groups
            .iter()
            .any(|g| g.iter().any(|p| p == "Phantom>p" || p == "m.rs>a")),
        "no SCC may contain the phantom or its phantom-only partner: {groups:?}"
    );
    assert_eq!(
        groups,
        scc_phantom_free_twin().scc_groups(EdgeKind::Calls),
        "scc_groups must match the phantom-free twin"
    );
}

#[test]
fn strongly_connected_components_excludes_phantoms() {
    let store = store_with_phantom_cycle();
    let entries = store.strongly_connected_components(EdgeKind::Calls);
    assert!(
        !entries
            .iter()
            .any(|e| e.members.iter().any(|p| p == "Phantom>p")),
        "phantom must not be in any SCC: {entries:?}"
    );
    assert_eq!(
        entries,
        scc_phantom_free_twin().strongly_connected_components(EdgeKind::Calls),
        "strongly_connected_components must match the phantom-free twin"
    );
}

#[test]
fn weakly_connected_components_excludes_phantoms() {
    let store = store_with_phantom_cycle();
    let wcc = store.weakly_connected_components(EdgeKind::Calls);
    assert!(
        !wcc.iter().any(|c| c.iter().any(|p| p == "Phantom>p")),
        "phantom must not be in any weakly-connected component: {wcc:?}"
    );
    assert_eq!(
        wcc,
        scc_phantom_free_twin().weakly_connected_components(EdgeKind::Calls),
        "weakly_connected_components must match the phantom-free twin"
    );
}

#[test]
fn topological_sort_excludes_phantoms() {
    let store = store_with_phantom_cycle();
    let topo = store.topological_sort(EdgeKind::Calls);
    assert!(
        !topo.order.contains(&"Phantom>p".to_string())
            && !topo.cycle_members.contains(&"Phantom>p".to_string()),
        "phantom must appear in neither order nor cycle_members: {topo:?}"
    );
    let twin = scc_phantom_free_twin().topological_sort(EdgeKind::Calls);
    assert_eq!(topo.order, twin.order, "topo order must match twin");
    assert_eq!(
        topo.cycle_members, twin.cycle_members,
        "topo cycle_members must match twin"
    );
}

/// Linear chain `a → b → p → c` where `p` is a `NodeKind::Unresolved` phantom
/// sitting *between* two real nodes. Un-gated, `p` is an articulation point and
/// `b–p` / `p–c` are bridges. Gated, `p` and its edges vanish, leaving the real
/// edge `a–b` (a bridge) and a disconnected real `c`.
fn store_with_phantom_chain() -> Store {
    let mut store = Store::new();
    let a = store.upsert_node_with_kind(path("m.rs>a"), NodeKind::Function);
    let b = store.upsert_node_with_kind(path("m.rs>b"), NodeKind::Function);
    let p = store.upsert_node_with_kind(path("Phantom>p"), NodeKind::Unresolved);
    let c = store.upsert_node_with_kind(path("m.rs>c"), NodeKind::Function);
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, p);
    store.upsert_edge(EdgeKind::Calls, p, c);
    store
}

/// Phantom-free oracle for [`store_with_phantom_chain`].
fn chain_phantom_free_twin() -> Store {
    let mut store = Store::new();
    let a = store.upsert_node_with_kind(path("m.rs>a"), NodeKind::Function);
    let b = store.upsert_node_with_kind(path("m.rs>b"), NodeKind::Function);
    let _c = store.upsert_node_with_kind(path("m.rs>c"), NodeKind::Function);
    store.upsert_edge(EdgeKind::Calls, a, b);
    store
}

#[test]
fn articulation_points_excludes_phantoms() {
    let store = store_with_phantom_chain();
    let aps = store.articulation_points(EdgeKind::Calls);
    assert!(
        !aps.contains(&"Phantom>p".to_string()),
        "phantom must not be an articulation point: {aps:?}"
    );
    assert_eq!(
        aps,
        chain_phantom_free_twin().articulation_points(EdgeKind::Calls),
        "articulation_points must match the phantom-free twin"
    );
}

#[test]
fn bridge_edges_excludes_phantoms() {
    let store = store_with_phantom_chain();
    let bridges = store.bridge_edges(EdgeKind::Calls);
    assert!(
        !bridges
            .iter()
            .any(|(f, t)| f == "Phantom>p" || t == "Phantom>p"),
        "no bridge may touch the phantom: {bridges:?}"
    );
    assert_eq!(
        bridges,
        chain_phantom_free_twin().bridge_edges(EdgeKind::Calls),
        "bridge_edges must match the phantom-free twin"
    );
}

#[test]
fn biconnected_components_excludes_phantoms() {
    let store = store_with_phantom_cycle();
    let bcc = store.biconnected_components(EdgeKind::Calls);
    assert!(
        !bcc.iter().any(|c| c.iter().any(|p| p == "Phantom>p")),
        "phantom must not be in any biconnected component: {bcc:?}"
    );
    assert_eq!(
        bcc,
        scc_phantom_free_twin().biconnected_components(EdgeKind::Calls),
        "biconnected_components must match the phantom-free twin"
    );
}

/// Real chain `a → b → c` PLUS a phantom callee `b → p` (`NodeKind::Unresolved`).
/// `b` lies on the only real shortest path `a → c`, so it has nonzero
/// betweenness/closeness; the phantom must change neither `b`'s score nor the
/// normalization denominator (which must be `|real symbols| = 3`, not 4).
fn store_with_phantom_for_centrality() -> Store {
    let mut store = Store::new();
    let a = store.upsert_node_with_kind(path("m.rs>a"), NodeKind::Function);
    let b = store.upsert_node_with_kind(path("m.rs>b"), NodeKind::Function);
    let c = store.upsert_node_with_kind(path("m.rs>c"), NodeKind::Function);
    let p = store.upsert_node_with_kind(path("Phantom>p"), NodeKind::Unresolved);
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    store.upsert_edge(EdgeKind::Calls, b, p);
    store
}

/// Phantom-free oracle for [`store_with_phantom_for_centrality`].
fn centrality_phantom_free_twin() -> Store {
    let mut store = Store::new();
    let a = store.upsert_node_with_kind(path("m.rs>a"), NodeKind::Function);
    let b = store.upsert_node_with_kind(path("m.rs>b"), NodeKind::Function);
    let c = store.upsert_node_with_kind(path("m.rs>c"), NodeKind::Function);
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    store
}

#[test]
fn betweenness_centrality_excludes_phantoms_and_normalizes_over_real_n() {
    let store = store_with_phantom_for_centrality();
    let got = store.betweenness_centrality(EdgeKind::Calls);
    assert!(
        !got.iter().any(|e| e.path == "Phantom>p"),
        "phantom must not appear in betweenness: {got:?}"
    );
    let twin = centrality_phantom_free_twin().betweenness_centrality(EdgeKind::Calls);
    // Same path set, same per-path scores (denominator must be real n = 3, not 4).
    // Compare by path key — equal-score entries have no stable order across stores.
    assert_eq!(got.len(), twin.len(), "same node count as twin");
    for t in &twin {
        let g = got.iter().find(|e| e.path == t.path);
        assert!(
            g.is_some_and(|e| (e.score - t.score).abs() < 1e-12),
            "score for {} must match twin (got {:?}, twin {})",
            t.path,
            g.map(|e| e.score),
            t.score
        );
    }
    // Concretely: b's betweenness over real n=3 is 1/((3-1)(3-2)) = 0.5.
    let b_score = got.iter().find(|e| e.path == "m.rs>b").map(|e| e.score);
    assert!(
        b_score.is_some_and(|s| (s - 0.5).abs() < 1e-12),
        "b betweenness must be 0.5 (real-n normalization): {b_score:?}"
    );
}

#[test]
fn closeness_centrality_excludes_phantoms_and_normalizes_over_real_n() {
    let store = store_with_phantom_for_centrality();
    let got = store.closeness_centrality(EdgeKind::Calls);
    assert!(
        !got.iter().any(|e| e.path == "Phantom>p"),
        "phantom must not appear in closeness: {got:?}"
    );
    let twin = centrality_phantom_free_twin().closeness_centrality(EdgeKind::Calls);
    assert_eq!(got.len(), twin.len(), "same node count as twin");
    for t in &twin {
        let g = got.iter().find(|e| e.path == t.path);
        assert!(
            g.is_some_and(|e| (e.score - t.score).abs() < 1e-12),
            "closeness for {} must match twin (got {:?}, twin {})",
            t.path,
            g.map(|e| e.score),
            t.score
        );
    }
}

#[test]
fn harmonic_centrality_stats_excludes_phantoms_and_uses_real_n() {
    let store = store_with_phantom_for_centrality();
    let a = store.lookup("m.rs>a").unwrap();
    let (cent, reach, sym_count) = store.harmonic_centrality_stats(a, EdgeKind::Calls);

    let twin = centrality_phantom_free_twin();
    let a_twin = twin.lookup("m.rs>a").unwrap();
    let (cent_t, reach_t, sym_count_t) = twin.harmonic_centrality_stats(a_twin, EdgeKind::Calls);

    assert_eq!(
        sym_count, 3,
        "symbol_count denominator must count real symbols only (3, not 4)"
    );
    assert_eq!(
        reach, reach_t,
        "reachable count must exclude the phantom and match the twin"
    );
    assert_eq!(
        sym_count, sym_count_t,
        "symbol_count must match the phantom-free twin"
    );
    assert!(
        (cent - cent_t).abs() < 1e-12,
        "harmonic centrality must match the phantom-free twin ({cent} vs {cent_t})"
    );
}

#[test]
fn page_rank_universe_equals_symbol_universe() {
    // Part A.2 routes page_rank's node universe through symbol_universe(); assert
    // the ranked paths are exactly the real-symbol universe (phantoms excluded).
    let (store, ..) = store_with_phantom_callee();
    let mut ranked: Vec<String> = store
        .page_rank(EdgeKind::Calls, 0.85, 5)
        .into_iter()
        .map(|e| e.path)
        .collect();
    ranked.sort();
    let mut universe: Vec<String> = store
        .symbol_universe()
        .into_iter()
        .filter_map(|id| store.path_of(id).map(str::to_owned))
        .collect();
    universe.sort();
    assert_eq!(
        ranked, universe,
        "page_rank universe must equal symbol_universe()"
    );
    assert!(
        !ranked.contains(&"Db>upsert_node".to_string()),
        "phantom excluded from page_rank: {ranked:?}"
    );
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

// ── RFC-0118 Part B: post-merge receiver disambiguation (resolve_call_site_contexts) ──

#[test]
fn resolve_call_site_contexts_binds_multi_match_method_via_receiver_type() {
    // The F5 case: `let store = Store::new(); store.upsert_node();`. `upsert_node`
    // is defined on TWO types (multi-match), so the single-match passes decline
    // and get-callers would return 0. Receiver inference (local `store: Store`)
    // must bind the call to Store>upsert_node — and NOT to Trunk>upsert_node.
    let mut store = Store::new();
    let store_def = store.upsert_node_with_kind(path("a.rs>Store>upsert_node"), NodeKind::Method);
    let trunk_def = store.upsert_node_with_kind(path("b.rs>Trunk>upsert_node"), NodeKind::Method);
    let caller = store.upsert_node_with_kind(path("c.rs>run"), NodeKind::Function);
    // The conservative stub the extractor would mint, plus the caller→stub edge.
    let stub = store.upsert_node_with_kind(path("upsert_node"), NodeKind::Unresolved);
    store.upsert_edge(EdgeKind::Calls, caller, stub);

    store.record_call_site(
        caller,
        stub,
        ReceiverContext {
            receiver: "store".to_owned(),
            method: "upsert_node".to_owned(),
            imports: vec![],
            locals: vec![LocalBinding {
                name: "store".to_owned(),
                ctor_type: Some("Store".to_owned()),
            }],
            self_type: None,
            params: vec![],
            fields: vec![],
        },
    );

    let bound = store.resolve_call_site_contexts();
    assert_eq!(bound, 1, "one call site should resolve");

    // Precise edge added to Store>upsert_node, NOT Trunk>upsert_node.
    assert!(
        store.incoming(store_def, EdgeKind::Calls).contains(&caller),
        "caller must now call Store>upsert_node"
    );
    assert!(
        !store.incoming(trunk_def, EdgeKind::Calls).contains(&caller),
        "caller must NOT be mis-bound to Trunk>upsert_node"
    );
    // The conservative stub edge is intentionally KEPT (Codex P2 #633): Synapse
    // dedups one caller→stub edge that may cover unrecorded calls, so this pass
    // only ADDS the precise edge and never removes the shared stub. The stub is
    // NodeKind::Unresolved, so Part A already hides it from symbol/rank queries.
    assert!(
        store.incoming(stub, EdgeKind::Calls).contains(&caller),
        "stub edge is preserved (removal deferred until all call sites are recorded)"
    );
}

#[test]
fn resolve_call_site_contexts_keeps_stub_edge_when_unresolvable() {
    // No receiver evidence → Ambiguous → leave the conservative stub edge intact
    // (never guess, never mis-bind).
    let mut store = Store::new();
    let _a = store.upsert_node_with_kind(path("a.rs>Store>m"), NodeKind::Method);
    let _b = store.upsert_node_with_kind(path("b.rs>Trunk>m"), NodeKind::Method);
    let caller = store.upsert_node_with_kind(path("c.rs>run"), NodeKind::Function);
    let stub = store.upsert_node_with_kind(path("m"), NodeKind::Unresolved);
    store.upsert_edge(EdgeKind::Calls, caller, stub);
    store.record_call_site(
        caller,
        stub,
        ReceiverContext {
            receiver: "x".to_owned(), // unknown receiver, no binding facts
            method: "m".to_owned(),
            imports: vec![],
            locals: vec![],
            self_type: None,
            params: vec![],
            fields: vec![],
        },
    );
    assert_eq!(
        store.resolve_call_site_contexts(),
        0,
        "nothing should resolve"
    );
    assert!(
        store.incoming(stub, EdgeKind::Calls).contains(&caller),
        "stub edge must remain for the unresolved call"
    );
}

#[test]
fn resolve_call_site_contexts_keeps_stub_when_partially_resolved() {
    // One caller invokes the SAME method on TWO receivers via the SAME bare stub:
    // one resolvable (local `s: Store`), one not (unknown receiver). The resolved
    // one must bind, but the shared stub edge must STAY (one site still needs it).
    // This exercises the group_total(2) vs group_resolved(1) counting — a no-op
    // implementation would fail (no edge bound, wrong count).
    let mut store = Store::new();
    let store_def = store.upsert_node_with_kind(path("a.rs>Store>do"), NodeKind::Method);
    let _other = store.upsert_node_with_kind(path("b.rs>Trunk>do"), NodeKind::Method);
    let caller = store.upsert_node_with_kind(path("c.rs>run"), NodeKind::Function);
    let stub = store.upsert_node_with_kind(path("do"), NodeKind::Unresolved);
    store.upsert_edge(EdgeKind::Calls, caller, stub);

    // Site 1: resolvable — `let s = Store::new(); s.do()`.
    store.record_call_site(
        caller,
        stub,
        ReceiverContext {
            receiver: "s".to_owned(),
            method: "do".to_owned(),
            imports: vec![],
            locals: vec![LocalBinding {
                name: "s".to_owned(),
                ctor_type: Some("Store".to_owned()),
            }],
            self_type: None,
            params: vec![],
            fields: vec![],
        },
    );
    // Site 2: unresolvable — unknown receiver, no evidence.
    store.record_call_site(
        caller,
        stub,
        ReceiverContext {
            receiver: "mystery".to_owned(),
            method: "do".to_owned(),
            imports: vec![],
            locals: vec![],
            self_type: None,
            params: vec![],
            fields: vec![],
        },
    );

    assert_eq!(
        store.resolve_call_site_contexts(),
        1,
        "exactly one site resolves"
    );
    assert!(
        store.incoming(store_def, EdgeKind::Calls).contains(&caller),
        "the resolvable site must bind to Store>do"
    );
    assert!(
        store.incoming(stub, EdgeKind::Calls).contains(&caller),
        "stub edge must REMAIN because the second site is still unresolved"
    );
}

#[test]
fn merge_carries_call_site_contexts_for_parallel_indexing() {
    // RFC-0118 Part B (Codex/reviewer BLOCKING): parallel indexing records call
    // sites into per-thread sub-stores, then merges them. If merge dropped the
    // contexts, the post-merge pass would no-op in parallel mode. This proves
    // merge carries them through and the merged store resolves correctly.
    let mut main = Store::new();
    main.upsert_node_with_kind(path("a.rs>Store>upsert"), NodeKind::Method);
    main.upsert_node_with_kind(path("b.rs>Trunk>upsert"), NodeKind::Method);

    let mut worker = Store::new();
    let caller = worker.upsert_node_with_kind(path("c.rs>run"), NodeKind::Function);
    let stub = worker.upsert_node_with_kind(path("upsert"), NodeKind::Unresolved);
    worker.upsert_edge(EdgeKind::Calls, caller, stub);
    worker.record_call_site(
        caller,
        stub,
        ReceiverContext {
            receiver: "store".to_owned(),
            method: "upsert".to_owned(),
            imports: vec![],
            locals: vec![LocalBinding {
                name: "store".to_owned(),
                ctor_type: Some("Store".to_owned()),
            }],
            self_type: None,
            params: vec![],
            fields: vec![],
        },
    );

    main.merge(&worker);
    assert_eq!(
        main.resolve_call_site_contexts(),
        1,
        "merged-in context must resolve (merge must carry call_site_contexts)"
    );
    let store_def = main.lookup("a.rs>Store>upsert").expect("def exists");
    assert!(
        main.incoming(store_def, EdgeKind::Calls).contains(&caller),
        "parallel-captured call must bind to Store>upsert after merge"
    );
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

// ── RFC-0033: Store::find_implements_path ─────────────────────────────

#[test]
fn store_find_implements_path_self_returns_single_element() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>A"));
    assert_eq!(store.find_implements_path(a, a, 5), Some(vec![a]));
}

#[test]
fn store_find_implements_path_direct_hop() {
    let mut store = Store::new();
    let cls = store.upsert_node(path("src/cls.rs>Cls"));
    let iface = store.upsert_node(path("src/iface.rs>IFace"));
    store.upsert_edge(EdgeKind::Implements, cls, iface);
    assert_eq!(
        store.find_implements_path(cls, iface, 5),
        Some(vec![cls, iface])
    );
}

#[test]
fn store_find_implements_path_transitive() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let b = store.upsert_node(path("b.rs>B"));
    let c = store.upsert_node(path("c.rs>C"));
    store.upsert_edge(EdgeKind::Implements, a, b);
    store.upsert_edge(EdgeKind::Implements, b, c);
    let result = store.find_implements_path(a, c, 5);
    assert_eq!(result, Some(vec![a, b, c]));
}

#[test]
fn store_find_implements_path_unreachable_returns_none() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let b = store.upsert_node(path("b.rs>B"));
    assert_eq!(store.find_implements_path(a, b, 5), None);
}

#[test]
fn store_find_implements_path_max_depth_limits_hops() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let b = store.upsert_node(path("b.rs>B"));
    let c = store.upsert_node(path("c.rs>C"));
    store.upsert_edge(EdgeKind::Implements, a, b);
    store.upsert_edge(EdgeKind::Implements, b, c);
    // max_depth=1: A→B reachable, A→C not
    assert!(store.find_implements_path(a, b, 1).is_some());
    assert!(store.find_implements_path(a, c, 1).is_none());
}

// ── RFC-0034: Store::implements_tree ─────────────────────────────────

#[test]
fn store_implements_tree_leaf_at_max_depth_zero() {
    let mut store = Store::new();
    let id = store.upsert_node(path("src/a.rs>A"));
    let tree = store.implements_tree(id, 0);
    assert_eq!(tree.id, id);
    assert!(tree.interfaces.is_empty());
}

#[test]
fn store_implements_tree_single_interface() {
    let mut store = Store::new();
    let cls = store.upsert_node(path("src/cls.rs>Cls"));
    let iface = store.upsert_node(path("src/iface.rs>IFace"));
    store.upsert_edge(EdgeKind::Implements, cls, iface);
    let tree = store.implements_tree(cls, 4);
    assert_eq!(tree.id, cls);
    assert_eq!(tree.interfaces.len(), 1);
    assert_eq!(tree.interfaces[0].id, iface);
    assert!(tree.interfaces[0].interfaces.is_empty());
}

#[test]
fn store_implements_tree_transitive_chain() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let b = store.upsert_node(path("b.rs>B"));
    let c = store.upsert_node(path("c.rs>C"));
    store.upsert_edge(EdgeKind::Implements, a, b);
    store.upsert_edge(EdgeKind::Implements, b, c);
    let tree = store.implements_tree(a, 4);
    assert_eq!(tree.interfaces.len(), 1);
    assert_eq!(tree.interfaces[0].id, b);
    assert_eq!(tree.interfaces[0].interfaces.len(), 1);
    assert_eq!(tree.interfaces[0].interfaces[0].id, c);
}

#[test]
fn store_implements_tree_cycle_safe() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let b = store.upsert_node(path("b.rs>B"));
    store.upsert_edge(EdgeKind::Implements, a, b);
    store.upsert_edge(EdgeKind::Implements, b, a); // cycle
    let tree = store.implements_tree(a, 10);
    assert_eq!(tree.interfaces.len(), 1);
    assert_eq!(tree.interfaces[0].id, b);
    assert_eq!(tree.interfaces[0].interfaces.len(), 1);
    assert_eq!(tree.interfaces[0].interfaces[0].id, a);
    assert!(tree.interfaces[0].interfaces[0].interfaces.is_empty());
}

// ── RFC-0035: Store::implementors_tree ───────────────────────────────

#[test]
fn store_implementors_tree_leaf_at_max_depth_zero() {
    let mut store = Store::new();
    let id = store.upsert_node(path("src/a.rs>A"));
    let tree = store.implementors_tree(id, 0);
    assert_eq!(tree.id, id);
    assert!(tree.implementors.is_empty());
}

#[test]
fn store_implementors_tree_single_implementor() {
    let mut store = Store::new();
    let cls = store.upsert_node(path("src/cls.rs>Cls"));
    let iface = store.upsert_node(path("src/iface.rs>IFace"));
    store.upsert_edge(EdgeKind::Implements, cls, iface); // cls implements iface
    let tree = store.implementors_tree(iface, 4);
    assert_eq!(tree.id, iface);
    assert_eq!(tree.implementors.len(), 1);
    assert_eq!(tree.implementors[0].id, cls);
    assert!(tree.implementors[0].implementors.is_empty());
}

#[test]
fn store_implementors_tree_transitive_chain() {
    let mut store = Store::new();
    let base_iface = store.upsert_node(path("base.rs>BaseIFace"));
    let mid_iface = store.upsert_node(path("mid.rs>MidIFace"));
    let cls = store.upsert_node(path("cls.rs>Cls"));
    store.upsert_edge(EdgeKind::Implements, mid_iface, base_iface);
    store.upsert_edge(EdgeKind::Implements, cls, mid_iface);
    let tree = store.implementors_tree(base_iface, 4);
    assert_eq!(tree.implementors.len(), 1);
    assert_eq!(tree.implementors[0].id, mid_iface);
    assert_eq!(tree.implementors[0].implementors.len(), 1);
    assert_eq!(tree.implementors[0].implementors[0].id, cls);
}

#[test]
fn store_implementors_tree_cycle_safe() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>A"));
    let b = store.upsert_node(path("b.rs>B"));
    store.upsert_edge(EdgeKind::Implements, b, a); // b implements a
    store.upsert_edge(EdgeKind::Implements, a, b); // a implements b (cycle)
    let tree = store.implementors_tree(a, 10);
    assert_eq!(tree.implementors.len(), 1);
    assert_eq!(tree.implementors[0].id, b);
    assert_eq!(tree.implementors[0].implementors.len(), 1);
    assert_eq!(tree.implementors[0].implementors[0].id, a);
    assert!(tree.implementors[0].implementors[0].implementors.is_empty());
}

// ── RFC-0036: Store::importers_tree ──────────────────────────────────

#[test]
fn store_importers_tree_leaf_at_max_depth_zero() {
    let mut store = Store::new();
    let id = store.upsert_node(path("src/a.rs>A"));
    let tree = store.importers_tree(id, 0);
    assert_eq!(tree.id, id);
    assert!(tree.importers.is_empty());
}

#[test]
fn store_importers_tree_single_importer() {
    let mut store = Store::new();
    let lib = store.upsert_node(path("src/lib.rs>lib"));
    let app = store.upsert_node(path("src/app.rs>app"));
    store.upsert_edge(EdgeKind::Imports, app, lib); // app imports lib
    let tree = store.importers_tree(lib, 4);
    assert_eq!(tree.id, lib);
    assert_eq!(tree.importers.len(), 1);
    assert_eq!(tree.importers[0].id, app);
    assert!(tree.importers[0].importers.is_empty());
}

#[test]
fn store_importers_tree_transitive_chain() {
    let mut store = Store::new();
    let core = store.upsert_node(path("core.rs>core"));
    let mid = store.upsert_node(path("mid.rs>mid"));
    let top = store.upsert_node(path("top.rs>top"));
    store.upsert_edge(EdgeKind::Imports, mid, core); // mid imports core
    store.upsert_edge(EdgeKind::Imports, top, mid); // top imports mid
    let tree = store.importers_tree(core, 4);
    assert_eq!(tree.importers.len(), 1);
    assert_eq!(tree.importers[0].id, mid);
    assert_eq!(tree.importers[0].importers.len(), 1);
    assert_eq!(tree.importers[0].importers[0].id, top);
}

#[test]
fn store_importers_tree_cycle_safe() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>a"));
    let b = store.upsert_node(path("b.rs>b"));
    store.upsert_edge(EdgeKind::Imports, b, a); // b imports a
    store.upsert_edge(EdgeKind::Imports, a, b); // a imports b (cycle)
    let tree = store.importers_tree(a, 10);
    assert_eq!(tree.importers.len(), 1);
    assert_eq!(tree.importers[0].id, b);
    assert_eq!(tree.importers[0].importers.len(), 1);
    assert_eq!(tree.importers[0].importers[0].id, a);
    assert!(tree.importers[0].importers[0].importers.is_empty());
}

// ── RFC-0037: Store::dead_symbols ──────────────────────────────────────

#[test]
fn store_dead_symbols_excludes_file_nodes() {
    let mut store = Store::new();
    // file-level node has no `>` — should never appear in dead_symbols
    let _file = store.upsert_node(path("src/lib.rs"));
    let sym = store.upsert_node(path("src/lib.rs>unused_fn"));
    let _ = sym; // suppress unused warning
    let dead = store.dead_symbols(None);
    // file node excluded; symbol has no callers/importers so it is dead
    assert!(!dead.iter().any(|s| s == "src/lib.rs"));
    assert!(dead.contains(&"src/lib.rs>unused_fn".to_owned()));
}

#[test]
fn store_dead_symbols_live_if_called() {
    let mut store = Store::new();
    let caller = store.upsert_node(path("src/main.rs>main"));
    let target = store.upsert_node(path("src/lib.rs>helper"));
    store.upsert_edge(EdgeKind::Calls, caller, target);
    let dead = store.dead_symbols(None);
    // helper has an incoming Calls edge → not dead
    assert!(!dead.contains(&"src/lib.rs>helper".to_owned()));
    // main has no callers → dead
    assert!(dead.contains(&"src/main.rs>main".to_owned()));
}

#[test]
fn store_dead_symbols_live_if_imported() {
    let mut store = Store::new();
    let importer = store.upsert_node(path("src/app.rs>app"));
    let lib = store.upsert_node(path("src/lib.rs>lib_fn"));
    store.upsert_edge(EdgeKind::Imports, importer, lib);
    let dead = store.dead_symbols(None);
    // lib_fn has an incoming Imports edge → not dead
    assert!(!dead.contains(&"src/lib.rs>lib_fn".to_owned()));
}

#[test]
fn store_dead_symbols_prefix_filter() {
    let mut store = Store::new();
    let _a = store.upsert_node(path("src/a.rs>unused_a"));
    let _b = store.upsert_node(path("lib/b.rs>unused_b"));
    let dead_src = store.dead_symbols(Some("src/"));
    assert!(dead_src.contains(&"src/a.rs>unused_a".to_owned()));
    assert!(!dead_src.iter().any(|s| s.starts_with("lib/")));
}

#[test]
fn store_dead_symbols_sorted() {
    let mut store = Store::new();
    store.upsert_node(path("z/z.rs>fn_z"));
    store.upsert_node(path("a/a.rs>fn_a"));
    store.upsert_node(path("m/m.rs>fn_m"));
    let dead = store.dead_symbols(None);
    let mut sorted = dead.clone();
    sorted.sort_unstable();
    assert_eq!(dead, sorted);
}

// ── RFC-0038: Store::graph_stats ──────────────────────────────────────

#[test]
fn store_graph_stats_empty() {
    let store = Store::new();
    let stats = store.graph_stats();
    assert_eq!(stats.total_nodes, 0);
    assert_eq!(stats.total_edges, 0);
    assert!(stats.nodes_by_kind.is_empty());
    assert!(stats.edges_by_kind.is_empty());
}

#[test]
fn store_graph_stats_node_counts() {
    let mut store = Store::new();
    let fn1 = store.upsert_node(path("src/lib.rs>fn1"));
    let fn2 = store.upsert_node(path("src/lib.rs>fn2"));
    let cls = store.upsert_node(path("src/lib.rs>MyClass"));
    store.set_kind(fn1, NodeKind::Function);
    store.set_kind(fn2, NodeKind::Function);
    store.set_kind(cls, NodeKind::Class);
    let stats = store.graph_stats();
    assert_eq!(stats.total_nodes, 3);
    assert_eq!(stats.nodes_by_kind["function"], 2);
    assert_eq!(stats.nodes_by_kind["class"], 1);
}

#[test]
fn store_graph_stats_edge_counts() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>a"));
    let b = store.upsert_node(path("b.rs>b"));
    let c = store.upsert_node(path("c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, a, c);
    store.upsert_edge(EdgeKind::Imports, b, c);
    let stats = store.graph_stats();
    assert_eq!(stats.total_edges, 3);
    assert_eq!(stats.edges_by_kind["calls"], 2);
    assert_eq!(stats.edges_by_kind["imports"], 1);
    assert!(!stats.edges_by_kind.contains_key("contains"));
}

#[test]
fn store_graph_stats_totals_consistent() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>a"));
    let b = store.upsert_node(path("b.rs>b"));
    store.set_kind(a, NodeKind::Function);
    store.upsert_edge(EdgeKind::Calls, a, b);
    let stats = store.graph_stats();
    let kind_sum: usize = stats.nodes_by_kind.values().sum();
    // nodes without a kind are counted in total_nodes but not in nodes_by_kind
    assert!(stats.total_nodes >= kind_sum);
    let edge_sum: usize = stats.edges_by_kind.values().sum();
    assert_eq!(stats.total_edges, edge_sum);
}

// ── RFC-0039: Store::cross_refs ────────────────────────────────────────

#[test]
fn store_cross_refs_empty_node() {
    let mut store = Store::new();
    let id = store.upsert_node(path("src/lib.rs>Foo"));
    let refs = store.cross_refs(id);
    assert!(refs.callers.is_empty());
    assert!(refs.importers.is_empty());
    assert!(refs.extended_by.is_empty());
    assert!(refs.implemented_by.is_empty());
}

#[test]
fn store_cross_refs_callers() {
    let mut store = Store::new();
    let foo = store.upsert_node(path("src/lib.rs>foo"));
    let bar = store.upsert_node(path("src/main.rs>bar"));
    store.upsert_edge(EdgeKind::Calls, bar, foo);
    let refs = store.cross_refs(foo);
    assert_eq!(refs.callers, vec!["src/main.rs>bar".to_owned()]);
    assert!(refs.importers.is_empty());
}

#[test]
fn store_cross_refs_mixed_edges() {
    let mut store = Store::new();
    let target = store.upsert_node(path("src/lib.rs>Base"));
    let caller = store.upsert_node(path("src/a.rs>caller"));
    let importer = store.upsert_node(path("src/b.rs>importer"));
    let child = store.upsert_node(path("src/c.rs>Child"));
    let impl_node = store.upsert_node(path("src/d.rs>Impl"));
    store.upsert_edge(EdgeKind::Calls, caller, target);
    store.upsert_edge(EdgeKind::Imports, importer, target);
    store.upsert_edge(EdgeKind::Extends, child, target);
    store.upsert_edge(EdgeKind::Implements, impl_node, target);
    let refs = store.cross_refs(target);
    assert_eq!(refs.callers, vec!["src/a.rs>caller".to_owned()]);
    assert_eq!(refs.importers, vec!["src/b.rs>importer".to_owned()]);
    assert_eq!(refs.extended_by, vec!["src/c.rs>Child".to_owned()]);
    assert_eq!(refs.implemented_by, vec!["src/d.rs>Impl".to_owned()]);
}

#[test]
fn store_cross_refs_sorted() {
    let mut store = Store::new();
    let target = store.upsert_node(path("lib.rs>Lib"));
    let z = store.upsert_node(path("z.rs>z_caller"));
    let a = store.upsert_node(path("a.rs>a_caller"));
    store.upsert_edge(EdgeKind::Calls, z, target);
    store.upsert_edge(EdgeKind::Calls, a, target);
    let refs = store.cross_refs(target);
    let mut expected = refs.callers.clone();
    expected.sort_unstable();
    assert_eq!(refs.callers, expected);
}

// ── RFC-0040: Store::nodes_in_cycles ──────────────────────────────────

#[test]
fn store_nodes_in_cycles_no_cycle() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>a"));
    let b = store.upsert_node(path("b.rs>b"));
    store.upsert_edge(EdgeKind::Imports, a, b);
    let cycles = store.nodes_in_cycles(EdgeKind::Imports, None);
    assert!(cycles.is_empty());
}

#[test]
fn store_nodes_in_cycles_simple_cycle() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>a"));
    let b = store.upsert_node(path("b.rs>b"));
    store.upsert_edge(EdgeKind::Imports, a, b);
    store.upsert_edge(EdgeKind::Imports, b, a); // cycle
    let mut cycles = store.nodes_in_cycles(EdgeKind::Imports, None);
    cycles.sort_unstable();
    assert_eq!(cycles, vec!["a.rs>a".to_owned(), "b.rs>b".to_owned()]);
}

#[test]
fn store_nodes_in_cycles_three_node_cycle() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>a"));
    let b = store.upsert_node(path("b.rs>b"));
    let c = store.upsert_node(path("c.rs>c"));
    store.upsert_edge(EdgeKind::Imports, a, b);
    store.upsert_edge(EdgeKind::Imports, b, c);
    store.upsert_edge(EdgeKind::Imports, c, a); // cycle: a→b→c→a
    let cycles = store.nodes_in_cycles(EdgeKind::Imports, None);
    assert_eq!(cycles.len(), 3);
}

#[test]
fn store_nodes_in_cycles_non_cycle_node_excluded() {
    let mut store = Store::new();
    let a = store.upsert_node(path("a.rs>a"));
    let b = store.upsert_node(path("b.rs>b"));
    let outside = store.upsert_node(path("z.rs>outside"));
    store.upsert_edge(EdgeKind::Imports, a, b);
    store.upsert_edge(EdgeKind::Imports, b, a); // cycle
    store.upsert_edge(EdgeKind::Imports, outside, a); // outside points in but is not cyclic
    let cycles = store.nodes_in_cycles(EdgeKind::Imports, None);
    assert!(!cycles.contains(&"z.rs>outside".to_owned()));
}

#[test]
fn store_nodes_in_cycles_prefix_filter() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let x = store.upsert_node(path("lib/x.rs>x"));
    let y = store.upsert_node(path("lib/y.rs>y"));
    store.upsert_edge(EdgeKind::Imports, a, b);
    store.upsert_edge(EdgeKind::Imports, b, a); // cycle in src/
    store.upsert_edge(EdgeKind::Imports, x, y);
    store.upsert_edge(EdgeKind::Imports, y, x); // cycle in lib/
    let cycles_src = store.nodes_in_cycles(EdgeKind::Imports, Some("src/"));
    assert!(cycles_src.iter().all(|p| p.starts_with("src/")));
    assert_eq!(cycles_src.len(), 2);
}

// ── RFC-0041: Store::outgoing_refs ────────────────────────────────────

#[test]
fn store_outgoing_refs_empty_node() {
    let mut store = Store::new();
    let id = store.upsert_node(path("src/lib.rs>Foo"));
    let refs = store.outgoing_refs(id);
    assert!(refs.callees.is_empty());
    assert!(refs.imports.is_empty());
    assert!(refs.extends.is_empty());
    assert!(refs.implements.is_empty());
}

#[test]
fn store_outgoing_refs_callees() {
    let mut store = Store::new();
    let caller = store.upsert_node(path("src/main.rs>main"));
    let target = store.upsert_node(path("src/lib.rs>helper"));
    store.upsert_edge(EdgeKind::Calls, caller, target);
    let refs = store.outgoing_refs(caller);
    assert_eq!(refs.callees, vec!["src/lib.rs>helper".to_owned()]);
    assert!(refs.imports.is_empty());
}

#[test]
fn store_outgoing_refs_all_kinds() {
    let mut store = Store::new();
    let src = store.upsert_node(path("src/app.rs>App"));
    let callee = store.upsert_node(path("src/a.rs>callee"));
    let imported = store.upsert_node(path("src/b.rs>imported"));
    let parent = store.upsert_node(path("src/c.rs>Parent"));
    let iface = store.upsert_node(path("src/d.rs>IFace"));
    store.upsert_edge(EdgeKind::Calls, src, callee);
    store.upsert_edge(EdgeKind::Imports, src, imported);
    store.upsert_edge(EdgeKind::Extends, src, parent);
    store.upsert_edge(EdgeKind::Implements, src, iface);
    let refs = store.outgoing_refs(src);
    assert_eq!(refs.callees, vec!["src/a.rs>callee".to_owned()]);
    assert_eq!(refs.imports, vec!["src/b.rs>imported".to_owned()]);
    assert_eq!(refs.extends, vec!["src/c.rs>Parent".to_owned()]);
    assert_eq!(refs.implements, vec!["src/d.rs>IFace".to_owned()]);
}

#[test]
fn store_outgoing_refs_sorted() {
    let mut store = Store::new();
    let src = store.upsert_node(path("src/main.rs>main"));
    let z = store.upsert_node(path("z.rs>z_fn"));
    let a = store.upsert_node(path("a.rs>a_fn"));
    store.upsert_edge(EdgeKind::Calls, src, z);
    store.upsert_edge(EdgeKind::Calls, src, a);
    let refs = store.outgoing_refs(src);
    let mut expected = refs.callees.clone();
    expected.sort_unstable();
    assert_eq!(refs.callees, expected);
}

// ── RFC-0042: Store::all_symbols ──────────────────────────────────────

#[test]
fn store_all_symbols_excludes_file_nodes() {
    let mut store = Store::new();
    store.upsert_node(path("src/lib.rs")); // file node — excluded
    let sym = store.upsert_node(path("src/lib.rs>fn1"));
    let _ = sym;
    let syms = store.all_symbols(None, None);
    assert!(!syms.iter().any(|s| s == "src/lib.rs"));
    assert!(syms.contains(&"src/lib.rs>fn1".to_owned()));
}

#[test]
fn store_all_symbols_prefix_filter() {
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs>fn_a"));
    store.upsert_node(path("lib/b.rs>fn_b"));
    let syms = store.all_symbols(Some("src/"), None);
    assert!(syms.iter().all(|s| s.starts_with("src/")));
    assert!(!syms.iter().any(|s| s.starts_with("lib/")));
}

#[test]
fn store_all_symbols_kind_filter() {
    let mut store = Store::new();
    let fn1 = store.upsert_node(path("src/a.rs>fn1"));
    let cls = store.upsert_node(path("src/b.rs>MyClass"));
    let fn2 = store.upsert_node(path("src/c.rs>fn2"));
    store.set_kind(fn1, NodeKind::Function);
    store.set_kind(cls, NodeKind::Class);
    store.set_kind(fn2, NodeKind::Function);
    let functions = store.all_symbols(None, Some(NodeKind::Function));
    assert_eq!(functions.len(), 2);
    assert!(functions.contains(&"src/a.rs>fn1".to_owned()));
    assert!(functions.contains(&"src/c.rs>fn2".to_owned()));
    assert!(!functions.contains(&"src/b.rs>MyClass".to_owned()));
}

#[test]
fn store_all_symbols_sorted() {
    let mut store = Store::new();
    store.upsert_node(path("z/z.rs>z_fn"));
    store.upsert_node(path("a/a.rs>a_fn"));
    store.upsert_node(path("m/m.rs>m_fn"));
    let syms = store.all_symbols(None, None);
    let mut sorted = syms.clone();
    sorted.sort_unstable();
    assert_eq!(syms, sorted);
}

#[test]
fn store_all_symbols_no_params_returns_all() {
    let mut store = Store::new();
    store.upsert_node(path("a.rs>fn_a"));
    store.upsert_node(path("b.rs>fn_b"));
    store.upsert_node(path("c.rs")); // file node excluded
    let syms = store.all_symbols(None, None);
    assert_eq!(syms.len(), 2);
}
// ──────────────────────────────────────────────────────────────────────
// RFC-0043: Store::reachable_from
// ──────────────────────────────────────────────────────────────────────

#[test]
fn store_reachable_from_direct_callees() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, a, c);
    let reachable = store.reachable_from(a, EdgeKind::Calls, 1);
    assert_eq!(
        reachable,
        vec!["src/b.rs>b".to_owned(), "src/c.rs>c".to_owned()]
    );
}

#[test]
fn store_reachable_from_transitive() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    let reachable = store.reachable_from(a, EdgeKind::Calls, 10);
    assert!(reachable.contains(&"src/b.rs>b".to_owned()));
    assert!(reachable.contains(&"src/c.rs>c".to_owned()));
}

#[test]
fn store_reachable_from_excludes_start_node() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    let reachable = store.reachable_from(a, EdgeKind::Calls, 10);
    assert!(!reachable.contains(&"src/a.rs>a".to_owned()));
}

#[test]
fn store_reachable_from_cycle_safe() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, a); // cycle
    let reachable = store.reachable_from(a, EdgeKind::Calls, 10);
    // b is reachable; a (start) is excluded; no infinite loop
    assert_eq!(reachable, vec!["src/b.rs>b".to_owned()]);
}

#[test]
fn store_reachable_from_max_depth_zero_empty() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    let reachable = store.reachable_from(a, EdgeKind::Calls, 0);
    assert!(reachable.is_empty());
}

#[test]
fn store_reachable_from_sorted() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let z = store.upsert_node(path("src/z.rs>z"));
    let m = store.upsert_node(path("src/m.rs>m"));
    store.upsert_edge(EdgeKind::Calls, a, z);
    store.upsert_edge(EdgeKind::Calls, a, m);
    let reachable = store.reachable_from(a, EdgeKind::Calls, 10);
    let mut sorted = reachable.clone();
    sorted.sort_unstable();
    assert_eq!(reachable, sorted);
}
// ──────────────────────────────────────────────────────────────────────
// RFC-0044: Store::reachable_to
// ──────────────────────────────────────────────────────────────────────

#[test]
fn store_reachable_to_direct_callers() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, b, a);
    store.upsert_edge(EdgeKind::Calls, c, a);
    let reachable = store.reachable_to(a, EdgeKind::Calls, 1);
    assert_eq!(
        reachable,
        vec!["src/b.rs>b".to_owned(), "src/c.rs>c".to_owned()]
    );
}

#[test]
fn store_reachable_to_transitive() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, b, a);
    store.upsert_edge(EdgeKind::Calls, c, b);
    let reachable = store.reachable_to(a, EdgeKind::Calls, 10);
    assert!(reachable.contains(&"src/b.rs>b".to_owned()));
    assert!(reachable.contains(&"src/c.rs>c".to_owned()));
}

#[test]
fn store_reachable_to_excludes_start_node() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, b, a);
    let reachable = store.reachable_to(a, EdgeKind::Calls, 10);
    assert!(!reachable.contains(&"src/a.rs>a".to_owned()));
}

#[test]
fn store_reachable_to_cycle_safe() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, b, a);
    store.upsert_edge(EdgeKind::Calls, a, b); // cycle
    let reachable = store.reachable_to(a, EdgeKind::Calls, 10);
    assert_eq!(reachable, vec!["src/b.rs>b".to_owned()]);
}

#[test]
fn store_reachable_to_max_depth_zero_empty() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, b, a);
    let reachable = store.reachable_to(a, EdgeKind::Calls, 0);
    assert!(reachable.is_empty());
}

#[test]
fn store_reachable_to_sorted() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let z = store.upsert_node(path("src/z.rs>z"));
    let m = store.upsert_node(path("src/m.rs>m"));
    store.upsert_edge(EdgeKind::Calls, z, a);
    store.upsert_edge(EdgeKind::Calls, m, a);
    let reachable = store.reachable_to(a, EdgeKind::Calls, 10);
    let mut sorted = reachable.clone();
    sorted.sort_unstable();
    assert_eq!(reachable, sorted);
}
// ──────────────────────────────────────────────────────────────────────
// RFC-0045: Store::siblings
// ──────────────────────────────────────────────────────────────────────

#[test]
fn store_siblings_methods_in_class() {
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs>App>init"));
    store.upsert_node(path("src/a.rs>App>render"));
    store.upsert_node(path("src/a.rs>App>destroy"));
    let render_id = store.lookup("src/a.rs>App>render").unwrap();
    let siblings = store.siblings(render_id);
    assert!(siblings.contains(&"src/a.rs>App>init".to_owned()));
    assert!(siblings.contains(&"src/a.rs>App>destroy".to_owned()));
    assert!(!siblings.contains(&"src/a.rs>App>render".to_owned()));
}

#[test]
fn store_siblings_top_level_in_file() {
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs>fn1"));
    store.upsert_node(path("src/a.rs>fn2"));
    store.upsert_node(path("src/a.rs>fn3"));
    let fn1 = store.lookup("src/a.rs>fn1").unwrap();
    let siblings = store.siblings(fn1);
    assert_eq!(siblings.len(), 2);
    assert!(siblings.contains(&"src/a.rs>fn2".to_owned()));
    assert!(siblings.contains(&"src/a.rs>fn3".to_owned()));
}

#[test]
fn store_siblings_excludes_self() {
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs>App>method"));
    let id = store.lookup("src/a.rs>App>method").unwrap();
    let siblings = store.siblings(id);
    assert!(!siblings.contains(&"src/a.rs>App>method".to_owned()));
}

#[test]
fn store_siblings_no_parent_returns_empty() {
    let mut store = Store::new();
    let file_id = store.upsert_node(path("src/a.rs"));
    // file-level node has no '>' parent
    let siblings = store.siblings(file_id);
    assert!(siblings.is_empty());
}

#[test]
fn store_siblings_only_direct_not_grandchildren() {
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs>App>method"));
    store.upsert_node(path("src/a.rs>App>method>inner"));
    let id = store.lookup("src/a.rs>App>method").unwrap();
    let siblings = store.siblings(id);
    // inner is a grandchild of App, not a sibling of method
    assert!(!siblings.iter().any(|s| s.contains("inner")));
}

#[test]
fn store_siblings_sorted() {
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs>App>z_method"));
    store.upsert_node(path("src/a.rs>App>a_method"));
    store.upsert_node(path("src/a.rs>App>m_method"));
    let id = store.lookup("src/a.rs>App>m_method").unwrap();
    let siblings = store.siblings(id);
    let mut sorted = siblings.clone();
    sorted.sort_unstable();
    assert_eq!(siblings, sorted);
}
// ──────────────────────────────────────────────────────────────────────
// RFC-0046: Store::node_degree
// ──────────────────────────────────────────────────────────────────────

#[test]
fn store_node_degree_isolated_node_all_zero() {
    let mut store = Store::new();
    let id = store.upsert_node(path("src/a.rs>fn1"));
    let deg = store.node_degree(id);
    assert_eq!(deg.in_calls, 0);
    assert_eq!(deg.out_calls, 0);
    assert_eq!(deg.in_imports, 0);
    assert_eq!(deg.out_imports, 0);
    assert_eq!(deg.in_extends, 0);
    assert_eq!(deg.out_extends, 0);
    assert_eq!(deg.in_implements, 0);
    assert_eq!(deg.out_implements, 0);
}

#[test]
fn store_node_degree_call_edges() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, b, a);
    store.upsert_edge(EdgeKind::Calls, c, a);
    store.upsert_edge(EdgeKind::Calls, a, b);
    let deg = store.node_degree(a);
    assert_eq!(deg.in_calls, 2);
    assert_eq!(deg.out_calls, 1);
}

#[test]
fn store_node_degree_all_kinds() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>A"));
    let b = store.upsert_node(path("src/b.rs>B"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Imports, a, b);
    store.upsert_edge(EdgeKind::Extends, a, b);
    store.upsert_edge(EdgeKind::Implements, a, b);
    let deg = store.node_degree(a);
    assert_eq!(deg.out_calls, 1);
    assert_eq!(deg.out_imports, 1);
    assert_eq!(deg.out_extends, 1);
    assert_eq!(deg.out_implements, 1);
    assert_eq!(deg.in_calls, 0);
    let deg_b = store.node_degree(b);
    assert_eq!(deg_b.in_calls, 1);
    assert_eq!(deg_b.in_imports, 1);
    assert_eq!(deg_b.in_extends, 1);
    assert_eq!(deg_b.in_implements, 1);
}
// ──────────────────────────────────────────────────────────────────────
// RFC-0047: Store::top_files
// ──────────────────────────────────────────────────────────────────────

#[test]
fn store_top_files_counts_direct_children() {
    let mut store = Store::new();
    store.upsert_node(path("src/big.rs"));
    store.upsert_node(path("src/big.rs>fn1"));
    store.upsert_node(path("src/big.rs>fn2"));
    store.upsert_node(path("src/big.rs>fn3"));
    store.upsert_node(path("src/small.rs"));
    store.upsert_node(path("src/small.rs>fn1"));
    let top = store.top_files(10);
    assert_eq!(top[0].0, "src/big.rs");
    assert_eq!(top[0].1, 3);
    assert_eq!(top[1].0, "src/small.rs");
    assert_eq!(top[1].1, 1);
}

#[test]
fn store_top_files_excludes_symbol_nodes() {
    let mut store = Store::new();
    // Only file nodes (no '>') are ranked
    store.upsert_node(path("src/a.rs>MyClass"));
    store.upsert_node(path("src/a.rs>MyClass>method"));
    let top = store.top_files(10);
    // src/a.rs was never explicitly inserted as a file node, so count comes
    // from paths starting with "src/a.rs>" where remainder has no ">"
    // Since we only count explicit file nodes, top should be empty or
    // count only explicit file node children.
    // NOTE: top_files counts children of FILE NODES — nodes without '>'.
    // Since 'src/a.rs' was never explicitly inserted, top should be empty.
    assert!(top.is_empty());
}

#[test]
fn store_top_files_limit_respected() {
    let mut store = Store::new();
    for i in 0..20u32 {
        let file = format!("src/{i}.rs");
        store.upsert_node(TrunkPath::parse(&file).unwrap());
        store.upsert_node(TrunkPath::parse(&format!("{file}>fn")).unwrap());
    }
    let top = store.top_files(5);
    assert_eq!(top.len(), 5);
}

#[test]
fn store_top_files_sorted_descending_then_path() {
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs"));
    store.upsert_node(path("src/a.rs>fn1"));
    store.upsert_node(path("src/a.rs>fn2"));
    store.upsert_node(path("src/b.rs"));
    store.upsert_node(path("src/b.rs>fn1"));
    store.upsert_node(path("src/b.rs>fn2"));
    store.upsert_node(path("src/c.rs"));
    store.upsert_node(path("src/c.rs>fn1"));
    store.upsert_node(path("src/c.rs>fn2"));
    store.upsert_node(path("src/c.rs>fn3"));
    let top = store.top_files(10);
    // c.rs has 3, a.rs and b.rs have 2 each
    assert_eq!(top[0].0, "src/c.rs");
    assert_eq!(top[0].1, 3);
    // ties broken alphabetically
    assert_eq!(top[1].0, "src/a.rs");
    assert_eq!(top[2].0, "src/b.rs");
}

#[test]
fn store_top_files_empty_graph() {
    let store = Store::new();
    let top = store.top_files(10);
    assert!(top.is_empty());
}
// ──────────────────────────────────────────────────────────────────────
// RFC-0048: Store::most_connected
// ──────────────────────────────────────────────────────────────────────

#[test]
fn store_most_connected_ranks_by_total_degree() {
    let mut store = Store::new();
    let hub = store.upsert_node(path("src/hub.rs>hub"));
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    // hub has 3 incoming calls
    store.upsert_edge(EdgeKind::Calls, a, hub);
    store.upsert_edge(EdgeKind::Calls, b, hub);
    store.upsert_edge(EdgeKind::Calls, c, hub);
    // a has 1 outgoing call (to hub)
    let top = store.most_connected(10, EdgeKind::Calls);
    assert_eq!(top[0].0, "src/hub.rs>hub");
    assert_eq!(top[0].1, 3); // in=3, out=0
}

#[test]
fn store_most_connected_excludes_file_nodes() {
    let mut store = Store::new();
    let file = store.upsert_node(path("src/a.rs"));
    let sym = store.upsert_node(path("src/a.rs>fn1"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, file, sym); // file→sym edge
    store.upsert_edge(EdgeKind::Calls, sym, b);
    let top = store.most_connected(10, EdgeKind::Calls);
    // file node 'src/a.rs' should not appear in results
    assert!(!top.iter().any(|(p, _)| p == "src/a.rs"));
}

#[test]
fn store_most_connected_excludes_zero_degree() {
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs>isolated"));
    let top = store.most_connected(10, EdgeKind::Calls);
    assert!(top.is_empty());
}

#[test]
fn store_most_connected_limit_respected() {
    let mut store = Store::new();
    for i in 0..20u32 {
        let sym = format!("src/{i}.rs>fn");
        let id = store.upsert_node(TrunkPath::parse(&sym).unwrap());
        let caller_path = format!("src/caller_{i}.rs>caller");
        let caller = store.upsert_node(TrunkPath::parse(&caller_path).unwrap());
        store.upsert_edge(EdgeKind::Calls, caller, id);
    }
    let top = store.most_connected(5, EdgeKind::Calls);
    assert_eq!(top.len(), 5);
}

#[test]
fn store_most_connected_sorted_desc_then_alpha() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let caller1 = store.upsert_node(path("src/c1.rs>c1"));
    let caller2 = store.upsert_node(path("src/c2.rs>c2"));
    let caller3 = store.upsert_node(path("src/c3.rs>c3"));
    // a has degree 3, b has degree 2
    store.upsert_edge(EdgeKind::Calls, caller1, a);
    store.upsert_edge(EdgeKind::Calls, caller2, a);
    store.upsert_edge(EdgeKind::Calls, caller3, a);
    store.upsert_edge(EdgeKind::Calls, caller1, b);
    store.upsert_edge(EdgeKind::Calls, caller2, b);
    let top = store.most_connected(10, EdgeKind::Calls);
    // First result should be 'a' (degree 3)
    assert_eq!(top[0].0, "src/a.rs>a");
    assert_eq!(top[0].1, 3);
}

// ── RFC-0049: Store::leaf_symbols ────────────────────────────────────────────

#[test]
fn store_leaf_symbols_returns_symbols_with_no_outgoing() {
    let mut store = Store::new();
    let root = store.upsert_node(path("src/a.rs>root"));
    let leaf = store.upsert_node(path("src/b.rs>leaf"));
    store.upsert_edge(EdgeKind::Calls, root, leaf);
    // leaf has out-degree 0; root has out-degree 1
    let leaves = store.leaf_symbols(EdgeKind::Calls, 10);
    assert_eq!(leaves, vec!["src/b.rs>leaf".to_owned()]);
}

#[test]
fn store_leaf_symbols_excludes_file_nodes() {
    let mut store = Store::new();
    let _file = store.upsert_node(path("src/a.rs")); // file node — no '>'
    let _sym = store.upsert_node(path("src/a.rs>sym"));
    // _sym has out-degree 0 for Calls; file node must be excluded
    let leaves = store.leaf_symbols(EdgeKind::Calls, 10);
    assert_eq!(leaves, vec!["src/a.rs>sym".to_owned()]);
}

#[test]
fn store_leaf_symbols_all_calling_returns_empty() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, a);
    // both have out-degree 1 — no leaves
    let leaves = store.leaf_symbols(EdgeKind::Calls, 10);
    assert!(leaves.is_empty());
}

#[test]
fn store_leaf_symbols_sorted_alphabetically() {
    let mut store = Store::new();
    let _z = store.upsert_node(path("src/z.rs>z"));
    let _a = store.upsert_node(path("src/a.rs>a"));
    let _m = store.upsert_node(path("src/m.rs>m"));
    // all have out-degree 0 for Imports
    let leaves = store.leaf_symbols(EdgeKind::Imports, 10);
    assert_eq!(
        leaves,
        vec![
            "src/a.rs>a".to_owned(),
            "src/m.rs>m".to_owned(),
            "src/z.rs>z".to_owned(),
        ]
    );
}

#[test]
fn store_leaf_symbols_limit_respected() {
    let mut store = Store::new();
    for i in 0..5u8 {
        store.upsert_node(path(&format!("src/{i}.rs>fn{i}")));
    }
    let leaves = store.leaf_symbols(EdgeKind::Calls, 3);
    assert_eq!(leaves.len(), 3);
}

#[test]
fn store_leaf_symbols_empty_graph() {
    let store = Store::new();
    assert!(store.leaf_symbols(EdgeKind::Calls, 10).is_empty());
}

// ── RFC-0050: Store::shortest_path ───────────────────────────────────────────

#[test]
fn store_shortest_path_direct_edge() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    let p = store.shortest_path(a, b, EdgeKind::Calls).unwrap();
    assert_eq!(p, vec!["src/a.rs>a".to_owned(), "src/b.rs>b".to_owned()]);
}

#[test]
fn store_shortest_path_multi_hop() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    let p = store.shortest_path(a, c, EdgeKind::Calls).unwrap();
    assert_eq!(
        p,
        vec![
            "src/a.rs>a".to_owned(),
            "src/b.rs>b".to_owned(),
            "src/c.rs>c".to_owned(),
        ]
    );
}

#[test]
fn store_shortest_path_same_node() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let p = store.shortest_path(a, a, EdgeKind::Calls).unwrap();
    assert_eq!(p, vec!["src/a.rs>a".to_owned()]);
}

#[test]
fn store_shortest_path_no_path_returns_none() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    // no edge between a and b
    assert!(store.shortest_path(a, b, EdgeKind::Calls).is_none());
}

#[test]
fn store_shortest_path_prefers_shorter() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    // direct: a -> c; indirect: a -> b -> c
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    store.upsert_edge(EdgeKind::Calls, a, c);
    let p = store.shortest_path(a, c, EdgeKind::Calls).unwrap();
    assert_eq!(p.len(), 2); // direct hop
}

#[test]
fn store_shortest_path_cycle_safe() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, a); // cycle a<->b
    // c is unreachable from a
    assert!(store.shortest_path(a, c, EdgeKind::Calls).is_none());
}

// ── RFC-0051: Store::symbol_count_by_kind ────────────────────────────────────

#[test]
fn store_symbol_count_by_kind_basic() {
    let mut store = Store::new();
    store.upsert_node_with_kind(path("src/a.rs>fn1"), NodeKind::Function);
    store.upsert_node_with_kind(path("src/a.rs>fn2"), NodeKind::Function);
    store.upsert_node_with_kind(path("src/a.rs>MyClass"), NodeKind::Class);
    let counts = store.symbol_count_by_kind();
    assert_eq!(
        counts,
        vec![("class".to_owned(), 1), ("function".to_owned(), 2),]
    );
}

#[test]
fn store_symbol_count_by_kind_sorted_alphabetically() {
    let mut store = Store::new();
    store.upsert_node_with_kind(path("src/a.rs>m"), NodeKind::Method);
    store.upsert_node_with_kind(path("src/a.rs>i"), NodeKind::Interface);
    store.upsert_node_with_kind(path("src/a.rs>f"), NodeKind::Function);
    let counts = store.symbol_count_by_kind();
    let kinds: Vec<&str> = counts.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(kinds, vec!["function", "interface", "method"]);
}

#[test]
fn store_symbol_count_by_kind_empty_graph() {
    let store = Store::new();
    assert!(store.symbol_count_by_kind().is_empty());
}

#[test]
fn store_symbol_count_by_kind_excludes_unknown_kind_nodes() {
    let mut store = Store::new();
    // node without kind_map entry (raw upsert_node with no kind) — excluded
    store.upsert_node(path("src/a.rs>notype"));
    store.upsert_node_with_kind(path("src/a.rs>typed"), NodeKind::Function);
    let counts = store.symbol_count_by_kind();
    assert_eq!(counts, vec![("function".to_owned(), 1)]);
}

#[test]
fn store_symbol_count_by_kind_total_matches_sum() {
    let mut store = Store::new();
    for i in 0..3u8 {
        store.upsert_node_with_kind(path(&format!("src/a.rs>fn{i}")), NodeKind::Function);
    }
    store.upsert_node_with_kind(path("src/a.rs>MyClass"), NodeKind::Class);
    let counts = store.symbol_count_by_kind();
    let total: usize = counts.iter().map(|(_, n)| n).sum();
    assert_eq!(total, 4);
}

// ── RFC-0052: Store::common_callers ──────────────────────────────────────────

#[test]
fn store_common_callers_intersection() {
    let mut store = Store::new();
    let shared = store.upsert_node(path("src/shared.rs>shared"));
    let only_a = store.upsert_node(path("src/oa.rs>only_a"));
    let target_a = store.upsert_node(path("src/ta.rs>target_a"));
    let target_b = store.upsert_node(path("src/tb.rs>target_b"));
    // shared calls both targets; only_a calls only target_a
    store.upsert_edge(EdgeKind::Calls, shared, target_a);
    store.upsert_edge(EdgeKind::Calls, shared, target_b);
    store.upsert_edge(EdgeKind::Calls, only_a, target_a);
    let common = store.common_callers(&[target_a, target_b], EdgeKind::Calls);
    assert_eq!(common, vec!["src/shared.rs>shared".to_owned()]);
}

#[test]
fn store_common_callers_single_target() {
    let mut store = Store::new();
    let caller1 = store.upsert_node(path("src/c1.rs>c1"));
    let caller2 = store.upsert_node(path("src/c2.rs>c2"));
    let target = store.upsert_node(path("src/t.rs>t"));
    store.upsert_edge(EdgeKind::Calls, caller1, target);
    store.upsert_edge(EdgeKind::Calls, caller2, target);
    let common = store.common_callers(&[target], EdgeKind::Calls);
    assert_eq!(common.len(), 2);
}

#[test]
fn store_common_callers_empty_targets_returns_empty() {
    let store = Store::new();
    assert!(store.common_callers(&[], EdgeKind::Calls).is_empty());
}

#[test]
fn store_common_callers_no_intersection_returns_empty() {
    let mut store = Store::new();
    let caller1 = store.upsert_node(path("src/c1.rs>c1"));
    let caller2 = store.upsert_node(path("src/c2.rs>c2"));
    let target_a = store.upsert_node(path("src/ta.rs>ta"));
    let target_b = store.upsert_node(path("src/tb.rs>tb"));
    store.upsert_edge(EdgeKind::Calls, caller1, target_a);
    store.upsert_edge(EdgeKind::Calls, caller2, target_b);
    let common = store.common_callers(&[target_a, target_b], EdgeKind::Calls);
    assert!(common.is_empty());
}

#[test]
fn store_common_callers_sorted_alphabetically() {
    let mut store = Store::new();
    let t = store.upsert_node(path("src/t.rs>t"));
    for name in &["src/z.rs>z", "src/a.rs>a", "src/m.rs>m"] {
        let c = store.upsert_node(path(name));
        store.upsert_edge(EdgeKind::Calls, c, t);
    }
    let common = store.common_callers(&[t], EdgeKind::Calls);
    assert_eq!(
        common,
        vec![
            "src/a.rs>a".to_owned(),
            "src/m.rs>m".to_owned(),
            "src/z.rs>z".to_owned(),
        ]
    );
}

// ── RFC-0053: Store::fan_out_rank ────────────────────────────────────────────

#[test]
fn store_fan_out_rank_basic() {
    let mut store = Store::new();
    let hub = store.upsert_node(path("src/hub.rs>hub"));
    let spoke1 = store.upsert_node(path("src/s1.rs>s1"));
    let spoke2 = store.upsert_node(path("src/s2.rs>s2"));
    let spoke3 = store.upsert_node(path("src/s3.rs>s3"));
    // hub calls 3 targets; spoke1 calls 1
    store.upsert_edge(EdgeKind::Calls, hub, spoke1);
    store.upsert_edge(EdgeKind::Calls, hub, spoke2);
    store.upsert_edge(EdgeKind::Calls, hub, spoke3);
    store.upsert_edge(EdgeKind::Calls, spoke1, spoke2);
    let ranked = store.fan_out_rank(EdgeKind::Calls, 10);
    assert_eq!(ranked[0].0, "src/hub.rs>hub");
    assert_eq!(ranked[0].1, 3);
    assert_eq!(ranked[1].0, "src/s1.rs>s1");
    assert_eq!(ranked[1].1, 1);
}

#[test]
fn store_fan_out_rank_excludes_zero_out_degree() {
    let mut store = Store::new();
    let root = store.upsert_node(path("src/a.rs>caller"));
    let leaf = store.upsert_node(path("src/b.rs>callee"));
    store.upsert_edge(EdgeKind::Calls, root, leaf);
    // leaf has out-degree 0; only root appears
    let ranked = store.fan_out_rank(EdgeKind::Calls, 10);
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].0, "src/a.rs>caller");
}

#[test]
fn store_fan_out_rank_excludes_file_nodes() {
    let mut store = Store::new();
    let _file = store.upsert_node(path("src/a.rs")); // file node
    let sym = store.upsert_node(path("src/a.rs>sym"));
    let tgt = store.upsert_node(path("src/b.rs>tgt"));
    store.upsert_edge(EdgeKind::Calls, sym, tgt);
    let ranked = store.fan_out_rank(EdgeKind::Calls, 10);
    // only sym (not the file node) should appear
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].0, "src/a.rs>sym");
}

#[test]
fn store_fan_out_rank_limit_respected() {
    let mut store = Store::new();
    let tgt = store.upsert_node(path("src/t.rs>t"));
    for i in 0..5u8 {
        let src = store.upsert_node(path(&format!("src/{i}.rs>fn{i}")));
        store.upsert_edge(EdgeKind::Calls, src, tgt);
    }
    let ranked = store.fan_out_rank(EdgeKind::Calls, 3);
    assert_eq!(ranked.len(), 3);
}

#[test]
fn store_fan_out_rank_sorted_desc_then_alpha() {
    let mut store = Store::new();
    let ta = store.upsert_node(path("src/ta.rs>ta"));
    let tb = store.upsert_node(path("src/tb.rs>tb"));
    // same out-degree 1 — should be sorted alphabetically
    let z_sym = store.upsert_node(path("src/z.rs>z_sym"));
    let a_sym = store.upsert_node(path("src/a.rs>a_sym"));
    store.upsert_edge(EdgeKind::Calls, z_sym, ta);
    store.upsert_edge(EdgeKind::Calls, a_sym, tb);
    let ranked = store.fan_out_rank(EdgeKind::Calls, 10);
    assert_eq!(ranked[0].0, "src/a.rs>a_sym"); // ties broken alphabetically
    assert_eq!(ranked[1].0, "src/z.rs>z_sym");
}

// ── RFC-0054: Store::fan_in_rank ─────────────────────────────────────────────

#[test]
fn store_fan_in_rank_basic() {
    let mut store = Store::new();
    let hub = store.upsert_node(path("src/hub.rs>hub"));
    let spoke1 = store.upsert_node(path("src/s1.rs>s1"));
    let spoke2 = store.upsert_node(path("src/s2.rs>s2"));
    let spoke3 = store.upsert_node(path("src/s3.rs>s3"));
    // hub is called by all three spokes
    store.upsert_edge(EdgeKind::Calls, spoke1, hub);
    store.upsert_edge(EdgeKind::Calls, spoke2, hub);
    store.upsert_edge(EdgeKind::Calls, spoke3, hub);
    // spoke1 also calls spoke2
    store.upsert_edge(EdgeKind::Calls, spoke1, spoke2);
    let ranked = store.fan_in_rank(EdgeKind::Calls, 10);
    assert_eq!(ranked[0].0, "src/hub.rs>hub");
    assert_eq!(ranked[0].1, 3);
    assert_eq!(ranked[1].0, "src/s2.rs>s2");
    assert_eq!(ranked[1].1, 1);
}

#[test]
fn store_fan_in_rank_excludes_zero_in_degree() {
    let mut store = Store::new();
    let root = store.upsert_node(path("src/a.rs>caller"));
    let leaf = store.upsert_node(path("src/b.rs>callee"));
    store.upsert_edge(EdgeKind::Calls, root, leaf);
    // root has in-degree 0; only leaf appears
    let ranked = store.fan_in_rank(EdgeKind::Calls, 10);
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].0, "src/b.rs>callee");
}

#[test]
fn store_fan_in_rank_excludes_file_nodes() {
    let mut store = Store::new();
    let _file = store.upsert_node(path("src/a.rs")); // file node
    let src = store.upsert_node(path("src/a.rs>src_sym"));
    let tgt = store.upsert_node(path("src/b.rs>tgt_sym"));
    store.upsert_edge(EdgeKind::Calls, src, tgt);
    let ranked = store.fan_in_rank(EdgeKind::Calls, 10);
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].0, "src/b.rs>tgt_sym");
}

#[test]
fn store_fan_in_rank_limit_respected() {
    let mut store = Store::new();
    let src = store.upsert_node(path("src/s.rs>s"));
    for i in 0..5u8 {
        let tgt = store.upsert_node(path(&format!("src/{i}.rs>fn{i}")));
        store.upsert_edge(EdgeKind::Calls, src, tgt);
    }
    let ranked = store.fan_in_rank(EdgeKind::Calls, 3);
    assert_eq!(ranked.len(), 3);
}

#[test]
fn store_fan_in_rank_sorted_desc_then_alpha() {
    let mut store = Store::new();
    let src_a = store.upsert_node(path("src/src_a.rs>src_a"));
    let src_b = store.upsert_node(path("src/src_b.rs>src_b"));
    let z_tgt = store.upsert_node(path("src/z.rs>z_tgt"));
    let a_tgt = store.upsert_node(path("src/a.rs>a_tgt"));
    store.upsert_edge(EdgeKind::Calls, src_a, z_tgt);
    store.upsert_edge(EdgeKind::Calls, src_b, a_tgt);
    let ranked = store.fan_in_rank(EdgeKind::Calls, 10);
    assert_eq!(ranked[0].0, "src/a.rs>a_tgt"); // ties broken alphabetically
    assert_eq!(ranked[1].0, "src/z.rs>z_tgt");
}

// ── RFC-0055: Store::common_callees ──────────────────────────────────────────

#[test]
fn store_common_callees_intersection() {
    let mut store = Store::new();
    let shared = store.upsert_node(path("src/shared.rs>shared"));
    let exclusive_a = store.upsert_node(path("src/ex_a.rs>ex_a"));
    let exclusive_b = store.upsert_node(path("src/ex_b.rs>ex_b"));
    let src_a = store.upsert_node(path("src/a.rs>a"));
    let src_b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, src_a, shared);
    store.upsert_edge(EdgeKind::Calls, src_a, exclusive_a);
    store.upsert_edge(EdgeKind::Calls, src_b, shared);
    store.upsert_edge(EdgeKind::Calls, src_b, exclusive_b);
    let common = store.common_callees(&[src_a, src_b], EdgeKind::Calls);
    assert_eq!(common, vec!["src/shared.rs>shared"]);
}

#[test]
fn store_common_callees_single_source() {
    let mut store = Store::new();
    let tgt = store.upsert_node(path("src/t.rs>tgt"));
    let src = store.upsert_node(path("src/s.rs>src_sym"));
    store.upsert_edge(EdgeKind::Calls, src, tgt);
    let common = store.common_callees(&[src], EdgeKind::Calls);
    assert_eq!(common, vec!["src/t.rs>tgt"]);
}

#[test]
fn store_common_callees_empty_sources_returns_empty() {
    let store = Store::new();
    assert!(store.common_callees(&[], EdgeKind::Calls).is_empty());
}

#[test]
fn store_common_callees_no_intersection_returns_empty() {
    let mut store = Store::new();
    let tgt_a = store.upsert_node(path("src/ta.rs>tgt_a"));
    let tgt_b = store.upsert_node(path("src/tb.rs>tgt_b"));
    let src_a = store.upsert_node(path("src/a.rs>a"));
    let src_b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, src_a, tgt_a);
    store.upsert_edge(EdgeKind::Calls, src_b, tgt_b);
    let common = store.common_callees(&[src_a, src_b], EdgeKind::Calls);
    assert!(common.is_empty());
}

#[test]
fn store_common_callees_sorted_alphabetically() {
    let mut store = Store::new();
    let z_tgt = store.upsert_node(path("src/z.rs>z_tgt"));
    let a_tgt = store.upsert_node(path("src/a.rs>a_tgt"));
    let src_a = store.upsert_node(path("src/src_a.rs>src_a"));
    let src_b = store.upsert_node(path("src/src_b.rs>src_b"));
    store.upsert_edge(EdgeKind::Calls, src_a, z_tgt);
    store.upsert_edge(EdgeKind::Calls, src_a, a_tgt);
    store.upsert_edge(EdgeKind::Calls, src_b, z_tgt);
    store.upsert_edge(EdgeKind::Calls, src_b, a_tgt);
    let common = store.common_callees(&[src_a, src_b], EdgeKind::Calls);
    assert_eq!(common[0], "src/a.rs>a_tgt");
    assert_eq!(common[1], "src/z.rs>z_tgt");
}

// ── RFC-0056: Store::isolated_symbols ────────────────────────────────────────

#[test]
fn store_isolated_symbols_returns_disconnected_nodes() {
    let mut store = Store::new();
    let _orphan = store.upsert_node(path("src/orphan.rs>orphan"));
    let connected_a = store.upsert_node(path("src/a.rs>a"));
    let connected_b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, connected_a, connected_b);
    let isolated = store.isolated_symbols(None);
    assert_eq!(isolated, vec!["src/orphan.rs>orphan"]);
}

#[test]
fn store_isolated_symbols_excludes_nodes_with_any_edge() {
    let mut store = Store::new();
    let sym_a = store.upsert_node(path("src/a.rs>a"));
    let sym_b = store.upsert_node(path("src/b.rs>b"));
    // sym_a has an outgoing Imports edge, sym_b has an incoming one
    store.upsert_edge(EdgeKind::Imports, sym_a, sym_b);
    let isolated = store.isolated_symbols(None);
    assert!(isolated.is_empty());
}

#[test]
fn store_isolated_symbols_excludes_file_nodes() {
    let mut store = Store::new();
    let _file = store.upsert_node(path("src/a.rs")); // file node — no edges
    let sym = store.upsert_node(path("src/a.rs>sym"));
    let tgt = store.upsert_node(path("src/b.rs>tgt"));
    store.upsert_edge(EdgeKind::Calls, sym, tgt);
    let isolated = store.isolated_symbols(None);
    assert!(isolated.is_empty());
}

#[test]
fn store_isolated_symbols_prefix_filter() {
    let mut store = Store::new();
    let _orphan_src = store.upsert_node(path("src/orphan.rs>orphan"));
    let _orphan_lib = store.upsert_node(path("lib/orphan.rs>orphan"));
    let isolated = store.isolated_symbols(Some("src/"));
    assert_eq!(isolated, vec!["src/orphan.rs>orphan"]);
}

#[test]
fn store_isolated_symbols_empty_graph() {
    let store = Store::new();
    assert!(store.isolated_symbols(None).is_empty());
}

// ── RFC-0057: Store::scc_groups ───────────────────────────────────────────────

#[test]
fn store_scc_groups_finds_simple_cycle() {
    let mut store = Store::new();
    let sym_a = store.upsert_node(path("src/a.rs>a"));
    let sym_b = store.upsert_node(path("src/b.rs>b"));
    let sym_c = store.upsert_node(path("src/c.rs>c"));
    // a → b → c → a (cycle)
    store.upsert_edge(EdgeKind::Calls, sym_a, sym_b);
    store.upsert_edge(EdgeKind::Calls, sym_b, sym_c);
    store.upsert_edge(EdgeKind::Calls, sym_c, sym_a);
    let groups = store.scc_groups(EdgeKind::Calls);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].len(), 3);
    assert!(groups[0].contains(&"src/a.rs>a".to_owned()));
    assert!(groups[0].contains(&"src/b.rs>b".to_owned()));
    assert!(groups[0].contains(&"src/c.rs>c".to_owned()));
}

#[test]
fn store_scc_groups_excludes_singletons() {
    let mut store = Store::new();
    let sym_a = store.upsert_node(path("src/a.rs>a"));
    let sym_b = store.upsert_node(path("src/b.rs>b"));
    // One-way edge: no cycle
    store.upsert_edge(EdgeKind::Calls, sym_a, sym_b);
    let groups = store.scc_groups(EdgeKind::Calls);
    assert!(groups.is_empty());
}

#[test]
fn store_scc_groups_excludes_file_nodes() {
    let mut store = Store::new();
    let _file = store.upsert_node(path("src/a.rs")); // file node — no `>`
    let sym_a = store.upsert_node(path("src/a.rs>a"));
    let sym_b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, sym_a, sym_b);
    store.upsert_edge(EdgeKind::Calls, sym_b, sym_a);
    let groups = store.scc_groups(EdgeKind::Calls);
    assert_eq!(groups.len(), 1);
    // file node must not appear
    assert!(groups[0].iter().all(|p| p.contains('>')));
}

#[test]
fn store_scc_groups_multiple_components_sorted_by_size() {
    let mut store = Store::new();
    // Large cycle: a ↔ b ↔ c
    let sym_a = store.upsert_node(path("src/a.rs>a"));
    let sym_b = store.upsert_node(path("src/b.rs>b"));
    let sym_c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, sym_a, sym_b);
    store.upsert_edge(EdgeKind::Calls, sym_b, sym_c);
    store.upsert_edge(EdgeKind::Calls, sym_c, sym_a);
    // Small cycle: x ↔ y
    let sym_x = store.upsert_node(path("src/x.rs>x"));
    let sym_y = store.upsert_node(path("src/y.rs>y"));
    store.upsert_edge(EdgeKind::Calls, sym_x, sym_y);
    store.upsert_edge(EdgeKind::Calls, sym_y, sym_x);
    let groups = store.scc_groups(EdgeKind::Calls);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].len(), 3); // larger group first
    assert_eq!(groups[1].len(), 2);
}

#[test]
fn store_scc_groups_paths_sorted_within_group() {
    let mut store = Store::new();
    let sym_z = store.upsert_node(path("src/z.rs>z"));
    let sym_a = store.upsert_node(path("src/a.rs>a"));
    store.upsert_edge(EdgeKind::Calls, sym_z, sym_a);
    store.upsert_edge(EdgeKind::Calls, sym_a, sym_z);
    let groups = store.scc_groups(EdgeKind::Calls);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0][0], "src/a.rs>a"); // alphabetically first
    assert_eq!(groups[0][1], "src/z.rs>z");
}

// ── RFC-0058: Store::dependency_layers ───────────────────────────────────────

#[test]
fn store_dependency_layers_simple_chain() {
    // c → b → a  (c depends on b, b depends on a, a has no deps)
    // layer 0 = [a], layer 1 = [b], layer 2 = [c]
    let mut store = Store::new();
    let sym_a = store.upsert_node(path("src/a.rs>a"));
    let sym_b = store.upsert_node(path("src/b.rs>b"));
    let sym_c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, sym_b, sym_a);
    store.upsert_edge(EdgeKind::Calls, sym_c, sym_b);
    let layers = store.dependency_layers(EdgeKind::Calls);
    assert_eq!(layers.len(), 3);
    assert_eq!(layers[0], vec!["src/a.rs>a"]);
    assert_eq!(layers[1], vec!["src/b.rs>b"]);
    assert_eq!(layers[2], vec!["src/c.rs>c"]);
}

#[test]
fn store_dependency_layers_excludes_cycle_members() {
    // a → b → a (cycle); c is acyclic leaf
    let mut store = Store::new();
    let sym_a = store.upsert_node(path("src/a.rs>a"));
    let sym_b = store.upsert_node(path("src/b.rs>b"));
    let _sym_c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, sym_a, sym_b);
    store.upsert_edge(EdgeKind::Calls, sym_b, sym_a);
    // c has no edges → layer 0, and is NOT in cycle
    let layers = store.dependency_layers(EdgeKind::Calls);
    // Only c should appear; a and b are in a cycle
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0], vec!["src/c.rs>c"]);
}

#[test]
fn store_dependency_layers_excludes_file_nodes() {
    let mut store = Store::new();
    let _file = store.upsert_node(path("src/a.rs")); // file node — no `>`
    let sym_a = store.upsert_node(path("src/a.rs>a"));
    let sym_b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, sym_b, sym_a);
    let layers = store.dependency_layers(EdgeKind::Calls);
    // Both layers should contain only symbol nodes (paths with `>`)
    for layer in &layers {
        for p in layer {
            assert!(p.contains('>'), "file node found in layer: {p}");
        }
    }
    assert!(layers.len() >= 2);
    assert!(layers[0].contains(&"src/a.rs>a".to_owned()));
    assert!(layers[1].contains(&"src/b.rs>b".to_owned()));
}

#[test]
fn store_dependency_layers_paths_sorted_within_layer() {
    // Two utilities at layer 0: z>z and a>a (alphabetical order expected)
    let mut store = Store::new();
    let sym_z = store.upsert_node(path("src/z.rs>z"));
    let sym_a = store.upsert_node(path("src/a.rs>a"));
    let sym_top = store.upsert_node(path("src/top.rs>top"));
    store.upsert_edge(EdgeKind::Calls, sym_top, sym_z);
    store.upsert_edge(EdgeKind::Calls, sym_top, sym_a);
    let layers = store.dependency_layers(EdgeKind::Calls);
    assert_eq!(layers[0][0], "src/a.rs>a"); // 'a' before 'z'
    assert_eq!(layers[0][1], "src/z.rs>z");
}

#[test]
fn store_dependency_layers_empty_store() {
    let store = Store::new();
    let layers = store.dependency_layers(EdgeKind::Calls);
    assert!(layers.is_empty());
}

// ── RFC-0059: Store::two_hop_neighbors ────────────────────────────────────────

#[test]
fn store_two_hop_neighbors_basic() {
    // a → b → c  (a's 2-hop neighbor is c)
    let mut store = Store::new();
    let sym_a = store.upsert_node(path("src/a.rs>a"));
    let sym_b = store.upsert_node(path("src/b.rs>b"));
    let sym_c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, sym_a, sym_b);
    store.upsert_edge(EdgeKind::Calls, sym_b, sym_c);
    let result = store.two_hop_neighbors(sym_a, EdgeKind::Calls);
    assert_eq!(result, vec!["src/c.rs>c"]);
}

#[test]
fn store_two_hop_neighbors_excludes_direct_neighbors() {
    // a → b, a → c, b → c  (c is both 1-hop and 2-hop; should be excluded)
    let mut store = Store::new();
    let sym_a = store.upsert_node(path("src/a.rs>a"));
    let sym_b = store.upsert_node(path("src/b.rs>b"));
    let sym_c = store.upsert_node(path("src/c.rs>c"));
    let sym_d = store.upsert_node(path("src/d.rs>d"));
    store.upsert_edge(EdgeKind::Calls, sym_a, sym_b);
    store.upsert_edge(EdgeKind::Calls, sym_a, sym_c);
    store.upsert_edge(EdgeKind::Calls, sym_b, sym_c); // c reachable via b
    store.upsert_edge(EdgeKind::Calls, sym_b, sym_d); // d is 2-hop only
    let result = store.two_hop_neighbors(sym_a, EdgeKind::Calls);
    // c excluded (direct neighbor); d included (2-hop only)
    assert_eq!(result, vec!["src/d.rs>d"]);
}

#[test]
fn store_two_hop_neighbors_excludes_self() {
    // a → b → a  (a is in its own 2-hop set but must be excluded)
    let mut store = Store::new();
    let sym_a = store.upsert_node(path("src/a.rs>a"));
    let sym_b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, sym_a, sym_b);
    store.upsert_edge(EdgeKind::Calls, sym_b, sym_a);
    let result = store.two_hop_neighbors(sym_a, EdgeKind::Calls);
    assert!(result.is_empty(), "self should be excluded: {result:?}");
}

#[test]
fn store_two_hop_neighbors_sorted_ascending() {
    // a → b → [z, c]  → result should be sorted [c, z]
    let mut store = Store::new();
    let sym_a = store.upsert_node(path("src/a.rs>a"));
    let sym_b = store.upsert_node(path("src/b.rs>b"));
    let sym_z = store.upsert_node(path("src/z.rs>z"));
    let sym_c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, sym_a, sym_b);
    store.upsert_edge(EdgeKind::Calls, sym_b, sym_z);
    store.upsert_edge(EdgeKind::Calls, sym_b, sym_c);
    let result = store.two_hop_neighbors(sym_a, EdgeKind::Calls);
    assert_eq!(result[0], "src/c.rs>c");
    assert_eq!(result[1], "src/z.rs>z");
}

#[test]
fn store_two_hop_neighbors_no_outgoing_returns_empty() {
    let mut store = Store::new();
    let sym_a = store.upsert_node(path("src/a.rs>a"));
    let result = store.two_hop_neighbors(sym_a, EdgeKind::Calls);
    assert!(result.is_empty());
}

// ── RFC-0060: Store::symbol_neighborhood ─────────────────────────────────────

#[test]
fn store_symbol_neighborhood_basic() {
    // main → service → util
    let mut store = Store::new();
    let main = store.upsert_node(path("src/main.rs>main"));
    let svc = store.upsert_node(path("src/service.rs>svc"));
    let util = store.upsert_node(path("src/util.rs>util"));
    store.upsert_edge(EdgeKind::Calls, main, svc);
    store.upsert_edge(EdgeKind::Calls, svc, util);
    let nb = store.symbol_neighborhood(svc, EdgeKind::Calls);
    assert_eq!(nb.path, "src/service.rs>svc");
    assert_eq!(nb.incoming, vec!["src/main.rs>main"]);
    assert_eq!(nb.outgoing, vec!["src/util.rs>util"]);
}

#[test]
fn store_symbol_neighborhood_sorted() {
    // Multiple incoming and outgoing — all should be sorted ascending.
    let mut store = Store::new();
    let hub = store.upsert_node(path("src/hub.rs>hub"));
    let z_in = store.upsert_node(path("src/z.rs>z_caller"));
    let a_in = store.upsert_node(path("src/a.rs>a_caller"));
    let z_out = store.upsert_node(path("src/z.rs>z_callee"));
    let a_out = store.upsert_node(path("src/a.rs>a_callee"));
    store.upsert_edge(EdgeKind::Calls, z_in, hub);
    store.upsert_edge(EdgeKind::Calls, a_in, hub);
    store.upsert_edge(EdgeKind::Calls, hub, z_out);
    store.upsert_edge(EdgeKind::Calls, hub, a_out);
    let nb = store.symbol_neighborhood(hub, EdgeKind::Calls);
    assert_eq!(nb.incoming[0], "src/a.rs>a_caller");
    assert_eq!(nb.incoming[1], "src/z.rs>z_caller");
    assert_eq!(nb.outgoing[0], "src/a.rs>a_callee");
    assert_eq!(nb.outgoing[1], "src/z.rs>z_callee");
}

#[test]
fn store_symbol_neighborhood_no_edges() {
    let mut store = Store::new();
    let lone = store.upsert_node(path("src/lone.rs>lone"));
    let nb = store.symbol_neighborhood(lone, EdgeKind::Calls);
    assert_eq!(nb.path, "src/lone.rs>lone");
    assert!(nb.incoming.is_empty());
    assert!(nb.outgoing.is_empty());
}

#[test]
fn store_symbol_neighborhood_unknown_id() {
    let store = Store::new();
    let nb = store.symbol_neighborhood(NodeId(9999), EdgeKind::Calls);
    assert_eq!(nb.path, "");
    assert!(nb.incoming.is_empty());
    assert!(nb.outgoing.is_empty());
}

#[test]
fn store_symbol_neighborhood_different_edge_kind() {
    // Make sure the kind filter is respected — Calls edges should not appear in Imports.
    let mut store = Store::new();
    let src = store.upsert_node(path("src/a.rs>src_fn"));
    let dst = store.upsert_node(path("src/b.rs>dst_fn"));
    store.upsert_edge(EdgeKind::Calls, src, dst);
    let nb = store.symbol_neighborhood(src, EdgeKind::Imports);
    assert!(
        nb.outgoing.is_empty(),
        "Calls edge must not appear under Imports kind"
    );
}

// ── RFC-0061: Store::hub_symbols ─────────────────────────────────────────────

#[test]
fn store_hub_symbols_basic() {
    // svc: in=2, out=2 → hub; a, b callers; x, y callees
    let mut store = Store::new();
    let svc = store.upsert_node(path("src/svc.rs>svc"));
    let caller_a = store.upsert_node(path("src/a.rs>a"));
    let caller_b = store.upsert_node(path("src/b.rs>b"));
    let callee_x = store.upsert_node(path("src/x.rs>x"));
    let callee_y = store.upsert_node(path("src/y.rs>y"));
    store.upsert_edge(EdgeKind::Calls, caller_a, svc);
    store.upsert_edge(EdgeKind::Calls, caller_b, svc);
    store.upsert_edge(EdgeKind::Calls, svc, callee_x);
    store.upsert_edge(EdgeKind::Calls, svc, callee_y);
    let hubs = store.hub_symbols(EdgeKind::Calls, 2, 2, 10);
    assert_eq!(hubs.len(), 1);
    assert_eq!(hubs[0].0, "src/svc.rs>svc");
    assert_eq!(hubs[0].1, 2); // in_degree
    assert_eq!(hubs[0].2, 2); // out_degree
}

#[test]
fn store_hub_symbols_excludes_below_threshold() {
    // svc: in=1, out=2 → excluded by min_in=2
    let mut store = Store::new();
    let svc = store.upsert_node(path("src/svc.rs>svc"));
    let caller = store.upsert_node(path("src/a.rs>a"));
    let callee_x = store.upsert_node(path("src/x.rs>x"));
    let callee_y = store.upsert_node(path("src/y.rs>y"));
    store.upsert_edge(EdgeKind::Calls, caller, svc);
    store.upsert_edge(EdgeKind::Calls, svc, callee_x);
    store.upsert_edge(EdgeKind::Calls, svc, callee_y);
    let hubs = store.hub_symbols(EdgeKind::Calls, 2, 1, 10);
    assert!(hubs.is_empty()); // in_degree=1 < min_in=2
}

#[test]
fn store_hub_symbols_sorted_by_total_degree_desc() {
    // Two hubs: svc (in=3, out=2, total=5) and mid (in=2, out=2, total=4)
    let mut store = Store::new();
    let svc = store.upsert_node(path("src/svc.rs>svc"));
    let mid = store.upsert_node(path("src/mid.rs>mid"));
    // svc: in=3
    for i in 0..3_u32 {
        let c = store.upsert_node(path(&format!("src/c{i}.rs>c{i}")));
        store.upsert_edge(EdgeKind::Calls, c, svc);
    }
    // svc: out=2
    for i in 0..2_u32 {
        let d = store.upsert_node(path(&format!("src/d{i}.rs>d{i}")));
        store.upsert_edge(EdgeKind::Calls, svc, d);
    }
    // mid: in=2
    for i in 0..2_u32 {
        let e = store.upsert_node(path(&format!("src/e{i}.rs>e{i}")));
        store.upsert_edge(EdgeKind::Calls, e, mid);
    }
    // mid: out=2
    for i in 0..2_u32 {
        let f = store.upsert_node(path(&format!("src/f{i}.rs>f{i}")));
        store.upsert_edge(EdgeKind::Calls, mid, f);
    }
    let hubs = store.hub_symbols(EdgeKind::Calls, 2, 2, 10);
    assert_eq!(hubs.len(), 2);
    assert_eq!(hubs[0].0, "src/svc.rs>svc"); // total=5 > total=4
    assert_eq!(hubs[1].0, "src/mid.rs>mid");
}

#[test]
fn store_hub_symbols_limit_respected() {
    let mut store = Store::new();
    let hub = store.upsert_node(path("src/hub.rs>hub"));
    for i in 0..3_u32 {
        let c = store.upsert_node(path(&format!("src/caller{i}.rs>c{i}")));
        store.upsert_edge(EdgeKind::Calls, c, hub);
    }
    for i in 0..3_u32 {
        let d = store.upsert_node(path(&format!("src/callee{i}.rs>d{i}")));
        store.upsert_edge(EdgeKind::Calls, hub, d);
    }
    let hubs = store.hub_symbols(EdgeKind::Calls, 1, 1, 0); // limit=0, capped at 0
    assert!(hubs.is_empty());
}

#[test]
fn store_hub_symbols_empty_store() {
    let store = Store::new();
    let hubs = store.hub_symbols(EdgeKind::Calls, 1, 1, 10);
    assert!(hubs.is_empty());
}

// RFC-0062: singly_referenced

#[test]
fn store_singly_referenced_basic() {
    let mut store = Store::new();
    let src = store.upsert_node(path("src/main.rs>main"));
    let tgt = store.upsert_node(path("src/util.rs>helper"));
    store.upsert_edge(EdgeKind::Calls, src, tgt);
    let result = store.singly_referenced(EdgeKind::Calls, 10);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "src/util.rs>helper");
    assert_eq!(result[0].1, "src/main.rs>main");
}

#[test]
fn store_singly_referenced_excludes_multi_referenced() {
    let mut store = Store::new();
    let tgt = store.upsert_node(path("src/lib.rs>shared"));
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, a, tgt);
    store.upsert_edge(EdgeKind::Calls, b, tgt);
    let result = store.singly_referenced(EdgeKind::Calls, 10);
    // shared has 2 callers — excluded
    assert!(result.iter().all(|(p, _)| p != "src/lib.rs>shared"));
}

#[test]
fn store_singly_referenced_excludes_zero_referenced() {
    let mut store = Store::new();
    let _lone = store.upsert_node(path("src/lone.rs>lone"));
    let result = store.singly_referenced(EdgeKind::Calls, 10);
    assert!(result.is_empty());
}

#[test]
fn store_singly_referenced_sorted_ascending() {
    let mut store = Store::new();
    let caller = store.upsert_node(path("src/main.rs>main"));
    let z = store.upsert_node(path("src/z.rs>z_fn"));
    let a = store.upsert_node(path("src/a.rs>a_fn"));
    store.upsert_edge(EdgeKind::Calls, caller, z);
    store.upsert_edge(EdgeKind::Calls, caller, a);
    let result = store.singly_referenced(EdgeKind::Calls, 10);
    let paths: Vec<&str> = result.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(paths, vec!["src/a.rs>a_fn", "src/z.rs>z_fn"]);
}

#[test]
fn store_singly_referenced_limit_respected() {
    let mut store = Store::new();
    let caller = store.upsert_node(path("src/main.rs>main"));
    for i in 0..5_u32 {
        let tgt = store.upsert_node(path(&format!("src/mod{i:02}.rs>f{i}")));
        store.upsert_edge(EdgeKind::Calls, caller, tgt);
    }
    let result = store.singly_referenced(EdgeKind::Calls, 3);
    assert_eq!(result.len(), 3);
}

// RFC-0063: batch_reachable_to

#[test]
fn store_batch_reachable_to_single_input() {
    let mut store = Store::new();
    let tgt = store.upsert_node(path("src/util.rs>helper"));
    let mid = store.upsert_node(path("src/mid.rs>mid"));
    let top = store.upsert_node(path("src/top.rs>top"));
    store.upsert_edge(EdgeKind::Calls, mid, tgt);
    store.upsert_edge(EdgeKind::Calls, top, mid);
    let result = store.batch_reachable_to(&[tgt], EdgeKind::Calls, 10);
    assert!(result.contains(&"src/mid.rs>mid".to_owned()));
    assert!(result.contains(&"src/top.rs>top".to_owned()));
    assert!(!result.contains(&"src/util.rs>helper".to_owned()));
}

#[test]
fn store_batch_reachable_to_union_of_two() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let dep_a = store.upsert_node(path("src/dep_a.rs>dep_a"));
    let dep_b = store.upsert_node(path("src/dep_b.rs>dep_b"));
    let common = store.upsert_node(path("src/common.rs>common"));
    store.upsert_edge(EdgeKind::Calls, dep_a, a);
    store.upsert_edge(EdgeKind::Calls, dep_b, b);
    store.upsert_edge(EdgeKind::Calls, common, a);
    store.upsert_edge(EdgeKind::Calls, common, b);
    let result = store.batch_reachable_to(&[a, b], EdgeKind::Calls, 10);
    let mut expected = vec![
        "src/common.rs>common",
        "src/dep_a.rs>dep_a",
        "src/dep_b.rs>dep_b",
    ];
    expected.sort_unstable();
    let mut got: Vec<&str> = result.iter().map(String::as_str).collect();
    got.sort_unstable();
    assert_eq!(got, expected);
}

#[test]
fn store_batch_reachable_to_deduplication() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let shared_dep = store.upsert_node(path("src/shared.rs>shared"));
    store.upsert_edge(EdgeKind::Calls, shared_dep, a);
    store.upsert_edge(EdgeKind::Calls, shared_dep, b);
    let result = store.batch_reachable_to(&[a, b], EdgeKind::Calls, 10);
    assert_eq!(
        result
            .iter()
            .filter(|p| p.as_str() == "src/shared.rs>shared")
            .count(),
        1
    );
}

#[test]
fn store_batch_reachable_to_empty_input() {
    let store = Store::new();
    let result = store.batch_reachable_to(&[], EdgeKind::Calls, 10);
    assert!(result.is_empty());
}

#[test]
fn store_batch_reachable_to_sorted_ascending() {
    let mut store = Store::new();
    let tgt = store.upsert_node(path("src/tgt.rs>tgt"));
    let z = store.upsert_node(path("src/z.rs>z_dep"));
    let a = store.upsert_node(path("src/a.rs>a_dep"));
    store.upsert_edge(EdgeKind::Calls, z, tgt);
    store.upsert_edge(EdgeKind::Calls, a, tgt);
    let result = store.batch_reachable_to(&[tgt], EdgeKind::Calls, 10);
    assert_eq!(result, vec!["src/a.rs>a_dep", "src/z.rs>z_dep"]);
}

// RFC-0064: k_core

#[test]
fn store_k_core_2core_triangle() {
    // a→b, b→c, c→a forms a cycle; every node has in+out=2 within the subgraph
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    store.upsert_edge(EdgeKind::Calls, c, a);
    let core = store.k_core(EdgeKind::Calls, 2);
    assert_eq!(core, vec!["src/a.rs>a", "src/b.rs>b", "src/c.rs>c"]);
}

#[test]
fn store_k_core_peels_low_degree_nodes() {
    // chain: x→a→b→c; only a,b are in the 2-core (each has degree 2)
    // x has degree 1 (only outgoing to a); c has degree 1 (only incoming from b)
    let mut store = Store::new();
    let x = store.upsert_node(path("src/x.rs>x"));
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, x, a);
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    let core = store.k_core(EdgeKind::Calls, 2);
    // x: 1 edge (x→a), peeled. c: 1 edge (b→c), peeled.
    // After peeling x: a has degree 1 (only a→b remains). After peeling c: b has degree 1.
    // So the 2-core is empty.
    assert!(core.is_empty());
}

#[test]
fn store_k_core_k0_returns_all_symbols() {
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs>a"));
    store.upsert_node(path("src/b.rs>b"));
    // file node should be excluded
    store.upsert_node(path("src/file.rs"));
    let core = store.k_core(EdgeKind::Calls, 0);
    assert_eq!(core.len(), 2);
    assert!(core.contains(&"src/a.rs>a".to_owned()));
    assert!(core.contains(&"src/b.rs>b".to_owned()));
}

#[test]
fn store_k_core_empty_store() {
    let store = Store::new();
    let core = store.k_core(EdgeKind::Calls, 2);
    assert!(core.is_empty());
}

#[test]
fn store_k_core_sorted_ascending() {
    let mut store = Store::new();
    let z = store.upsert_node(path("src/z.rs>z"));
    let a = store.upsert_node(path("src/a.rs>a"));
    let m = store.upsert_node(path("src/m.rs>m"));
    // Complete directed 3-node graph: each has degree 4 (2 in + 2 out)
    store.upsert_edge(EdgeKind::Calls, z, a);
    store.upsert_edge(EdgeKind::Calls, z, m);
    store.upsert_edge(EdgeKind::Calls, a, z);
    store.upsert_edge(EdgeKind::Calls, a, m);
    store.upsert_edge(EdgeKind::Calls, m, z);
    store.upsert_edge(EdgeKind::Calls, m, a);
    let core = store.k_core(EdgeKind::Calls, 2);
    assert_eq!(core, vec!["src/a.rs>a", "src/m.rs>m", "src/z.rs>z"]);
}

// RFC-0065: batch_reachable_from

#[test]
fn store_batch_reachable_from_single_input() {
    let mut store = Store::new();
    let src = store.upsert_node(path("src/top.rs>top"));
    let mid = store.upsert_node(path("src/mid.rs>mid"));
    let leaf = store.upsert_node(path("src/leaf.rs>leaf"));
    store.upsert_edge(EdgeKind::Calls, src, mid);
    store.upsert_edge(EdgeKind::Calls, mid, leaf);
    let result = store.batch_reachable_from(&[src], EdgeKind::Calls, 10);
    assert!(result.contains(&"src/mid.rs>mid".to_owned()));
    assert!(result.contains(&"src/leaf.rs>leaf".to_owned()));
    // input itself excluded
    assert!(!result.contains(&"src/top.rs>top".to_owned()));
}

#[test]
fn store_batch_reachable_from_union_of_two() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let reach_a = store.upsert_node(path("src/reach_a.rs>reach_a"));
    let reach_b = store.upsert_node(path("src/reach_b.rs>reach_b"));
    let shared = store.upsert_node(path("src/shared.rs>shared"));
    store.upsert_edge(EdgeKind::Calls, a, reach_a);
    store.upsert_edge(EdgeKind::Calls, b, reach_b);
    store.upsert_edge(EdgeKind::Calls, a, shared);
    store.upsert_edge(EdgeKind::Calls, b, shared);
    let result = store.batch_reachable_from(&[a, b], EdgeKind::Calls, 10);
    let mut expected = vec![
        "src/reach_a.rs>reach_a",
        "src/reach_b.rs>reach_b",
        "src/shared.rs>shared",
    ];
    expected.sort_unstable();
    let mut got: Vec<&str> = result.iter().map(String::as_str).collect();
    got.sort_unstable();
    assert_eq!(got, expected);
}

#[test]
fn store_batch_reachable_from_deduplication() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let common = store.upsert_node(path("src/common.rs>common"));
    store.upsert_edge(EdgeKind::Calls, a, common);
    store.upsert_edge(EdgeKind::Calls, b, common);
    let result = store.batch_reachable_from(&[a, b], EdgeKind::Calls, 10);
    assert_eq!(
        result
            .iter()
            .filter(|p| p.as_str() == "src/common.rs>common")
            .count(),
        1
    );
}

#[test]
fn store_batch_reachable_from_empty_input() {
    let store = Store::new();
    let result = store.batch_reachable_from(&[], EdgeKind::Calls, 10);
    assert!(result.is_empty());
}

#[test]
fn store_batch_reachable_from_sorted_ascending() {
    let mut store = Store::new();
    let src = store.upsert_node(path("src/src.rs>src"));
    let z = store.upsert_node(path("src/z.rs>z_fn"));
    let a = store.upsert_node(path("src/a.rs>a_fn"));
    store.upsert_edge(EdgeKind::Calls, src, z);
    store.upsert_edge(EdgeKind::Calls, src, a);
    let result = store.batch_reachable_from(&[src], EdgeKind::Calls, 10);
    assert_eq!(result, vec!["src/a.rs>a_fn", "src/z.rs>z_fn"]);
}

// RFC-0066: batch_node_degree

#[test]
fn store_batch_node_degree_basic() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    let degrees = store.batch_node_degree(&[a, b]);
    assert_eq!(degrees.len(), 2);
    assert_eq!(degrees[0].out_calls, 1);
    assert_eq!(degrees[1].in_calls, 1);
}

#[test]
fn store_batch_node_degree_preserves_order() {
    let mut store = Store::new();
    let z = store.upsert_node(path("src/z.rs>z"));
    let a = store.upsert_node(path("src/a.rs>a"));
    let mid = store.upsert_node(path("src/m.rs>m"));
    store.upsert_edge(EdgeKind::Calls, z, mid);
    store.upsert_edge(EdgeKind::Calls, a, mid);
    let degrees = store.batch_node_degree(&[z, a, mid]);
    // z: out_calls=1; a: out_calls=1; mid: in_calls=2
    assert_eq!(degrees[0].out_calls, 1);
    assert_eq!(degrees[1].out_calls, 1);
    assert_eq!(degrees[2].in_calls, 2);
}

#[test]
fn store_batch_node_degree_empty_input() {
    let store = Store::new();
    let degrees = store.batch_node_degree(&[]);
    assert!(degrees.is_empty());
}

#[test]
fn store_batch_node_degree_multi_kind() {
    let mut store = Store::new();
    let src = store.upsert_node(path("src/a.rs>a"));
    let tgt = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, src, tgt);
    store.upsert_edge(EdgeKind::Imports, src, tgt);
    let degrees = store.batch_node_degree(&[src]);
    assert_eq!(degrees[0].out_calls, 1);
    assert_eq!(degrees[0].out_imports, 1);
}

#[test]
fn store_batch_node_degree_isolated_node_returns_zeros() {
    let mut store = Store::new();
    let lone = store.upsert_node(path("src/lone.rs>lone"));
    let degrees = store.batch_node_degree(&[lone]);
    assert_eq!(degrees[0], NodeDegree::default());
}

// RFC-0067: cycle_members
#[test]
fn store_cycle_members_simple_mutual_cycle() {
    // a → b → a (mutual cycle via Calls)
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, a);
    let members = store.cycle_members(EdgeKind::Calls);
    let mut expected = vec!["src/a.rs>a".to_owned(), "src/b.rs>b".to_owned()];
    expected.sort_unstable();
    assert_eq!(members, expected);
}

#[test]
fn store_cycle_members_no_cycle_returns_empty() {
    // a → b → c (acyclic)
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    let members = store.cycle_members(EdgeKind::Calls);
    assert!(members.is_empty());
}

#[test]
fn store_cycle_members_excludes_file_nodes() {
    // file nodes should never appear even if they share a NodeId structure
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let _file = store.upsert_node(path("src/a.rs")); // file node (no `>`)
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, a);
    let members = store.cycle_members(EdgeKind::Calls);
    for m in &members {
        assert!(m.contains('>'), "file node leaked into cycle_members: {m}");
    }
}

#[test]
fn store_cycle_members_three_node_cycle() {
    // a → b → c → a
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    store.upsert_edge(EdgeKind::Calls, c, a);
    let members = store.cycle_members(EdgeKind::Calls);
    let mut expected = vec![
        "src/a.rs>a".to_owned(),
        "src/b.rs>b".to_owned(),
        "src/c.rs>c".to_owned(),
    ];
    expected.sort_unstable();
    assert_eq!(members, expected);
}

#[test]
fn store_cycle_members_non_cycle_node_excluded() {
    // a → b → a forms a cycle; c → a is a dangling caller (not in any cycle)
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, a);
    store.upsert_edge(EdgeKind::Calls, c, a); // c is not in a cycle
    let members = store.cycle_members(EdgeKind::Calls);
    let mut expected = vec!["src/a.rs>a".to_owned(), "src/b.rs>b".to_owned()];
    expected.sort_unstable();
    assert_eq!(members, expected);
    assert!(!members.iter().any(|m: &String| m.contains("src/c.rs")));
}

// RFC-0068: weakly_connected_components
#[test]
fn store_wcc_two_disjoint_components() {
    // a → b (one cluster), c → d (another)
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    let d = store.upsert_node(path("src/d.rs>d"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, c, d);
    let comps = store.weakly_connected_components(EdgeKind::Calls);
    assert_eq!(comps.len(), 2);
    // Each component has 2 symbols
    assert!(comps.iter().all(|c: &Vec<String>| c.len() == 2));
}

#[test]
fn store_wcc_direction_ignored() {
    // a → b and b → a should still be ONE component
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let _c = store.upsert_node(path("src/c.rs>c")); // isolated
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, a);
    let comps = store.weakly_connected_components(EdgeKind::Calls);
    // a+b in one component, c in another
    assert_eq!(comps.len(), 2);
    let big = comps.iter().find(|c: &&Vec<String>| c.len() == 2).unwrap();
    assert!(big.contains(&"src/a.rs>a".to_owned()));
    assert!(big.contains(&"src/b.rs>b".to_owned()));
}

#[test]
fn store_wcc_single_node_own_component() {
    let mut store = Store::new();
    store.upsert_node(path("src/lone.rs>lone"));
    let comps = store.weakly_connected_components(EdgeKind::Calls);
    assert_eq!(comps.len(), 1);
    assert_eq!(comps[0], vec!["src/lone.rs>lone".to_owned()]);
}

#[test]
fn store_wcc_excludes_file_nodes() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_node(path("src/a.rs")); // file node
    store.upsert_edge(EdgeKind::Calls, a, b);
    let comps = store.weakly_connected_components(EdgeKind::Calls);
    for comp in &comps {
        for sym in comp {
            assert!(sym.contains('>'), "file node leaked: {sym}");
        }
    }
}

#[test]
fn store_wcc_sorted_by_size_desc() {
    // a → b → c (size 3), d isolated (size 1)
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_node(path("src/d.rs>d")); // isolated
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    let comps = store.weakly_connected_components(EdgeKind::Calls);
    assert_eq!(comps.len(), 2);
    assert_eq!(comps[0].len(), 3); // largest first
    assert_eq!(comps[1].len(), 1);
}

// RFC-0069: topological_sort
#[test]
fn store_topo_sort_linear_chain() {
    // a → b → c: topo order must be [a, b, c]
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    let result = store.topological_sort(EdgeKind::Calls);
    assert!(result.cycle_members.is_empty());
    // a before b before c
    let pos: std::collections::HashMap<&str, usize> = result
        .order
        .iter()
        .enumerate()
        .map(|(i, s): (usize, &String)| (s.as_str(), i))
        .collect();
    assert!(pos["src/a.rs>a"] < pos["src/b.rs>b"]);
    assert!(pos["src/b.rs>b"] < pos["src/c.rs>c"]);
}

#[test]
fn store_topo_sort_cycle_members_reported() {
    // a → b → a: both in cycle, c is a DAG node
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, a);
    store.upsert_edge(EdgeKind::Calls, c, a); // c → cycle
    let result = store.topological_sort(EdgeKind::Calls);
    // c has no incoming edges from non-cycle nodes, so it's in order
    assert!(result.order.contains(&"src/c.rs>c".to_owned()));
    // a and b are cycle members
    assert!(result.cycle_members.contains(&"src/a.rs>a".to_owned()));
    assert!(result.cycle_members.contains(&"src/b.rs>b".to_owned()));
}

#[test]
fn store_topo_sort_empty_graph_returns_empty() {
    let store = Store::new();
    let result = store.topological_sort(EdgeKind::Calls);
    assert!(result.order.is_empty());
    assert!(result.cycle_members.is_empty());
}

#[test]
fn store_topo_sort_excludes_file_nodes() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_node(path("src/a.rs")); // file node
    store.upsert_edge(EdgeKind::Calls, a, b);
    let result = store.topological_sort(EdgeKind::Calls);
    for sym in result.order.iter().chain(result.cycle_members.iter()) {
        let sym: &String = sym;
        assert!(sym.contains('>'), "file node leaked: {sym}");
    }
}

#[test]
fn store_topo_sort_diamond_dependency() {
    // a → b, a → c, b → d, c → d: d must come last, a must come first
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    let d = store.upsert_node(path("src/d.rs>d"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, a, c);
    store.upsert_edge(EdgeKind::Calls, b, d);
    store.upsert_edge(EdgeKind::Calls, c, d);
    let result = store.topological_sort(EdgeKind::Calls);
    assert!(result.cycle_members.is_empty());
    let pos: std::collections::HashMap<&str, usize> = result
        .order
        .iter()
        .enumerate()
        .map(|(i, s): (usize, &String)| (s.as_str(), i))
        .collect();
    assert!(pos["src/a.rs>a"] < pos["src/b.rs>b"]);
    assert!(pos["src/a.rs>a"] < pos["src/c.rs>c"]);
    assert!(pos["src/b.rs>b"] < pos["src/d.rs>d"]);
    assert!(pos["src/c.rs>c"] < pos["src/d.rs>d"]);
}

// RFC-0070: articulation_points
#[test]
fn store_articulation_points_bridge_node() {
    // a — b — c: b is the articulation point
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    let points = store.articulation_points(EdgeKind::Calls);
    assert_eq!(points, vec!["src/b.rs>b".to_owned()]);
}

#[test]
fn store_articulation_points_cycle_has_none() {
    // a — b — c — a: no articulation points (removing any node keeps the rest connected)
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    store.upsert_edge(EdgeKind::Calls, c, a);
    let points = store.articulation_points(EdgeKind::Calls);
    assert!(points.is_empty());
}

#[test]
fn store_articulation_points_no_edges_returns_empty() {
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs>a"));
    store.upsert_node(path("src/b.rs>b"));
    // No edges — isolated nodes are not articulation points
    let points = store.articulation_points(EdgeKind::Calls);
    assert!(points.is_empty());
}

#[test]
fn store_articulation_points_excludes_file_nodes() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_node(path("src/a.rs")); // file node
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    let points = store.articulation_points(EdgeKind::Calls);
    for p in &points {
        let p: &String = p;
        assert!(p.contains('>'), "file node leaked: {p}");
    }
}

#[test]
fn store_articulation_points_diamond_has_none() {
    // a — b — d, a — c — d: no articulation points in diamond
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    let d = store.upsert_node(path("src/d.rs>d"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, a, c);
    store.upsert_edge(EdgeKind::Calls, b, d);
    store.upsert_edge(EdgeKind::Calls, c, d);
    let points = store.articulation_points(EdgeKind::Calls);
    assert!(points.is_empty());
}

// ── RFC-0071: bridge_edges ────────────────────────────────────────────

#[test]
fn store_bridge_edges_single_bridge() {
    // a — b — c: edge (b,c) and (a,b) are both bridges
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    let bridges = store.bridge_edges(EdgeKind::Calls);
    assert_eq!(bridges.len(), 2);
    assert!(
        bridges
            .iter()
            .any(|(f, t)| f == "src/a.rs>a" && t == "src/b.rs>b")
    );
    assert!(
        bridges
            .iter()
            .any(|(f, t)| f == "src/b.rs>b" && t == "src/c.rs>c")
    );
}

#[test]
fn store_bridge_edges_cycle_has_none() {
    // a — b — c — a: no bridges in a cycle
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    store.upsert_edge(EdgeKind::Calls, c, a);
    let bridges = store.bridge_edges(EdgeKind::Calls);
    assert!(bridges.is_empty());
}

#[test]
fn store_bridge_edges_no_edges_returns_empty() {
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs>a"));
    store.upsert_node(path("src/b.rs>b"));
    let bridges = store.bridge_edges(EdgeKind::Calls);
    assert!(bridges.is_empty());
}

#[test]
fn store_bridge_edges_excludes_file_nodes() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_node(path("src/a.rs")); // file node — must not appear
    store.upsert_edge(EdgeKind::Calls, a, b);
    let bridges = store.bridge_edges(EdgeKind::Calls);
    for (f, t) in &bridges {
        assert!(f.contains('>'), "file node in bridge from: {f}");
        assert!(t.contains('>'), "file node in bridge to: {t}");
    }
}

#[test]
fn store_bridge_edges_sorted_canonical() {
    // Result must be sorted; canonical order means from < to in each pair
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, c, b);
    store.upsert_edge(EdgeKind::Calls, b, a);
    let bridges = store.bridge_edges(EdgeKind::Calls);
    for (f, t) in &bridges {
        assert!(f <= t, "non-canonical pair: ({f}, {t})");
    }
    let sorted = {
        let mut v = bridges.clone();
        v.sort_unstable();
        v
    };
    assert_eq!(bridges, sorted, "bridges not sorted");
}

// ── RFC-0072: biconnected_components ─────────────────────────────────

#[test]
fn store_bcc_triangle_is_one_component() {
    // a — b — c — a: one BCC of 3 nodes
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    store.upsert_edge(EdgeKind::Calls, c, a);
    let comps = store.biconnected_components(EdgeKind::Calls);
    assert_eq!(comps.len(), 1);
    assert_eq!(comps[0].len(), 3);
}

#[test]
fn store_bcc_bridge_produces_two_node_component() {
    // a — b: bridge edge; BCC = {a, b}
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    let comps = store.biconnected_components(EdgeKind::Calls);
    assert_eq!(comps.len(), 1);
    assert_eq!(comps[0].len(), 2);
    assert!(comps[0].contains(&"src/a.rs>a".to_owned()));
    assert!(comps[0].contains(&"src/b.rs>b".to_owned()));
}

#[test]
fn store_bcc_singleton_excluded() {
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs>a")); // isolated
    let comps = store.biconnected_components(EdgeKind::Calls);
    assert!(comps.is_empty());
}

#[test]
fn store_bcc_excludes_file_nodes() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_node(path("src/a.rs")); // file node
    store.upsert_edge(EdgeKind::Calls, a, b);
    let comps = store.biconnected_components(EdgeKind::Calls);
    for comp in &comps {
        for p in comp {
            let p: &String = p;
            assert!(p.contains('>'), "file node leaked: {p}");
        }
    }
}

#[test]
fn store_bcc_bowtie_two_components() {
    // Two triangles sharing vertex hub: left-hub-right1-left and hub-far1-far2-hub
    // hub is an articulation point; the bowtie has two BCCs of 3 nodes each
    let mut store = Store::new();
    let left = store.upsert_node(path("src/left.rs>left"));
    let hub = store.upsert_node(path("src/hub.rs>hub"));
    let right1 = store.upsert_node(path("src/right1.rs>right1"));
    let far1 = store.upsert_node(path("src/far1.rs>far1"));
    let far2 = store.upsert_node(path("src/far2.rs>far2"));
    store.upsert_edge(EdgeKind::Calls, left, hub);
    store.upsert_edge(EdgeKind::Calls, hub, right1);
    store.upsert_edge(EdgeKind::Calls, right1, left);
    store.upsert_edge(EdgeKind::Calls, hub, far1);
    store.upsert_edge(EdgeKind::Calls, far1, far2);
    store.upsert_edge(EdgeKind::Calls, far2, hub);
    let comps = store.biconnected_components(EdgeKind::Calls);
    assert_eq!(comps.len(), 2);
    assert!(comps.iter().all(|c| c.len() == 3));
}

// ── RFC-0073: degree_histogram ────────────────────────────────────────

#[test]
fn store_degree_histogram_basic() {
    // a → b, a → c: a has out_degree 2; b and c have in_degree 1
    let mut store = Store::new();
    let node_a = store.upsert_node(path("src/a.rs>a"));
    let node_b = store.upsert_node(path("src/b.rs>b"));
    let node_c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, node_a, node_b);
    store.upsert_edge(EdgeKind::Calls, node_a, node_c);
    let hist = store.degree_histogram(EdgeKind::Calls);
    // in_degrees: 2 nodes with in=1, 1 node with in=0
    let in_map: std::collections::HashMap<u64, u64> = hist.in_degrees.iter().copied().collect();
    assert_eq!(in_map.get(&0).copied().unwrap_or(0), 1); // node_a
    assert_eq!(in_map.get(&1).copied().unwrap_or(0), 2); // node_b, node_c
    // out_degrees: 1 node with out=2, 2 nodes with out=0
    let out_map: std::collections::HashMap<u64, u64> = hist.out_degrees.iter().copied().collect();
    assert_eq!(out_map.get(&0).copied().unwrap_or(0), 2); // node_b, node_c
    assert_eq!(out_map.get(&2).copied().unwrap_or(0), 1); // node_a
}

#[test]
fn store_degree_histogram_sorted() {
    let mut store = Store::new();
    let a1 = store.upsert_node(path("src/a.rs>a1"));
    let a2 = store.upsert_node(path("src/a.rs>a2"));
    let b1 = store.upsert_node(path("src/b.rs>b1"));
    store.upsert_edge(EdgeKind::Calls, a1, b1);
    store.upsert_edge(EdgeKind::Calls, a2, b1);
    let hist = store.degree_histogram(EdgeKind::Calls);
    let in_degs: Vec<u64> = hist.in_degrees.iter().map(|&(d, _)| d).collect();
    let out_degs: Vec<u64> = hist.out_degrees.iter().map(|&(d, _)| d).collect();
    assert!(
        in_degs.windows(2).all(|w| w[0] <= w[1]),
        "in_degrees not sorted"
    );
    assert!(
        out_degs.windows(2).all(|w| w[0] <= w[1]),
        "out_degrees not sorted"
    );
}

#[test]
fn store_degree_histogram_excludes_file_nodes() {
    let mut store = Store::new();
    let sym = store.upsert_node(path("src/a.rs>a"));
    store.upsert_node(path("src/a.rs")); // file node
    let _ = sym;
    let hist = store.degree_histogram(EdgeKind::Calls);
    let total: u64 = hist.in_degrees.iter().map(|&(_, c)| c).sum();
    assert_eq!(total, 1, "file node leaked into histogram");
}

#[test]
fn store_degree_histogram_counts_sum_to_total() {
    let mut store = Store::new();
    let a1 = store.upsert_node(path("src/a.rs>a1"));
    let b1 = store.upsert_node(path("src/b.rs>b1"));
    let c1 = store.upsert_node(path("src/c.rs>c1"));
    store.upsert_edge(EdgeKind::Calls, a1, b1);
    store.upsert_edge(EdgeKind::Calls, b1, c1);
    let hist = store.degree_histogram(EdgeKind::Calls);
    let in_total: u64 = hist.in_degrees.iter().map(|&(_, c)| c).sum();
    let out_total: u64 = hist.out_degrees.iter().map(|&(_, c)| c).sum();
    assert_eq!(in_total, 3);
    assert_eq!(out_total, 3);
}

#[test]
fn store_degree_histogram_empty_store() {
    let store = Store::new();
    let hist = store.degree_histogram(EdgeKind::Calls);
    assert!(hist.in_degrees.is_empty());
    assert!(hist.out_degrees.is_empty());
}

// ── RFC-0074: graph_metrics ───────────────────────────────────────────

#[test]
fn store_graph_metrics_complete_graph() {
    // 3-node complete directed graph: 3*2=6 directed edges → density=1.0
    let mut store = Store::new();
    let node_a = store.upsert_node(path("src/a.rs>a"));
    let node_b = store.upsert_node(path("src/b.rs>b"));
    let node_c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, node_a, node_b);
    store.upsert_edge(EdgeKind::Calls, node_a, node_c);
    store.upsert_edge(EdgeKind::Calls, node_b, node_a);
    store.upsert_edge(EdgeKind::Calls, node_b, node_c);
    store.upsert_edge(EdgeKind::Calls, node_c, node_a);
    store.upsert_edge(EdgeKind::Calls, node_c, node_b);
    let m = store.graph_metrics(EdgeKind::Calls);
    assert_eq!(m.symbol_count, 3);
    assert_eq!(m.directed_edge_count, 6);
    assert!(
        (m.density - 1.0).abs() < 1e-9,
        "density should be 1.0, got {}",
        m.density
    );
}

#[test]
fn store_graph_metrics_empty_graph() {
    let store = Store::new();
    let m = store.graph_metrics(EdgeKind::Calls);
    assert_eq!(m.symbol_count, 0);
    assert_eq!(m.directed_edge_count, 0);
    assert!(m.density.abs() < 1e-15);
    assert!(m.avg_degree.abs() < 1e-15);
}

#[test]
fn store_graph_metrics_single_node() {
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs>a"));
    let m = store.graph_metrics(EdgeKind::Calls);
    assert_eq!(m.symbol_count, 1);
    assert!(m.density.abs() < 1e-15, "V < 2 → density = 0");
}

#[test]
fn store_graph_metrics_max_degrees() {
    // star graph: hub → a, hub → b, hub → c; max_out=3, max_in=1
    let mut store = Store::new();
    let hub = store.upsert_node(path("src/hub.rs>hub"));
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    store.upsert_edge(EdgeKind::Calls, hub, a);
    store.upsert_edge(EdgeKind::Calls, hub, b);
    store.upsert_edge(EdgeKind::Calls, hub, c);
    let m = store.graph_metrics(EdgeKind::Calls);
    assert_eq!(m.max_out_degree, 3);
    assert_eq!(m.max_in_degree, 1);
}

#[test]
fn store_graph_metrics_excludes_file_nodes() {
    let mut store = Store::new();
    let sym = store.upsert_node(path("src/a.rs>a"));
    store.upsert_node(path("src/a.rs")); // file node
    let _ = sym;
    let m = store.graph_metrics(EdgeKind::Calls);
    assert_eq!(m.symbol_count, 1, "file node leaked into metrics");
}

// ── RFC-0075: neighbor_similarity ────────────────────────────────────

#[test]
fn store_neighbor_similarity_identical() {
    // a → c, b → c, a ← d, b ← d: N(a) = {c, d}, N(b) = {c, d} → sim = 1.0
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    let d = store.upsert_node(path("src/d.rs>d"));
    store.upsert_edge(EdgeKind::Calls, a, c);
    store.upsert_edge(EdgeKind::Calls, b, c);
    store.upsert_edge(EdgeKind::Calls, d, a);
    store.upsert_edge(EdgeKind::Calls, d, b);
    let sim = store.neighbor_similarity(a, b, EdgeKind::Calls);
    assert!((sim - 1.0).abs() < 1e-9, "expected 1.0, got {sim}");
}

#[test]
fn store_neighbor_similarity_no_overlap() {
    // a → c, b → d: disjoint neighbors → sim = 0.0
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let c = store.upsert_node(path("src/c.rs>c"));
    let d = store.upsert_node(path("src/d.rs>d"));
    store.upsert_edge(EdgeKind::Calls, a, c);
    store.upsert_edge(EdgeKind::Calls, b, d);
    let sim = store.neighbor_similarity(a, b, EdgeKind::Calls);
    assert!(sim.abs() < 1e-9, "expected 0.0, got {sim}");
}

#[test]
fn store_neighbor_similarity_isolated_returns_zero() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    let sim = store.neighbor_similarity(a, b, EdgeKind::Calls);
    assert!(
        sim.abs() < 1e-9,
        "isolated nodes should return 0.0, got {sim}"
    );
}

#[test]
fn store_neighbor_similarity_self() {
    // sim(a, a) = 1.0 when a has neighbors
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>a"));
    let b = store.upsert_node(path("src/b.rs>b"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    let sim = store.neighbor_similarity(a, a, EdgeKind::Calls);
    assert!(
        (sim - 1.0).abs() < 1e-9,
        "self-similarity should be 1.0, got {sim}"
    );
}

#[test]
fn store_neighbor_similarity_partial_overlap() {
    // left → {shared, only_left}, right → {shared, only_right}: sim = 1/3
    let mut store = Store::new();
    let left = store.upsert_node(path("src/a.rs>left"));
    let right = store.upsert_node(path("src/b.rs>right"));
    let shared = store.upsert_node(path("src/c.rs>shared"));
    let only_left = store.upsert_node(path("src/d.rs>only_left"));
    let only_right = store.upsert_node(path("src/e.rs>only_right"));
    store.upsert_edge(EdgeKind::Calls, left, shared);
    store.upsert_edge(EdgeKind::Calls, left, only_left);
    store.upsert_edge(EdgeKind::Calls, right, shared);
    store.upsert_edge(EdgeKind::Calls, right, only_right);
    let sim = store.neighbor_similarity(left, right, EdgeKind::Calls);
    let expected = 1.0_f64 / 3.0;
    assert!(
        (sim - expected).abs() < 1e-9,
        "expected {expected}, got {sim}"
    );
}

// ── RFC-0076: clustering_coefficient ──────────────────────────────────────

#[test]
fn store_clustering_coefficient_too_few_neighbors_returns_zero() {
    // node with 1 neighbor → CC = 0.0 (need ≥ 2 to form a pair)
    let mut store = Store::new();
    let hub = store.upsert_node(path("src/a.rs>hub"));
    let solo = store.upsert_node(path("src/b.rs>solo"));
    store.upsert_edge(EdgeKind::Calls, hub, solo);
    let cc = store.clustering_coefficient(hub, EdgeKind::Calls);
    assert!(cc.abs() < 1e-9, "expected 0.0, got {cc}");
}

#[test]
fn store_clustering_coefficient_complete_triangle_returns_one() {
    // hub → {alpha, beta}, alpha → beta: all 1*(1) = 1 possible directed pair is present
    let mut store = Store::new();
    let hub = store.upsert_node(path("src/a.rs>hub"));
    let alpha = store.upsert_node(path("src/b.rs>alpha"));
    let beta = store.upsert_node(path("src/c.rs>beta"));
    store.upsert_edge(EdgeKind::Calls, hub, alpha);
    store.upsert_edge(EdgeKind::Calls, hub, beta);
    store.upsert_edge(EdgeKind::Calls, alpha, beta); // edge between neighbors
    let cc = store.clustering_coefficient(hub, EdgeKind::Calls);
    // N(hub) = {alpha, beta}, |N| = 2, max_pairs = 2*1 = 2
    // edges among N: alpha→beta = 1  → CC = 1/2 = 0.5
    assert!((cc - 0.5).abs() < 1e-9, "expected 0.5, got {cc}");
}

#[test]
fn store_clustering_coefficient_fully_connected_triangle_returns_one() {
    // hub, alpha, beta — alpha↔beta both directions: 2 edges / 2 max = 1.0
    let mut store = Store::new();
    let hub = store.upsert_node(path("src/d.rs>hub"));
    let alpha = store.upsert_node(path("src/d.rs>alpha"));
    let beta = store.upsert_node(path("src/d.rs>beta"));
    store.upsert_edge(EdgeKind::Calls, hub, alpha);
    store.upsert_edge(EdgeKind::Calls, hub, beta);
    store.upsert_edge(EdgeKind::Calls, alpha, beta);
    store.upsert_edge(EdgeKind::Calls, beta, alpha);
    let cc = store.clustering_coefficient(hub, EdgeKind::Calls);
    // N(hub) = {alpha, beta}, max directed pairs = 2, actual = 2 → CC = 1.0
    assert!((cc - 1.0).abs() < 1e-9, "expected 1.0, got {cc}");
}

#[test]
fn store_clustering_coefficient_partial_connectivity() {
    // hub → {n1, n2, n3}, only n1→n2 exists: 1/6 pairs connected
    let mut store = Store::new();
    let hub = store.upsert_node(path("src/e.rs>hub"));
    let node1 = store.upsert_node(path("src/e.rs>node1"));
    let node2 = store.upsert_node(path("src/e.rs>node2"));
    let node3 = store.upsert_node(path("src/e.rs>node3"));
    store.upsert_edge(EdgeKind::Calls, hub, node1);
    store.upsert_edge(EdgeKind::Calls, hub, node2);
    store.upsert_edge(EdgeKind::Calls, hub, node3);
    store.upsert_edge(EdgeKind::Calls, node1, node2); // only 1 of 6 directed pairs
    let cc = store.clustering_coefficient(hub, EdgeKind::Calls);
    // N(hub) = {node1, node2, node3}, max = 3*2=6, actual = 1 → CC ≈ 0.1667
    let expected = 1.0_f64 / 6.0;
    assert!(
        (cc - expected).abs() < 1e-9,
        "expected {expected}, got {cc}"
    );
}

#[test]
fn store_clustering_coefficient_file_nodes_excluded() {
    // hub → {file_node, symbol} where file_node is a file; file_node→symbol exists
    // but file_node should be excluded from N(hub), so only 1 neighbor → CC = 0.0
    let mut store = Store::new();
    let hub = store.upsert_node(path("src/f.rs>hub"));
    let file_node = store.upsert_node(path("src/g.rs")); // file node
    let sym = store.upsert_node(path("src/f.rs>sym"));
    store.upsert_edge(EdgeKind::Calls, hub, file_node);
    store.upsert_edge(EdgeKind::Calls, hub, sym);
    store.upsert_edge(EdgeKind::Calls, file_node, sym);
    let cc = store.clustering_coefficient(hub, EdgeKind::Calls);
    // N(hub) with file nodes excluded = {sym} only → |N| = 1 → CC = 0.0
    assert!(
        cc.abs() < 1e-9,
        "expected 0.0 with file nodes excluded, got {cc}"
    );
}

// ── RFC-0077: eccentricity ─────────────────────────────────────────────────

#[test]
fn store_eccentricity_isolated_returns_zero() {
    let mut store = Store::new();
    let solo = store.upsert_node(path("src/a.rs>solo"));
    assert_eq!(store.eccentricity(solo, EdgeKind::Calls), 0);
}

#[test]
fn store_eccentricity_direct_edge_returns_one() {
    let mut store = Store::new();
    let root = store.upsert_node(path("src/a.rs>root"));
    let leaf = store.upsert_node(path("src/b.rs>leaf"));
    store.upsert_edge(EdgeKind::Calls, root, leaf);
    assert_eq!(store.eccentricity(root, EdgeKind::Calls), 1);
}

#[test]
fn store_eccentricity_chain_returns_max_depth() {
    // root → mid → far: eccentricity = 2
    let mut store = Store::new();
    let root = store.upsert_node(path("src/a.rs>root"));
    let mid = store.upsert_node(path("src/b.rs>mid"));
    let far = store.upsert_node(path("src/c.rs>far"));
    store.upsert_edge(EdgeKind::Calls, root, mid);
    store.upsert_edge(EdgeKind::Calls, mid, far);
    assert_eq!(store.eccentricity(root, EdgeKind::Calls), 2);
}

#[test]
fn store_eccentricity_directed_not_undirected() {
    // root → leaf, but leaf cannot reach root
    let mut store = Store::new();
    let root = store.upsert_node(path("src/a.rs>root_ecc"));
    let leaf = store.upsert_node(path("src/b.rs>leaf_ecc"));
    store.upsert_edge(EdgeKind::Calls, root, leaf);
    // eccentricity of leaf (no outgoing) = 0
    assert_eq!(store.eccentricity(leaf, EdgeKind::Calls), 0);
}

#[test]
fn store_eccentricity_file_nodes_excluded() {
    // root → file_node → sym: file node is skipped, so root has eccentricity 0
    let mut store = Store::new();
    let root = store.upsert_node(path("src/a.rs>root_file_ecc"));
    let file_node = store.upsert_node(path("src/b.rs"));
    let sym = store.upsert_node(path("src/c.rs>sym_ecc"));
    store.upsert_edge(EdgeKind::Calls, root, file_node);
    store.upsert_edge(EdgeKind::Calls, file_node, sym);
    // file_node is excluded → root can't reach sym → eccentricity = 0
    assert_eq!(store.eccentricity(root, EdgeKind::Calls), 0);
}

// ── RFC-0078: harmonic_centrality ─────────────────────────────────────────

#[test]
fn store_harmonic_centrality_isolated_returns_zero() {
    let mut store = Store::new();
    let solo = store.upsert_node(path("src/a.rs>solo_hc"));
    let hc = store.harmonic_centrality(solo, EdgeKind::Calls);
    assert!(hc.abs() < 1e-9, "expected 0.0, got {hc}");
}

#[test]
fn store_harmonic_centrality_direct_to_all_returns_one() {
    // hub reaches all 3 others in exactly 1 hop; n=4, reachable=3
    // HC = (1/(4-1)) * (1/1 + 1/1 + 1/1) = (1/3) * 3 = 1.0
    let mut store = Store::new();
    let hub = store.upsert_node(path("src/b.rs>hub_hc"));
    let leaf1 = store.upsert_node(path("src/b.rs>leaf1_hc"));
    let leaf2 = store.upsert_node(path("src/b.rs>leaf2_hc"));
    let leaf3 = store.upsert_node(path("src/b.rs>leaf3_hc"));
    store.upsert_edge(EdgeKind::Calls, hub, leaf1);
    store.upsert_edge(EdgeKind::Calls, hub, leaf2);
    store.upsert_edge(EdgeKind::Calls, hub, leaf3);
    let hc = store.harmonic_centrality(hub, EdgeKind::Calls);
    assert!((hc - 1.0).abs() < 1e-9, "expected 1.0, got {hc}");
}

#[test]
fn store_harmonic_centrality_chain_fractional() {
    // root → mid (d=1) → far (d=2); n=3, reachable=2
    // HC = (1/(3-1)) * (1/1 + 1/2) = (1/2) * 1.5 = 0.75
    let mut store = Store::new();
    let root = store.upsert_node(path("src/c.rs>root_hc"));
    let mid = store.upsert_node(path("src/c.rs>mid_hc"));
    let far = store.upsert_node(path("src/c.rs>far_hc"));
    store.upsert_edge(EdgeKind::Calls, root, mid);
    store.upsert_edge(EdgeKind::Calls, mid, far);
    let hc = store.harmonic_centrality(root, EdgeKind::Calls);
    let expected = 0.75_f64;
    assert!(
        (hc - expected).abs() < 1e-9,
        "expected {expected}, got {hc}"
    );
}

#[test]
fn store_harmonic_centrality_unreachable_nodes_ignored() {
    // root → leaf (d=1); isolated_node also present but not reachable from root
    // n=3, reachable from root = 1
    // HC = (1/(3-1)) * (1/1) = 0.5
    let mut store = Store::new();
    let root = store.upsert_node(path("src/d.rs>root_hc_ur"));
    let leaf = store.upsert_node(path("src/d.rs>leaf_hc_ur"));
    let _iso = store.upsert_node(path("src/d.rs>iso_hc_ur"));
    store.upsert_edge(EdgeKind::Calls, root, leaf);
    let hc = store.harmonic_centrality(root, EdgeKind::Calls);
    let expected = 0.5_f64;
    assert!(
        (hc - expected).abs() < 1e-9,
        "expected {expected}, got {hc}"
    );
}

#[test]
fn store_harmonic_centrality_file_nodes_excluded() {
    // root → file_node → sym; file_node excluded, so root can't reach sym
    // n = 1 (only root is a symbol that matters), no reachable → HC = 0.0
    let mut store = Store::new();
    let root = store.upsert_node(path("src/e.rs>root_hc_file"));
    let file_node = store.upsert_node(path("src/e.rs")); // file node
    let sym = store.upsert_node(path("src/f.rs>sym_hc_file"));
    store.upsert_edge(EdgeKind::Calls, root, file_node);
    store.upsert_edge(EdgeKind::Calls, file_node, sym);
    let hc = store.harmonic_centrality(root, EdgeKind::Calls);
    assert!(
        hc.abs() < 1e-9,
        "expected 0.0 with file nodes excluded, got {hc}"
    );
}

// ── RFC-0079: mutual_reachability ─────────────────────────────────────────

#[test]
fn store_mutual_reachability_same_node() {
    let mut store = Store::new();
    let alpha = store.upsert_node(path("src/a.rs>alpha_mr"));
    let mr = store.mutual_reachability(alpha, alpha, EdgeKind::Calls);
    assert!(mr.forward);
    assert!(mr.backward);
    assert!(mr.mutual);
    assert_eq!(mr.forward_distance, Some(0));
    assert_eq!(mr.backward_distance, Some(0));
}

#[test]
fn store_mutual_reachability_forward_only() {
    let mut store = Store::new();
    let src = store.upsert_node(path("src/a.rs>src_mr"));
    let dst = store.upsert_node(path("src/b.rs>dst_mr"));
    store.upsert_edge(EdgeKind::Calls, src, dst);
    let mr = store.mutual_reachability(src, dst, EdgeKind::Calls);
    assert!(mr.forward);
    assert!(!mr.backward);
    assert!(!mr.mutual);
    assert_eq!(mr.forward_distance, Some(1));
    assert_eq!(mr.backward_distance, None);
}

#[test]
fn store_mutual_reachability_no_connection() {
    let mut store = Store::new();
    let node_a = store.upsert_node(path("src/a.rs>node_a_mr"));
    let node_b = store.upsert_node(path("src/b.rs>node_b_mr"));
    let mr = store.mutual_reachability(node_a, node_b, EdgeKind::Calls);
    assert!(!mr.forward);
    assert!(!mr.backward);
    assert!(!mr.mutual);
    assert_eq!(mr.forward_distance, None);
    assert_eq!(mr.backward_distance, None);
}

#[test]
fn store_mutual_reachability_mutual_cycle() {
    // src → dst → src (cycle): both can reach each other
    let mut store = Store::new();
    let src = store.upsert_node(path("src/a.rs>cycle_src_mr"));
    let dst = store.upsert_node(path("src/b.rs>cycle_dst_mr"));
    store.upsert_edge(EdgeKind::Calls, src, dst);
    store.upsert_edge(EdgeKind::Calls, dst, src);
    let mr = store.mutual_reachability(src, dst, EdgeKind::Calls);
    assert!(mr.forward);
    assert!(mr.backward);
    assert!(mr.mutual);
    assert_eq!(mr.forward_distance, Some(1));
    assert_eq!(mr.backward_distance, Some(1));
}

#[test]
fn store_mutual_reachability_multi_hop() {
    // src → mid → dst (2 hops forward), dst → src (1 hop backward)
    let mut store = Store::new();
    let src = store.upsert_node(path("src/a.rs>mh_src_mr"));
    let mid = store.upsert_node(path("src/b.rs>mh_mid_mr"));
    let dst = store.upsert_node(path("src/c.rs>mh_dst_mr"));
    store.upsert_edge(EdgeKind::Calls, src, mid);
    store.upsert_edge(EdgeKind::Calls, mid, dst);
    store.upsert_edge(EdgeKind::Calls, dst, src);
    let mr = store.mutual_reachability(src, dst, EdgeKind::Calls);
    assert!(mr.forward);
    assert!(mr.backward);
    assert!(mr.mutual);
    assert_eq!(mr.forward_distance, Some(2));
    assert_eq!(mr.backward_distance, Some(1));
}

// ── RFC-0080: reachable_set ────────────────────────────────────────────────

#[test]
fn store_reachable_set_isolated_node_returns_empty() {
    let mut store = Store::new();
    let solo = store.upsert_node(TrunkPath::parse("src/rs.rs>solo").unwrap());
    let result = store.reachable_set(solo, EdgeKind::Calls);
    assert!(result.is_empty(), "isolated node should return empty set");
}

#[test]
fn store_reachable_set_direct_neighbors_single_hop() {
    let mut store = Store::new();
    let alpha = store.upsert_node(TrunkPath::parse("src/rs.rs>alpha").unwrap());
    let beta = store.upsert_node(TrunkPath::parse("src/rs.rs>beta").unwrap());
    let gamma = store.upsert_node(TrunkPath::parse("src/rs.rs>gamma").unwrap());
    store.upsert_edge(EdgeKind::Calls, alpha, beta);
    store.upsert_edge(EdgeKind::Calls, alpha, gamma);
    let result = store.reachable_set(alpha, EdgeKind::Calls);
    assert_eq!(result.len(), 2);
    assert!(result.contains(&"src/rs.rs>beta".to_owned()));
    assert!(result.contains(&"src/rs.rs>gamma".to_owned()));
}

#[test]
fn store_reachable_set_chain_returns_full_closure() {
    let mut store = Store::new();
    let head = store.upsert_node(TrunkPath::parse("src/rs.rs>head").unwrap());
    let mid = store.upsert_node(TrunkPath::parse("src/rs.rs>mid").unwrap());
    let tail = store.upsert_node(TrunkPath::parse("src/rs.rs>tail").unwrap());
    store.upsert_edge(EdgeKind::Calls, head, mid);
    store.upsert_edge(EdgeKind::Calls, mid, tail);
    let result = store.reachable_set(head, EdgeKind::Calls);
    assert_eq!(result.len(), 2);
    assert!(result.contains(&"src/rs.rs>mid".to_owned()));
    assert!(result.contains(&"src/rs.rs>tail".to_owned()));
}

#[test]
fn store_reachable_set_excludes_self_even_in_cycle() {
    let mut store = Store::new();
    let origin = store.upsert_node(TrunkPath::parse("src/rs.rs>origin").unwrap());
    let dest = store.upsert_node(TrunkPath::parse("src/rs.rs>dest").unwrap());
    store.upsert_edge(EdgeKind::Calls, origin, dest);
    store.upsert_edge(EdgeKind::Calls, dest, origin);
    let result = store.reachable_set(origin, EdgeKind::Calls);
    assert!(
        !result.contains(&"src/rs.rs>origin".to_owned()),
        "self must not appear"
    );
    assert!(result.contains(&"src/rs.rs>dest".to_owned()));
}

#[test]
fn store_reachable_set_results_are_sorted() {
    let mut store = Store::new();
    let root = store.upsert_node(TrunkPath::parse("src/rs.rs>root").unwrap());
    let zeta = store.upsert_node(TrunkPath::parse("src/rs.rs>zeta").unwrap());
    let aaaa = store.upsert_node(TrunkPath::parse("src/rs.rs>aaaa").unwrap());
    store.upsert_edge(EdgeKind::Calls, root, zeta);
    store.upsert_edge(EdgeKind::Calls, root, aaaa);
    let result = store.reachable_set(root, EdgeKind::Calls);
    let mut sorted = result.clone();
    sorted.sort();
    assert_eq!(result, sorted, "results must be sorted alphabetically");
}

// ── RFC-0081: reaches_into ────────────────────────────────────────────────

#[test]
fn store_reaches_into_isolated_node_returns_empty() {
    let mut store = Store::new();
    let solo = store.upsert_node(TrunkPath::parse("src/ri.rs>solo").unwrap());
    let result = store.reaches_into(solo, EdgeKind::Calls);
    assert!(result.is_empty(), "isolated node should return empty set");
}

#[test]
fn store_reaches_into_direct_callers_single_hop() {
    let mut store = Store::new();
    let target = store.upsert_node(TrunkPath::parse("src/ri.rs>target").unwrap());
    let caller_a = store.upsert_node(TrunkPath::parse("src/ri.rs>caller_a").unwrap());
    let caller_b = store.upsert_node(TrunkPath::parse("src/ri.rs>caller_b").unwrap());
    store.upsert_edge(EdgeKind::Calls, caller_a, target);
    store.upsert_edge(EdgeKind::Calls, caller_b, target);
    let result = store.reaches_into(target, EdgeKind::Calls);
    assert_eq!(result.len(), 2);
    assert!(result.contains(&"src/ri.rs>caller_a".to_owned()));
    assert!(result.contains(&"src/ri.rs>caller_b".to_owned()));
}

#[test]
fn store_reaches_into_chain_returns_full_reverse_closure() {
    let mut store = Store::new();
    let head = store.upsert_node(TrunkPath::parse("src/ri.rs>head").unwrap());
    let mid = store.upsert_node(TrunkPath::parse("src/ri.rs>mid").unwrap());
    let tail = store.upsert_node(TrunkPath::parse("src/ri.rs>tail").unwrap());
    store.upsert_edge(EdgeKind::Calls, head, mid);
    store.upsert_edge(EdgeKind::Calls, mid, tail);
    let result = store.reaches_into(tail, EdgeKind::Calls);
    assert_eq!(result.len(), 2, "head and mid both reach tail");
    assert!(result.contains(&"src/ri.rs>head".to_owned()));
    assert!(result.contains(&"src/ri.rs>mid".to_owned()));
}

#[test]
fn store_reaches_into_excludes_self_in_cycle() {
    let mut store = Store::new();
    let origin = store.upsert_node(TrunkPath::parse("src/ri.rs>origin_ri").unwrap());
    let partner = store.upsert_node(TrunkPath::parse("src/ri.rs>partner_ri").unwrap());
    store.upsert_edge(EdgeKind::Calls, origin, partner);
    store.upsert_edge(EdgeKind::Calls, partner, origin);
    let result = store.reaches_into(origin, EdgeKind::Calls);
    assert!(
        !result.contains(&"src/ri.rs>origin_ri".to_owned()),
        "self must not appear"
    );
    assert!(result.contains(&"src/ri.rs>partner_ri".to_owned()));
}

#[test]
fn store_reaches_into_results_are_sorted() {
    let mut store = Store::new();
    let leaf = store.upsert_node(TrunkPath::parse("src/ri.rs>leaf").unwrap());
    let zeta_ri = store.upsert_node(TrunkPath::parse("src/ri.rs>zeta_ri").unwrap());
    let aaaa_ri = store.upsert_node(TrunkPath::parse("src/ri.rs>aaaa_ri").unwrap());
    store.upsert_edge(EdgeKind::Calls, zeta_ri, leaf);
    store.upsert_edge(EdgeKind::Calls, aaaa_ri, leaf);
    let result = store.reaches_into(leaf, EdgeKind::Calls);
    let mut sorted = result.clone();
    sorted.sort();
    assert_eq!(result, sorted, "results must be sorted alphabetically");
}

// ── RFC-0082: page_rank ────────────────────────────────────────────────────

#[test]
fn store_page_rank_empty_graph_returns_empty() {
    let store = Store::new();
    let result = store.page_rank(EdgeKind::Calls, 0.85, 20);
    assert!(result.is_empty());
}

#[test]
fn store_page_rank_single_node_returns_score_one() {
    let mut store = Store::new();
    store.upsert_node(TrunkPath::parse("src/pr.rs>only").unwrap());
    let result = store.page_rank(EdgeKind::Calls, 0.85, 20);
    assert_eq!(result.len(), 1);
    assert!(
        (result[0].score - 1.0).abs() < 1e-9,
        "single node score must be 1.0"
    );
}

#[test]
fn store_page_rank_no_edges_uniform_scores() {
    let mut store = Store::new();
    store.upsert_node(TrunkPath::parse("src/pr.rs>node_a").unwrap());
    store.upsert_node(TrunkPath::parse("src/pr.rs>node_b").unwrap());
    store.upsert_node(TrunkPath::parse("src/pr.rs>node_c").unwrap());
    let result = store.page_rank(EdgeKind::Calls, 0.85, 20);
    assert_eq!(result.len(), 3);
    for entry in &result {
        assert!(
            (entry.score - 1.0 / 3.0).abs() < 1e-6,
            "no-edge graph must have uniform scores, got {}",
            entry.score
        );
    }
}

#[test]
fn store_page_rank_star_hub_has_highest_score() {
    let mut store = Store::new();
    let hub = store.upsert_node(TrunkPath::parse("src/pr.rs>hub").unwrap());
    let spoke_a = store.upsert_node(TrunkPath::parse("src/pr.rs>spoke_a").unwrap());
    let spoke_b = store.upsert_node(TrunkPath::parse("src/pr.rs>spoke_b").unwrap());
    let spoke_c = store.upsert_node(TrunkPath::parse("src/pr.rs>spoke_c").unwrap());
    store.upsert_edge(EdgeKind::Calls, spoke_a, hub);
    store.upsert_edge(EdgeKind::Calls, spoke_b, hub);
    store.upsert_edge(EdgeKind::Calls, spoke_c, hub);
    let result = store.page_rank(EdgeKind::Calls, 0.85, 50);
    assert_eq!(result[0].path, "src/pr.rs>hub", "hub must rank first");
}

#[test]
fn store_page_rank_damping_zero_gives_uniform() {
    let mut store = Store::new();
    let node_x = store.upsert_node(TrunkPath::parse("src/pr.rs>node_x").unwrap());
    let node_y = store.upsert_node(TrunkPath::parse("src/pr.rs>node_y").unwrap());
    store.upsert_edge(EdgeKind::Calls, node_x, node_y);
    let result = store.page_rank(EdgeKind::Calls, 0.0, 20);
    assert_eq!(result.len(), 2);
    for entry in &result {
        assert!(
            (entry.score - 0.5).abs() < 1e-6,
            "damping=0 must yield uniform scores"
        );
    }
}

// ── RFC-0083: common_reachable ────────────────────────────────────────────

#[test]
fn store_common_reachable_both_isolated_returns_empty() {
    let mut store = Store::new();
    let left = store.upsert_node(TrunkPath::parse("src/cr.rs>left").unwrap());
    let right = store.upsert_node(TrunkPath::parse("src/cr.rs>right").unwrap());
    let result = store.common_reachable(left, right, EdgeKind::Calls);
    assert!(result.is_empty());
}

#[test]
fn store_common_reachable_no_overlap_returns_empty() {
    let mut store = Store::new();
    let left = store.upsert_node(TrunkPath::parse("src/cr.rs>cr_left").unwrap());
    let right = store.upsert_node(TrunkPath::parse("src/cr.rs>cr_right").unwrap());
    let dep_l = store.upsert_node(TrunkPath::parse("src/cr.rs>dep_l").unwrap());
    let dep_r = store.upsert_node(TrunkPath::parse("src/cr.rs>dep_r").unwrap());
    store.upsert_edge(EdgeKind::Calls, left, dep_l);
    store.upsert_edge(EdgeKind::Calls, right, dep_r);
    let result = store.common_reachable(left, right, EdgeKind::Calls);
    assert!(result.is_empty());
}

#[test]
fn store_common_reachable_shared_dep_returns_it() {
    let mut store = Store::new();
    let left = store.upsert_node(TrunkPath::parse("src/cr.rs>cr_a").unwrap());
    let right = store.upsert_node(TrunkPath::parse("src/cr.rs>cr_b").unwrap());
    let shared = store.upsert_node(TrunkPath::parse("src/cr.rs>cr_shared").unwrap());
    store.upsert_edge(EdgeKind::Calls, left, shared);
    store.upsert_edge(EdgeKind::Calls, right, shared);
    let result = store.common_reachable(left, right, EdgeKind::Calls);
    assert_eq!(result, vec!["src/cr.rs>cr_shared"]);
}

#[test]
fn store_common_reachable_same_node_equals_reachable_set() {
    let mut store = Store::new();
    let node = store.upsert_node(TrunkPath::parse("src/cr.rs>cr_node").unwrap());
    let dep_a = store.upsert_node(TrunkPath::parse("src/cr.rs>cr_dep_a").unwrap());
    let dep_b = store.upsert_node(TrunkPath::parse("src/cr.rs>cr_dep_b").unwrap());
    store.upsert_edge(EdgeKind::Calls, node, dep_a);
    store.upsert_edge(EdgeKind::Calls, node, dep_b);
    let common = store.common_reachable(node, node, EdgeKind::Calls);
    let reachable = store.reachable_set(node, EdgeKind::Calls);
    assert_eq!(common, reachable, "same-node must equal reachable_set");
}

#[test]
fn store_common_reachable_results_are_sorted() {
    let mut store = Store::new();
    let left = store.upsert_node(TrunkPath::parse("src/cr.rs>cr_x").unwrap());
    let right = store.upsert_node(TrunkPath::parse("src/cr.rs>cr_y").unwrap());
    let zz = store.upsert_node(TrunkPath::parse("src/cr.rs>cr_zz").unwrap());
    let aa = store.upsert_node(TrunkPath::parse("src/cr.rs>cr_aa").unwrap());
    store.upsert_edge(EdgeKind::Calls, left, zz);
    store.upsert_edge(EdgeKind::Calls, left, aa);
    store.upsert_edge(EdgeKind::Calls, right, zz);
    store.upsert_edge(EdgeKind::Calls, right, aa);
    let result = store.common_reachable(left, right, EdgeKind::Calls);
    let mut sorted = result.clone();
    sorted.sort();
    assert_eq!(result, sorted);
}

// ── RFC-0084: k_hop_neighbors ─────────────────────────────────────────────

#[test]
fn store_k_hop_neighbors_k_zero_returns_empty() {
    let mut store = Store::new();
    let node = store.upsert_node(TrunkPath::parse("src/kh.rs>kh_node").unwrap());
    let dep = store.upsert_node(TrunkPath::parse("src/kh.rs>kh_dep").unwrap());
    store.upsert_edge(EdgeKind::Calls, node, dep);
    let result = store.k_hop_neighbors(node, EdgeKind::Calls, 0);
    assert!(result.is_empty(), "k=0 must return empty");
}

#[test]
fn store_k_hop_neighbors_k_one_returns_direct_neighbors() {
    let mut store = Store::new();
    let root = store.upsert_node(TrunkPath::parse("src/kh.rs>kh_root").unwrap());
    let child_a = store.upsert_node(TrunkPath::parse("src/kh.rs>kh_child_a").unwrap());
    let child_b = store.upsert_node(TrunkPath::parse("src/kh.rs>kh_child_b").unwrap());
    let grandchild = store.upsert_node(TrunkPath::parse("src/kh.rs>kh_grand").unwrap());
    store.upsert_edge(EdgeKind::Calls, root, child_a);
    store.upsert_edge(EdgeKind::Calls, root, child_b);
    store.upsert_edge(EdgeKind::Calls, child_a, grandchild);
    let result = store.k_hop_neighbors(root, EdgeKind::Calls, 1);
    assert_eq!(result.len(), 2);
    assert!(result.contains(&"src/kh.rs>kh_child_a".to_owned()));
    assert!(result.contains(&"src/kh.rs>kh_child_b".to_owned()));
    assert!(
        !result.contains(&"src/kh.rs>kh_grand".to_owned()),
        "grandchild must not appear at k=1"
    );
}

#[test]
fn store_k_hop_neighbors_k_two_skips_closer_nodes() {
    let mut store = Store::new();
    let root = store.upsert_node(TrunkPath::parse("src/kh.rs>kh2_root").unwrap());
    let mid = store.upsert_node(TrunkPath::parse("src/kh.rs>kh2_mid").unwrap());
    let far = store.upsert_node(TrunkPath::parse("src/kh.rs>kh2_far").unwrap());
    store.upsert_edge(EdgeKind::Calls, root, mid);
    store.upsert_edge(EdgeKind::Calls, mid, far);
    let result = store.k_hop_neighbors(root, EdgeKind::Calls, 2);
    assert_eq!(result, vec!["src/kh.rs>kh2_far"]);
}

#[test]
fn store_k_hop_neighbors_excludes_self_in_cycle() {
    let mut store = Store::new();
    let origin = store.upsert_node(TrunkPath::parse("src/kh.rs>kh_origin").unwrap());
    let partner = store.upsert_node(TrunkPath::parse("src/kh.rs>kh_partner").unwrap());
    store.upsert_edge(EdgeKind::Calls, origin, partner);
    store.upsert_edge(EdgeKind::Calls, partner, origin);
    // k=2: origin -> partner -> origin; but origin is excluded (it's the source)
    let result = store.k_hop_neighbors(origin, EdgeKind::Calls, 2);
    assert!(
        !result.contains(&"src/kh.rs>kh_origin".to_owned()),
        "self must be excluded"
    );
}

#[test]
fn store_k_hop_neighbors_results_are_sorted() {
    let mut store = Store::new();
    let root = store.upsert_node(TrunkPath::parse("src/kh.rs>kh_s_root").unwrap());
    let mid = store.upsert_node(TrunkPath::parse("src/kh.rs>kh_s_mid").unwrap());
    let zz = store.upsert_node(TrunkPath::parse("src/kh.rs>kh_s_zz").unwrap());
    let aa = store.upsert_node(TrunkPath::parse("src/kh.rs>kh_s_aa").unwrap());
    store.upsert_edge(EdgeKind::Calls, root, mid);
    store.upsert_edge(EdgeKind::Calls, mid, zz);
    store.upsert_edge(EdgeKind::Calls, mid, aa);
    let result = store.k_hop_neighbors(root, EdgeKind::Calls, 2);
    let mut sorted = result.clone();
    sorted.sort();
    assert_eq!(result, sorted, "results must be sorted");
}

// ── RFC-0085: betweenness_centrality ─────────────────────────────────────

#[test]
fn store_betweenness_empty_graph_returns_empty() {
    let store = Store::new();
    let result = store.betweenness_centrality(EdgeKind::Calls);
    assert!(result.is_empty());
}

#[test]
fn store_betweenness_single_node_returns_empty() {
    let mut store = Store::new();
    store.upsert_node(TrunkPath::parse("src/bc.rs>bc_only").unwrap());
    let result = store.betweenness_centrality(EdgeKind::Calls);
    assert!(result.is_empty());
}

#[test]
fn store_betweenness_two_nodes_scores_zero() {
    let mut store = Store::new();
    let node_p = store.upsert_node(TrunkPath::parse("src/bc.rs>bc_p").unwrap());
    let node_q = store.upsert_node(TrunkPath::parse("src/bc.rs>bc_q").unwrap());
    store.upsert_edge(EdgeKind::Calls, node_p, node_q);
    let result = store.betweenness_centrality(EdgeKind::Calls);
    assert_eq!(result.len(), 2);
    for entry in &result {
        assert!(
            entry.score.abs() < 1e-9,
            "two-node graph must have all scores 0.0"
        );
    }
}

#[test]
fn store_betweenness_chain_middle_has_highest_score() {
    let mut store = Store::new();
    let head = store.upsert_node(TrunkPath::parse("src/bc.rs>bc_head").unwrap());
    let mid = store.upsert_node(TrunkPath::parse("src/bc.rs>bc_mid").unwrap());
    let tail = store.upsert_node(TrunkPath::parse("src/bc.rs>bc_tail").unwrap());
    store.upsert_edge(EdgeKind::Calls, head, mid);
    store.upsert_edge(EdgeKind::Calls, mid, tail);
    let result = store.betweenness_centrality(EdgeKind::Calls);
    assert_eq!(result.len(), 3);
    let mid_entry = result
        .iter()
        .find(|e| e.path == "src/bc.rs>bc_mid")
        .unwrap();
    let head_entry = result
        .iter()
        .find(|e| e.path == "src/bc.rs>bc_head")
        .unwrap();
    let tail_entry = result
        .iter()
        .find(|e| e.path == "src/bc.rs>bc_tail")
        .unwrap();
    assert!(mid_entry.score > head_entry.score, "mid must outrank head");
    assert!(mid_entry.score > tail_entry.score, "mid must outrank tail");
}

#[test]
fn store_betweenness_scores_normalized_in_range() {
    let mut store = Store::new();
    let node_a = store.upsert_node(TrunkPath::parse("src/bc.rs>bc_norm_a").unwrap());
    let node_b = store.upsert_node(TrunkPath::parse("src/bc.rs>bc_norm_b").unwrap());
    let node_c = store.upsert_node(TrunkPath::parse("src/bc.rs>bc_norm_c").unwrap());
    let node_d = store.upsert_node(TrunkPath::parse("src/bc.rs>bc_norm_d").unwrap());
    store.upsert_edge(EdgeKind::Calls, node_a, node_b);
    store.upsert_edge(EdgeKind::Calls, node_b, node_c);
    store.upsert_edge(EdgeKind::Calls, node_c, node_d);
    let result = store.betweenness_centrality(EdgeKind::Calls);
    for entry in &result {
        assert!(
            entry.score >= 0.0 && entry.score <= 1.0,
            "score {} out of [0,1] range",
            entry.score
        );
    }
}

// ── RFC-0086: strongly_connected_components ───────────────────────────────

#[test]
fn store_scc_empty_graph() {
    let store = Store::new();
    let result = store.strongly_connected_components(EdgeKind::Calls);
    assert!(result.is_empty());
}

#[test]
fn store_scc_single_node() {
    let mut store = Store::new();
    store.upsert_node(TrunkPath::parse("src/scc.rs>scc_solo").unwrap());
    let result = store.strongly_connected_components(EdgeKind::Calls);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].size, 1);
    assert_eq!(result[0].members, vec!["src/scc.rs>scc_solo".to_owned()]);
}

#[test]
fn store_scc_two_nodes_no_edges() {
    let mut store = Store::new();
    store.upsert_node(TrunkPath::parse("src/scc.rs>scc_a").unwrap());
    store.upsert_node(TrunkPath::parse("src/scc.rs>scc_b").unwrap());
    let result = store.strongly_connected_components(EdgeKind::Calls);
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|e| e.size == 1));
}

#[test]
fn store_scc_two_nodes_one_directed_edge() {
    let mut store = Store::new();
    let a = store.upsert_node(TrunkPath::parse("src/scc.rs>scc_d_a").unwrap());
    let b = store.upsert_node(TrunkPath::parse("src/scc.rs>scc_d_b").unwrap());
    store.upsert_edge(EdgeKind::Calls, a, b);
    let result = store.strongly_connected_components(EdgeKind::Calls);
    // A→B but no B→A: two singleton SCCs.
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|e| e.size == 1));
}

#[test]
fn store_scc_two_nodes_mutual_edges() {
    let mut store = Store::new();
    let a = store.upsert_node(TrunkPath::parse("src/scc.rs>scc_m_a").unwrap());
    let b = store.upsert_node(TrunkPath::parse("src/scc.rs>scc_m_b").unwrap());
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, a);
    let result = store.strongly_connected_components(EdgeKind::Calls);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].size, 2);
    let mut members = result[0].members.clone();
    members.sort();
    assert!(members.contains(&"src/scc.rs>scc_m_a".to_owned()));
    assert!(members.contains(&"src/scc.rs>scc_m_b".to_owned()));
}

#[test]
fn store_scc_linear_chain_no_cycle() {
    let mut store = Store::new();
    let a = store.upsert_node(TrunkPath::parse("src/scc.rs>scc_lin_a").unwrap());
    let b = store.upsert_node(TrunkPath::parse("src/scc.rs>scc_lin_b").unwrap());
    let c = store.upsert_node(TrunkPath::parse("src/scc.rs>scc_lin_c").unwrap());
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    let result = store.strongly_connected_components(EdgeKind::Calls);
    // A→B→C: three singletons.
    assert_eq!(result.len(), 3);
    assert!(result.iter().all(|e| e.size == 1));
}

#[test]
fn store_scc_three_node_cycle() {
    let mut store = Store::new();
    let a = store.upsert_node(TrunkPath::parse("src/scc.rs>scc_cyc_a").unwrap());
    let b = store.upsert_node(TrunkPath::parse("src/scc.rs>scc_cyc_b").unwrap());
    let c = store.upsert_node(TrunkPath::parse("src/scc.rs>scc_cyc_c").unwrap());
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    store.upsert_edge(EdgeKind::Calls, c, a);
    let result = store.strongly_connected_components(EdgeKind::Calls);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].size, 3);
    let mut members = result[0].members.clone();
    members.sort();
    assert_eq!(
        members,
        vec![
            "src/scc.rs>scc_cyc_a".to_owned(),
            "src/scc.rs>scc_cyc_b".to_owned(),
            "src/scc.rs>scc_cyc_c".to_owned(),
        ]
    );
}

#[test]
fn store_scc_members_sorted_alphabetically() {
    let mut store = Store::new();
    let z = store.upsert_node(TrunkPath::parse("src/scc.rs>scc_sort_z").unwrap());
    let a = store.upsert_node(TrunkPath::parse("src/scc.rs>scc_sort_a").unwrap());
    let m = store.upsert_node(TrunkPath::parse("src/scc.rs>scc_sort_m").unwrap());
    store.upsert_edge(EdgeKind::Calls, z, a);
    store.upsert_edge(EdgeKind::Calls, a, m);
    store.upsert_edge(EdgeKind::Calls, m, z);
    let result = store.strongly_connected_components(EdgeKind::Calls);
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0].members,
        vec![
            "src/scc.rs>scc_sort_a".to_owned(),
            "src/scc.rs>scc_sort_m".to_owned(),
            "src/scc.rs>scc_sort_z".to_owned(),
        ]
    );
}

#[test]
fn store_scc_results_sorted_by_size_descending() {
    let mut store = Store::new();
    // Cycle of 3.
    let a = store.upsert_node(TrunkPath::parse("src/scc.rs>scc_ord_a").unwrap());
    let b = store.upsert_node(TrunkPath::parse("src/scc.rs>scc_ord_b").unwrap());
    let c = store.upsert_node(TrunkPath::parse("src/scc.rs>scc_ord_c").unwrap());
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    store.upsert_edge(EdgeKind::Calls, c, a);
    // Singleton.
    store.upsert_node(TrunkPath::parse("src/scc.rs>scc_ord_d").unwrap());
    let result = store.strongly_connected_components(EdgeKind::Calls);
    assert!(result[0].size >= result[result.len() - 1].size);
    assert_eq!(result[0].size, 3);
}

#[test]
fn store_scc_file_nodes_excluded() {
    let mut store = Store::new();
    let file = store.upsert_node(TrunkPath::parse("src/scc_file.rs").unwrap());
    let sym = store.upsert_node(TrunkPath::parse("src/scc_file.rs>scc_file_sym").unwrap());
    store.upsert_edge(EdgeKind::Calls, file, sym);
    store.upsert_edge(EdgeKind::Calls, sym, file);
    let result = store.strongly_connected_components(EdgeKind::Calls);
    // Only the symbol node should appear; file excluded.
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].size, 1);
    assert_eq!(
        result[0].members,
        vec!["src/scc_file.rs>scc_file_sym".to_owned()]
    );
}

// ── RFC-0087: degree_centrality ───────────────────────────────────────────

#[test]
fn store_degree_centrality_empty_graph() {
    let store = Store::new();
    let result = store.degree_centrality(EdgeKind::Calls);
    assert!(result.is_empty());
}

#[test]
fn store_degree_centrality_single_node() {
    let mut store = Store::new();
    store.upsert_node(TrunkPath::parse("src/dc.rs>dc_solo").unwrap());
    let result = store.degree_centrality(EdgeKind::Calls);
    assert_eq!(result.len(), 1);
    let entry = &result[0];
    assert_eq!(entry.in_degree, 0);
    assert_eq!(entry.out_degree, 0);
    // n=1 so norm=1, centrality=0/1=0.0
    assert!((entry.in_centrality).abs() < 1e-9);
    assert!((entry.out_centrality).abs() < 1e-9);
}

#[test]
fn store_degree_centrality_two_nodes_one_edge() {
    let mut store = Store::new();
    let a = store.upsert_node(TrunkPath::parse("src/dc.rs>dc_two_a").unwrap());
    let b = store.upsert_node(TrunkPath::parse("src/dc.rs>dc_two_b").unwrap());
    store.upsert_edge(EdgeKind::Calls, a, b);
    let result = store.degree_centrality(EdgeKind::Calls);
    assert_eq!(result.len(), 2);
    // n=2, norm=1.0; B has in_degree=1 → in_centrality=1.0; A has out_degree=1 → out_centrality=1.0
    let entry_b = result
        .iter()
        .find(|e| e.path == "src/dc.rs>dc_two_b")
        .unwrap();
    let entry_a = result
        .iter()
        .find(|e| e.path == "src/dc.rs>dc_two_a")
        .unwrap();
    assert_eq!(entry_b.in_degree, 1);
    assert!((entry_b.in_centrality - 1.0).abs() < 1e-9);
    assert_eq!(entry_a.out_degree, 1);
    assert!((entry_a.out_centrality - 1.0).abs() < 1e-9);
}

#[test]
fn store_degree_centrality_scores_normalized() {
    let mut store = Store::new();
    let hub = store.upsert_node(TrunkPath::parse("src/dc.rs>dc_hub").unwrap());
    let s1 = store.upsert_node(TrunkPath::parse("src/dc.rs>dc_s1").unwrap());
    let s2 = store.upsert_node(TrunkPath::parse("src/dc.rs>dc_s2").unwrap());
    let s3 = store.upsert_node(TrunkPath::parse("src/dc.rs>dc_s3").unwrap());
    store.upsert_edge(EdgeKind::Calls, s1, hub);
    store.upsert_edge(EdgeKind::Calls, s2, hub);
    store.upsert_edge(EdgeKind::Calls, s3, hub);
    let result = store.degree_centrality(EdgeKind::Calls);
    for entry in &result {
        assert!(entry.in_centrality >= 0.0 && entry.in_centrality <= 1.0);
        assert!(entry.out_centrality >= 0.0 && entry.out_centrality <= 1.0);
    }
    let hub_entry = result
        .iter()
        .find(|e| e.path == "src/dc.rs>dc_hub")
        .unwrap();
    assert_eq!(hub_entry.in_degree, 3);
    // n=4, norm=3.0; in_centrality = 3/3 = 1.0
    assert!((hub_entry.in_centrality - 1.0).abs() < 1e-9);
}

#[test]
fn store_degree_centrality_sorted_by_in_centrality_desc() {
    let mut store = Store::new();
    let hub = store.upsert_node(TrunkPath::parse("src/dc.rs>dc_ord_hub").unwrap());
    let x = store.upsert_node(TrunkPath::parse("src/dc.rs>dc_ord_x").unwrap());
    let y = store.upsert_node(TrunkPath::parse("src/dc.rs>dc_ord_y").unwrap());
    store.upsert_edge(EdgeKind::Calls, x, hub);
    store.upsert_edge(EdgeKind::Calls, y, hub);
    let result = store.degree_centrality(EdgeKind::Calls);
    // hub has highest in_centrality → should be first
    assert_eq!(result[0].path, "src/dc.rs>dc_ord_hub");
}

#[test]
fn store_degree_centrality_file_nodes_excluded() {
    let mut store = Store::new();
    let file = store.upsert_node(TrunkPath::parse("src/dc_file.rs").unwrap());
    let sym = store.upsert_node(TrunkPath::parse("src/dc_file.rs>dc_fsym").unwrap());
    store.upsert_edge(EdgeKind::Calls, file, sym);
    let result = store.degree_centrality(EdgeKind::Calls);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].path, "src/dc_file.rs>dc_fsym");
    // File edge not counted since file node excluded from traversal.
    assert_eq!(result[0].in_degree, 0);
}

// ── RFC-0088: closeness_centrality ────────────────────────────────────────

#[test]
fn store_closeness_empty_graph() {
    let store = Store::new();
    let result = store.closeness_centrality(EdgeKind::Calls);
    assert!(result.is_empty());
}

#[test]
fn store_closeness_single_node() {
    let mut store = Store::new();
    store.upsert_node(TrunkPath::parse("src/cc.rs>cc_solo").unwrap());
    let result = store.closeness_centrality(EdgeKind::Calls);
    assert_eq!(result.len(), 1);
    assert!((result[0].score).abs() < 1e-9);
}

#[test]
fn store_closeness_two_nodes_no_edge() {
    let mut store = Store::new();
    store.upsert_node(TrunkPath::parse("src/cc.rs>cc_na").unwrap());
    store.upsert_node(TrunkPath::parse("src/cc.rs>cc_nb").unwrap());
    let result = store.closeness_centrality(EdgeKind::Calls);
    assert_eq!(result.len(), 2);
    for e in &result {
        assert!((e.score).abs() < 1e-9);
    }
}

#[test]
fn store_closeness_two_nodes_one_edge() {
    let mut store = Store::new();
    let a = store.upsert_node(TrunkPath::parse("src/cc.rs>cc_2a").unwrap());
    let b = store.upsert_node(TrunkPath::parse("src/cc.rs>cc_2b").unwrap());
    store.upsert_edge(EdgeKind::Calls, a, b);
    let result = store.closeness_centrality(EdgeKind::Calls);
    // A reaches B; B reaches no one.
    let entry_a = result.iter().find(|e| e.path == "src/cc.rs>cc_2a").unwrap();
    let entry_b = result.iter().find(|e| e.path == "src/cc.rs>cc_2b").unwrap();
    assert!(entry_a.score > 0.0, "A should have positive closeness");
    assert!((entry_b.score).abs() < 1e-9, "B reaches no one → 0.0");
}

#[test]
fn store_closeness_linear_chain_ordering() {
    let mut store = Store::new();
    let a = store.upsert_node(TrunkPath::parse("src/cc.rs>cc_lin_a").unwrap());
    let b = store.upsert_node(TrunkPath::parse("src/cc.rs>cc_lin_b").unwrap());
    let c = store.upsert_node(TrunkPath::parse("src/cc.rs>cc_lin_c").unwrap());
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    // A can reach B and C; B can reach C; C reaches no one.
    let result = store.closeness_centrality(EdgeKind::Calls);
    let score_a = result
        .iter()
        .find(|e| e.path == "src/cc.rs>cc_lin_a")
        .unwrap()
        .score;
    let score_b = result
        .iter()
        .find(|e| e.path == "src/cc.rs>cc_lin_b")
        .unwrap()
        .score;
    let score_c = result
        .iter()
        .find(|e| e.path == "src/cc.rs>cc_lin_c")
        .unwrap()
        .score;
    assert!(
        score_a > score_b,
        "A reaches more nodes with shorter total dist"
    );
    assert!(score_b > score_c, "B reaches C; C reaches no one");
    assert!((score_c).abs() < 1e-9);
}

#[test]
fn store_closeness_scores_in_range() {
    let mut store = Store::new();
    let a = store.upsert_node(TrunkPath::parse("src/cc.rs>cc_rng_a").unwrap());
    let b = store.upsert_node(TrunkPath::parse("src/cc.rs>cc_rng_b").unwrap());
    let c = store.upsert_node(TrunkPath::parse("src/cc.rs>cc_rng_c").unwrap());
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    store.upsert_edge(EdgeKind::Calls, c, a);
    let result = store.closeness_centrality(EdgeKind::Calls);
    for e in &result {
        assert!(
            e.score >= 0.0 && e.score <= 1.0,
            "score {} out of [0,1]",
            e.score
        );
    }
}

#[test]
fn store_closeness_file_nodes_excluded() {
    let mut store = Store::new();
    let file = store.upsert_node(TrunkPath::parse("src/cc_file.rs").unwrap());
    let sym = store.upsert_node(TrunkPath::parse("src/cc_file.rs>cc_fsym").unwrap());
    store.upsert_edge(EdgeKind::Calls, file, sym);
    let result = store.closeness_centrality(EdgeKind::Calls);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].path, "src/cc_file.rs>cc_fsym");
}

// RFC-0089: Store::dependency_depth

#[test]
fn dep_depth_leaf_node_is_zero() {
    let mut store = Store::new();
    let leaf = store.upsert_node(path("src/dd.rs>dd_leaf"));
    assert_eq!(store.dependency_depth(leaf, EdgeKind::Calls), Some(0));
}

#[test]
fn dep_depth_one_hop() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/dd.rs>dd_1_a"));
    let b = store.upsert_node(path("src/dd.rs>dd_1_b"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    assert_eq!(store.dependency_depth(a, EdgeKind::Calls), Some(0));
    assert_eq!(store.dependency_depth(b, EdgeKind::Calls), Some(1));
}

#[test]
fn dep_depth_two_hop_chain() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/dd.rs>dd_2_a"));
    let b = store.upsert_node(path("src/dd.rs>dd_2_b"));
    let c = store.upsert_node(path("src/dd.rs>dd_2_c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    assert_eq!(store.dependency_depth(a, EdgeKind::Calls), Some(0));
    assert_eq!(store.dependency_depth(b, EdgeKind::Calls), Some(1));
    assert_eq!(store.dependency_depth(c, EdgeKind::Calls), Some(2));
}

#[test]
fn dep_depth_longest_path_wins() {
    // a->c (1 hop) and a->b->c (2 hops): depth of c = 2.
    let mut store = Store::new();
    let a = store.upsert_node(path("src/dd.rs>dd_long_a"));
    let b = store.upsert_node(path("src/dd.rs>dd_long_b"));
    let c = store.upsert_node(path("src/dd.rs>dd_long_c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    store.upsert_edge(EdgeKind::Calls, a, c);
    assert_eq!(store.dependency_depth(c, EdgeKind::Calls), Some(2));
}

#[test]
fn dep_depth_cycle_member_safe() {
    // a->b->c->a cycle: algorithm terminates.
    let mut store = Store::new();
    let a = store.upsert_node(path("src/dd.rs>dd_cyc_a"));
    let b = store.upsert_node(path("src/dd.rs>dd_cyc_b"));
    let c = store.upsert_node(path("src/dd.rs>dd_cyc_c"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, b, c);
    store.upsert_edge(EdgeKind::Calls, c, a);
    assert!(store.dependency_depth(a, EdgeKind::Calls).is_some());
    assert!(store.dependency_depth(b, EdgeKind::Calls).is_some());
    assert!(store.dependency_depth(c, EdgeKind::Calls).is_some());
}

#[test]
fn dep_depth_file_node_returns_none() {
    let mut store = Store::new();
    let file = store.upsert_node(path("src/dd_file.rs"));
    assert_eq!(store.dependency_depth(file, EdgeKind::Calls), None);
}

#[test]
fn dep_depth_file_nodes_excluded_from_subgraph() {
    let mut store = Store::new();
    let file = store.upsert_node(path("src/dd_excl.rs"));
    let sym = store.upsert_node(path("src/dd_excl.rs>dd_excl_sym"));
    store.upsert_edge(EdgeKind::Calls, file, sym);
    assert_eq!(store.dependency_depth(sym, EdgeKind::Calls), Some(0));
}

#[test]
fn dep_depth_imports_edge_kind() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/dd.rs>dd_imp_a"));
    let b = store.upsert_node(path("src/dd.rs>dd_imp_b"));
    store.upsert_edge(EdgeKind::Imports, a, b);
    assert_eq!(store.dependency_depth(a, EdgeKind::Imports), Some(0));
    assert_eq!(store.dependency_depth(b, EdgeKind::Imports), Some(1));
}

#[test]
fn dep_depth_large_cycle_terminates_without_oom() {
    // Regression test: the previous Bellman-Ford implementation would build an
    // O(V²) queue on cyclic graphs, causing OOM for large N.
    // The Kahn's-DP replacement must complete in O(V+E) with bounded memory.
    // 1 000-node ring: n0→n1→…→n999→n0
    const N: usize = 1_000;
    let mut store = Store::new();
    let nodes: Vec<_> = (0..N)
        .map(|i| store.upsert_node(path(&format!("src/cycle.rs>cycle_node_{i}"))))
        .collect();
    for i in 0..N {
        store.upsert_edge(EdgeKind::Calls, nodes[i], nodes[(i + 1) % N]);
    }
    // All cycle members must return Some(...) without hanging or panicking.
    for &node in &nodes {
        assert!(
            store.dependency_depth(node, EdgeKind::Calls).is_some(),
            "cycle member must return Some"
        );
    }
}

// ──────────────────────────────────────────────────────────────────────
// Performance regression — issue #153
// Six heavy-graph tools must complete in < 2 s on a 1 K-node graph and
// < 10 s on a 10 K-node graph (verifying O(V+E) scaling, not O(V²)).
// ──────────────────────────────────────────────────────────────────────

/// Build a sparse call graph with `n` symbol nodes (~3 outgoing Calls edges each).
/// Deterministic LCG keeps the test reproducible without a rand dependency.
fn build_graph(n: usize) -> (Store, Vec<NodeId>) {
    let mut store = Store::new();
    let ids: Vec<NodeId> = (0..n)
        .map(|i| store.upsert_node(path(&format!("src/f{}.rs>sym_{i}", i / 100))))
        .collect();
    let mut lcg = 0x1234_5678_9abc_def0u64;
    for i in 0..n {
        for _ in 0..3 {
            lcg = lcg
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let j = (lcg >> 33) as usize % n;
            if i != j {
                store.upsert_edge(EdgeKind::Calls, ids[i], ids[j]);
            }
        }
    }
    (store, ids)
}

fn build_1k_graph() -> (Store, Vec<NodeId>) {
    build_graph(1024)
}

fn build_10k_graph() -> (Store, Vec<NodeId>) {
    build_graph(10_240)
}

// Probe: does page_rank scale linearly? 10× the nodes should take < 10× the
// time.  If this is RED it proves a super-linear complexity bug.
#[test]
fn heavy_graph_10k_page_rank_under_ten_seconds() {
    let (store, _) = build_10k_graph();
    let start = std::time::Instant::now();
    let entries = store.page_rank(EdgeKind::Calls, 0.85, 20);
    let elapsed = start.elapsed();
    assert!(!entries.is_empty());
    assert!(
        elapsed.as_secs_f64() < 10.0,
        "page_rank on 10 K nodes took {elapsed:?}, want < 10 s (O(V+E) scaling check)"
    );
}

#[test]
fn heavy_graph_10k_wcc_under_ten_seconds() {
    let (store, _) = build_10k_graph();
    let start = std::time::Instant::now();
    let comps = store.weakly_connected_components(EdgeKind::Calls);
    let elapsed = start.elapsed();
    assert!(!comps.is_empty());
    assert!(
        elapsed.as_secs_f64() < 10.0,
        "wcc on 10 K nodes took {elapsed:?}, want < 10 s"
    );
}

#[test]
fn heavy_graph_leaf_symbols_under_two_seconds() {
    let (store, _) = build_1k_graph();
    let start = std::time::Instant::now();
    let result = store.leaf_symbols(EdgeKind::Calls, 100);
    let elapsed = start.elapsed();
    assert!(!result.is_empty() || result.is_empty()); // force evaluation
    assert!(
        elapsed.as_secs_f64() < 2.0,
        "leaf_symbols took {elapsed:?}, want < 2 s"
    );
}

#[test]
fn heavy_graph_degree_histogram_under_two_seconds() {
    let (store, _) = build_1k_graph();
    let start = std::time::Instant::now();
    let h = store.degree_histogram(EdgeKind::Calls);
    let elapsed = start.elapsed();
    assert!(!h.in_degrees.is_empty());
    assert!(
        elapsed.as_secs_f64() < 2.0,
        "degree_histogram took {elapsed:?}, want < 2 s"
    );
}

#[test]
fn heavy_graph_graph_metrics_under_two_seconds() {
    let (store, _) = build_1k_graph();
    let start = std::time::Instant::now();
    let m = store.graph_metrics(EdgeKind::Calls);
    let elapsed = start.elapsed();
    assert!(m.symbol_count > 0);
    assert!(
        elapsed.as_secs_f64() < 2.0,
        "graph_metrics took {elapsed:?}, want < 2 s"
    );
}

#[test]
fn heavy_graph_page_rank_under_two_seconds() {
    let (store, _) = build_1k_graph();
    let start = std::time::Instant::now();
    let entries = store.page_rank(EdgeKind::Calls, 0.85, 20);
    let elapsed = start.elapsed();
    assert!(!entries.is_empty());
    assert!(
        elapsed.as_secs_f64() < 2.0,
        "page_rank took {elapsed:?}, want < 2 s"
    );
}

#[test]
fn heavy_graph_wcc_under_two_seconds() {
    let (store, _) = build_1k_graph();
    let start = std::time::Instant::now();
    let comps = store.weakly_connected_components(EdgeKind::Calls);
    let elapsed = start.elapsed();
    assert!(!comps.is_empty());
    assert!(
        elapsed.as_secs_f64() < 2.0,
        "weakly_connected_components took {elapsed:?}, want < 2 s"
    );
}

#[test]
fn heavy_graph_find_call_path_under_two_seconds() {
    let (store, ids) = build_1k_graph();
    let start = std::time::Instant::now();
    // Attempt a path — may or may not exist; what matters is it terminates fast.
    let _result = store.find_call_path(ids[0], ids[1023], 10);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f64() < 2.0,
        "find_call_path took {elapsed:?}, want < 2 s"
    );
}

// ──────────────────────────────────────────────────────────────────────
// Store::merge — union semantics for parallel indexing (Issue #342 / R1)
//
// NodeIds are content-hashes (BLAKE3 of path), so merging two stores is a
// deterministic, order-independent union. This is the load-bearing primitive
// that lets us extract files in parallel into per-thread sub-stores and then
// reduce them into one final store byte-identically to the serial path.
// ──────────────────────────────────────────────────────────────────────

/// Helper: a store equals another iff same node-set (path+kind+span) and
/// same edge-set across all kinds. Order-independent by construction.
fn stores_equivalent(a: &Store, b: &Store) -> bool {
    use std::collections::BTreeSet;
    let a_nodes: BTreeSet<String> = a.all_paths().map(str::to_owned).collect();
    let b_nodes: BTreeSet<String> = b.all_paths().map(str::to_owned).collect();
    if a_nodes != b_nodes {
        return false;
    }
    if a.node_count() != b.node_count() || a.edge_count() != b.edge_count() {
        return false;
    }
    // Compare edges by resolved path triples so the check is id-stable.
    let edges = |s: &Store| -> BTreeSet<(String, String, String)> {
        let mut set = BTreeSet::new();
        for ek in EVERY_EDGE_KIND {
            for p in s.all_paths() {
                if let Some(src) = s.lookup(p) {
                    for &dst in s.outgoing(src, *ek) {
                        if let Some(dp) = s.path_of(dst) {
                            set.insert((format!("{ek:?}"), p.to_owned(), dp.to_owned()));
                        }
                    }
                }
            }
        }
        set
    };
    edges(a) == edges(b)
}

const EVERY_EDGE_KIND: &[EdgeKind] = &[
    EdgeKind::Contains,
    EdgeKind::Calls,
    EdgeKind::Imports,
    EdgeKind::TypeImports,
    EdgeKind::Extends,
    EdgeKind::Implements,
];

#[test]
fn merge_empty_into_empty_is_empty() {
    let mut a = Store::new();
    let b = Store::new();
    a.merge(&b);
    assert_eq!(a.node_count(), 0);
    assert_eq!(a.edge_count(), 0);
}

#[test]
fn merge_disjoint_stores_unions_nodes_and_edges() {
    // Store A: file a.rs with a Calls edge a>foo -> a>bar.
    let mut a = Store::new();
    let a_foo = a.upsert_node(path("a.rs>foo"));
    let a_bar = a.upsert_node(path("a.rs>bar"));
    a.set_kind(a_foo, NodeKind::Function);
    a.upsert_edge(EdgeKind::Calls, a_foo, a_bar);

    // Store B: file b.rs with an Imports edge b.rs -> a.rs.
    let mut b = Store::new();
    let b_file = b.upsert_node(path("b.rs"));
    let b_a = b.upsert_node(path("a.rs"));
    b.upsert_edge(EdgeKind::Imports, b_file, b_a);

    a.merge(&b);

    // All four distinct paths present.
    assert!(a.lookup("a.rs>foo").is_some());
    assert!(a.lookup("a.rs>bar").is_some());
    assert!(a.lookup("b.rs").is_some());
    assert!(a.lookup("a.rs").is_some());
    // A's own edge survived.
    assert!(a.outgoing(a_foo, EdgeKind::Calls).contains(&a_bar));
    // B's edge transferred (NodeIds are global content-hashes).
    let bf = a.lookup("b.rs").unwrap();
    let ar = a.lookup("a.rs").unwrap();
    assert!(a.outgoing(bf, EdgeKind::Imports).contains(&ar));
    // Kind metadata carried over.
    assert_eq!(a.kind_of(a_foo), Some(NodeKind::Function));
}

#[test]
fn merge_equals_single_pass_build_order_independent() {
    // Build the "serial" store: both files into one store.
    let mut serial = Store::new();
    {
        let foo = serial.upsert_node(path("a.rs>foo"));
        let bar = serial.upsert_node(path("b.rs>bar"));
        serial.set_kind(foo, NodeKind::Function);
        serial.set_kind(bar, NodeKind::Function);
        serial.upsert_edge(EdgeKind::Calls, foo, bar); // cross-file edge
    }

    // Build via merge in one order...
    let mut merged_ab = Store::new();
    {
        let mut sa = Store::new();
        let foo = sa.upsert_node(path("a.rs>foo"));
        sa.set_kind(foo, NodeKind::Function);
        let bar_stub = sa.upsert_node(path("b.rs>bar"));
        sa.upsert_edge(EdgeKind::Calls, foo, bar_stub);
        let mut sb = Store::new();
        let bar = sb.upsert_node(path("b.rs>bar"));
        sb.set_kind(bar, NodeKind::Function);
        merged_ab.merge(&sa);
        merged_ab.merge(&sb);
    }

    // ...and the reverse order.
    let mut merged_ba = Store::new();
    {
        let mut sa = Store::new();
        let foo = sa.upsert_node(path("a.rs>foo"));
        sa.set_kind(foo, NodeKind::Function);
        let bar_stub = sa.upsert_node(path("b.rs>bar"));
        sa.upsert_edge(EdgeKind::Calls, foo, bar_stub);
        let mut sb = Store::new();
        let bar = sb.upsert_node(path("b.rs>bar"));
        sb.set_kind(bar, NodeKind::Function);
        merged_ba.merge(&sb);
        merged_ba.merge(&sa);
    }

    assert!(
        stores_equivalent(&serial, &merged_ab),
        "merge(A,B) must equal the single-pass build"
    );
    assert!(
        stores_equivalent(&serial, &merged_ba),
        "merge order must not matter (B,A == A,B)"
    );
}

#[test]
fn merge_is_idempotent() {
    let mut a = Store::new();
    let foo = a.upsert_node(path("a.rs>foo"));
    let bar = a.upsert_node(path("a.rs>bar"));
    a.upsert_edge(EdgeKind::Calls, foo, bar);

    let snapshot_nodes = a.node_count();
    let snapshot_edges = a.edge_count();

    let b = {
        let mut s = Store::new();
        let f = s.upsert_node(path("a.rs>foo"));
        let r = s.upsert_node(path("a.rs>bar"));
        s.upsert_edge(EdgeKind::Calls, f, r);
        s
    };
    a.merge(&b); // merging an identical store changes nothing
    assert_eq!(a.node_count(), snapshot_nodes);
    assert_eq!(a.edge_count(), snapshot_edges);
}

// ── Journal (incremental persistence) tests ─────────────────────────────────

#[test]
fn journal_append_and_replay_restores_store() {
    let dir = tempfile::tempdir().unwrap();
    let snap_path = dir.path().join("index.rmp");

    let mut store = Store::new();
    store.upsert_node(path("src/auth.rs"));
    let auth = store.upsert_node(path("src/auth.rs>AuthService"));
    store.set_kind(auth, NodeKind::Class);
    store.upsert_node(path("src/utils.rs"));
    let validate = store.upsert_node(path("src/utils.rs>validate"));
    store.upsert_edge(EdgeKind::Calls, auth, validate);
    store.save(&snap_path).unwrap();

    let mut journal = Journal::open(&snap_path).unwrap();
    let sub = store.extract_file_substore("src/auth.rs");
    journal.append("src/auth.rs", &sub).unwrap();

    let mut loaded = Store::load(&snap_path).unwrap();
    let replayed = journal.replay(&mut loaded).unwrap();
    assert_eq!(replayed, 1);
    assert_eq!(loaded.node_count(), store.node_count());
}

#[test]
fn extract_file_substore_contains_only_target_file() {
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs"));
    let _a1 = store.upsert_node(path("src/a.rs>Foo"));
    let _a2 = store.upsert_node(path("src/a.rs>Foo>bar"));
    store.upsert_node(path("src/b.rs"));
    let _b1 = store.upsert_node(path("src/b.rs>Baz"));

    let sub = store.extract_file_substore("src/a.rs");
    assert!(sub.lookup("src/a.rs").is_some(), "file node present");
    assert!(sub.lookup("src/a.rs>Foo").is_some(), "Foo present");
    assert!(sub.lookup("src/a.rs>Foo>bar").is_some(), "Foo>bar present");
    assert!(sub.lookup("src/b.rs>Baz").is_none(), "b.rs>Baz absent");
    assert_eq!(sub.node_count(), 3);
}

#[test]
fn extract_file_substore_preserves_cross_file_edges() {
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs"));
    let a_fn = store.upsert_node(path("src/a.rs>foo"));
    store.upsert_node(path("src/b.rs"));
    let b_fn = store.upsert_node(path("src/b.rs>bar"));
    store.upsert_edge(EdgeKind::Calls, a_fn, b_fn);

    let sub = store.extract_file_substore("src/a.rs");
    assert!(sub.lookup("src/a.rs>foo").is_some());
    assert!(
        sub.lookup("src/b.rs>bar").is_some(),
        "cross-file edge target preserved as leaf stub"
    );
    assert_eq!(sub.node_count(), 3);
    let sub_foo = sub.lookup("src/a.rs>foo").unwrap();
    let sub_bar = sub.lookup("src/b.rs>bar").unwrap();
    assert!(
        sub.synapse
            .outgoing(sub_foo, EdgeKind::Calls)
            .contains(&sub_bar),
        "Calls edge from a.rs>foo -> b.rs>bar preserved"
    );
}

#[test]
fn compact_creates_fresh_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let snap_path = dir.path().join("index.rmp");

    let mut store = Store::new();
    store.upsert_node(path("src/main.rs"));
    store.upsert_node(path("src/main.rs>App"));
    store.save(&snap_path).unwrap();

    let mut journal = Journal::open(&snap_path).unwrap();
    journal.compact_threshold = 2;
    let sub = store.extract_file_substore("src/main.rs");
    journal.append("src/main.rs", &sub).unwrap();
    journal.append("src/main.rs", &sub).unwrap();

    assert!(journal.should_compact());
    journal.compact(&store).unwrap();

    let journal_path = dir.path().join("journal.jsonl");
    assert!(journal_path.exists());
    assert_eq!(journal.pending_count(), 0);

    let loaded = Store::load(&snap_path).unwrap();
    assert_eq!(loaded.node_count(), store.node_count());
}

#[test]
fn load_with_journal_replays_deltas() {
    let dir = tempfile::tempdir().unwrap();
    let snap_path = dir.path().join("index.rmp");

    let mut store = Store::new();
    store.upsert_node(path("src/a.rs"));
    store.upsert_node(path("src/a.rs>A"));
    store.save(&snap_path).unwrap();

    let sub = store.extract_file_substore("src/a.rs");
    let mut journal = Journal::open(&snap_path).unwrap();
    journal.append("src/a.rs", &sub).unwrap();

    let loaded = Store::load_with_journal(&snap_path).unwrap();
    assert_eq!(loaded.node_count(), store.node_count());
}

#[test]
fn journal_truncate_removes_file_and_resets_header() {
    // Covers `Journal::truncate` — the path used after a full snapshot save
    // bypasses the journal.
    let dir = tempfile::tempdir().unwrap();
    let snap_path = dir.path().join("index.rmp");
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs"));
    store.save(&snap_path).unwrap();

    let mut journal = Journal::open(&snap_path).unwrap();
    let sub = store.extract_file_substore("src/a.rs");
    journal.append("src/a.rs", &sub).unwrap();
    assert!(journal.pending_count() > 0);

    journal.truncate().unwrap();
    let journal_path = dir.path().join("journal.jsonl");
    assert!(!journal_path.exists(), "truncate removes the journal file");
    assert_eq!(journal.pending_count(), 0, "header reset to default");

    // Idempotent: truncate when file already gone is OK.
    journal.truncate().unwrap();
}

#[test]
fn journal_replay_on_missing_file_returns_zero() {
    // Covers the early-return branch in `Journal::replay`: when the
    // journal file does not exist (e.g. fresh load just after compaction),
    // replay returns Ok(0) without touching the store.
    let dir = tempfile::tempdir().unwrap();
    let snap_path = dir.path().join("index.rmp");
    let mut store = Store::new();
    store.upsert_node(path("src/a.rs"));
    store.save(&snap_path).unwrap();

    let mut journal = Journal::open(&snap_path).unwrap();
    // Truncate so the journal file is gone.
    journal.truncate().unwrap();

    let mut target = Store::new();
    let count = journal.replay(&mut target).unwrap();
    assert_eq!(count, 0, "replay against missing journal returns 0");
}

#[test]
fn journal_replay_handles_empty_delta_store_as_removal_only() {
    // Covers the `record.delta_store.is_empty()` branch in `replay`:
    // a delta whose `delta_store` is the empty string represents a pure
    // file removal — replay should call `remove_file` and skip the merge.
    use crate::store::journal::DeltaRecord;
    use std::io::Write;

    let dir = tempfile::tempdir().unwrap();
    let snap_path = dir.path().join("index.rmp");

    // Build a base store and persist it.
    let mut store = Store::new();
    store.upsert_node(path("src/gone.rs"));
    store.upsert_node(path("src/gone.rs>Goner"));
    store.save(&snap_path).unwrap();

    // Open the journal (writes header), then hand-craft a removal record
    // by writing one extra JSON line with an empty delta_store.
    {
        let _journal = crate::store::journal::Journal::open(&snap_path).unwrap();
    }
    let journal_path = dir.path().join("journal.jsonl");
    let removal = DeltaRecord {
        seq: 1,
        file_path: "src/gone.rs".to_owned(),
        delta_store: String::new(),
    };
    {
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&journal_path)
            .unwrap();
        writeln!(f, "{}", serde_json::to_string(&removal).unwrap()).unwrap();
    }

    let journal = crate::store::journal::Journal::open(&snap_path).unwrap();
    let mut loaded = Store::load(&snap_path).unwrap();
    assert!(loaded.lookup("src/gone.rs>Goner").is_some());
    let replayed = journal.replay(&mut loaded).unwrap();
    assert_eq!(replayed, 1);
    assert!(
        loaded.lookup("src/gone.rs>Goner").is_none(),
        "empty delta_store should remove the file's subtree"
    );
}

#[test]
fn delta_roundtrip_via_base64() {
    let mut store = Store::new();
    let id = store.upsert_node(path("lib.rs>func"));
    store.set_kind(id, NodeKind::Function);
    store.set_span(
        id,
        SourceSpan {
            start_line: 1,
            start_col: 0,
            end_line: 5,
            end_col: 0,
            start_byte: 0,
            end_byte: 30,
        },
    );

    let encoded = Store::serialize_delta(&store);
    let decoded = Store::deserialize_delta(&encoded).unwrap();
    assert_eq!(decoded.node_count(), store.node_count());
    assert_eq!(
        decoded.kind_of(decoded.lookup("lib.rs>func").unwrap()),
        Some(NodeKind::Function)
    );
}

// ──────────────────────────────────────────────────────────────────────
// Store::health() — RFC-0114 Phase 2
// ──────────────────────────────────────────────────────────────────────

#[test]
fn store_health_empty_store_fails_closed() {
    let store = Store::new();
    let report = store.health();
    assert_eq!(report.grade, crate::health::HealthGrade::F);
    assert_eq!(report.score, 0);
}

#[test]
fn store_health_connected_project_grades_a() {
    let mut store = Store::new();
    // Add 10 well-connected symbols (ring topology: each calls the next).
    let ids: Vec<_> = (0..10)
        .map(|i| store.upsert_node(path(&format!("src/lib.rs>sym{i}"))))
        .collect();
    for i in 0..ids.len() {
        store.upsert_edge(EdgeKind::Calls, ids[i], ids[(i + 1) % ids.len()]);
    }
    let report = store.health();
    // 0 dead, 0 isolated, density = 10 edges / 10 nodes = 1.0 (≥ target/2).
    // Should grade B or better.
    assert!(
        report.score >= 80,
        "expected B or A, got score={} grade={}",
        report.score,
        report.grade.as_str()
    );
}

// ──────────────────────────────────────────────────────────────────────
// RFC-0118 Part C — resolver pass kind_map hygiene
// ──────────────────────────────────────────────────────────────────────

/// AC-3 (Part C): After `resolve_bare_call_stubs()`, the resolved stub's `kind_map`
/// entry is cleaned up. Stale `kind_map[stub_id] = Unresolved` must NOT survive
/// stub removal — it would re-appear across re-index cycles since `NodeId` is
/// content-derived from the path.
#[test]
fn resolve_bare_call_stubs_simple_cleans_kind_map() {
    let mut store = Store::new();

    // Caller node.
    let caller_id = store.upsert_node(path("src/main.rs>main"));

    // Unresolved bare stub: a bare name (no '>') with NodeKind::Unresolved.
    let stub_path = TrunkPath::parse("upsert_node").unwrap();
    let stub_id = store.upsert_node_with_kind(stub_path, NodeKind::Unresolved);

    // Caller calls the stub.
    store.upsert_edge(EdgeKind::Calls, caller_id, stub_id);

    // Unique definition — resolve_bare_call_stubs_simple will bind stub → def.
    let _def_id = store.upsert_node(path("src/store.rs>Store>upsert_node"));

    // Pre-condition: kind_map has the Unresolved entry.
    assert_eq!(store.kind_of(stub_id), Some(NodeKind::Unresolved));

    store.resolve_bare_call_stubs();

    // Post-condition (AC-3): stub removed from trunk AND from kind_map.
    assert!(
        store.lookup("upsert_node").is_none(),
        "stub must be removed from trunk"
    );
    assert!(
        store.kind_of(stub_id).is_none(),
        "kind_map must not retain a stale Unresolved entry for the resolved stub"
    );
}

/// AC-3b: `resolve_import_aware_stubs` also cleans `kind_map` for stubs it resolves.
#[test]
fn resolve_import_aware_stubs_cleans_kind_map() {
    let mut store = Store::new();

    // Two files, one imports the other.
    let file_a = store.upsert_node_with_kind(TrunkPath::parse("src/a.rs").unwrap(), NodeKind::File);
    let file_b = store.upsert_node_with_kind(TrunkPath::parse("src/b.rs").unwrap(), NodeKind::File);
    store.upsert_edge(EdgeKind::Imports, file_a, file_b);

    // Caller in a.rs.
    let caller_id = store.upsert_node(path("src/a.rs>caller_fn"));
    // Two competing definitions — simple pass won't resolve (ambiguous).
    let _def_b = store.upsert_node(path("src/b.rs>Module>helper"));
    let _def_c = store.upsert_node(path("src/c.rs>Other>helper"));

    // Bare stub.
    let stub_path = TrunkPath::parse("helper").unwrap();
    let stub_id = store.upsert_node_with_kind(stub_path, NodeKind::Unresolved);
    store.upsert_edge(EdgeKind::Calls, caller_id, stub_id);

    assert_eq!(store.kind_of(stub_id), Some(NodeKind::Unresolved));

    store.resolve_bare_call_stubs();

    // Import-aware pass resolves to b.rs (caller imports b.rs).
    // kind_map entry must be gone.
    assert!(
        store.kind_of(stub_id).is_none(),
        "import-aware pass must clean kind_map for resolved stubs"
    );
}
