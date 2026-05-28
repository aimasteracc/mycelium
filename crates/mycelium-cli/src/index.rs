//! `mycelium index` — walk a directory, extract symbols, report statistics.
//!
//! This is the first end-user-visible command that produces real output.
//! It exercises the full RFC-0001 + RFC-0002 stack.
// Items here are `pub` so main.rs can use them, but this module is private to
// the binary.  `unreachable_pub` is a false positive in this pattern.
#![allow(unreachable_pub)]

use std::path::Path;

use anyhow::{Context, Result};
use mycelium_core::{extractor::Extractor, store::Store};
use walkdir::WalkDir;

// ── embedded pack sources ─────────────────────────────────────────────────────

const JAVASCRIPT_QUERIES: &str = include_str!("../../../packs/javascript/queries.scm");
const PYTHON_QUERIES: &str = include_str!("../../../packs/python/queries.scm");
const TYPESCRIPT_QUERIES: &str = include_str!("../../../packs/typescript/queries.scm");
const RUST_QUERIES: &str = include_str!("../../../packs/rust/queries.scm");

// ── public surface ────────────────────────────────────────────────────────────

/// Statistics returned from a successful index run.
#[derive(Debug, Default)]
pub struct IndexStats {
    /// Number of source files processed.
    pub files: usize,
    /// Files that could not be read or extracted (non-fatal).
    pub errors: usize,
}

