use super::ranking::{RankOpts, ScoredCandidate, TestKind, classify_test_path, rank_entry_points};

fn sc(path: &str, exact_match: bool, importance: f64, order: usize) -> ScoredCandidate {
    ScoredCandidate {
        path: path.to_owned(),
        exact_match,
        importance,
        order,
    }
}

// AC-1: test helper dropped even with highest in-degree (demotion beats importance)
#[test]
fn rank_demotes_test_helper_below_real_subsystem() {
    let candidates = vec![
        sc(
            "crates/mycelium-core/tests.rs>prepare_indexed_project",
            false,
            30.0,
            0,
        ),
        sc("crates/mycelium-core/index.rs>index", false, 1.0, 1),
        sc("crates/mycelium-core/extractor.rs>Extractor", false, 9.0, 2),
    ];
    let result = rank_entry_points(
        &candidates,
        RankOpts {
            max_nodes: 30,
            exclude_tests: true,
        },
    );
    assert!(
        !result.contains(&"crates/mycelium-core/tests.rs>prepare_indexed_project".to_owned()),
        "test helper must be dropped even with highest importance"
    );
    assert!(result.contains(&"crates/mycelium-core/index.rs>index".to_owned()));
    assert!(result.contains(&"crates/mycelium-core/extractor.rs>Extractor".to_owned()));
}

// AC-2: file-level test signals
#[test]
fn classify_detects_test_files() {
    assert_eq!(
        classify_test_path("crates/foo/tests/bar.rs>helper"),
        TestKind::TestFile,
        "tests/ dir"
    );
    assert_eq!(
        classify_test_path("src/foo_test.go>TestX"),
        TestKind::TestFile,
        "*_test.go"
    );
    assert_eq!(
        classify_test_path("src/a.test.ts>thing"),
        TestKind::TestFile,
        "*.test.ts"
    );
    assert_eq!(
        classify_test_path("src/a.spec.js>thing"),
        TestKind::TestFile,
        "*.spec.js"
    );
    assert_eq!(
        classify_test_path("pkg/test_utils.py>setup"),
        TestKind::TestFile,
        "test_*.py"
    );
    assert_eq!(
        classify_test_path("src/__tests__/x.jsx>f"),
        TestKind::TestFile,
        "__tests__/ dir"
    );
    assert_eq!(
        classify_test_path("conftest.py>fixture"),
        TestKind::TestFile,
        "conftest.py"
    );
}

// AC-2a: bare tests.rs (Mycelium's dominant convention)
#[test]
fn classify_detects_bare_tests_rs() {
    assert_eq!(
        classify_test_path("crates/mycelium-core/src/context/tests.rs>helper"),
        TestKind::TestFile
    );
    assert_eq!(
        classify_test_path("crates/mycelium-core/src/store/tests.rs>x"),
        TestKind::TestFile
    );
}

// AC-2b: test_gap.rs is a real production module — must NOT be demoted
#[test]
fn classify_real_test_prefix_module_is_not_test() {
    assert_eq!(
        classify_test_path("crates/mycelium-core/src/test_gap.rs>rank"),
        TestKind::None,
        "test_*.rs is Rust convention for test modules — only test_*.py is demoted"
    );
}

// AC-3: symbol-level test signals
#[test]
fn classify_detects_test_symbols_not_files() {
    assert_eq!(
        classify_test_path("src/lib.rs>test_parses_input"),
        TestKind::TestSymbol,
        "test_ prefix"
    );
    assert_eq!(
        classify_test_path("src/svc.go>testHelper"),
        TestKind::TestSymbol,
        "testCamel"
    );
    assert_eq!(classify_test_path("src/index.rs>index"), TestKind::None);
    assert_eq!(
        classify_test_path("src/extractor.rs>Extractor"),
        TestKind::None
    );
    assert_eq!(
        classify_test_path("src/attestation.rs>Attest"),
        TestKind::None,
        "Attest does not start with test"
    );
}

// AC-3a: camelCase word-boundary: only test+Uppercase triggers TestSymbol
#[test]
fn classify_camelcase_boundary() {
    assert_eq!(
        classify_test_path("src/cfg.rs>testbed"),
        TestKind::None,
        "lowercase 'b' after test"
    );
    assert_eq!(
        classify_test_path("src/x.rs>testimony"),
        TestKind::None,
        "lowercase 'i' after test"
    );
    assert_eq!(
        classify_test_path("src/c.ts>testableConfig"),
        TestKind::None,
        "lowercase 'a' after test"
    );
}

// AC-4: importance desc, then order asc as tiebreak
#[test]
fn rank_orders_by_importance_desc_then_order() {
    let candidates = vec![
        sc("src/a.rs>a", false, 3.0, 0),
        sc("src/b.rs>b", false, 9.0, 1),
        sc("src/c.rs>c", false, 9.0, 2),
    ];
    let result = rank_entry_points(
        &candidates,
        RankOpts {
            max_nodes: 30,
            exclude_tests: false,
        },
    );
    assert_eq!(
        result,
        vec![
            "src/b.rs>b".to_owned(),
            "src/c.rs>c".to_owned(),
            "src/a.rs>a".to_owned()
        ]
    );
}

