//! Shared one-shot architecture-context builder (RFC-0101).
//!
//! This is the **single source of truth** for the `mycelium_context` capability.
//! Both the MCP tool (`mycelium-mcp`) and the CLI twin (`mycelium context`)
//! call into here, so their JSON output is byte-identical by construction —
//! satisfying the Three-Surface Rule (Charter §5.13 / RFC-0090) rather than
//! maintaining two hand-synced copies that drift (the pre-RFC-0101 state, where
//! `extract_symbol_candidates` had diverged between the two surfaces).
//!
//! The natural-language → candidate extraction lives here. The Hyphae-selector
//! routing lives in each surface (the `mycelium-hyphae` evaluator depends on
//! `mycelium-core`, so core cannot depend back on it); each surface seeds
//! `entry_points` and calls [`build_payload`] with `Routing::Hyphae`.

use serde_json::{Value, json};

use crate::store::Store;
use crate::types::EdgeKind;

/// Words that are never useful symbol candidates on their own.
const CONTEXT_STOP_WORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "by", "call", "calls", "does", "flow", "for", "from",
    "how", "in", "into", "is", "of", "on", "or", "through", "to", "trace", "what", "when", "where",
    "which", "why", "with", "work", "works",
];

/// How the entry points for a context request were resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routing {
    /// Natural-language task → [`extract_symbol_candidates`] → symbol search.
    Natural,
    /// The task parsed as a Hyphae selector and was evaluated by the DSL engine.
    Hyphae,
}

impl Routing {
    /// Wire string for the `routing` response field (`"natural"` / `"hyphae"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Natural => "natural",
            Self::Hyphae => "hyphae",
        }
    }
}

/// Knobs for [`build_payload`].
///
/// `edge_kinds` controls which adjacency kinds the one-hop expansion walks
/// (RFC-0101 `edge_kinds` request field); an empty list defaults to `Calls`.
#[derive(Debug, Clone)]
pub struct ContextOptions {
    /// Maximum graph nodes to return (entry points + one-hop expansion).
    pub max_nodes: usize,
    /// Maximum source snippets (one per distinct file).
    pub max_code_blocks: usize,
    /// Edge kinds to expand. Empty ⇒ `[Calls]`.
    pub edge_kinds: Vec<EdgeKind>,
}

impl ContextOptions {
    /// Resolve the effective edge-kind list (never empty).
    fn effective_edge_kinds(&self) -> Vec<EdgeKind> {
        if self.edge_kinds.is_empty() {
            vec![EdgeKind::Calls]
        } else {
            self.edge_kinds.clone()
        }
    }
}

/// Extract candidate symbol tokens from a natural-language task.
///
/// Dependency-free equivalent of the prior regex tokenizer: split on any
/// non-`[A-Za-z0-9_]` character, drop tokens starting with a digit, then keep
/// tokens that are ≥ 3 chars, not a stop word, and have "structure"
/// (an underscore, an uppercase letter, or length ≥ 4).
#[must_use]
pub fn extract_symbol_candidates(task: &str) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for token in task.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
        if token.is_empty() {
            continue;
        }
        // Match the old `[A-Za-z_]`-anchored identifier rule: skip digit-led.
        if token.as_bytes()[0].is_ascii_digit() {
            continue;
        }
        if token.len() < 3 {
            continue;
        }
        let lower = token.to_ascii_lowercase();
        if CONTEXT_STOP_WORDS.contains(&lower.as_str()) {
            continue;
        }
        let has_structure = token.contains('_')
            || token.chars().any(|c| c.is_ascii_uppercase())
            || token.len() >= 4;
        if !has_structure {
            continue;
        }
        if seen.insert(token.to_owned()) {
            out.push(token.to_owned());
        }
    }
    out
}

