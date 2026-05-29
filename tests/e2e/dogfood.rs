//! Dogfood e2e test — Charter §5.10
//!
//! "Mycelium indexes itself; CI runs queries against the Mycelium codebase."
//!
//! These tests index the Mycelium source tree using the real extractor
//! pipeline and assert structural properties we know to be true.
//! If these tests ever fail, the indexer is broken.

use std::path::PathBuf;

use mycelium_core::{extractor::Extractor, store::Store, trunk::TrunkPath};

/// Locate the workspace root (directory containing `[workspace]` Cargo.toml).
fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR points to the crate being tested; walk up to workspace.
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            let content = std::fs::read_to_string(&candidate).unwrap_or_default();
            if content.contains("[workspace]") {
                return dir;
            }
        }
        dir = dir
            .parent()
            .expect("reached filesystem root without finding workspace Cargo.toml")
            .to_owned();
    }
}

/// Index a source tree with the Rust extractor. Returns `(store, file_count)`.
fn index_rust_tree(root: &std::path::Path) -> (Store, usize) {
    let rs_queries = include_str!("../../packs/rust/queries.scm");
    let rs_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let extractor = Extractor::new(rs_lang, rs_queries).expect("Rust extractor");

    let mut store = Store::new();
    let mut files = 0usize;

    let walker = ignore::WalkBuilder::new(root)
        .follow_links(false)
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            !matches!(n.as_ref(), "target" | ".mycelium" | ".git")
        })
        .build();

    for entry in walker
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_some_and(|ft| ft.is_file()))
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let rel = path
            .strip_prefix(root)
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
    (store, files)
}

// ── Dogfood tests ─────────────────────────────────────────────────────────────

#[test]
fn dogfood_indexes_own_rust_files() {
    let root = workspace_root();
    let (store, file_count) = index_rust_tree(&root);

    assert!(
        file_count >= 10,
        "expected ≥10 Rust files indexed from own codebase, got {file_count}"
    );
    assert!(
        store.node_count() >= 50,
        "expected ≥50 nodes from own codebase, got {}",
        store.node_count()
    );
}

#[test]
fn dogfood_store_struct_is_indexed() {
    let root = workspace_root();
    let (store, _) = index_rust_tree(&root);

    // The Store struct itself must appear as a symbol.
    let has_store = store
        .all_symbols(None, None)
        .iter()
        .any(|p| p.ends_with(">Store"));

    assert!(
        has_store,
        "expected to find a 'Store' symbol; indexer failed to parse store module.\n\
         Indexed symbols: {:?}",
        store
            .all_symbols(None, None)
            .iter()
            .take(10)
            .collect::<Vec<_>>()
    );
}

#[test]
fn dogfood_trunk_struct_is_indexed() {
    let root = workspace_root();
    let (store, _) = index_rust_tree(&root);

    let has_trunk = store
        .all_symbols(None, None)
        .iter()
        .any(|p| p.ends_with(">Trunk"));

    assert!(
        has_trunk,
        "expected 'Trunk' struct to be indexed in own codebase"
    );
}

#[test]
fn dogfood_edge_count_positive() {
    let root = workspace_root();
    let (store, _) = index_rust_tree(&root);

    assert!(
        store.edge_count() >= 10,
        "expected ≥10 call/import edges in own codebase, got {}",
        store.edge_count()
    );
}

#[test]
fn dogfood_upsert_lookup_roundtrip() {
    // Unit-level smoke test: Store works on a known symbol path.
    let mut store = Store::new();
    let id = store.upsert_node(TrunkPath::parse("tests/e2e/dogfood.rs>dogfood_smoke").unwrap());
    assert_eq!(
        store.path_of(id),
        Some("tests/e2e/dogfood.rs>dogfood_smoke")
    );
}
