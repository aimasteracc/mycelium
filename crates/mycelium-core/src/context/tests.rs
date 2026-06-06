//! Tests for the shared context builder — written RED-first (Charter §5.1).

use super::{
    ContextOptions, Routing, build_payload, extract_symbol_candidates, looks_like_hyphae,
    seed_entry_points,
};
use crate::store::Store;
use crate::trunk::TrunkPath;
use crate::types::{EdgeKind, NodeKind};

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
fn agent_summary_is_branded_mycelium_not_a_competitor() {
    // Regression: the agent-facing summary_line must not leak another product's
    // name. Both the empty (NOT_FOUND) branch and the populated branch are checked.
    let mut store = Store::new();

    // NOT_FOUND branch.
    let empty = build_payload(&store, "nothing", &[], &[], Routing::Natural, &opts(vec![]));
    let empty_summary = empty["agent_summary"]["summary_line"]
        .as_str()
        .expect("summary_line");
    assert!(
        empty_summary.starts_with("mycelium_context"),
        "empty summary leaked branding: {empty_summary}"
    );
    assert!(!empty_summary.to_lowercase().contains("codegraph"));

    // Populated branch.
    let a = store.upsert_node(path("src/auth.rs>AuthService>login"));
    let b = store.upsert_node(path("src/db.rs>Db>query"));
    store.upsert_edge(EdgeKind::Calls, a, b);
    let eps = vec!["src/auth.rs>AuthService>login".to_owned()];
    let full = build_payload(&store, "login", &[], &eps, Routing::Natural, &opts(vec![]));
    let full_summary = full["agent_summary"]["summary_line"]
        .as_str()
        .expect("summary_line");
    assert!(
        full_summary.starts_with("mycelium_context"),
        "populated summary leaked branding: {full_summary}"
    );
    assert!(!full_summary.to_lowercase().contains("codegraph"));
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

// RFC-0119 Phase 2 — AC-4b: dedup must merge exact_match via |= across candidates.
// A path first seen via a fuzzy candidate and later via an exact candidate must
// surface with exact_match=true, ranking it above a higher-alphabetical competitor.
#[test]
fn seed_dedup_merges_later_exact_match() {
    let mut store = Store::new();
    // "src/aaa.rs>build_main" — found by "build" (fuzzy), NOT by "build_index"
    store.upsert_node(path("src/aaa.rs>build_main"));
    // "src/bbb.rs>build_index" — found by "build" (fuzzy) AND "build_index" (exact)
    store.upsert_node(path("src/bbb.rs>build_index"));

    // Alphabetical order (old code): aaa < bbb → build_main first ❌
    // Merge-based ranking (new code): build_index gets exact_match=true → ranks first ✓
    let cands = vec!["build".to_owned(), "build_index".to_owned()];
    let eps = seed_entry_points(&store, &cands, 30);
    assert!(!eps.is_empty(), "no results returned");
    assert!(
        eps[0].contains("build_index"),
        "expected build_index (exact match) first, got: {eps:?}"
    );
}

// RFC-0119 Phase 2 — AC-10: real subsystem ranks above test fixture.
// A path in a tests/ directory must be demoted below a real implementation.
#[test]
fn context_indexing_query_ranks_subsystem_over_test_fixture() {
    let mut store = Store::new();
    // Real indexing subsystem: 1 caller
    let build_idx = store.upsert_node(path("src/index.rs>build_index"));
    let main_node = store.upsert_node(path("src/main.rs>main"));
    store.upsert_edge(EdgeKind::Calls, main_node, build_idx);

    // Test fixture in tests/ directory: 3 callers (but test code → demoted)
    let fixture = store.upsert_node(path("crates/foo/tests/helpers.rs>prepare_indexed_project"));
    let ta = store.upsert_node(path("crates/foo/tests/test_a.rs>test_a"));
    let tb = store.upsert_node(path("crates/foo/tests/test_b.rs>test_b"));
    let tc = store.upsert_node(path("crates/foo/tests/test_c.rs>test_c"));
    store.upsert_edge(EdgeKind::Calls, ta, fixture);
    store.upsert_edge(EdgeKind::Calls, tb, fixture);
    store.upsert_edge(EdgeKind::Calls, tc, fixture);

    // Alphabetical: "crates/..." < "src/..." → old code returns fixture first ❌
    // Test demotion: new code excludes tests/ fixture → only build_index returned ✓
    let cands = vec!["index".to_owned()];
    let eps = seed_entry_points(&store, &cands, 30);
    assert!(
        eps.iter().any(|p| p.ends_with(">build_index")),
        "build_index missing: {eps:?}"
    );
    assert!(
        !eps.iter().any(|p| p.contains("prepare_indexed_project")),
        "test fixture must not appear when non-test candidate exists: {eps:?}"
    );
}

// RFC-0119 Phase 2 — AC-11: stub callers do not inflate in-degree.
// When all of a node's callers are NodeKind::Unresolved, its importance is 0,
// so a node with even 1 real caller ranks above it.
#[test]
fn stub_callers_do_not_inflate_importance() {
    let mut store = Store::new();
    // "src/aaa.rs>util_fn" has 5 stub callers — alphabetically first
    let util_fn = store.upsert_node(path("src/aaa.rs>util_fn"));
    for i in 0..5u8 {
        let stub = store.upsert_node(path(&format!("std>stub{i}")));
        store.set_kind(stub, NodeKind::Unresolved);
        store.upsert_edge(EdgeKind::Calls, stub, util_fn);
    }
    // "src/zzz.rs>core_process" has 1 real caller — alphabetically last
    let core_proc = store.upsert_node(path("src/zzz.rs>core_process"));
    let real_caller = store.upsert_node(path("src/main.rs>main"));
    store.upsert_edge(EdgeKind::Calls, real_caller, core_proc);

    // Candidates ordered so old code (sequential append) returns util first:
    // "util" finds util_fn first, "process" finds core_process second → util first ❌
    // New code: real_in_degree(util_fn)=0, real_in_degree(core_proc)=1 → process first ✓
    let cands = vec!["util".to_owned(), "process".to_owned()];
    let eps = seed_entry_points(&store, &cands, 30);
    let idx_core = eps.iter().position(|p| p.ends_with(">core_process"));
    let idx_util = eps.iter().position(|p| p.ends_with(">util_fn"));
    assert!(idx_core.is_some(), "core_process not found: {eps:?}");
    assert!(idx_util.is_some(), "util_fn not found: {eps:?}");
    assert!(
        idx_core.unwrap() < idx_util.unwrap(),
        "core_process (1 real caller) must rank above util_fn (stub-only callers): {eps:?}"
    );
}
