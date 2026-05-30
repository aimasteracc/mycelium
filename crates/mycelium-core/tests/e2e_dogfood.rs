//! Charter §5.10 Dogfood: Mycelium indexes itself.
//!
//! This e2e test walks the Mycelium repository, runs the Rust tree-sitter
//! extractor over every `.rs` source file we own, and asserts that:
//!
//! 1. We can index at least 100 of our own Rust files (sanity ratio — the
//!    workspace is far larger than that, so the test fails if the walker
//!    silently skips a category of files).
//! 2. Zero extraction errors (every `.rs` file we ship must parse cleanly
//!    with the bundled `packs/rust/queries.scm` — if our own dogfood breaks,
//!    so does every downstream consumer).
//! 3. Specific known symbols resolve, proving the trunk path layout is
//!    correct end-to-end.
//!
//! Triggered by `.github/workflows/e2e.yml::dogfood`.

use std::{fs, path::Path};

use mycelium_core::{extractor::Extractor, store::Store};

const RUST_QUERIES: &str = include_str!("../packs/rust/queries.scm");

/// The workspace root. Every Mycelium crate lives at
/// `<workspace>/crates/<name>/`, so the root is exactly two parents above
/// `CARGO_MANIFEST_DIR`. The earlier "ascend until Cargo.lock" heuristic
/// misfires on CI when cargo's package / vendor layout puts a Cargo.lock
/// next to the crate's Cargo.toml — the walker then sees only the
/// individual crate (~46 .rs files) instead of the whole workspace (~145).
fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("CARGO_MANIFEST_DIR must be <workspace>/crates/<name>/")
}

/// Recursively walk `dir`, calling `f` for every file. Skips well-known
/// non-source directories so the test mirrors what `mycelium index` does.
fn walk(dir: &Path, f: &mut impl FnMut(&Path)) {
    if let Some(name) = dir.file_name().and_then(|n| n.to_str())
        && matches!(
            name,
            "target" | ".git" | ".mycelium" | "node_modules" | ".idea" | ".vscode"
        )
    {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, f);
        } else {
            f(&path);
        }
    }
}

#[test]
fn mycelium_indexes_its_own_rust_sources_without_errors() {
    let root = workspace_root();
    let rs_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let extractor = Extractor::new(rs_lang, RUST_QUERIES)
        .expect("Rust extractor must compile against bundled queries.scm");
    let mut store = Store::new();

    let mut files_indexed = 0_usize;
    let mut errors = 0_usize;

    walk(root, &mut |path| {
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            return;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let Ok(source) = fs::read(path) else {
            errors += 1;
            return;
        };
        if extractor.extract(&rel, &source, &mut store).is_err() {
            errors += 1;
            return;
        }
        files_indexed += 1;
    });

    eprintln!("Dogfood: indexed {files_indexed} Rust files, {errors} errors");

    assert!(
        files_indexed >= 100,
        "expected ≥ 100 .rs files in the Mycelium workspace, got {files_indexed} — \
         walker may have skipped a category of files"
    );
    assert_eq!(
        errors, 0,
        "every Rust source file we ship must extract cleanly with the bundled \
         packs/rust/queries.scm — got {errors} extraction errors"
    );

    // Spot-check: load-bearing files must resolve.
    assert!(
        store.lookup("crates/mycelium-core/src/lib.rs").is_some(),
        "mycelium-core lib.rs must produce a file node"
    );
    assert!(
        store
            .lookup("crates/mycelium-core/src/store/mod.rs")
            .is_some(),
        "mycelium-core store module must produce a file node"
    );
}
