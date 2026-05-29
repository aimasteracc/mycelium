//! v0.1.4 CLI parity backfill — the human-facing twins of MCP tools
//! `mycelium_search_symbol`, `mycelium_get_symbol_info`,
//! `mycelium_get_ancestors`.
//!
//! Three-Surface Rule (Charter §5.13 / RFC-0090) parity: the CLI output
//! shape here MUST match the MCP tool output shape byte-for-byte (modulo
//! timestamps). The fixtures in `skills/basic-queries/tests/parity.test.json`
//! exercise that contract.

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use mycelium_core::store::Store;
use mycelium_core::types::EdgeKind;

/// Output format requested by the user (or by the MCP wrapper).
#[derive(Debug, Clone, Copy)]
pub(crate) enum Format {
    /// One result per line (or human-friendly key-value), default for terminals.
    Text,
    /// JSON — the stable contract used by the MCP twin tool.
    Json,
}

fn load_index(root: &Path) -> Result<Store> {
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

pub(crate) fn run_search_symbol(
    root: &Path,
    query: &str,
    limit: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let matches = store.search_symbol(query, limit);
    match format {
        Format::Text => {
            for m in &matches {
                println!("{m}");
            }
        }
        Format::Json => {
            println!("{}", serde_json::to_string(&matches)?);
        }
    }
    Ok(())
}

// ── get-symbol-info ───────────────────────────────────────────────────────────

pub(crate) fn run_get_symbol_info(root: &Path, path: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let value = symbol_info(&store, path)?;
    match format {
        Format::Text => {
            println!("path:        {}", value["path"]);
            println!("ancestors:   {}", value["ancestors"]);
            println!("descendants: {}", value["descendants"]);
            println!("callers:     {}", value["callers"]);
            println!("callees:     {}", value["callees"]);
        }
        Format::Json => {
            println!("{}", serde_json::to_string(&value)?);
        }
    }
    Ok(())
}

/// Same shape as the MCP `mycelium_get_symbol_info` tool's success envelope.
/// Three-Surface Rule single-source-of-truth.
#[allow(
    clippy::similar_names,
    reason = "callers/callees are the canonical field names matched by the MCP tool"
)]
fn symbol_info(store: &Store, path: &str) -> Result<serde_json::Value> {
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;

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

    Ok(serde_json::json!({
        "path": path,
        "ancestors": ancestors,
        "descendants": descendants,
        "callers": callers,
        "callees": callees,
    }))
}

// ── get-ancestors ─────────────────────────────────────────────────────────────

