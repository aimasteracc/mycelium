//! `mycelium query <hyphae>` — execute a Hyphae DSL selector against the
//! project's `.mycelium/index.rmp` snapshot and print matching symbol paths.
//!
//! Implements issue #151 (closes the v0.1.x advertising gap where Hyphae was
//! the README's marquee feature but had no CLI surface).
//!
//! Three-Surface Rule (Charter §5.13 / RFC-0090): byte-identical contract
//! with the MCP twin tool `mycelium_query`. Both call into this module's
//! `execute` function so drift is impossible by construction.

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use mycelium_core::store::Store;
use mycelium_hyphae::{evaluator::Evaluator, parser::parse};

/// Output format requested by the user (or by the MCP wrapper).
#[derive(Debug, Clone, Copy)]
pub(crate) enum Format {
    /// One match per line, plain text. Default for human terminals.
    Text,
    /// A JSON array of strings. Stable contract for downstream tooling and
    /// the MCP twin tool.
    Json,
}

/// Execute a Hyphae selector against the snapshot at `index_path` and return
/// the matched symbol paths.
///
/// This is the single source of truth shared by the CLI subcommand and the
/// MCP twin tool. Three-Surface Rule parity is built in.
///
/// Errors:
/// - The snapshot file is missing → user-recovery hint to run
///   `mycelium index`.
/// - The selector fails to parse → returns the parser's `ParseError`
///   formatted with `mycelium-hyphae` prefix.
pub(crate) fn execute(index_path: &Path, selector: &str) -> Result<Vec<String>> {
    if !index_path.exists() {
        return Err(anyhow!(
            "no index found at {} — run `mycelium index <root>` first",
            index_path.display()
        ));
    }
    let store = Store::load(index_path)
        .with_context(|| format!("failed to load index from {}", index_path.display()))?;
    let ast = parse(selector).map_err(|e| anyhow!("hyphae parse error: {e:?}"))?;
    let evaluator = Evaluator::new(&store);
    Ok(evaluator.eval(&ast))
}

/// CLI entry point. Loads the snapshot from `.mycelium/index.rmp` at `root`,
/// executes the selector, and writes results to stdout.
///
/// Returns `Err` (and the binary exits non-zero) on missing snapshot,
/// malformed selector, or I/O failure. Matches the test contract in
/// `tests/cli_query.rs`.
pub(crate) fn run(root: &Path, selector: &str, format: Format) -> Result<()> {
    let index_path = root.join(".mycelium").join("index.rmp");
    let matches = execute(&index_path, selector)?;
    match format {
        Format::Text => {
            for m in &matches {
                println!("{m}");
            }
        }
        Format::Json => {
            let json = serde_json::to_string(&matches)?;
            println!("{json}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// End-to-end CLI behaviour lives in `tests/cli_query.rs`. The unit tests
    /// here exercise the failure surfaces of `execute` without touching the
    /// `Extractor` (which would couple this module to internal core APIs).
    #[test]
    fn execute_returns_helpful_error_when_index_missing() {
        let dir = tempdir().unwrap();
        let snap = dir.path().join(".mycelium").join("index.rmp");
        let err = execute(&snap, "#anything").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("no index"), "got: {msg}");
        assert!(msg.contains("mycelium index"), "got: {msg}");
    }

    #[test]
    fn execute_reports_parse_error_for_garbage_selector() {
        // An empty store on disk is enough to clear the "no index" branch and
        // reach the parser.
        let dir = tempdir().unwrap();
        let snap = dir.path().join(".mycelium").join("index.rmp");
        std::fs::create_dir_all(snap.parent().unwrap()).unwrap();
        Store::default().save(&snap).unwrap();

        let err = execute(&snap, "this is not a selector >>").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("parse") || msg.to_lowercase().contains("hyphae"),
            "got: {msg}"
        );
    }
}