// AC-4a: exact match beats fuzzy with high importance (RFC-0101 §3)
#[test]
fn rank_exact_match_beats_fuzzy_high_importance() {
    let candidates = vec![
        sc("src/build.rs>build", true, 0.0, 0),
        sc("src/core.rs>build_index", false, 12.0, 1),
    ];
    let result = rank_entry_points(
        &candidates,
        RankOpts {
            max_nodes: 30,
            exclude_tests: false,
        },
    );
    assert_eq!(
        result[0], "src/build.rs>build",
        "exact match must rank first"
    );
    assert_eq!(result[1], "src/core.rs>build_index");
}

// AC-5: all-test corpus must never return empty
#[test]
fn all_test_corpus_never_empty() {
    let candidates = vec![
        sc("crates/x/tests.rs>helper_a", false, 5.0, 0),
        sc("crates/y/tests.rs>helper_b", false, 2.0, 1),
    ];
    let result = rank_entry_points(
        &candidates,
        RankOpts {
            max_nodes: 30,
            exclude_tests: true,
        },
    );
    assert!(
        !result.is_empty(),
        "all-test corpus must fall back to importance-ranked tests"
    );
}

// AC-5a: all-test corpus still caps to max_nodes
#[test]
fn all_test_corpus_caps_to_max_nodes() {
    // Use pre-typed f64 values to avoid usize→f64 precision-loss cast.
    let candidates: Vec<ScoredCandidate> = [0.0_f64, 1., 2., 3., 4., 5., 6., 7., 8., 9.]
        .into_iter()
        .enumerate()
        .map(|(i, imp)| sc(&format!("crates/tests.rs>helper_{i}"), false, imp, i))
        .collect();
    let result = rank_entry_points(
        &candidates,
        RankOpts {
            max_nodes: 3,
            exclude_tests: true,
        },
    );
    assert!(!result.is_empty());
    assert!(result.len() <= 3);
}

// AC-6: with exclude_tests=false, tests are demoted (appended) not dropped
#[test]
fn retain_tests_when_not_excluding() {
    let candidates = vec![
        sc("crates/tests.rs>helper", false, 10.0, 0),
        sc("src/real.rs>real_fn", false, 1.0, 1),
    ];
    let result = rank_entry_points(
        &candidates,
        RankOpts {
            max_nodes: 30,
            exclude_tests: false,
        },
    );
    let real_pos = result.iter().position(|p| p.contains("real_fn")).unwrap();
    let test_pos = result.iter().position(|p| p.contains("helper")).unwrap();
    assert!(
        real_pos < test_pos,
        "non-test must precede test even when not excluding"
    );
}

// AC-7: dedup by path + cap to max_nodes
#[test]
fn rank_dedups_and_caps() {
    let candidates = vec![
        sc("src/a.rs>foo", false, 2.0, 0),
        sc("src/a.rs>foo", false, 1.0, 1), // duplicate path
        sc("src/b.rs>bar", false, 1.0, 2),
        sc("src/c.rs>baz", false, 1.0, 3),
        sc("src/d.rs>qux", false, 1.0, 4),
    ];
    let result = rank_entry_points(
        &candidates,
        RankOpts {
            max_nodes: 2,
            exclude_tests: false,
        },
    );
    assert!(result.len() <= 2, "result must be capped to max_nodes");
    let mut sorted = result.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(result.len(), sorted.len(), "no duplicate paths");
}

// AC-7 addendum: first-seen wins even when a later duplicate has higher importance (Codex P2 #623)
#[test]
fn rank_first_seen_wins_over_higher_importance_duplicate() {
    let candidates = vec![
        sc("src/a.rs>foo", false, 1.0, 0), // first-seen foo, importance=1
        sc("src/b.rs>bar", false, 5.0, 1),
        sc("src/a.rs>foo", false, 10.0, 2), // duplicate: higher importance, must NOT win
    ];
    let result = rank_entry_points(
        &candidates,
        RankOpts {
            max_nodes: 30,
            exclude_tests: false,
        },
    );
    assert_eq!(result.len(), 2, "duplicate must collapse to single entry");
    // first-seen foo survives with importance=1; bar(5) > foo(1) → [bar, foo]
    assert_eq!(
        result[0], "src/b.rs>bar",
        "bar (importance=5) above first-seen foo (importance=1)"
    );
    assert_eq!(
        result[1], "src/a.rs>foo",
        "first-seen (importance=1) not the duplicate (importance=10)"
    );
}

// AC-8: paths without '>' separator (bare file nodes)
#[test]
fn classify_handles_paths_without_separator() {
    assert_eq!(classify_test_path("src/index.rs"), TestKind::None);
    assert_eq!(
        classify_test_path("src/tests.rs"),
        TestKind::TestFile,
        "stem==tests with no symbol"
    );
}

// AC-9: bare stubs (no '/', no '.', no '>') classify None
#[test]
fn classify_bare_stub_is_none() {
    assert_eq!(classify_test_path("unwrap"), TestKind::None);
    assert_eq!(
        classify_test_path("std::collections::HashMap"),
        TestKind::None
    );
}