pub(crate) fn run_get_ancestors(root: &Path, path: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let ancestors = store
        .ancestors_of_path(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    print_string_list(&ancestors, format)
}

// ── get-descendants ───────────────────────────────────────────────────────────

pub(crate) fn run_get_descendants(root: &Path, path: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let descendants = store
        .descendants_of_path(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    print_string_list(&descendants, format)
}

// ── get-node-kind ─────────────────────────────────────────────────────────────

pub(crate) fn run_get_node_kind(root: &Path, path: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let kind = store.kind_of(id).map(|k| k.as_str().to_owned());
    let value = serde_json::json!({ "path": path, "kind": kind });
    match format {
        Format::Text => match kind {
            Some(k) => println!("{k}"),
            None => println!("(no kind recorded)"),
        },
        Format::Json => println!("{}", serde_json::to_string(&value)?),
    }
    Ok(())
}

// ── get-symbols-by-kind ───────────────────────────────────────────────────────

pub(crate) fn run_get_symbols_by_kind(
    root: &Path,
    kind_str: &str,
    prefix: Option<&str>,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = mycelium_core::types::NodeKind::try_from_wire(kind_str)
        .ok_or_else(|| anyhow!("unknown kind: {kind_str}"))?;
    let symbols = store.symbols_of_kind(kind, prefix);
    print_string_list(&symbols, format)
}

// ── get-source-span ───────────────────────────────────────────────────────────

pub(crate) fn run_get_source_span(root: &Path, path: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let value = store.span_of(id).map_or_else(
        || serde_json::json!({ "path": path, "span": serde_json::Value::Null }),
        |span| {
            serde_json::json!({
                "path": path,
                "start_line": span.start_line,
                "start_col":  span.start_col,
                "end_line":   span.end_line,
                "end_col":    span.end_col,
                "start_byte": span.start_byte,
                "end_byte":   span.end_byte,
            })
        },
    );
    match format {
        Format::Text => {
            if value["span"].is_null() {
                println!("(no source span recorded)");
            } else {
                println!(
                    "{}:{}:{}-{}:{}",
                    value["path"].as_str().unwrap_or(""),
                    value["start_line"],
                    value["start_col"],
                    value["end_line"],
                    value["end_col"],
                );
            }
        }
        Format::Json => println!("{}", serde_json::to_string(&value)?),
    }
    Ok(())
}

// ── get-siblings ──────────────────────────────────────────────────────────────

pub(crate) fn run_get_siblings(root: &Path, path: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let siblings = store.siblings(id);
    print_string_list(&siblings, format)
}

// ── get-all-symbols ───────────────────────────────────────────────────────────

pub(crate) fn run_get_all_symbols(
    root: &Path,
    prefix: Option<&str>,
    kind_str: Option<&str>,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = match kind_str {
        None => None,
        Some(k) => Some(
            mycelium_core::types::NodeKind::try_from_wire(k)
                .ok_or_else(|| anyhow!("unknown kind: {k}"))?,
        ),
    };
    let symbols = store.all_symbols(prefix, kind);
    print_string_list(&symbols, format)
}

// ── server-status ─────────────────────────────────────────────────────────────

pub(crate) fn run_server_status(root: &Path, format: Format) -> Result<()> {
    let index_path = root.join(".mycelium").join("index.rmp");
    let is_loaded = index_path.exists();
    let (node_count, edge_count) = if is_loaded {
        let store = Store::load(&index_path)
            .with_context(|| format!("failed to load index from {}", index_path.display()))?;
        (store.node_count(), store.edge_count())
    } else {
        (0, 0)
    };
    let value = serde_json::json!({
        "node_count":   node_count,
        "edge_count":   edge_count,
        "indexed_root": root.to_string_lossy(),
        "is_loaded":    is_loaded,
    });
    match format {
        Format::Text => {
            println!("indexed_root: {}", root.display());
            println!("is_loaded:    {is_loaded}");
            println!("node_count:   {node_count}");
            println!("edge_count:   {edge_count}");
        }
        Format::Json => println!("{}", serde_json::to_string(&value)?),
    }
    Ok(())
}

// ── call-graph: get-callees / get-callers ─────────────────────────────────────

pub(crate) fn run_get_callees(root: &Path, path: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let mut paths: Vec<String> = store
        .outgoing(id, EdgeKind::Calls)
        .iter()
        .filter_map(|&t| store.path_of(t).map(str::to_owned))
        .collect();
    paths.sort_unstable();
    paths.dedup();
    print_string_list(&paths, format)
}

pub(crate) fn run_get_callers(root: &Path, path: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let mut paths: Vec<String> = store
        .incoming(id, EdgeKind::Calls)
        .iter()
        .filter_map(|&t| store.path_of(t).map(str::to_owned))
        .collect();
    paths.sort_unstable();
    paths.dedup();
    print_string_list(&paths, format)
}

// ── call-graph: get-callee-tree / get-caller-tree ─────────────────────────────

fn callee_node_to_json(node: &mycelium_core::CalleeNode, store: &Store) -> serde_json::Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let children: Vec<serde_json::Value> = node
        .children
        .iter()
        .map(|c| callee_node_to_json(c, store))
        .collect();
    serde_json::json!({ "path": path, "children": children })
}

fn caller_node_to_json(node: &mycelium_core::CallerNode, store: &Store) -> serde_json::Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let callers: Vec<serde_json::Value> = node
        .callers
        .iter()
        .map(|c| caller_node_to_json(c, store))
        .collect();
    serde_json::json!({ "path": path, "callers": callers })
}

