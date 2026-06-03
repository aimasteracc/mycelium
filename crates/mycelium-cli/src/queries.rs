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

pub(crate) fn run_get_descendants(
    root: &Path,
    path: &str,
    include_inherited: bool,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let descendants = store
        .descendants_of_path(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    if !include_inherited {
        return print_string_list(&descendants, format);
    }
    let inherited = store
        .inherited_descendants_of_path(path)
        .unwrap_or_default();
    match format {
        Format::Json => {
            let value = serde_json::json!({
                "descendants": descendants,
                "inherited_descendants": inherited.into_iter()
                    .map(|(p, from)| serde_json::json!({ "path": p, "from": from }))
                    .collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
        Format::Text => {
            for d in &descendants {
                println!("{d}");
            }
            if !inherited.is_empty() {
                println!("\ninherited:");
                for (p, from) in &inherited {
                    println!("  {p}  (from {from})");
                }
            }
        }
    }
    Ok(())
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
    limit: usize,
    offset: usize,
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
    let all_symbols = store.all_symbols(prefix, kind);
    let page: Vec<String> = all_symbols
        .into_iter()
        .skip(offset)
        .take(if limit == 0 { usize::MAX } else { limit })
        .collect();
    print_string_list(&page, format)
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

pub(crate) fn run_get_callees(
    root: &Path,
    path: &str,
    edge_kind: &str,
    budget: Option<&str>,
    format: Format,
) -> Result<()> {
    use mycelium_core::budget::{BudgetOverride, OutputBudget, apply_budget};

    let kind = parse_edge_kind(edge_kind)?;
    let budget_override = budget
        .map(str::parse::<BudgetOverride>)
        .transpose()
        .map_err(|e| anyhow!(e))?;
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    // Shared core builder → byte-identical with the MCP tool (RFC-0109 Option A).
    let mut value = mycelium_core::queries::callees_payload(&store, id, kind);
    // Budget in JSON mode (parity with the MCP tool) or when `--budget` is
    // explicit. Default text mode prints the full list — no silent truncation
    // of human-facing output (RFC-0102 text-mode rule; CLI text unchanged).
    if matches!(format, Format::Json) || budget_override.is_some() {
        apply_budget(
            &mut value,
            &OutputBudget::resolve(budget_override, store.node_count()),
        );
    }
    print_object_with_list(&value, "callee_paths", format)
}

/// Print a graph-list payload object: the full object in `--format json` (the
/// byte-identical twin of the MCP tool), or just the named list, one item per
/// line, in text mode (RFC-0109 Option A).
fn print_object_with_list(value: &serde_json::Value, list_key: &str, format: Format) -> Result<()> {
    match format {
        Format::Json => println!("{}", serde_json::to_string(value)?),
        Format::Text => {
            if let Some(arr) = value[list_key].as_array() {
                for item in arr {
                    if let Some(s) = item.as_str() {
                        println!("{s}");
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn run_get_callers(
    root: &Path,
    path: &str,
    edge_kind: &str,
    include_virtual: bool,
    budget: Option<&str>,
    format: Format,
) -> Result<()> {
    use mycelium_core::budget::{BudgetOverride, OutputBudget, apply_budget};

    let kind = parse_edge_kind(edge_kind)?;
    let budget_override = budget
        .map(str::parse::<BudgetOverride>)
        .transpose()
        .map_err(|e| anyhow!(e))?;
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    // Shared core builder → byte-identical with the MCP tool (RFC-0109 Option A).
    let mut value =
        mycelium_core::queries::callers_payload(&store, id, path, kind, include_virtual);
    // Budget in JSON mode (parity with the MCP tool) or when `--budget` is
    // explicit. Default text mode prints the full list — no silent truncation
    // of human-facing output (RFC-0102 text-mode rule; CLI text unchanged).
    if matches!(format, Format::Json) || budget_override.is_some() {
        apply_budget(
            &mut value,
            &OutputBudget::resolve(budget_override, store.node_count()),
        );
    }
    print_object_with_list(&value, "caller_paths", format)
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
    edge_kind: Option<&str>,
    budget: Option<&str>,
    format: Format,
) -> Result<()> {
    use mycelium_core::budget::{BudgetOverride, OutputBudget, apply_budget};

    let budget_override = budget
        .map(str::parse::<BudgetOverride>)
        .transpose()
        .map_err(|e| anyhow!(e))?;
    let store = load_index(root)?;
    let symbols = match edge_kind {
        None => store.dead_symbols(prefix),
        Some(ek) => {
            let kind = parse_edge_kind(ek)?;
            store.dead_symbols_for_kind(kind, prefix)
        }
    };
    // Shared core builder → byte-identical with the MCP tool (RFC-0109 Option A).
    let mut value = mycelium_core::queries::dead_symbols_payload(&symbols);
    // Budget in JSON mode (MCP parity) or with explicit --budget; default text
    // prints the full list (RFC-0102 text-mode rule).
    if matches!(format, Format::Json) || budget_override.is_some() {
        apply_budget(
            &mut value,
            &OutputBudget::resolve(budget_override, store.node_count()),
        );
    }
    print_object_with_list(&value, "dead_symbols", format)
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

// ── inheritance: extends / subclasses / implements / implementors ─────────────

fn sorted_paths_for(
    store: &Store,
    id: mycelium_core::types::NodeId,
    kind: EdgeKind,
    outgoing: bool,
) -> Vec<String> {
    let raw = if outgoing {
        store.outgoing(id, kind)
    } else {
        store.incoming(id, kind)
    };
    let mut v: Vec<String> = raw
        .iter()
        .filter_map(|&t| store.path_of(t).map(str::to_owned))
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

pub(crate) fn run_get_extends(root: &Path, path: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let value = serde_json::json!({
        "extends":     sorted_paths_for(&store, id, EdgeKind::Extends, true),
        "extended_by": sorted_paths_for(&store, id, EdgeKind::Extends, false),
    });
    match format {
        Format::Text => println!("{value}"),
        Format::Json => println!("{}", serde_json::to_string(&value)?),
    }
    Ok(())
}

pub(crate) fn run_get_implements(root: &Path, path: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let value = serde_json::json!({
        "implements":     sorted_paths_for(&store, id, EdgeKind::Implements, true),
        "implemented_by": sorted_paths_for(&store, id, EdgeKind::Implements, false),
    });
    match format {
        Format::Text => println!("{value}"),
        Format::Json => println!("{}", serde_json::to_string(&value)?),
    }
    Ok(())
}

fn extends_node_to_json(node: &mycelium_core::ExtendsNode, store: &Store) -> serde_json::Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let parents: Vec<serde_json::Value> = node
        .parents
        .iter()
        .map(|c| extends_node_to_json(c, store))
        .collect();
    serde_json::json!({ "path": path, "parents": parents })
}

fn subclass_node_to_json(node: &mycelium_core::SubclassNode, store: &Store) -> serde_json::Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let subclasses: Vec<serde_json::Value> = node
        .subclasses
        .iter()
        .map(|c| subclass_node_to_json(c, store))
        .collect();
    serde_json::json!({ "path": path, "subclasses": subclasses })
}

fn implements_node_to_json(
    node: &mycelium_core::ImplementsNode,
    store: &Store,
) -> serde_json::Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let interfaces: Vec<serde_json::Value> = node
        .interfaces
        .iter()
        .map(|c| implements_node_to_json(c, store))
        .collect();
    serde_json::json!({ "path": path, "interfaces": interfaces })
}

fn implementor_node_to_json(
    node: &mycelium_core::ImplementorNode,
    store: &Store,
) -> serde_json::Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let implementors: Vec<serde_json::Value> = node
        .implementors
        .iter()
        .map(|c| implementor_node_to_json(c, store))
        .collect();
    serde_json::json!({ "path": path, "implementors": implementors })
}

pub(crate) fn run_extends_tree(
    root: &Path,
    path: &str,
    max_depth: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let tree = store.extends_tree(id, max_depth);
    let value = serde_json::json!({ "root": extends_node_to_json(&tree, &store) });
    print_tree_value(&value, format)
}

pub(crate) fn run_subclasses_tree(
    root: &Path,
    path: &str,
    max_depth: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let tree = store.subclasses_tree(id, max_depth);
    let value = serde_json::json!({ "root": subclass_node_to_json(&tree, &store) });
    print_tree_value(&value, format)
}

pub(crate) fn run_implements_tree(
    root: &Path,
    path: &str,
    max_depth: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let tree = store.implements_tree(id, max_depth);
    let value = serde_json::json!({ "root": implements_node_to_json(&tree, &store) });
    print_tree_value(&value, format)
}

pub(crate) fn run_implementors_tree(
    root: &Path,
    path: &str,
    max_depth: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let tree = store.implementors_tree(id, max_depth);
    let value = serde_json::json!({ "root": implementor_node_to_json(&tree, &store) });
    print_tree_value(&value, format)
}

fn print_tree_value(value: &serde_json::Value, format: Format) -> Result<()> {
    match format {
        Format::Text => println!("{value}"),
        Format::Json => println!("{}", serde_json::to_string(value)?),
    }
    Ok(())
}

pub(crate) fn run_find_extends_path(
    root: &Path,
    from_path: &str,
    to_path: &str,
    max_depth: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let from_id = store
        .lookup(from_path)
        .ok_or_else(|| anyhow!("path not found: {from_path}"))?;
    let to_id = store
        .lookup(to_path)
        .ok_or_else(|| anyhow!("path not found: {to_path}"))?;
    let maybe = store.find_extends_path(from_id, to_id, max_depth);
    let value = maybe.map_or_else(
        || {
            serde_json::json!({
                "path": [],
                "hops": serde_json::Value::Null,
                "message": format!("no extends path found within max_depth={max_depth}"),
            })
        },
        |ids| {
            let path: Vec<String> = ids
                .iter()
                .filter_map(|&id| store.path_of(id).map(str::to_owned))
                .collect();
            let hops = path.len().saturating_sub(1);
            serde_json::json!({ "path": path, "hops": hops })
        },
    );
    print_tree_value(&value, format)
}

pub(crate) fn run_find_implements_path(
    root: &Path,
    from_path: &str,
    to_path: &str,
    max_depth: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let from_id = store
        .lookup(from_path)
        .ok_or_else(|| anyhow!("path not found: {from_path}"))?;
    let to_id = store
        .lookup(to_path)
        .ok_or_else(|| anyhow!("path not found: {to_path}"))?;
    let maybe = store.find_implements_path(from_id, to_id, max_depth);
    let value = maybe.map_or_else(
        || {
            serde_json::json!({
                "path": [],
                "hops": serde_json::Value::Null,
                "message": format!("no implements path found within max_depth={max_depth}"),
            })
        },
        |ids| {
            let path: Vec<String> = ids
                .iter()
                .filter_map(|&id| store.path_of(id).map(str::to_owned))
                .collect();
            let hops = path.len().saturating_sub(1);
            serde_json::json!({ "path": path, "hops": hops })
        },
    );
    print_tree_value(&value, format)
}

// ── reachability: 12 multi-hop tools ──────────────────────────────────────────

fn parse_edge_kind(s: &str) -> Result<EdgeKind> {
    match s.to_ascii_lowercase().as_str() {
        "calls" => Ok(EdgeKind::Calls),
        "imports" => Ok(EdgeKind::Imports),
        "extends" => Ok(EdgeKind::Extends),
        "implements" => Ok(EdgeKind::Implements),
        other => Err(anyhow!(
            "unknown edge_kind '{other}'; expected: calls, imports, extends, implements"
        )),
    }
}

pub(crate) fn run_get_reachable(
    root: &Path,
    path: &str,
    edge_kind: &str,
    max_depth: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let reachable = store.reachable_from(id, kind, max_depth);
    let count = reachable.len();
    let value = serde_json::json!({ "reachable": reachable, "count": count });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_reachable_to(
    root: &Path,
    path: &str,
    edge_kind: &str,
    max_depth: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let reachable = store.reachable_to(id, kind, max_depth);
    let count = reachable.len();
    let value = serde_json::json!({ "reachable": reachable, "count": count });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_k_hop_neighbors(
    root: &Path,
    path: &str,
    k: usize,
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let neighbors = store.k_hop_neighbors(id, kind, k);
    let count = neighbors.len();
    let value = serde_json::json!({ "neighbors": neighbors, "count": count, "k": k });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_two_hop_neighbors(
    root: &Path,
    path: &str,
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let neighbors = store.two_hop_neighbors(id, kind);
    let count = neighbors.len();
    let value = serde_json::json!({ "neighbors": neighbors, "count": count });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_shortest_path(
    root: &Path,
    from_path: &str,
    to_path: &str,
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let from_id = store
        .lookup(from_path)
        .ok_or_else(|| anyhow!("path not found: {from_path}"))?;
    let to_id = store
        .lookup(to_path)
        .ok_or_else(|| anyhow!("path not found: {to_path}"))?;
    let value = store.shortest_path(from_id, to_id, kind).map_or_else(
        || serde_json::json!({ "path": serde_json::Value::Null, "length": serde_json::Value::Null }),
        |p| {
            let length = p.len() - 1;
            serde_json::json!({ "path": p, "length": length })
        },
    );
    print_tree_value(&value, format)
}

pub(crate) fn run_get_symbol_neighborhood(
    root: &Path,
    path: &str,
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let nb = store
        .lookup(path)
        .map_or_else(mycelium_core::SymbolNeighborhood::default, |id| {
            store.symbol_neighborhood(id, kind)
        });
    let incoming_count = nb.incoming.len();
    let outgoing_count = nb.outgoing.len();
    let value = serde_json::json!({
        "path": nb.path,
        "incoming": nb.incoming,
        "outgoing": nb.outgoing,
        "incoming_count": incoming_count,
        "outgoing_count": outgoing_count,
    });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_cross_refs(root: &Path, path: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let refs = store.cross_refs(id);
    let value = serde_json::json!({
        "callers": refs.callers,
        "importers": refs.importers,
        "extended_by": refs.extended_by,
        "implemented_by": refs.implemented_by,
    });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_outgoing_refs(root: &Path, path: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let refs = store.outgoing_refs(id);
    let value = serde_json::json!({
        "callees": refs.callees,
        "imports": refs.imports,
        "extends": refs.extends,
        "implements": refs.implements,
    });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_dependency_depth(
    root: &Path,
    path: &str,
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let depth = store
        .dependency_depth(id, kind)
        .ok_or_else(|| anyhow!("not a symbol node: {path}"))?;
    let value = serde_json::json!({
        "path": path,
        "depth": depth,
        "edge_kind": edge_kind,
    });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_reachable_set(
    root: &Path,
    path: &str,
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let reachable = store.reachable_set(id, kind);
    let count = reachable.len();
    let value = serde_json::json!({ "reachable": reachable, "count": count });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_reaches_into(
    root: &Path,
    path: &str,
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let callers = store.reaches_into(id, kind);
    let count = callers.len();
    let value = serde_json::json!({ "callers": callers, "count": count });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_singly_referenced(
    root: &Path,
    edge_kind: &str,
    limit: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let pairs = store.singly_referenced(kind, limit);
    let count = pairs.len();
    let symbols: Vec<serde_json::Value> = pairs
        .into_iter()
        .map(|(p, ref_by)| serde_json::json!({ "path": p, "referenced_by": ref_by }))
        .collect();
    let value = serde_json::json!({ "symbols": symbols, "count": count });
    print_tree_value(&value, format)
}

// ── centrality: 14 ranking/scoring tools ──────────────────────────────────────

pub(crate) fn run_rank_symbols(
    root: &Path,
    limit: usize,
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let kind = parse_edge_kind(edge_kind)?;
    let store = load_index(root)?;
    let ranked = if kind == EdgeKind::Calls {
        store.top_callee_symbols(limit.min(100))
    } else {
        store.top_symbols_by_incoming(kind, limit.min(100))
    };
    let symbols: Vec<serde_json::Value> = ranked
        .into_iter()
        .map(|(p, c)| serde_json::json!({ "path": p, "caller_count": c }))
        .collect();
    print_tree_value(&serde_json::json!({ "symbols": symbols }), format)
}

pub(crate) fn run_get_top_files(root: &Path, limit: usize, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let entries = store.top_files(limit);
    let count = entries.len();
    let files: Vec<serde_json::Value> = entries
        .into_iter()
        .map(|(p, c)| serde_json::json!({ "path": p, "symbol_count": c }))
        .collect();
    print_tree_value(
        &serde_json::json!({ "files": files, "count": count }),
        format,
    )
}

pub(crate) fn run_get_most_connected(
    root: &Path,
    edge_kind: &str,
    limit: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let entries = store.most_connected(limit, kind);
    let count = entries.len();
    let symbols: Vec<serde_json::Value> = entries
        .into_iter()
        .map(|(p, d)| serde_json::json!({ "path": p, "degree": d }))
        .collect();
    print_tree_value(
        &serde_json::json!({ "symbols": symbols, "count": count }),
        format,
    )
}

pub(crate) fn run_get_hub_symbols(
    root: &Path,
    edge_kind: &str,
    min_in: usize,
    min_out: usize,
    limit: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let hubs = store.hub_symbols(kind, min_in, min_out, limit);
    let count = hubs.len();
    let hubs_json: Vec<serde_json::Value> = hubs
        .into_iter()
        .map(|(p, inn, out)| serde_json::json!({ "path": p, "in_degree": inn, "out_degree": out }))
        .collect();
    print_tree_value(
        &serde_json::json!({ "hubs": hubs_json, "count": count }),
        format,
    )
}

pub(crate) fn run_get_fan_out_rank(
    root: &Path,
    edge_kind: &str,
    limit: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let entries = store.fan_out_rank(kind, limit);
    let count = entries.len();
    let symbols: Vec<serde_json::Value> = entries
        .into_iter()
        .map(|(p, d)| serde_json::json!({ "path": p, "out_degree": d }))
        .collect();
    print_tree_value(
        &serde_json::json!({ "symbols": symbols, "count": count }),
        format,
    )
}

pub(crate) fn run_get_fan_in_rank(
    root: &Path,
    edge_kind: &str,
    limit: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let entries = store.fan_in_rank(kind, limit);
    let count = entries.len();
    let symbols: Vec<serde_json::Value> = entries
        .into_iter()
        .map(|(p, d)| serde_json::json!({ "path": p, "in_degree": d }))
        .collect();
    print_tree_value(
        &serde_json::json!({ "symbols": symbols, "count": count }),
        format,
    )
}

pub(crate) fn run_betweenness_centrality(
    root: &Path,
    edge_kind: &str,
    top_n: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let entries = store.betweenness_centrality(kind);
    let symbol_count = entries.len();
    let nodes: Vec<serde_json::Value> = entries
        .into_iter()
        .take(top_n)
        .map(|e| serde_json::json!({ "path": e.path, "score": e.score }))
        .collect();
    print_tree_value(
        &serde_json::json!({ "nodes": nodes, "symbol_count": symbol_count, "top_n": top_n }),
        format,
    )
}

pub(crate) fn run_closeness_centrality(
    root: &Path,
    edge_kind: &str,
    top_n: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let entries = store.closeness_centrality(kind);
    let symbol_count = entries.len();
    let nodes: Vec<serde_json::Value> = entries
        .into_iter()
        .take(top_n)
        .map(|e| serde_json::json!({ "path": e.path, "score": e.score }))
        .collect();
    print_tree_value(
        &serde_json::json!({ "nodes": nodes, "symbol_count": symbol_count, "top_n": top_n }),
        format,
    )
}

pub(crate) fn run_degree_centrality(
    root: &Path,
    edge_kind: &str,
    sort_by: &str,
    top_n: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    if sort_by != "in" && sort_by != "out" {
        return Err(anyhow!(
            "unknown sort_by: {sort_by}; expected 'in' or 'out'"
        ));
    }
    let mut entries = store.degree_centrality(kind);
    let symbol_count = entries.len();
    if sort_by == "out" {
        entries.sort_by(|a, b| {
            b.out_centrality
                .partial_cmp(&a.out_centrality)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.path.cmp(&b.path))
        });
    }
    let nodes: Vec<serde_json::Value> = entries
        .into_iter()
        .take(top_n)
        .map(|e| {
            serde_json::json!({
                "path": e.path,
                "in_degree":     e.in_degree,
                "out_degree":    e.out_degree,
                "in_centrality": e.in_centrality,
                "out_centrality":e.out_centrality,
            })
        })
        .collect();
    print_tree_value(
        &serde_json::json!({
            "nodes": nodes, "symbol_count": symbol_count, "top_n": top_n, "sort_by": sort_by,
        }),
        format,
    )
}

pub(crate) fn run_clustering_coefficient(
    root: &Path,
    path: &str,
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let (coefficient, neighbor_count, neighbor_edge_count) =
        store.clustering_coefficient_stats(id, kind);
    print_tree_value(
        &serde_json::json!({
            "coefficient": coefficient,
            "neighbor_count": neighbor_count,
            "neighbor_edge_count": neighbor_edge_count,
        }),
        format,
    )
}

pub(crate) fn run_eccentricity(
    root: &Path,
    path: &str,
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let (eccentricity, reachable_count) = store.eccentricity_stats(id, kind);
    print_tree_value(
        &serde_json::json!({
            "eccentricity": eccentricity,
            "reachable_count": reachable_count,
        }),
        format,
    )
}

pub(crate) fn run_page_rank(
    root: &Path,
    edge_kind: &str,
    damping: f64,
    iterations: usize,
    top_n: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let entries = store.page_rank(kind, damping, iterations);
    let symbol_count = entries.len();
    let nodes: Vec<serde_json::Value> = entries
        .into_iter()
        .take(top_n)
        .map(|e| serde_json::json!({ "path": e.path, "score": e.score }))
        .collect();
    print_tree_value(
        &serde_json::json!({ "nodes": nodes, "symbol_count": symbol_count, "top_n": top_n }),
        format,
    )
}

pub(crate) fn run_harmonic_centrality(
    root: &Path,
    path: &str,
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let (harmonic_centrality, reachable_count, symbol_count) =
        store.harmonic_centrality_stats(id, kind);
    print_tree_value(
        &serde_json::json!({
            "harmonic_centrality": harmonic_centrality,
            "reachable_count": reachable_count,
            "symbol_count": symbol_count,
        }),
        format,
    )
}

pub(crate) fn run_neighbor_similarity(
    root: &Path,
    path1: &str,
    path2: &str,
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let id1 = store
        .lookup(path1)
        .ok_or_else(|| anyhow!("path not found: {path1}"))?;
    let id2 = store
        .lookup(path2)
        .ok_or_else(|| anyhow!("path not found: {path2}"))?;
    let (similarity, shared, total) = store.neighbor_similarity_stats(id1, id2, kind);
    print_tree_value(
        &serde_json::json!({ "similarity": similarity, "shared": shared, "total": total }),
        format,
    )
}

// ── graph-structure: 14 structural-analysis tools ────────────────────────────

pub(crate) fn run_get_stats(root: &Path, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let stats = store.graph_stats();
    let value = serde_json::json!({
        "total_nodes":   stats.total_nodes,
        "total_edges":   stats.total_edges,
        "nodes_by_kind": stats.nodes_by_kind,
        "edges_by_kind": stats.edges_by_kind,
    });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_graph_metrics(root: &Path, edge_kind: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let m = store.graph_metrics(kind);
    let value = serde_json::json!({
        "symbol_count":        m.symbol_count,
        "directed_edge_count": m.directed_edge_count,
        "density":             m.density,
        "avg_degree":          m.avg_degree,
        "max_in_degree":       m.max_in_degree,
        "max_out_degree":      m.max_out_degree,
    });
    print_tree_value(&value, format)
}

pub(crate) fn run_detect_cycles(
    root: &Path,
    edge_kind: &str,
    prefix: Option<&str>,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let nodes = store.nodes_in_cycles(kind, prefix);
    let count = nodes.len();
    let value = serde_json::json!({ "cycle_nodes": nodes, "count": count });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_scc_groups(root: &Path, edge_kind: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let groups = store.scc_groups(kind);
    let group_count = groups.len();
    let total_symbols: usize = groups.iter().map(Vec::len).sum();
    let value = serde_json::json!({
        "groups": groups,
        "group_count": group_count,
        "total_symbols": total_symbols,
    });
    print_tree_value(&value, format)
}

pub(crate) fn run_topological_sort(root: &Path, edge_kind: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let order = store.topological_sort(kind);
    let ordered_count = order.order.len();
    let cycle_count = order.cycle_members.len();
    let value = serde_json::json!({
        "order": order.order,
        "cycle_members": order.cycle_members,
        "ordered_count": ordered_count,
        "cycle_count": cycle_count,
    });
    print_tree_value(&value, format)
}

pub(crate) fn run_find_articulation_points(
    root: &Path,
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let points = store.articulation_points(kind);
    let count = points.len();
    let value = serde_json::json!({ "points": points, "count": count });
    print_tree_value(&value, format)
}

pub(crate) fn run_find_bridge_edges(root: &Path, edge_kind: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let bridges = store.bridge_edges(kind);
    let count = bridges.len();
    let bridge_list: Vec<serde_json::Value> = bridges
        .into_iter()
        .map(|(from, to)| serde_json::json!({ "from": from, "to": to }))
        .collect();
    let value = serde_json::json!({ "bridges": bridge_list, "count": count });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_biconnected_components(
    root: &Path,
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let comps = store.biconnected_components(kind);
    let component_count = comps.len();
    let total_symbols: usize = comps.iter().map(Vec::len).sum();
    let value = serde_json::json!({
        "components": comps,
        "component_count": component_count,
        "total_symbols": total_symbols,
    });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_k_core(root: &Path, edge_kind: &str, k: usize, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let core = store.k_core(kind, k);
    let count = core.len();
    let value = serde_json::json!({ "core": core, "count": count, "k": k });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_dependency_layers(
    root: &Path,
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let layers = store.dependency_layers(kind);
    let all_symbol_count = store.all_symbols(None, None).len();
    let layer_count = layers.len();
    let total_symbols: usize = layers.iter().map(Vec::len).sum();
    let cycle_excluded_count = all_symbol_count.saturating_sub(total_symbols);
    let value = serde_json::json!({
        "layers": layers,
        "layer_count": layer_count,
        "total_symbols": total_symbols,
        "cycle_excluded_count": cycle_excluded_count,
    });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_scc(
    root: &Path,
    edge_kind: &str,
    min_size: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let all_sccs = store.strongly_connected_components(kind);
    let symbol_count: usize = all_sccs.iter().map(|e| e.size).sum();
    let total_components = all_sccs.len();
    let components: Vec<serde_json::Value> = all_sccs
        .into_iter()
        .filter(|e| e.size >= min_size)
        .map(|e| serde_json::json!({ "members": e.members, "size": e.size }))
        .collect();
    let value = serde_json::json!({
        "components": components,
        "total_components": total_components,
        "symbol_count": symbol_count,
        "min_size": min_size,
    });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_wcc(
    root: &Path,
    edge_kind: &str,
    min_size: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let all = store.weakly_connected_components(kind);
    let min_size = min_size.max(1);
    let components: Vec<Vec<String>> = all.into_iter().filter(|c| c.len() >= min_size).collect();
    let component_count = components.len();
    let total_symbols: usize = components.iter().map(Vec::len).sum();
    let value = serde_json::json!({
        "components": components,
        "component_count": component_count,
        "total_symbols": total_symbols,
    });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_degree_histogram(root: &Path, edge_kind: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let hist = store.degree_histogram(kind);
    let total_symbols: u64 = hist.in_degrees.iter().map(|&(_, c)| c).sum();
    let in_list: Vec<serde_json::Value> = hist
        .in_degrees
        .iter()
        .map(|&(d, c)| serde_json::json!({ "degree": d, "count": c }))
        .collect();
    let out_list: Vec<serde_json::Value> = hist
        .out_degrees
        .iter()
        .map(|&(d, c)| serde_json::json!({ "degree": d, "count": c }))
        .collect();
    let value = serde_json::json!({
        "in_degrees": in_list,
        "out_degrees": out_list,
        "total_symbols": total_symbols,
    });
    print_tree_value(&value, format)
}

pub(crate) fn run_find_cycle_members(root: &Path, edge_kind: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let members = store.cycle_members(kind);
    let count = members.len();
    let value = serde_json::json!({ "members": members, "count": count });
    print_tree_value(&value, format)
}

// ── batch-ops: 4 token-efficient batch tools ─────────────────────────────────

#[allow(
    clippy::similar_names,
    reason = "callers/callees are canonical field names matched by the MCP tool"
)]
fn symbol_info_value(store: &Store, path: &str) -> serde_json::Value {
    let Some(id) = store.lookup(path) else {
        return serde_json::json!({ "path": path, "error": "path not found" });
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
    serde_json::json!({
        "path": path,
        "ancestors": ancestors,
        "descendants": descendants,
        "callers": callers,
        "callees": callees,
    })
}

pub(crate) fn run_batch_symbol_info(root: &Path, paths: &[String], format: Format) -> Result<()> {
    let store = load_index(root)?;
    let symbols: Vec<serde_json::Value> = paths
        .iter()
        .take(50)
        .map(|p| symbol_info_value(&store, p))
        .collect();
    let value = serde_json::json!({ "symbols": symbols });
    print_tree_value(&value, format)
}

pub(crate) fn run_batch_node_degree(root: &Path, paths: &[String], format: Format) -> Result<()> {
    let store = load_index(root)?;
    let degrees: Vec<serde_json::Value> = paths
        .iter()
        .take(50)
        .map(|p| {
            store.lookup(p).map_or_else(
                || serde_json::json!({ "path": p, "error": "path not found" }),
                |id| {
                    let d = store.node_degree(id);
                    serde_json::json!({
                        "path": p,
                        "in_calls":       d.in_calls,
                        "out_calls":      d.out_calls,
                        "in_imports":     d.in_imports,
                        "out_imports":    d.out_imports,
                        "in_extends":     d.in_extends,
                        "out_extends":    d.out_extends,
                        "in_implements":  d.in_implements,
                        "out_implements": d.out_implements,
                    })
                },
            )
        })
        .collect();
    let count = degrees.len();
    let value = serde_json::json!({ "degrees": degrees, "count": count });
    print_tree_value(&value, format)
}

pub(crate) fn run_batch_reachable_from(
    root: &Path,
    paths: &[String],
    edge_kind: &str,
    max_depth: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let ids: Vec<_> = paths
        .iter()
        .take(20)
        .filter_map(|p| store.lookup(p))
        .collect();
    let reachable = store.batch_reachable_from(&ids, kind, max_depth);
    let count = reachable.len();
    let value = serde_json::json!({ "reachable": reachable, "count": count });
    print_tree_value(&value, format)
}

pub(crate) fn run_batch_reachable_to(
    root: &Path,
    paths: &[String],
    edge_kind: &str,
    max_depth: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let ids: Vec<_> = paths
        .iter()
        .take(20)
        .filter_map(|p| store.lookup(p))
        .collect();
    let reachable = store.batch_reachable_to(&ids, kind, max_depth);
    let count = reachable.len();
    let value = serde_json::json!({ "reachable": reachable, "count": count });
    print_tree_value(&value, format)
}

// ── batch 10: 11 remaining cross-category commands ───────────────────────────

pub(crate) fn run_get_node_degree(root: &Path, path: &str, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let id = store
        .lookup(path)
        .ok_or_else(|| anyhow!("path not found: {path}"))?;
    let d = store.node_degree(id);
    let value = serde_json::json!({
        "in_calls":       d.in_calls,
        "out_calls":      d.out_calls,
        "in_imports":     d.in_imports,
        "out_imports":    d.out_imports,
        "in_extends":     d.in_extends,
        "out_extends":    d.out_extends,
        "in_implements":  d.in_implements,
        "out_implements": d.out_implements,
    });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_files(root: &Path, path_prefix: Option<&str>, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let mut files = store.all_file_paths();
    if let Some(prefix) = path_prefix {
        files.retain(|p| p.starts_with(prefix));
    }
    let value = serde_json::json!({ "files": files });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_symbol_count_by_kind(root: &Path, format: Format) -> Result<()> {
    let store = load_index(root)?;
    let counts = store.symbol_count_by_kind();
    let total: usize = counts.iter().map(|(_, n)| n).sum();
    let kinds: Vec<serde_json::Value> = counts
        .into_iter()
        .map(|(k, c)| serde_json::json!({ "kind": k, "count": c }))
        .collect();
    let value = serde_json::json!({ "kinds": kinds, "total": total });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_leaf_symbols(
    root: &Path,
    edge_kind: &str,
    limit: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let symbols = store.leaf_symbols(kind, limit);
    let count = symbols.len();
    let value = serde_json::json!({ "symbols": symbols, "count": count });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_common_callers(
    root: &Path,
    paths: &[String],
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    if paths.is_empty() {
        let value = serde_json::json!({ "callers": [], "count": 0 });
        return print_tree_value(&value, format);
    }
    let mut ids = Vec::with_capacity(paths.len());
    for p in paths {
        let id = store
            .lookup(p)
            .ok_or_else(|| anyhow!("path not found: {p}"))?;
        ids.push(id);
    }
    let callers = store.common_callers(&ids, kind);
    let count = callers.len();
    let value = serde_json::json!({ "callers": callers, "count": count });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_common_callees(
    root: &Path,
    paths: &[String],
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    if paths.is_empty() {
        let value = serde_json::json!({ "callees": [], "count": 0 });
        return print_tree_value(&value, format);
    }
    let mut ids = Vec::with_capacity(paths.len());
    for p in paths {
        let id = store
            .lookup(p)
            .ok_or_else(|| anyhow!("path not found: {p}"))?;
        ids.push(id);
    }
    let callees = store.common_callees(&ids, kind);
    let count = callees.len();
    let value = serde_json::json!({ "callees": callees, "count": count });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_common_reachable(
    root: &Path,
    path1: &str,
    path2: &str,
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let id1 = store
        .lookup(path1)
        .ok_or_else(|| anyhow!("path not found: {path1}"))?;
    let id2 = store
        .lookup(path2)
        .ok_or_else(|| anyhow!("path not found: {path2}"))?;
    let common = store.common_reachable(id1, id2, kind);
    let count = common.len();
    let value = serde_json::json!({ "common": common, "count": count });
    print_tree_value(&value, format)
}

pub(crate) fn run_get_mutual_reachability(
    root: &Path,
    path1: &str,
    path2: &str,
    edge_kind: &str,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let kind = parse_edge_kind(edge_kind)?;
    let id1 = store
        .lookup(path1)
        .ok_or_else(|| anyhow!("path not found: {path1}"))?;
    let id2 = store
        .lookup(path2)
        .ok_or_else(|| anyhow!("path not found: {path2}"))?;
    let result = store.mutual_reachability(id1, id2, kind);
    let value = serde_json::json!({
        "forward": result.forward,
        "backward": result.backward,
        "mutual": result.mutual,
        "forward_distance": result.forward_distance,
        "backward_distance": result.backward_distance,
    });
    print_tree_value(&value, format)
}

fn path_or_unreachable(
    maybe: Option<Vec<mycelium_core::types::NodeId>>,
    store: &Store,
    max_depth: usize,
    kind_label: &str,
) -> serde_json::Value {
    maybe.map_or_else(
        || {
            serde_json::json!({
                "path": [],
                "hops": serde_json::Value::Null,
                "message": format!("no {kind_label} path found within depth {max_depth}"),
            })
        },
        |ids| {
            let path: Vec<String> = ids
                .iter()
                .filter_map(|&id| store.path_of(id).map(str::to_owned))
                .collect();
            let hops = path.len().saturating_sub(1);
            serde_json::json!({ "path": path, "hops": hops })
        },
    )
}

pub(crate) fn run_find_call_path(
    root: &Path,
    from_path: &str,
    to_path: &str,
    max_depth: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let from_id = store
        .lookup(from_path)
        .ok_or_else(|| anyhow!("path not found: {from_path}"))?;
    let to_id = store
        .lookup(to_path)
        .ok_or_else(|| anyhow!("path not found: {to_path}"))?;
    let maybe = store.find_call_path(from_id, to_id, max_depth);
    let value = path_or_unreachable(maybe, &store, max_depth, "call");
    print_tree_value(&value, format)
}

pub(crate) fn run_find_import_path(
    root: &Path,
    from_path: &str,
    to_path: &str,
    max_depth: usize,
    format: Format,
) -> Result<()> {
    let store = load_index(root)?;
    let from_id = store
        .lookup(from_path)
        .ok_or_else(|| anyhow!("path not found: {from_path}"))?;
    let to_id = store
        .lookup(to_path)
        .ok_or_else(|| anyhow!("path not found: {to_path}"))?;
    let maybe = store.find_import_path(from_id, to_id, max_depth);
    let value = path_or_unreachable(maybe, &store, max_depth, "import");
    print_tree_value(&value, format)
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

// ── context (RFC-0101) ────────────────────────────────────────────────────────

// Candidate extraction, stop-words, and the path/payload helpers used to live
// here as a near-duplicate of the MCP tool (with a *different* tokenizer — the
// exact RFC-0101 parity bug). They now live once in `mycelium_core::context`,
// which both surfaces call, so CLI and MCP JSON are identical by construction.

fn print_context_text(task: &str, value: &serde_json::Value) {
    let arr = |k: &str| value[k].as_array().cloned().unwrap_or_default();
    let str_at = |v: &serde_json::Value, k: &str| v[k].as_str().unwrap_or("").to_owned();

    println!("task: {task}");
    println!("verdict: {}", value["verdict"].as_str().unwrap_or(""));
    println!("routing: {}", value["routing"].as_str().unwrap_or(""));

    let eps = arr("entry_points");
    println!("entry_points ({}):", eps.len());
    for ep in &eps {
        println!("  {}", ep.as_str().unwrap_or(""));
    }
    let nodes = arr("nodes");
    println!("nodes ({}):", nodes.len());
    for n in &nodes {
        println!("  {}", str_at(n, "path"));
    }
    let edges = arr("edges");
    println!("edges ({}):", edges.len());
    for e in &edges {
        println!(
            "  {} --{}--> {}",
            str_at(e, "source"),
            str_at(e, "kind"),
            str_at(e, "target")
        );
    }
    let related = arr("related_files");
    println!("related_files ({}):", related.len());
    for f in &related {
        println!("  {}", f.as_str().unwrap_or(""));
    }
    let blocks = arr("code_blocks");
    println!("code_blocks ({}):", blocks.len());
    for b in &blocks {
        println!("  {}", str_at(b, "file"));
    }
    println!(
        "summary: {}",
        value["agent_summary"]["summary_line"]
            .as_str()
            .unwrap_or("")
    );
}

pub(crate) fn run_context(
    root: &Path,
    task: &str,
    max_nodes: Option<usize>,
    max_code_blocks: Option<usize>,
    edge_kinds: &[String],
    budget: Option<&str>,
    format: Format,
) -> Result<()> {
    use mycelium_core::budget::BudgetOverride;
    use mycelium_core::context::{self, ContextOptions, Routing};

    // Per-call budget override (RFC-0102) — parsed via the same core `FromStr`
    // the MCP tool uses, so both surfaces resolve the identical budget. An
    // invalid value fails fast (mirrors the MCP application error).
    let budget_override = budget
        .map(str::parse::<BudgetOverride>)
        .transpose()
        .map_err(|e| anyhow::anyhow!(e))?;

    let store = load_index(root)?;
    let max_n = max_nodes.unwrap_or(30).min(100);
    let max_b = max_code_blocks.unwrap_or(6).min(25);
    let kinds: Vec<EdgeKind> = edge_kinds
        .iter()
        .filter_map(|s| context::parse_edge_kind(s))
        .collect();
    let opts = ContextOptions {
        max_nodes: max_n,
        max_code_blocks: max_b,
        edge_kinds: kinds,
    };

    // Same routing logic as the MCP tool, so the two produce identical JSON.
    let natural = |store: &Store| {
        let c = context::extract_symbol_candidates(task);
        let e = context::seed_entry_points(store, &c, max_n);
        (Routing::Natural, c, e)
    };
    let (routing, candidates, entry_points) = if context::looks_like_hyphae(task) {
        mycelium_hyphae::parse(task).map_or_else(
            |_| natural(&store),
            |ast| {
                let eps = mycelium_hyphae::evaluator::Evaluator::new(&store)
                    .eval(&ast)
                    .into_iter()
                    .take(max_n)
                    .collect::<Vec<String>>();
                (Routing::Hyphae, Vec::new(), eps)
            },
        )
    } else {
        natural(&store)
    };
    let mut value =
        context::build_payload(&store, task, &candidates, &entry_points, routing, &opts);
    // Same resolution as the MCP tool over the same payload → byte-identical
    // JSON (RFC-0102 / Three-Surface Rule).
    mycelium_core::budget::apply_budget(
        &mut value,
        &mycelium_core::budget::OutputBudget::resolve(budget_override, store.node_count()),
    );

    match format {
        Format::Json => println!("{}", serde_json::to_string(&value)?),
        Format::Text => print_context_text(task, &value),
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

    // RFC-0101 Phase 2 — CLI twin for mycelium_context (RED-first)

    #[test]
    fn run_context_no_index_errors_clearly() {
        let dir = tempdir().unwrap();
        let err = run_context(
            dir.path(),
            "trace foo to bar",
            None,
            None,
            &[],
            None,
            Format::Json,
        )
        .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("no index"), "got: {msg}");
    }

    #[test]
    fn run_context_gibberish_returns_not_found_json() {
        let dir = tempdir().unwrap();
        let index_dir = dir.path().join(".mycelium");
        std::fs::create_dir_all(&index_dir).unwrap();
        let store = Store::default();
        store.save(&index_dir.join("index.rmp")).unwrap();

        let output =
            capture_context_output(dir.path(), "xyzzy_nonexistent_gibberish_xyz", None, None);
        let v: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(v["verdict"], "NOT_FOUND");
    }

    #[test]
    fn run_context_json_output_has_required_keys() {
        let dir = tempdir().unwrap();
        let index_dir = dir.path().join(".mycelium");
        std::fs::create_dir_all(&index_dir).unwrap();
        let store = Store::default();
        store.save(&index_dir.join("index.rmp")).unwrap();

        let output = capture_context_output(dir.path(), "some task", None, None);
        let v: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(v.get("entry_points").is_some(), "missing entry_points");
        assert!(v.get("nodes").is_some(), "missing nodes");
        assert!(v.get("edges").is_some(), "missing edges");
        assert!(v.get("code_blocks").is_some(), "missing code_blocks");
        assert!(v.get("stats").is_some(), "missing stats");
        assert!(v.get("agent_summary").is_some(), "missing agent_summary");
    }

    fn capture_context_output(
        root: &Path,
        task: &str,
        max_nodes: Option<usize>,
        max_code_blocks: Option<usize>,
    ) -> String {
        use mycelium_core::context::{self, ContextOptions, Routing};
        let store = load_index(root).unwrap();
        let max_n = max_nodes.unwrap_or(30).min(100);
        let candidates = context::extract_symbol_candidates(task);
        let entry_points = context::seed_entry_points(&store, &candidates, max_n);
        let opts = ContextOptions {
            max_nodes: max_n,
            max_code_blocks: max_code_blocks.unwrap_or(6).min(25),
            edge_kinds: vec![],
        };
        let value = context::build_payload(
            &store,
            task,
            &candidates,
            &entry_points,
            Routing::Natural,
            &opts,
        );
        serde_json::to_string(&value).unwrap()
    }
}
