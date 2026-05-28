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

const PYTHON_QUERIES: &str = include_str!("../../../packs/python/queries.scm");

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
/// Supported languages at v0.1: Python (`.py`, `.pyi`).
///
/// # Errors
///
/// Returns an error only if `root` cannot be accessed. Individual file errors
/// are counted in [`IndexStats::errors`] but do not stop the run.
pub fn index_path(root: &Path) -> Result<(Store, IndexStats)> {
    let python_lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
    let python_ext = Extractor::new(python_lang, PYTHON_QUERIES)
        .context("failed to compile Python extractor")?;

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
        if ext != "py" && ext != "pyi" {
            continue;
        }

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

        if let Err(e) = python_ext.extract(&rel, &source, &mut store) {
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
}
