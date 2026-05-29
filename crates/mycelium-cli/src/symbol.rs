//! CLI entry points for symbol-query subcommands: `search-symbol`,
//! `get-ancestors`, `get-symbol-info`.
//!
//! Three-Surface Rule (Charter §5.13 / RFC-0090): each function is the single
//! source of truth shared by the CLI subcommand and the corresponding MCP
//! tool, so CLI ↔ MCP output drift is impossible by construction.

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use mycelium_core::{store::Store, types::EdgeKind};

/// Output format requested by the caller.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Format {
    /// One result per line. Default for human terminals.
    Text,
    /// JSON — stable contract shared with the MCP twin tools.
    Json,
}

// ── shared helpers ────────────────────────────────────────────────────────────

fn load_store(root: &Path) -> Result<Store> {
    let index_path = root.join(".mycelium").join("index.rmp");
    if !index_path.exists() {
        return Err(anyhow!(
            "no index found at {} — run `mycelium index <root>` first",
            index_path.display()
        ));
    }
    Store::load(&index_path)
        .with_context(|| format!("failed to load index from {}", index_path.display()))
}

// ── search-symbol ─────────────────────────────────────────────────────────────

/// `mycelium search-symbol` — search symbols by name fragment.
///
/// Mirrors `mycelium_search_symbol` MCP tool (byte-identical JSON output).
pub(crate) fn run_search_symbol(
    root: &Path,
    query: &str,
    limit: usize,
    format: Format,
) -> Result<()> {
    let store = load_store(root)?;
    let results = store.search_symbol(query, limit);
    match format {
        Format::Text => {
            for r in &results {
                println!("{r}");
            }
        }
        Format::Json => {
            println!("{}", serde_json::to_string(&results)?);
        }
    }
    Ok(())
}

// ── get-ancestors ─────────────────────────────────────────────────────────────

/// `mycelium get-ancestors` — return the containment chain for a trunk path.
///
/// Mirrors `mycelium_get_ancestors` MCP tool (byte-identical JSON output).
pub(crate) fn run_get_ancestors(root: &Path, path: &str, format: Format) -> Result<()> {
    let store = load_store(root)?;
    let ancestors = store
        .ancestors_of_path(path)
        .ok_or_else(|| anyhow!("path not found in index: {path}"))?;
    match format {
        Format::Text => {
            for a in &ancestors {
                println!("{a}");
            }
        }
        Format::Json => {
            println!("{}", serde_json::to_string(&ancestors)?);
        }
    }
    Ok(())
}

// ── get-symbol-info ───────────────────────────────────────────────────────────

/// `mycelium get-symbol-info` — return ancestors, descendants, callers, and
/// callees for a symbol in one call.
///
/// Mirrors `mycelium_get_symbol_info` MCP tool (byte-identical JSON output).
#[allow(clippy::similar_names)]
pub(crate) fn run_get_symbol_info(root: &Path, path: &str, format: Format) -> Result<()> {
    let store = load_store(root)?;
    let Some(id) = store.lookup(path) else {
        return Err(anyhow!("path not found in index: {path}"));
    };

    let ancestors: Vec<String> = store
        .ancestors(id)
        .filter_map(|aid| store.path_of(aid).map(str::to_owned))
        .collect();

    let mut descendants: Vec<String> = store
        .descendants(id)
        .filter_map(|did| store.path_of(did).map(str::to_owned))
        .collect();
    descendants.sort_unstable();

    let mut callers: Vec<String> = store
        .incoming(id, EdgeKind::Calls)
        .iter()
        .filter_map(|&src| store.path_of(src).map(str::to_owned))
        .collect();
    callers.sort_unstable();
    callers.dedup();

    let mut callees: Vec<String> = store
        .outgoing(id, EdgeKind::Calls)
        .iter()
        .filter_map(|&dst| store.path_of(dst).map(str::to_owned))
        .collect();
    callees.sort_unstable();
    callees.dedup();

    match format {
        Format::Text => {
            println!("path:        {path}");
            if ancestors.is_empty() {
                println!("ancestors:   (none)");
            } else {
                println!("ancestors:   {}", ancestors.join(", "));
            }
            if descendants.is_empty() {
                println!("descendants: (none)");
            } else {
                println!("descendants: {}", descendants.join(", "));
            }
            if callers.is_empty() {
                println!("callers:     (none)");
            } else {
                println!("callers:     {}", callers.join(", "));
            }
            if callees.is_empty() {
                println!("callees:     (none)");
            } else {
                println!("callees:     {}", callees.join(", "));
            }
        }
        Format::Json => {
            let info = serde_json::json!({
                "path": path,
                "ancestors": ancestors,
                "descendants": descendants,
                "callers": callers,
                "callees": callees,
            });
            println!("{}", serde_json::to_string(&info)?);
        }
    }
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    // Helper: create an empty store snapshot at `<dir>/.mycelium/index.rmp`.
    fn empty_index(dir: &Path) {
        let snap = dir.join(".mycelium").join("index.rmp");
        fs::create_dir_all(snap.parent().unwrap()).unwrap();
        Store::default().save(&snap).unwrap();
    }

    // ── search-symbol ─────────────────────────────────────────────────────────

    #[test]
    fn search_symbol_errors_on_missing_index() {
        let dir = tempdir().unwrap();
        let err = run_search_symbol(dir.path(), "foo", 20, Format::Text).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("no index"), "got: {msg}");
        assert!(msg.contains("mycelium index"), "got: {msg}");
    }

    #[test]
    fn search_symbol_returns_empty_on_no_matches() {
        let dir = tempdir().unwrap();
        empty_index(dir.path());
        // Empty store → no results; should not error.
        run_search_symbol(dir.path(), "nonexistent_zzz", 20, Format::Text).unwrap();
    }

    #[test]
    fn search_symbol_json_returns_valid_json_array() {
        let dir = tempdir().unwrap();
        empty_index(dir.path());
        // Capture not possible via run_*, so we exercise the store path directly.
        let snap = dir.path().join(".mycelium").join("index.rmp");
        let store = Store::load(&snap).unwrap();
        let results = store.search_symbol("anything", 20);
        // Empty array is valid JSON.
        let _: Vec<String> =
            serde_json::from_str(&serde_json::to_string(&results).unwrap()).unwrap();
    }

    // ── get-ancestors ─────────────────────────────────────────────────────────

    #[test]
    fn get_ancestors_errors_on_missing_index() {
        let dir = tempdir().unwrap();
        let err = run_get_ancestors(dir.path(), "src/foo.rs>bar", Format::Text).unwrap_err();
        assert!(format!("{err}").contains("no index"));
    }

    #[test]
    fn get_ancestors_errors_on_unknown_path() {
        let dir = tempdir().unwrap();
        empty_index(dir.path());
        let err =
            run_get_ancestors(dir.path(), "src/does_not_exist.rs>fn", Format::Text).unwrap_err();
        assert!(format!("{err}").contains("not found"));
    }

    // ── get-symbol-info ───────────────────────────────────────────────────────

    #[test]
    fn get_symbol_info_errors_on_missing_index() {
        let dir = tempdir().unwrap();
        let err = run_get_symbol_info(dir.path(), "src/foo.rs>bar", Format::Text).unwrap_err();
        assert!(format!("{err}").contains("no index"));
    }

    #[test]
    fn get_symbol_info_errors_on_unknown_path() {
        let dir = tempdir().unwrap();
        empty_index(dir.path());
        let err =
            run_get_symbol_info(dir.path(), "src/does_not_exist.rs>fn", Format::Text).unwrap_err();
        assert!(format!("{err}").contains("not found"));
    }
}
