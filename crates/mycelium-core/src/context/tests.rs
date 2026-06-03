//! Tests for the shared context builder — written RED-first (Charter §5.1).

use super::{
    ContextOptions, Routing, build_payload, extract_symbol_candidates, looks_like_hyphae,
    seed_entry_points,
};
use crate::store::Store;
use crate::trunk::TrunkPath;
use crate::types::EdgeKind;

fn path(s: &str) -> TrunkPath {
    TrunkPath::parse(s).unwrap()
}

fn opts(edge_kinds: Vec<EdgeKind>) -> ContextOptions {
    ContextOptions {
        max_nodes: 30,
        max_code_blocks: 6,
        edge_kinds,
    }
}

#[test]
fn extract_drops_stop_words_and_keeps_symbols() {
    let cands = extract_symbol_candidates("trace `handle_request` to get_user");
    assert!(
        cands.contains(&"handle_request".to_owned()),
        "got: {cands:?}"
    );
    assert!(cands.contains(&"get_user".to_owned()), "got: {cands:?}");
    assert!(
        !cands.contains(&"to".to_owned()),
        "stop word leaked: {cands:?}"
    );
    assert!(
        !cands.contains(&"trace".to_owned()),
        "stop word leaked: {cands:?}"
    );
}

#[test]
fn extract_is_deterministic_and_deduped() {
    let cands = extract_symbol_candidates("AuthService AuthService login");
    assert_eq!(cands.iter().filter(|c| *c == "AuthService").count(), 1);
}

#[test]
fn not_found_payload_has_all_seven_keys() {
    let store = Store::new();
    let value = build_payload(&store, "nothing", &[], &[], Routing::Natural, &opts(vec![]));
    assert_eq!(value["verdict"], "NOT_FOUND");
    for key in [
        "entry_points",
        "nodes",
        "edges",
        "code_blocks",
        "related_files",
        "stats",
        "agent_summary",
    ] {
        assert!(value.get(key).is_some(), "missing key: {key}");
    }
    assert_eq!(value["routing"], "natural");
}

#[test]
fn payload_includes_distinct_related_files() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/auth.rs>AuthService>login"));
    let b = store.upsert_node(path("src/db.rs>Db>query"));
    let c = store.upsert_node(path("src/auth.rs>AuthService>logout"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Calls, a, c);

    let eps = vec!["src/auth.rs>AuthService>login".to_owned()];
    let value = build_payload(&store, "login", &[], &eps, Routing::Natural, &opts(vec![]));

    let related = value["related_files"]
        .as_array()
        .expect("related_files array");
    let files: Vec<&str> = related.iter().filter_map(|v| v.as_str()).collect();
    assert!(files.contains(&"src/auth.rs"), "related_files: {files:?}");
    assert!(files.contains(&"src/db.rs"), "related_files: {files:?}");
    // distinct — src/auth.rs appears once even though two nodes live there
    assert_eq!(files.iter().filter(|f| **f == "src/auth.rs").count(), 1);
    assert_eq!(value["stats"]["related_files"], related.len());
}

#[test]
fn edge_kinds_option_controls_expansion() {
    let mut store = Store::new();
    let a = store.upsert_node(path("src/a.rs>A>run"));
    let b = store.upsert_node(path("src/b.rs>B>helper"));
    let m = store.upsert_node(path("src/a.rs>A>imported_mod"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    store.upsert_edge(EdgeKind::Imports, a, m);

    let eps = vec!["src/a.rs>A>run".to_owned()];

    // Default (empty ⇒ Calls only): the Imports target must NOT appear as an edge.
    let calls_only = build_payload(&store, "run", &[], &eps, Routing::Natural, &opts(vec![]));
    let edge_kinds: Vec<&str> = calls_only["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["kind"].as_str())
        .collect();
    assert!(edge_kinds.contains(&"calls"), "edges: {edge_kinds:?}");
    assert!(
        !edge_kinds.contains(&"imports"),
        "imports leaked: {edge_kinds:?}"
    );

    // Explicitly request Imports: an imports-kind edge must now appear.
    let with_imports = build_payload(
        &store,
        "run",
        &[],
        &eps,
        Routing::Natural,
        &opts(vec![EdgeKind::Imports]),
    );
    let edge_kinds2: Vec<&str> = with_imports["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["kind"].as_str())
        .collect();
    assert!(edge_kinds2.contains(&"imports"), "edges: {edge_kinds2:?}");
}

#[test]
fn hyphae_detection_distinguishes_selectors_from_prose() {
    assert!(looks_like_hyphae("function:calls(#AuthService)"));
    assert!(looks_like_hyphae("#AuthService"));
    assert!(looks_like_hyphae("[lang=rust]"));
    assert!(!looks_like_hyphae("how does authentication work"));
    assert!(!looks_like_hyphae("trace ServeHTTP to HandlerFunc"));
}

#[test]
fn seed_entry_points_finds_indexed_symbol() {
    let mut store = Store::new();
    store.upsert_node(path("src/auth.rs>AuthService>login"));
    let cands = extract_symbol_candidates("how does login work");
    let eps = seed_entry_points(&store, &cands, 30);
    assert!(
        eps.iter().any(|p| p.ends_with(">login")),
        "expected a login entry point, got: {eps:?}"
    );
}