/// Parse an edge-kind wire name (case-insensitive) into an [`EdgeKind`].
///
/// Shared by both surfaces so `edge_kinds` is interpreted identically. Returns
/// `None` for an unknown name (the caller silently drops it).
#[must_use]
pub fn parse_edge_kind(name: &str) -> Option<EdgeKind> {
    let lower = name.trim().to_ascii_lowercase();
    [
        EdgeKind::Contains,
        EdgeKind::Calls,
        EdgeKind::Imports,
        EdgeKind::TypeImports,
        EdgeKind::Exports,
        EdgeKind::Extends,
        EdgeKind::Implements,
        EdgeKind::References,
        EdgeKind::TypeOf,
        EdgeKind::Returns,
        EdgeKind::Instantiates,
        EdgeKind::Overrides,
        EdgeKind::Decorates,
        EdgeKind::Aggregates,
        EdgeKind::Composes,
        EdgeKind::Uses,
    ]
    .into_iter()
    .find(|k| k.as_str() == lower)
}

/// Does this task look like a Hyphae selector rather than prose?
///
/// Shared by both surfaces so the routing decision is identical
/// (RFC-0101 §classify). True when it carries a selector sigil: an id
/// (`#Foo`), an attribute (`[lang=rust]`), or a `:pseudo(` class.
#[must_use]
pub fn looks_like_hyphae(task: &str) -> bool {
    if task.contains('#') || task.contains('[') {
        return true;
    }
    // ":pseudo" immediately followed by an ASCII letter, e.g. `function:calls(...)`.
    task.find(':')
        .and_then(|i| task[i + 1..].chars().next())
        .is_some_and(|c| c.is_ascii_alphabetic())
}

/// Seed entry points by symbol-searching each candidate, deduped and capped.
#[must_use]
pub fn seed_entry_points(store: &Store, candidates: &[String], max_nodes: usize) -> Vec<String> {
    let mut eps: Vec<String> = Vec::new();
    for candidate in candidates.iter().take(10) {
        let matches = store.search_symbol(candidate, std::cmp::max(5, max_nodes / 3));
        for m in matches {
            if !eps.contains(&m) {
                eps.push(m);
            }
            if eps.len() >= max_nodes {
                return eps;
            }
        }
    }
    eps
}

fn path_leaf_name(trunk_path: &str) -> &str {
    trunk_path
        .rsplit('>')
        .next()
        .unwrap_or(trunk_path)
        .rsplit("::")
        .next()
        .unwrap_or(trunk_path)
}

fn file_part(trunk_path: &str) -> &str {
    trunk_path.split('>').next().unwrap_or(trunk_path)
}

/// One-hop expansion: starting from `entry_points`, walk `edge_kinds` in both
/// directions to build the node and edge lists (capped at `max_nodes`).
fn expand_one_hop(
    store: &Store,
    entry_points: &[String],
    max_nodes: usize,
    edge_kinds: &[EdgeKind],
) -> (Vec<Value>, Vec<Value>) {
    let mut nodes: Vec<Value> = entry_points
        .iter()
        .take(max_nodes)
        .map(|p| json!({ "id": p, "name": path_leaf_name(p), "path": p }))
        .collect();
    let mut edges: Vec<Value> = Vec::new();
    let mut seen: std::collections::BTreeSet<(String, String, &'static str)> =
        std::collections::BTreeSet::new();
    let push_node = |nodes: &mut Vec<Value>, p: &str| {
        if !nodes.iter().any(|n| n["path"] == p) {
            nodes.push(json!({ "id": p, "name": path_leaf_name(p), "path": p }));
        }
    };

    'outer: for ep in entry_points.iter().take(max_nodes) {
        let Some(id) = store.lookup(ep) else { continue };
        for &kind in edge_kinds {
            let label = kind.as_str();
            for &cid in store.outgoing(id, kind) {
                if nodes.len() >= max_nodes {
                    break 'outer;
                }
                let Some(cp) = store.path_of(cid) else {
                    continue;
                };
                let cp = cp.to_owned();
                push_node(&mut nodes, &cp);
                if seen.insert((ep.clone(), cp.clone(), label)) {
                    edges.push(json!({ "source": ep, "target": cp, "kind": label }));
                }
            }
            for &cid in store.incoming(id, kind) {
                if nodes.len() >= max_nodes {
                    break 'outer;
                }
                let Some(cp) = store.path_of(cid) else {
                    continue;
                };
                let cp = cp.to_owned();
                push_node(&mut nodes, &cp);
                if seen.insert((cp.clone(), ep.clone(), label)) {
                    edges.push(json!({ "source": cp, "target": ep, "kind": label }));
                }
            }
        }
    }
    (nodes, edges)
}

