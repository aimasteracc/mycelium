//! Real-project validation — Charter §5.10
//!
//! "Every feature is validated against real open-source projects."
//!
//! These tests clone or use cached snapshots of real open-source projects
//! and assert that the indexer produces correct, non-empty results.
//!
//! In CI, real projects are downloaded via `tests/e2e/fixtures/`.
//! Locally, the test is skipped if the fixture doesn't exist (use
//! `scripts/fetch-e2e-fixtures.sh` to download them).

use std::path::PathBuf;

use mycelium_core::{extractor::Extractor, store::Store};

/// Path to the e2e fixture cache (workspace-relative).
fn fixture_dir() -> PathBuf {
    // Walk up from CARGO_MANIFEST_DIR to find workspace root, then fixtures/.
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            let content = std::fs::read_to_string(&candidate).unwrap_or_default();
            if content.contains("[workspace]") {
                return dir.join("tests/e2e/fixtures");
            }
        }
        if let Some(parent) = dir.parent() {
            dir = parent.to_owned();
        } else {
            break;
        }
    }
    PathBuf::from("tests/e2e/fixtures")
}

/// Try to index a fixture directory; skip if it doesn't exist.
/// Returns `None` if skipped, `Some((store, file_count))` otherwise.
fn try_index_fixture(
    name: &str,
    ext: &str,
    queries: &str,
    lang: tree_sitter::Language,
) -> Option<(Store, usize)> {
    let fixture = fixture_dir().join(name);
    if !fixture.exists() {
        eprintln!("SKIP: fixture '{name}' not found at {}", fixture.display());
        eprintln!("      Run: bash scripts/fetch-e2e-fixtures.sh");
        return None;
    }
    let extractor = Extractor::new(lang, queries).expect("extractor");
    let mut store = Store::new();
    let mut files = 0usize;
    let walker = ignore::WalkBuilder::new(&fixture)
        .follow_links(false)
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            !matches!(n.as_ref(), "target" | ".git" | "vendor" | "node_modules")
        })
        .build();
    for entry in walker
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_some_and(|ft| ft.is_file()))
    {
        let path = entry.path();
        let file_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if file_ext != ext {
            continue;
        }
        let rel = path
            .strip_prefix(&fixture)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if let Ok(src) = std::fs::read(path) {
            if extractor.extract(&rel, &src, &mut store).is_ok() {
                files += 1;
            }
        }
    }
    store.resolve_bare_call_stubs();
    Some((store, files))
}

// ── Python: requests library ──────────────────────────────────────────────────

#[test]
fn real_project_requests_python_indexes_cleanly() {
    let py_queries = include_str!("../../packs/python/queries.scm");
    let py_lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();

    let Some((store, file_count)) = try_index_fixture("requests", "py", py_queries, py_lang) else {
        return; // skip
    };

    assert!(
        file_count >= 10,
        "requests: expected ≥10 .py files, got {file_count}"
    );
    assert!(store.node_count() >= 50, "requests: expected ≥50 nodes");
    assert!(store.edge_count() >= 10, "requests: expected ≥10 edges");

    // The Session class is a key symbol in requests.
    let has_session = store.all_symbols(None, None).iter().any(|p| p.ends_with(">Session"));
    assert!(has_session, "requests: expected to find 'Session' class");
}

// ── Rust: ripgrep (small, well-structured) ────────────────────────────────────

#[test]
fn real_project_ripgrep_rust_indexes_cleanly() {
    let rs_queries = include_str!("../../packs/rust/queries.scm");
    let rs_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();

    let Some((store, file_count)) = try_index_fixture("ripgrep", "rs", rs_queries, rs_lang) else {
        return; // skip
    };

    assert!(
        file_count >= 5,
        "ripgrep: expected ≥5 .rs files, got {file_count}"
    );
    assert!(store.node_count() >= 20, "ripgrep: expected ≥20 nodes");
}

// ── TypeScript: small real TS project ────────────────────────────────────────

#[test]
fn real_project_typescript_indexes_cleanly() {
    let ts_queries = include_str!("../../packs/typescript/queries.scm");
    let ts_lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();

    let Some((store, file_count)) =
        try_index_fixture("typescript-sample", "ts", ts_queries, ts_lang)
    else {
        return; // skip
    };

    assert!(
        file_count >= 2,
        "ts-sample: expected ≥2 .ts files, got {file_count}"
    );
    assert!(store.node_count() >= 5, "ts-sample: expected ≥5 nodes");
}