/// Walk `root`, extract all recognised source files, and return stats.
///
/// Supported languages: JavaScript (`.js`, `.jsx`), Python (`.py`, `.pyi`),
/// TypeScript (`.ts`, `.tsx`), Rust (`.rs`).
///
/// # Errors
///
/// Returns an error only if `root` cannot be accessed. Individual file errors
/// are counted in [`IndexStats::errors`] but do not stop the run.
pub fn index_path(root: &Path) -> Result<(Store, IndexStats)> {
    let js_lang: tree_sitter::Language = tree_sitter_javascript::LANGUAGE.into();
    let js_ext = Extractor::new(js_lang, JAVASCRIPT_QUERIES)
        .context("failed to compile JavaScript extractor")?;

    let python_lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
    let python_ext = Extractor::new(python_lang, PYTHON_QUERIES)
        .context("failed to compile Python extractor")?;

    let ts_lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    let ts_ext = Extractor::new(ts_lang, TYPESCRIPT_QUERIES)
        .context("failed to compile TypeScript extractor")?;

    let rs_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let rs_ext =
        Extractor::new(rs_lang, RUST_QUERIES).context("failed to compile Rust extractor")?;

    let mut store = Store::new();
    let mut stats = IndexStats::default();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };

        let extractor = match ext {
            "js" | "jsx" => &js_ext,
            "py" | "pyi" => &python_ext,
            "ts" | "tsx" => &ts_ext,
            "rs" => &rs_ext,
            _ => continue,
        };

        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        let source = match std::fs::read(path) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("could not read {}: {e}", path.display());
                stats.errors += 1;
                continue;
            }
        };

        if let Err(e) = extractor.extract(&rel, &source, &mut store) {
            tracing::warn!("extraction failed for {}: {e}", path.display());
            stats.errors += 1;
            continue;
        }

        stats.files += 1;
    }

    // Count nodes and edges via descendant traversal from store internals.
    // For v0.1 we use a proxy: the number of unique paths looked up is
    // unavailable, so we fall back to a simple walk of everything reachable
    // from the root nodes (files with no parent).
    //
    // A proper node/edge count will arrive with the persistence layer (P4).
    // For now, the caller may inspect the store directly.

    Ok((store, stats))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn write_temp_py(dir: &Path, name: &str, src: &str) {
        fs::write(dir.join(name), src).unwrap();
    }

    fn write_temp_ts(dir: &Path, name: &str, src: &str) {
        fs::write(dir.join(name), src).unwrap();
    }

    #[test]
    fn index_path_extracts_single_file() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_py(
            tmp.path(),
            "hello.py",
            "def greet(): pass\nclass World: pass",
        );
        let (store, stats) = index_path(tmp.path()).unwrap();
        assert_eq!(stats.files, 1, "should process one file");
        assert!(stats.errors == 0);
        assert!(store.lookup("hello.py").is_some(), "file node should exist");
        assert!(store.lookup("hello.py>greet").is_some());
        assert!(store.lookup("hello.py>World").is_some());
    }

    #[test]
    fn index_path_skips_non_python_files() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_py(tmp.path(), "main.py", "x = 1");
        fs::write(tmp.path().join("README.md"), "# docs").unwrap();
        let (_, stats) = index_path(tmp.path()).unwrap();
        assert_eq!(stats.files, 1, "only .py file should be counted");
    }

    #[test]
    fn index_path_recurses_into_subdirectories() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub");
        fs::create_dir(&sub).unwrap();
        write_temp_py(&sub, "deep.py", "def deep(): pass");
        let (store, stats) = index_path(tmp.path()).unwrap();
        assert_eq!(stats.files, 1);
        // On Unix, the relative path uses forward slash
        assert!(store.lookup("sub/deep.py").is_some());
    }

    #[test]
    fn index_path_handles_empty_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let (_, stats) = index_path(tmp.path()).unwrap();
        assert_eq!(stats.files, 0);
        assert_eq!(stats.errors, 0);
    }

    // ── TypeScript tests ─────────────────────────────────────────────

    #[test]
    fn index_path_extracts_typescript_function() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_ts(
            tmp.path(),
            "greet.ts",
            "function greet(name: string): void {}",
        );
        let (store, stats) = index_path(tmp.path()).unwrap();
        assert_eq!(stats.files, 1);
        assert_eq!(stats.errors, 0);
        assert!(store.lookup("greet.ts").is_some(), "module node must exist");
        assert!(
            store.lookup("greet.ts>greet").is_some(),
            "function node must exist"
        );
    }

    #[test]
    fn index_path_extracts_typescript_class_and_method() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_ts(
            tmp.path(),
            "greeter.ts",
            "class Greeter { greet(): string { return ''; } }",
        );
        let (store, stats) = index_path(tmp.path()).unwrap();
        assert_eq!(stats.files, 1);
        assert!(store.lookup("greeter.ts>Greeter").is_some());
        assert!(store.lookup("greeter.ts>Greeter>greet").is_some());
    }

    #[test]
    fn index_path_extracts_typescript_interface() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_ts(
            tmp.path(),
            "types.ts",
            "interface IGreeter { greet(): void; }",
        );
        let (store, stats) = index_path(tmp.path()).unwrap();
        assert_eq!(stats.files, 1);
        assert!(store.lookup("types.ts>IGreeter").is_some());
    }

    #[test]
    fn index_path_indexes_both_python_and_typescript() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_py(tmp.path(), "mod.py", "def foo(): pass");
        write_temp_ts(tmp.path(), "mod.ts", "function bar(): void {}");
        let (store, stats) = index_path(tmp.path()).unwrap();
        assert_eq!(stats.files, 2);
        assert!(store.lookup("mod.py>foo").is_some());
        assert!(store.lookup("mod.ts>bar").is_some());
    }

    #[test]
    fn index_path_indexes_tsx_as_typescript() {
        let tmp = tempfile::tempdir().unwrap();
        // TSX is treated as TypeScript; JSX nodes are transparent to symbol extraction.
        write_temp_ts(tmp.path(), "app.tsx", "function App() { return null; }");
        let (store, stats) = index_path(tmp.path()).unwrap();
        assert_eq!(
            stats.files, 1,
            "tsx should be indexed via the TypeScript extractor"
        );
        assert!(
            store.lookup("app.tsx>App").is_some(),
            "App function must be extracted"
        );
    }

    #[test]
    fn index_path_indexes_jsx_as_javascript() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_ts(tmp.path(), "app.jsx", "function App() { return null; }");
        let (store, stats) = index_path(tmp.path()).unwrap();
        assert_eq!(
            stats.files, 1,
            "jsx should be indexed via the JavaScript extractor"
        );
        assert!(
            store.lookup("app.jsx>App").is_some(),
            "App function must be extracted"
        );
    }

    #[test]
    fn index_path_indexes_js_function() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_ts(
            tmp.path(),
            "utils.js",
            "function add(a, b) { return a + b; }",
        );
        let (store, stats) = index_path(tmp.path()).unwrap();
        assert_eq!(stats.files, 1);
        assert!(store.lookup("utils.js>add").is_some());
    }

    // ── Rust tests ───────────────────────────────────────────────────

    fn write_temp_rs(dir: &Path, name: &str, src: &str) {
        fs::write(dir.join(name), src).unwrap();
    }

    #[test]
    fn index_path_extracts_rust_function() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_rs(tmp.path(), "lib.rs", "fn greet() {}");
        let (store, stats) = index_path(tmp.path()).unwrap();
        assert_eq!(stats.files, 1);
        assert_eq!(stats.errors, 0);
        assert!(store.lookup("lib.rs").is_some());
        assert!(store.lookup("lib.rs>greet").is_some());
    }

    #[test]
    fn index_path_extracts_rust_struct_and_impl_method() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_rs(
            tmp.path(),
            "model.rs",
            "struct Point { x: i32 } impl Point { fn new() -> Self { Point { x: 0 } } }",
        );
        let (store, stats) = index_path(tmp.path()).unwrap();
        assert_eq!(stats.files, 1);
        assert!(store.lookup("model.rs>Point").is_some());
        assert!(store.lookup("model.rs>Point>new").is_some());
    }

    #[test]
    fn index_path_extracts_rust_enum_and_trait() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_rs(
            tmp.path(),
            "types.rs",
            "enum Color { Red } trait Drawable { fn draw(&self); }",
        );
        let (store, stats) = index_path(tmp.path()).unwrap();
        assert_eq!(stats.files, 1);
        assert!(store.lookup("types.rs>Color").is_some());
        assert!(store.lookup("types.rs>Drawable").is_some());
    }
}
