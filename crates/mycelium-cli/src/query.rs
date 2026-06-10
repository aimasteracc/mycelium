//! `mycelium query <hyphae>` — execute a Hyphae DSL selector against the
//! project's `.mycelium/index.rmp` snapshot and print matching symbol paths.
//!
//! Implements issue #151 (closes the v0.1.x advertising gap where Hyphae was
//! the README's marquee feature but had no CLI surface).
//!
//! Three-Surface Rule (Charter §5.13 / RFC-0090): byte-identical contract
//! with the MCP twin tool `mycelium_query`. JSON output is the
//! `{ matches, count, total_count }` object built by
//! `mycelium_core::queries::query_matches_payload`, with the RFC-0102 output
//! budget applied the same way on both surfaces.

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use mycelium_core::store::Store;
use mycelium_hyphae::{evaluator::Evaluator, parser::parse};

use crate::queries::{Format, print_object_with_list};

/// Execute a Hyphae selector against the snapshot at `index_path` and return
/// the matched symbol paths together with the store's node count (the budget
/// tier input — returned here so the snapshot is loaded exactly once).
///
/// This is the single source of truth shared by the CLI subcommand and the
/// MCP twin tool. Three-Surface Rule parity is built in.
///
/// Errors:
/// - The snapshot file is missing → user-recovery hint to run
///   `mycelium index`.
/// - The selector fails to parse → returns the parser's `ParseError`
///   formatted with `mycelium-hyphae` prefix.
pub(crate) fn execute(index_path: &Path, selector: &str) -> Result<(Vec<String>, usize)> {
    if !index_path.exists() {
        return Err(anyhow!(
            "no index found at {} — run `mycelium index <root>` first",
            index_path.display()
        ));
    }
    let store = Store::load(index_path)
        .with_context(|| format!("failed to load index from {}", index_path.display()))?;
    // `{e}` (Display), not `{e:?}` (Debug): ParseError's Display carries the
    // grammar hint + did-you-mean + docs pointer; Debug would dump the bare enum.
    let ast = parse(selector).map_err(|e| anyhow!("hyphae parse error: {e}"))?;
    let evaluator = Evaluator::new(&store);
    // `eval_checked` (not `eval`): a selector that parses but names an
    // unsupported attribute (`[lang=…]`) or pseudo-class (`:frobnicate()`)
    // returns an explicit `EvalError` here instead of a silent empty set that
    // an agent would misread as "no matches". `{e}` (Display) carries the
    // supported-name hint.
    let matches = evaluator
        .eval_checked(&ast)
        .map_err(|e| anyhow!("hyphae query error: {e}"))?;
    Ok((matches, store.node_count()))
}

/// CLI entry point. Loads the snapshot from `.mycelium/index.rmp` at `root`,
/// executes the selector, and writes results to stdout.
///
/// The budget (RFC-0102) applies in JSON mode (parity with the MCP twin) or
/// when `--budget` is explicit; default text mode prints the full list — no
/// silent truncation of human-facing output (a truncation footer goes to
/// stderr when an explicit budget cuts text output).
///
/// Returns `Err` (and the binary exits non-zero) on missing snapshot,
/// malformed selector, unknown budget value, or I/O failure. Matches the
/// test contract in `tests/cli_query.rs`.
pub(crate) fn run(root: &Path, selector: &str, budget: Option<&str>, format: Format) -> Result<()> {
    use mycelium_core::budget::{BudgetOverride, OutputBudget, apply_budget};

    let budget_override = budget
        .map(str::parse::<BudgetOverride>)
        .transpose()
        .map_err(|e| anyhow!(e))?;
    let index_path = root.join(".mycelium").join("index.rmp");
    let (matches, node_count) = execute(&index_path, selector)?;
    // Shared core builder → byte-identical with the MCP tool (RFC-0109).
    let mut value = mycelium_core::queries::query_matches_payload(&matches);
    if matches!(format, Format::Json) || budget_override.is_some() {
        apply_budget(
            &mut value,
            &OutputBudget::resolve(budget_override, node_count),
        );
    }
    print_object_with_list(&value, "matches", format)
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

    #[test]
    fn execute_errors_on_unsupported_selector_instead_of_empty() {
        // An agent writing `[lang=rust]` (the supported name is `language`)
        // must get an actionable ERROR, not a silent empty match set that
        // reads as "no Rust functions exist".
        let dir = tempdir().unwrap();
        let snap = dir.path().join(".mycelium").join("index.rmp");
        std::fs::create_dir_all(snap.parent().unwrap()).unwrap();
        Store::default().save(&snap).unwrap();

        let err = execute(&snap, ".function[lang=rust]").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("unsupported"), "got: {msg}");
        assert!(msg.contains("language"), "suggests the right name: {msg}");
    }

    #[test]
    fn execute_errors_on_unknown_kind_with_suggestion() {
        // `.fn` (the Rust keyword guess; the supported token is `.function`)
        // must get an actionable ERROR with a did-you-mean, not a silent
        // empty match set that reads as "no functions exist".
        let dir = tempdir().unwrap();
        let snap = dir.path().join(".mycelium").join("index.rmp");
        std::fs::create_dir_all(snap.parent().unwrap()).unwrap();
        Store::default().save(&snap).unwrap();

        let err = execute(&snap, ".fn").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("unsupported"), "got: {msg}");
        assert!(msg.contains(".function"), "suggests `.function`: {msg}");
    }

    #[test]
    fn execute_parse_error_has_no_debug_noise() {
        // `#a + #b` previously leaked the raw Debug `LexError(3)`; the CLI
        // must render the human Display message instead.
        let dir = tempdir().unwrap();
        let snap = dir.path().join(".mycelium").join("index.rmp");
        std::fs::create_dir_all(snap.parent().unwrap()).unwrap();
        Store::default().save(&snap).unwrap();

        let err = execute(&snap, "#a + #b").unwrap_err();
        let msg = format!("{err}");
        assert!(!msg.contains("LexError("), "no Debug noise: {msg}");
        assert!(msg.contains("position"), "names the position: {msg}");
    }
}