pub(crate) fn run_get_callee_tree(
    root: &Path,
    path: &str,
    max_depth: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let tree = store.callee_tree(id, max_depth);
    let value = callee_node_to_json(&tree, &store);
    match format {
        Format::Text => println!("{value}"),
        Format::Json => println!("{}", serde_json::to_string(&value)?),
    }
    Ok(())
}

pub(crate) fn run_get_caller_tree(
    root: &Path,
    path: &str,
    max_depth: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let tree = store.caller_tree(id, max_depth);
    let value = caller_node_to_json(&tree, &store);
    match format {
        Format::Text => println!("{value}"),
        Format::Json => println!("{}", serde_json::to_string(&value)?),
    }
    Ok(())
}

// ── call-graph: entry-points / dead-symbols / isolated-symbols ────────────────

pub(crate) fn run_get_entry_points(
    root: &Path,
    prefix: Option<&str>,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let symbols = store.entry_points(prefix);
    print_string_list(&symbols, format)
}

pub(crate) fn run_get_dead_symbols(
    root: &Path,
    prefix: Option<&str>,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let symbols = store.dead_symbols(prefix);
    print_string_list(&symbols, format)
}

pub(crate) fn run_get_isolated_symbols(
    root: &Path,
    prefix: Option<&str>,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let symbols = store.isolated_symbols(prefix);
    print_string_list(&symbols, format)
}

// ── import-graph: get-imports / get-import-tree / get-importers-tree ──────────

pub(crate) fn run_get_imports(root: &Path, path: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let imports = store.imports_of(id);
    let imported_by = store.imported_by(id);
    let value = serde_json::json!({ "imports": imports, "imported_by": imported_by });
    match format {
        Format::Text => {
            println!("# imports");
            for p in &imports {
                println!("  {p}");
            }
            println!("# imported_by");
            for p in &imported_by {
                println!("  {p}");
            }
        }
        Format::Json => println!("{}", serde_json::to_string(&value)?),
    }
    Ok(())
}

fn import_node_to_json(node: &mycelium_core::ImportNode, store: &Store) -> serde_json::Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let imports: Vec<serde_json::Value> = node
        .imports
        .iter()
        .map(|c| import_node_to_json(c, store))
        .collect();
    serde_json::json!({ "path": path, "imports": imports })
}

fn importer_node_to_json(node: &mycelium_core::ImporterNode, store: &Store) -> serde_json::Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let importers: Vec<serde_json::Value> = node
        .importers
        .iter()
        .map(|c| importer_node_to_json(c, store))
        .collect();
    serde_json::json!({ "path": path, "importers": importers })
}

pub(crate) fn run_get_import_tree(
    root: &Path,
    path: &str,
    max_depth: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let tree = store.import_tree(id, max_depth);
    let value = serde_json::json!({ "root": import_node_to_json(&tree, &store) });
    match format {
        Format::Text => println!("{value}"),
        Format::Json => println!("{}", serde_json::to_string(&value)?),
    }
    Ok(())
}

pub(crate) fn run_get_importers_tree(
    root: &Path,
    path: &str,
    max_depth: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let tree = store.importers_tree(id, max_depth);
    let value = serde_json::json!({ "root": importer_node_to_json(&tree, &store) });
    match format {
        Format::Text => println!("{value}"),
        Format::Json => println!("{}", serde_json::to_string(&value)?),
    }
    Ok(())
}

// ── shared output helper ──────────────────────────────────────────────────────

fn print_string_list(items: &[String], format: Format) -> Result<()> {
    match format {
        Format::Text => {
            for item in items {
                println!("{item}");
            }
        }
        Format::Json => println!("{}", serde_json::to_string(items)?),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn run_search_symbol_no_index_errors_clearly() {
        let dir = tempdir().unwrap();
        let err = run_search_symbol(dir.path(), "x", 10, Format::Text).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("no index"), "got: {msg}");
        assert!(msg.contains("mycelium index"), "got: {msg}");
    }

    #[test]
    fn symbol_info_unknown_path_errors() {
        let store = Store::default();
        let err = symbol_info(&store, "nonexistent").unwrap_err();
        assert!(format!("{err}").contains("path not found"));
    }
}