/// Distinct file parts across `nodes`, in first-seen order.
fn collect_related_files(nodes: &[Value]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for node in nodes {
        let fp = file_part(node["path"].as_str().unwrap_or("")).to_owned();
        if !fp.is_empty() && seen.insert(fp.clone()) {
            out.push(fp);
        }
    }
    out
}

/// One code snippet (file + symbol + span) per distinct file, capped at `max`.
fn collect_code_blocks(store: &Store, nodes: &[Value], max: usize) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for node in nodes {
        if out.len() >= max {
            break;
        }
        let path_str = node["path"].as_str().unwrap_or("");
        let fp = file_part(path_str).to_owned();
        if !seen.insert(fp.clone()) {
            continue;
        }
        let Some(id) = store.lookup(path_str) else {
            continue;
        };
        let span = store.span_of(id);
        out.push(json!({
            "file": fp,
            "symbol": path_leaf_name(path_str),
            "span": span.map_or(Value::Null, |s| json!({
                "start_line": s.start_line, "start_col": s.start_col,
                "end_line": s.end_line, "end_col": s.end_col,
            }))
        }));
    }
    out
}

/// Build the full seven-key context payload (RFC-0101 response contract).
///
/// Keys: `entry_points`, `nodes`, `edges`, `code_blocks`, `related_files`,
/// `stats`, `agent_summary` (plus `success`, `verdict`, `task`, `candidates`,
/// `routing`). `entry_points` are pre-resolved by the caller so the Hyphae path
/// can supply them directly. An empty list yields a `NOT_FOUND` payload.
#[must_use]
pub fn build_payload(
    store: &Store,
    task: &str,
    candidates: &[String],
    entry_points: &[String],
    routing: Routing,
    opts: &ContextOptions,
) -> Value {
    if entry_points.is_empty() {
        return json!({
            "success": true,
            "verdict": "NOT_FOUND",
            "task": task,
            "routing": routing.as_str(),
            "candidates": candidates,
            "entry_points": [],
            "nodes": [],
            "edges": [],
            "code_blocks": [],
            "related_files": [],
            "stats": { "entry_points": 0, "nodes": 0, "edges": 0, "code_blocks": 0, "related_files": 0 },
            "agent_summary": {
                "summary_line": "mycelium_context: no entry points found",
                "verdict": "NOT_FOUND",
                "next_step": "Try mycelium_search_symbol with an exact symbol name or broaden the task."
            }
        });
    }

    let edge_kinds = opts.effective_edge_kinds();
    let (nodes, edges) = expand_one_hop(store, entry_points, opts.max_nodes, &edge_kinds);
    let related_files = collect_related_files(&nodes);
    let code_blocks = collect_code_blocks(store, &nodes, opts.max_code_blocks);

    let summary_line = format!(
        "mycelium_context: {} entry points, {} nodes, {} edges, {} code blocks",
        entry_points.len(),
        nodes.len(),
        edges.len(),
        code_blocks.len()
    );
    let next_step = if code_blocks.is_empty() {
        "Use the nodes and edges to answer; code snippets were not available."
    } else {
        "Answer from code_blocks and the graph now. Only call a narrower tool if a specific edge or symbol is missing."
    };

    json!({
        "success": true,
        "verdict": "INFO",
        "task": task,
        "routing": routing.as_str(),
        "candidates": candidates,
        "entry_points": entry_points,
        "nodes": nodes,
        "edges": edges,
        "code_blocks": code_blocks,
        "related_files": related_files,
        "stats": {
            "entry_points": entry_points.len(),
            "nodes": nodes.len(),
            "edges": edges.len(),
            "code_blocks": code_blocks.len(),
            "related_files": related_files.len(),
        },
        "agent_summary": {
            "summary_line": summary_line,
            "verdict": "INFO",
            "next_step": next_step,
        }
    })
}

#[cfg(test)]
mod tests;
