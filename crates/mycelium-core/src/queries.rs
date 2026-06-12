//! Shared graph-list query payload builders (RFC-0109 Option A).
//!
//! Each function returns the canonical JSON object a graph-list tool emits.
//! **Both** the MCP tool and its CLI twin call the same builder, so their JSON
//! is byte-identical by construction (Charter §5.13 / RFC-0090 Three-Surface
//! Rule) — there is no per-surface payload code to drift. Budgeting
//! ([`crate::budget::apply_budget`]) is applied by the caller *after* building,
//! so the budget knob (RFC-0102) layers on uniformly.

use serde_json::{Value, json};

use crate::store::Store;
use crate::test_gap::CoverageFacts;
use crate::types::{EdgeKind, NodeId};

/// Build the `{ "callee_paths": [...], "callees": [{path, class}, ...] }` payload
/// for `get_callees` (RFC-0109 Option A shape + RFC-0113 class field).
///
/// `callee_paths` is kept for backward compatibility. `callees` is the additive
/// array where each entry carries the trunk path **and** a static classification:
/// - Paths containing `>` are project-defined symbols → `"project"`.
/// - Bare stub paths (no `>`) are classified against the language-appropriate
///   allowlists: TypeScript/JS (RFC-0113 Phase 2) for `.ts/.tsx/.js/.jsx/.mjs/.cjs`;
///   Go (RFC-0113 Phase 3) for `.go`; Rust (RFC-0113 Phase 4) for `.rs`;
///   Python otherwise. Callers must apply the project-ownership shadow first
///   (only unresolved stubs reach here).
///
/// Both arrays are sorted lexicographically by path and deduplicated.
#[must_use]
pub fn callees_payload(store: &Store, id: NodeId, kind: EdgeKind) -> Value {
    use crate::classify::{
        CalleeClass, classify_go_import_gated, classify_go_qualified, classify_python_import_gated,
        classify_rust_import_gated, classify_rust_qualified, classify_typescript_import_gated,
    };
    use std::collections::HashSet;

    // RFC-0113 Phase 3: build the caller file's import-module set for gating.
    // Extract the file prefix of the caller's path, look up its NodeId, then
    // collect the module-name stems of all its Imports edges.
    let caller_file = store
        .path_of(id)
        .map(|p| p.split('>').next().unwrap_or(p).to_owned());

    let caller_imports: HashSet<String> = caller_file
        .as_deref()
        .and_then(|file_path| store.lookup(file_path))
        .map(|file_id| {
            store
                .imports_of(file_id)
                .into_iter()
                .map(|imp| {
                    imp.split_once('>')
                        .map_or_else(|| imp.clone(), |(stem, _)| stem.to_owned())
                })
                .collect()
        })
        .unwrap_or_default();

    // RFC-0113 Phase 2+3: dispatch to language-appropriate classifier.
    let ext = caller_file
        .as_deref()
        .and_then(|f| std::path::Path::new(f).extension().and_then(|e| e.to_str()));
    let is_ts_js = matches!(ext, Some("ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs"));
    let is_go = matches!(ext, Some("go"));
    let is_rust = matches!(ext, Some("rs"));

    let mut entries: Vec<(String, &'static str)> = store
        .outgoing(id, kind)
        .iter()
        .filter_map(|&dst| {
            store.path_of(dst).map(|path| {
                let class = if path.contains('>') {
                    // RFC-0113 Phase 3b: for Go, a qualified path like `fmt>Println`
                    // or `net/http>Get` may be a stdlib call stored via the
                    // import-alias mechanism (Pass 1b-go maps local → full import path).
                    // Third-party packages always have a domain in the first path
                    // component (e.g. `github.com/acme/http>Get`), which is identified
                    // by a `.` before the first `/`. Stdlib and internal packages have
                    // no domain (e.g. `net/http`, `fmt`). (Codex P2 fix.)
                    if is_go {
                        let (pkg_path, method) = path.split_once('>').unwrap_or((path, ""));
                        let first_component = pkg_path.split('/').next().unwrap_or(pkg_path);
                        if first_component.contains('.') {
                            // Has domain prefix → third-party package → project node.
                            CalleeClass::Project.as_str()
                        } else {
                            // No domain → stdlib candidate; use last segment to look up
                            // in the stdlib table (`"net/http"` → local `"http"`).
                            let local = pkg_path.rsplit('/').next().unwrap_or(pkg_path);
                            match classify_go_qualified(local, method) {
                                CalleeClass::Unknown => CalleeClass::Project.as_str(),
                                known => known.as_str(),
                            }
                        }
                    } else if is_rust {
                        // RFC-0113 Phase 4: qualified Rust stdlib calls like
                        // `fs>read_to_string` or `std::io>stdout`. Receiver is
                        // the path component before the first `>`.
                        let (receiver, method) = path.split_once('>').unwrap_or((path, ""));
                        match classify_rust_qualified(receiver, method) {
                            CalleeClass::Unknown => CalleeClass::Project.as_str(),
                            known => known.as_str(),
                        }
                    } else {
                        CalleeClass::Project.as_str()
                    }
                } else if is_ts_js {
                    classify_typescript_import_gated(path, &caller_imports).as_str()
                } else if is_go {
                    classify_go_import_gated(path, &caller_imports).as_str()
                } else if is_rust {
                    classify_rust_import_gated(path, &caller_imports).as_str()
                } else {
                    classify_python_import_gated(path, &caller_imports).as_str()
                };
                (path.to_owned(), class)
            })
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries.dedup_by_key(|e| e.0.clone());

    let paths: Vec<&str> = entries.iter().map(|(p, _)| p.as_str()).collect();
    let callees: Vec<Value> = entries
        .iter()
        .map(|(p, c)| json!({ "path": p, "class": c }))
        .collect();

    json!({ "callee_paths": paths, "callees": callees })
}

/// Build the `{ "caller_paths": [...] }` payload for `get_callers`.
///
/// The sorted, deduplicated trunk paths that reach `id` via one incoming `kind`
/// edge. When `include_virtual` and `kind == Calls`, virtual-dispatch callers of
/// `path` (callers of an ancestor method of the same name) are merged in.
#[must_use]
pub fn callers_payload(
    store: &Store,
    id: NodeId,
    path: &str,
    kind: EdgeKind,
    include_virtual: bool,
) -> Value {
    let mut paths: Vec<String> = store
        .incoming(id, kind)
        .iter()
        .filter_map(|&src| store.path_of(src).map(str::to_owned))
        .collect();
    if kind == EdgeKind::Calls && include_virtual {
        paths.extend(
            store
                .virtual_dispatch_callers_of_path(path)
                .unwrap_or_default(),
        );
    }
    paths.sort();
    paths.dedup();
    json!({ "caller_paths": paths })
}

/// Build the `{ "matches": [...], "count": N, "total_count": N }` payload for
/// `query` / `mycelium_query` from an already-evaluated Hyphae match set.
///
/// `count` is the returned page length; `total_count` is the full match count.
/// Both start equal — budgeting (if any) is applied by the caller *after* this:
/// [`apply_budget`](crate::budget::apply_budget) caps `matches` at `max_nodes`
/// and rewrites `count` to the post-truncation length (the #746 rule), so
/// `count` always equals the array actually returned while `total_count`
/// keeps the true total.
#[must_use]
pub fn query_matches_payload(matches: &[String]) -> Value {
    json!({ "matches": matches, "count": matches.len(), "total_count": matches.len() })
}

/// Build the `{ callers, importers, extended_by, implemented_by }` payload for
/// `get_cross_refs` / `mycelium_get_cross_refs` from an already-computed
/// [`crate::store::CrossRefs`].
///
/// Each group is a flat top-level key so
/// [`apply_budget`](crate::budget::apply_budget) caps every group at
/// `max_edges` (the flat-key lookup sees them all). Both surfaces call this
/// builder (Charter §5.13 Three-Surface Rule).
#[must_use]
pub fn cross_refs_payload(refs: &crate::store::CrossRefs) -> Value {
    json!({
        "callers": refs.callers,
        "importers": refs.importers,
        "extended_by": refs.extended_by,
        "implemented_by": refs.implemented_by,
    })
}

/// Serialize a [`crate::store::CalleeNode`] subtree to the canonical
/// `{ path, children, unresolved_callees? }` JSON shape (ADR-0013: the
/// unresolved count is omitted when 0).
fn callee_node_to_json(node: &crate::store::CalleeNode, store: &Store) -> Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let children: Vec<Value> = node
        .children
        .iter()
        .map(|child| callee_node_to_json(child, store))
        .collect();
    let mut value = json!({ "path": path, "children": children });
    if node.unresolved_callees > 0 {
        value["unresolved_callees"] = json!(node.unresolved_callees);
    }
    value
}

/// Serialize a [`crate::store::CallerNode`] subtree to the canonical
/// `{ path, callers }` JSON shape.
fn caller_node_to_json(node: &crate::store::CallerNode, store: &Store) -> Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let callers: Vec<Value> = node
        .callers
        .iter()
        .map(|caller| caller_node_to_json(caller, store))
        .collect();
    json!({ "path": path, "callers": callers })
}

/// Build the `{ "root": { path, children, … } }` payload for
/// `get_callee_tree` / `mycelium_get_callee_tree`.
///
/// Tree budgeting (RFC-0102) is applied by the caller *after* this via
/// [`apply_tree_budget`](crate::budget::apply_tree_budget) with children key
/// `"children"`. Both surfaces call this builder (Charter §5.13).
#[must_use]
pub fn callee_tree_payload(store: &Store, root: NodeId, max_depth: usize) -> Value {
    let tree = store.callee_tree(root, max_depth);
    json!({ "root": callee_node_to_json(&tree, store) })
}

/// Build the `{ "root": { path, callers, … } }` payload for
/// `get_caller_tree` / `mycelium_get_caller_tree`.
///
/// Tree budgeting (RFC-0102) is applied by the caller *after* this via
/// [`apply_tree_budget`](crate::budget::apply_tree_budget) with children key
/// `"callers"`. Both surfaces call this builder (Charter §5.13).
#[must_use]
pub fn caller_tree_payload(store: &Store, root: NodeId, max_depth: usize) -> Value {
    let tree = store.caller_tree(root, max_depth);
    json!({ "root": caller_node_to_json(&tree, store) })
}

/// Build the `{ "dead_symbols": [...], "count": N }` payload for
/// `get_dead_symbols` from an already-computed list of dead symbols.
///
/// `count` is the full pre-budget total, so a caller still learns the true size
/// when [`apply_budget`](crate::budget::apply_budget) later truncates the array.
#[must_use]
pub fn dead_symbols_payload(dead: &[String]) -> Value {
    json!({ "dead_symbols": dead, "count": dead.len() })
}

/// Build the `{ "isolated_symbols": [...], "count": N }` payload for
/// `get_isolated_symbols` from an already-computed list.
///
/// `count` is the full pre-budget total (see [`dead_symbols_payload`]).
#[must_use]
pub fn isolated_symbols_payload(isolated: &[String]) -> Value {
    json!({ "isolated_symbols": isolated, "count": isolated.len() })
}

/// Build the `{ "reachable": [...], "count": N }` payload shared by
/// `get_reachable` and `get_reachable_to` from an already-computed BFS result.
///
/// `count` is the full pre-budget total (see [`dead_symbols_payload`]).
#[must_use]
pub fn reachable_payload(reachable: &[String]) -> Value {
    json!({ "reachable": reachable, "count": reachable.len() })
}

/// Build the `{ "symbols": [...], "count": N, "total_count": M }` payload for
/// `get_all_symbols` from an already-paginated `page` and the pre-pagination
/// `total_count`.
///
/// `count` is the returned page length; `total_count` is the full match count
/// before `limit`/`offset`. Budgeting (if any) is applied by the caller *after*
/// this, capping the page — [`apply_budget`](crate::budget::apply_budget)
/// rewrites `count` to the post-truncation length, so `count` always equals
/// the array actually returned while `total_count` keeps the true total
/// (RFC-0109: budget caps the selected page).
#[must_use]
pub fn all_symbols_payload(page: &[String], total_count: usize) -> Value {
    json!({ "symbols": page, "count": page.len(), "total_count": total_count })
}

/// Build the `{ "entry_points": [...], "count": N, "total_count": M }` payload
/// for `get_entry_points` from an already-paginated `page` and the
/// pre-pagination `total_count`.
///
/// `count` is the returned page length; `total_count` is the full match count
/// before `limit`/`offset`. Budgeting (if any) is applied by the caller *after*
/// this — [`apply_budget`](crate::budget::apply_budget) rewrites `count` to the
/// post-truncation length, so `count` always equals the array actually returned
/// while `total_count` keeps the true total (mirrors [`all_symbols_payload`];
/// RFC-0109: budget caps the page).
#[must_use]
pub fn entry_points_payload(page: &[String], total_count: usize) -> Value {
    json!({ "entry_points": page, "count": page.len(), "total_count": total_count })
}

/// Build the `{ verdict, reasons, checklist, blast_radius, direct_callers }`
/// payload for `safe_to_edit` / `mycelium_safe_to_edit` (RFC-0116 Phase 2).
///
/// Thin Store adapter: assembles [`crate::verdict::EditMetrics`] from the existing
/// call-graph surface (blast radius via [`Store::reachable_to`] + direct-caller
/// count via [`Store::incoming`]) and delegates to the pure
/// [`crate::verdict::edit_verdict`] core. `health` and `test_gap_uncovered` are
/// left `None` until RFC-0114 / RFC-0115 land (Phase 3). Max BFS depth is capped
/// at 20 — sufficient to saturate any realistic blast radius without scanning
/// unreachable nodes.
///
/// Output is byte-identical across CLI and MCP by construction (both call this
/// builder), satisfying Charter §5.13 / RFC-0090 Three-Surface Rule.
#[must_use]
pub fn safe_to_edit_payload(store: &Store, path: &str) -> Value {
    use crate::verdict::{EditMetrics, edit_verdict};

    let Some(id) = store.lookup(path) else {
        let ev = edit_verdict(&EditMetrics {
            symbol_found: false,
            parse_broken: false,
            direct_callers: 0,
            blast_radius: 0,
            health: None,
            test_gap_uncovered: None,
        });
        return json!({
            "verdict": ev.verdict.as_str(),
            "reasons": ev.reasons,
            "checklist": ev.checklist,
            "blast_radius": 0u32,
            "direct_callers": 0u32,
        });
    };

    let reachable = store.reachable_to(id, EdgeKind::Calls, 20);
    let blast_radius = u32::try_from(reachable.len()).unwrap_or(u32::MAX);
    let direct_callers =
        u32::try_from(store.incoming(id, EdgeKind::Calls).len()).unwrap_or(u32::MAX);

    let m = EditMetrics {
        symbol_found: true,
        parse_broken: false,
        direct_callers,
        blast_radius,
        health: None,
        test_gap_uncovered: None,
    };
    let ev = edit_verdict(&m);
    json!({
        "verdict": ev.verdict.as_str(),
        "reasons": ev.reasons,
        "checklist": ev.checklist,
        "blast_radius": blast_radius,
        "direct_callers": direct_callers,
    })
}

/// Parse a `coverage.py coverage.json` content string into [`CoverageFacts`].
///
/// Accepts the coverage.py JSON schema (`files.<path>.executed_lines` array).
/// Unknown extra fields (meta, totals, etc.) are ignored. Returns an error
/// string if the content is not valid JSON or the `files` key is absent.
///
/// # Errors
/// Returns a descriptive string if JSON is malformed or structurally unexpected.
pub fn parse_coverage_json(content: &str) -> Result<CoverageFacts, String> {
    use crate::test_gap::CoverageFacts;
    use std::collections::{BTreeMap, BTreeSet};

    let root: serde_json::Value =
        serde_json::from_str(content).map_err(|e| format!("invalid JSON: {e}"))?;
    let files = root
        .get("files")
        .ok_or_else(|| "coverage.json missing 'files' key".to_owned())?
        .as_object()
        .ok_or_else(|| "'files' must be an object".to_owned())?;

    let mut executed_lines: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
    for (file, file_data) in files {
        let lines: BTreeSet<u32> = file_data
            .get("executed_lines")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|n| n.as_u64().and_then(|n| u32::try_from(n).ok()))
                    .collect()
            })
            .unwrap_or_default();
        executed_lines.insert(file.clone(), lines);
    }
    Ok(CoverageFacts { executed_lines })
}

/// Build the `{gaps, gap_count, total_symbols, coverage_source, truncated}`
/// payload for `test-gap` / `mycelium_test_gap` (RFC-0115 Phase 2).
///
/// Thin Store adapter: enumerates all non-file symbols, derives a
/// [`crate::test_gap::SymbolSpan`] for each (using `start_line + 1` as
/// `body_start` — the TSA heuristic; a future ADR will extend the indexed
/// span to include an explicit `body_start`), then fills
/// [`crate::test_gap::GraphReach`] from the existing call-graph surface
/// (blast radius via [`Store::reachable_to`] + direct caller count). The
/// pure [`crate::test_gap::rank`] core is then called.
///
/// `top` caps the returned list; `None` returns all gaps. `coverage_source`
/// is echoed back verbatim in the output (the caller supplies the file path).
///
/// Output is byte-identical across CLI and MCP by construction (both call this
/// builder), satisfying Charter §5.13 / RFC-0090.
#[must_use]
pub fn test_gap_payload(
    store: &Store,
    facts: &crate::test_gap::CoverageFacts,
    top: Option<usize>,
    coverage_source: &str,
) -> Value {
    use crate::test_gap::{GraphReach, SymbolSpan, rank};

    let all_symbols = store.all_symbols(None, None);
    let total_symbols = all_symbols.len();

    // Build (SymbolSpan, GraphReach) pairs for the pure core.
    // Body-start approximation: start_line + 1 (TSA heuristic).
    // Symbols with no span use line 0/0 (body_start = 1, end_line = 0) — the
    // inverted span causes is_covered() to return false (gap), which is the
    // conservative/correct default for symbols the index has no span for.
    let inputs: Vec<(SymbolSpan, GraphReach)> = all_symbols
        .iter()
        .filter_map(|path| {
            let id = store.lookup(path)?;
            // Extract file from "file>Class>method" trunk path.
            let file = path.split('>').next()?.to_owned();
            // body_start = start_line + 1 (TSA heuristic; RFC-0115 §Phase-2).
            // No span → (0, 0): body_start ≤ end_line holds, and line-0 never
            // appears in coverage.json (1-indexed), so the symbol reads as a gap.
            let (body_start, end_line) = store
                .span_of(id)
                .map_or((0, 0), |s| (s.start_line.saturating_add(1), s.end_line));
            let blast_radius = u32::try_from(
                store
                    .reachable_to(id, crate::types::EdgeKind::Calls, 20)
                    .len(),
            )
            .unwrap_or(u32::MAX);
            let in_degree = u32::try_from(store.incoming(id, crate::types::EdgeKind::Calls).len())
                .unwrap_or(u32::MAX);
            Some((
                SymbolSpan {
                    name: path.clone(),
                    file,
                    body_start,
                    end_line,
                },
                GraphReach {
                    blast_radius,
                    in_degree,
                    degree_centrality: 0.0, // degree_centrality is a batch op; 0.0 keeps
                                            // blast_radius as primary rank signal (RFC-0115).
                },
            ))
        })
        .collect();

    let all_gaps = rank(&inputs, facts);
    let gap_count = all_gaps.len();
    let truncated = top.is_some_and(|t| gap_count > t);
    let page: Vec<Value> = all_gaps
        .into_iter()
        .take(top.unwrap_or(usize::MAX))
        .map(|g| {
            json!({
                "name": g.name,
                "file": g.file,
                "rank_score": g.rank_score,
            })
        })
        .collect();

    json!({
        "gaps": page,
        "gap_count": gap_count,
        "total_symbols": total_symbols,
        "coverage_source": coverage_source,
        "truncated": truncated,
    })
}

// ─────────────────────────── RFC-0117 Phase 2 helpers ────────────────────────

/// Deserialization model for `.mycelium/constraints.yml` (RFC-0117 §YAML schema).
#[derive(serde::Deserialize)]
struct ConstraintsFile {
    #[serde(default = "constraints_default_version")]
    version: u32,
    #[serde(default)]
    constraints: Vec<ConstraintEntry>,
}

const fn constraints_default_version() -> u32 {
    1
}

#[derive(serde::Deserialize)]
struct ConstraintEntry {
    id: String,
    #[serde(default)]
    severity: SeverityYaml,
    #[serde(rename = "from")]
    from_glob: String,
    #[serde(rename = "to")]
    to_glob: String,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    applies_to: AppliesToYaml,
    #[serde(default)]
    exceptions: Vec<String>,
    #[serde(default = "forbid_default")]
    rule: String,
}

fn forbid_default() -> String {
    "forbid".to_owned()
}

#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "lowercase")]
enum SeverityYaml {
    #[default]
    Error,
    Warn,
    Info,
}

#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "lowercase")]
enum AppliesToYaml {
    #[default]
    Any,
    Calls,
    Imports,
}

/// Load and validate `.mycelium/constraints.yml` from `workspace_root`.
///
/// Returns `None` if the file is missing, unreadable, malformed, or has an
/// unsupported `version`. Skips entries whose `rule` is not `"forbid"` or
/// whose `id`/`from`/`to` fields are empty (fail-fast per RFC-0117 §Phase-2).
fn load_constraints_yml(
    workspace_root: &std::path::Path,
) -> Option<Vec<crate::constraints::Constraint>> {
    use crate::constraints::{Constraint, EdgeKindFilter, Severity};

    let path = workspace_root.join(".mycelium").join("constraints.yml");
    let content = std::fs::read_to_string(path).ok()?;
    let file: ConstraintsFile = serde_yaml::from_str(&content).ok()?;
    if file.version != 1 {
        return None;
    }
    let constraints: Vec<Constraint> = file
        .constraints
        .into_iter()
        .filter(|e| {
            e.rule == "forbid"
                && !e.id.is_empty()
                && !e.from_glob.is_empty()
                && !e.to_glob.is_empty()
        })
        .map(|e| Constraint {
            id: e.id,
            severity: match e.severity {
                SeverityYaml::Error => Severity::Error,
                SeverityYaml::Warn => Severity::Warn,
                SeverityYaml::Info => Severity::Info,
            },
            applies_to: match e.applies_to {
                AppliesToYaml::Any => EdgeKindFilter::Any,
                AppliesToYaml::Calls => EdgeKindFilter::Calls,
                AppliesToYaml::Imports => EdgeKindFilter::Imports,
            },
            from_glob: e.from_glob,
            to_glob: e.to_glob,
            reason: e.reason,
            exceptions: e.exceptions,
        })
        .collect();
    Some(constraints)
}

/// Build the `{ violations, violation_count, error_count, warn_count }` payload
/// for `check-architecture` (RFC-0117 Phase 2).
///
/// Reads `.mycelium/constraints.yml` from `workspace_root`; if absent returns an
/// empty report. Enumerates all `Calls` and `Imports` edges in the `store`,
/// projects them into [`crate::constraints::EdgeRef`]s, and runs the pure
/// [`crate::constraints::evaluate`] function. Both CLI and MCP call this builder,
/// so their JSON output is byte-identical (Charter §5.13 / RFC-0090).
#[must_use]
pub fn check_architecture_payload(store: &Store, workspace_root: &std::path::Path) -> Value {
    use crate::constraints::{EdgeKindFilter, EdgeRef, Severity, evaluate};
    use crate::types::EdgeKind;

    let constraints = load_constraints_yml(workspace_root).unwrap_or_default();
    if constraints.is_empty() {
        return json!({
            "violations": [],
            "violation_count": 0,
            "error_count": 0,
            "warn_count": 0,
        });
    }

    // Project all Calls+Imports edges into (owned) path strings before building
    // EdgeRef borrows, to satisfy the borrow checker.
    let mut edge_data: Vec<(String, String, u32, EdgeKindFilter)> = Vec::new();
    for from_path in store.all_symbols(None, None) {
        let Some(from_id) = store.lookup(&from_path) else {
            continue;
        };
        let from_line = store.span_of(from_id).map_or(0, |s| s.start_line);
        for kind in [EdgeKind::Calls, EdgeKind::Imports] {
            let filter = if kind == EdgeKind::Calls {
                EdgeKindFilter::Calls
            } else {
                EdgeKindFilter::Imports
            };
            for &to_id in store.outgoing(from_id, kind) {
                let Some(to_path) = store.path_of(to_id) else {
                    continue;
                };
                edge_data.push((from_path.clone(), to_path.to_owned(), from_line, filter));
            }
        }
    }

    let refs: Vec<EdgeRef<'_>> = edge_data
        .iter()
        .map(|(from, to, line, kind)| EdgeRef {
            kind: *kind,
            from_path: from.as_str(),
            to_path: to.as_str(),
            from_line: *line,
        })
        .collect();

    let violations = evaluate(&constraints, &refs);
    let violation_count = violations.len();
    let error_count = violations
        .iter()
        .filter(|v| v.severity == Severity::Error)
        .count();
    let warn_count = violations
        .iter()
        .filter(|v| v.severity == Severity::Warn)
        .count();

    let viol_json: Vec<Value> = violations
        .into_iter()
        .map(|v| {
            json!({
                "rule_id": v.rule_id,
                "severity": match v.severity {
                    Severity::Error => "error",
                    Severity::Warn => "warn",
                    Severity::Info => "info",
                },
                "kind": match v.kind {
                    EdgeKindFilter::Calls => "calls",
                    EdgeKindFilter::Imports => "imports",
                    EdgeKindFilter::Any => "any",
                },
                "from_path": v.from_path,
                "to_path": v.to_path,
                "from_line": v.from_line,
            })
        })
        .collect();

    json!({
        "violations": viol_json,
        "violation_count": violation_count,
        "error_count": error_count,
        "warn_count": warn_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trunk::TrunkPath;

    fn p(s: &str) -> TrunkPath {
        TrunkPath::parse(s).unwrap()
    }

    #[test]
    fn callees_payload_is_a_sorted_deduped_object() {
        let mut store = Store::new();
        let a = store.upsert_node(p("src/a.rs>A>run"));
        let z = store.upsert_node(p("src/z.rs>Z>zeta"));
        let b = store.upsert_node(p("src/b.rs>B>beta"));
        store.upsert_edge(EdgeKind::Calls, a, z);
        store.upsert_edge(EdgeKind::Calls, a, b);

        let v = callees_payload(&store, a, EdgeKind::Calls);

        // Object shape with the `callee_paths` key (RFC-0109 Option A) ...
        let arr = v["callee_paths"]
            .as_array()
            .expect("callee_paths must be an array");
        // ... sorted lexicographically.
        assert_eq!(arr[0], "src/b.rs>B>beta");
        assert_eq!(arr[1], "src/z.rs>Z>zeta");
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn callees_payload_empty_for_leaf() {
        let mut store = Store::new();
        let leaf = store.upsert_node(p("src/a.rs>A>leaf"));
        let v = callees_payload(&store, leaf, EdgeKind::Calls);
        assert_eq!(v["callee_paths"].as_array().unwrap().len(), 0);
        assert_eq!(v["callees"].as_array().unwrap().len(), 0);
    }

    // RFC-0113 Phase 2: callee classification ─────────────────────────────────

    #[test]
    fn callees_payload_project_callee_has_class_project() {
        let mut store = Store::new();
        let src = store.upsert_node(p("src/a.py>A>run"));
        let dst = store.upsert_node(p("src/b.py>B>helper"));
        store.upsert_edge(EdgeKind::Calls, src, dst);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().expect("callees must be an array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["path"], "src/b.py>B>helper");
        assert_eq!(entries[0]["class"], "project");
    }

    #[test]
    fn callees_payload_bare_builtin_stub_classified() {
        // bare stub "len" — Python builtin; project resolution already failed
        let mut store = Store::new();
        let src = store.upsert_node(p("src/a.py>A>run"));
        let stub = store.upsert_node(p("len"));
        store.upsert_edge(EdgeKind::Calls, src, stub);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().expect("callees must be an array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["path"], "len");
        assert_eq!(entries[0]["class"], "builtin");
    }

    #[test]
    fn callees_payload_bare_stdlib_method_without_import_is_unknown() {
        // RFC-0113 Phase 3: bare stub "write_text" with no stdlib import → unknown
        // (import gate blocks the stdlib tier when no stdlib module is imported).
        let mut store = Store::new();
        let src = store.upsert_node(p("src/a.py>A>run"));
        let stub = store.upsert_node(p("write_text"));
        store.upsert_edge(EdgeKind::Calls, src, stub);
        // intentionally no Imports edge on src/a.py

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().expect("callees must be an array");
        assert_eq!(entries[0]["class"], "unknown");
    }

    #[test]
    fn callees_payload_bare_stdlib_method_with_stdlib_import_is_stdlib() {
        // RFC-0113 Phase 3: bare stub "write_text" + pathlib import → stdlib.
        // The file node must be explicitly upserted (trunk does not auto-create
        // intermediate nodes for ancestor paths).
        use crate::types::NodeKind;
        let mut store = Store::new();
        let file = store.upsert_node_with_kind(p("src/a.py"), NodeKind::File);
        let src = store.upsert_node(p("src/a.py>A>run"));
        let stub = store.upsert_node(p("write_text"));
        store.upsert_edge(EdgeKind::Calls, src, stub);
        let pathlib_mod = store.upsert_node_with_kind(p("pathlib"), NodeKind::Module);
        store.upsert_edge(EdgeKind::Imports, file, pathlib_mod);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().expect("callees must be an array");
        assert_eq!(entries[0]["path"], "write_text");
        assert_eq!(entries[0]["class"], "stdlib");
    }

    #[test]
    fn callees_payload_stdlib_function_with_import_is_stdlib() {
        // RFC-0113 Phase 3: bare stub "getcwd" + os import → stdlib.
        use crate::types::NodeKind;
        let mut store = Store::new();
        let file = store.upsert_node_with_kind(p("src/a.py"), NodeKind::File);
        let src = store.upsert_node(p("src/a.py>A>run"));
        let stub = store.upsert_node(p("getcwd"));
        store.upsert_edge(EdgeKind::Calls, src, stub);
        let os_mod = store.upsert_node_with_kind(p("os"), NodeKind::Module);
        store.upsert_edge(EdgeKind::Imports, file, os_mod);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().expect("callees must be an array");
        assert_eq!(entries[0]["class"], "stdlib");
    }

    #[test]
    fn callees_payload_bare_unknown_stub_classified() {
        // bare stub "frobnicate" — no match in any table
        let mut store = Store::new();
        let src = store.upsert_node(p("src/a.py>A>run"));
        let stub = store.upsert_node(p("frobnicate"));
        store.upsert_edge(EdgeKind::Calls, src, stub);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().expect("callees must be an array");
        assert_eq!(entries[0]["class"], "unknown");
    }

    #[test]
    fn callees_payload_mixed_project_and_stubs_sorted_by_path() {
        // project symbol + two stubs; result sorted lexicographically
        let mut store = Store::new();
        let src = store.upsert_node(p("src/a.py>A>run"));
        let proj = store.upsert_node(p("src/b.py>B>helper"));
        let b_stub = store.upsert_node(p("len"));
        let u_stub = store.upsert_node(p("frobnicate"));
        store.upsert_edge(EdgeKind::Calls, src, proj);
        store.upsert_edge(EdgeKind::Calls, src, b_stub);
        store.upsert_edge(EdgeKind::Calls, src, u_stub);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let paths = v["callee_paths"]
            .as_array()
            .expect("callee_paths must be present (backward compat)");
        let entries = v["callees"].as_array().expect("callees must be an array");

        // Both arrays must have the same length and the same sort order.
        assert_eq!(paths.len(), 3);
        assert_eq!(entries.len(), 3);

        // Sorted: "frobnicate" < "len" < "src/b.py>B>helper"
        assert_eq!(entries[0]["path"], "frobnicate");
        assert_eq!(entries[0]["class"], "unknown");
        assert_eq!(entries[1]["path"], "len");
        assert_eq!(entries[1]["class"], "builtin");
        assert_eq!(entries[2]["path"], "src/b.py>B>helper");
        assert_eq!(entries[2]["class"], "project");
    }

    #[test]
    fn callers_payload_is_a_sorted_deduped_object() {
        let mut store = Store::new();
        let target = store.upsert_node(p("src/t.rs>T>target"));
        let z = store.upsert_node(p("src/z.rs>Z>zeta"));
        let b = store.upsert_node(p("src/b.rs>B>beta"));
        store.upsert_edge(EdgeKind::Calls, z, target);
        store.upsert_edge(EdgeKind::Calls, b, target);

        let v = callers_payload(&store, target, "src/t.rs>T>target", EdgeKind::Calls, false);
        let arr = v["caller_paths"]
            .as_array()
            .expect("caller_paths must be an array");
        assert_eq!(arr[0], "src/b.rs>B>beta");
        assert_eq!(arr[1], "src/z.rs>Z>zeta");
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn dead_symbols_payload_has_array_and_count() {
        let v = dead_symbols_payload(&["a".to_owned(), "b".to_owned()]);
        assert_eq!(v["dead_symbols"], serde_json::json!(["a", "b"]));
        assert_eq!(v["count"], 2);
    }

    #[test]
    fn isolated_symbols_payload_has_array_and_count() {
        let v = isolated_symbols_payload(&["x".to_owned()]);
        assert_eq!(v["isolated_symbols"], serde_json::json!(["x"]));
        assert_eq!(v["count"], 1);
    }

    #[test]
    fn reachable_payload_has_array_and_count() {
        let v = reachable_payload(&["a".to_owned(), "b".to_owned(), "c".to_owned()]);
        assert_eq!(v["reachable"], serde_json::json!(["a", "b", "c"]));
        assert_eq!(v["count"], 3);
    }

    #[test]
    fn all_symbols_payload_reports_page_and_total() {
        // A 2-item page out of a 10-match total.
        let v = all_symbols_payload(&["a".to_owned(), "b".to_owned()], 10);
        assert_eq!(v["symbols"], serde_json::json!(["a", "b"]));
        assert_eq!(v["count"], 2);
        assert_eq!(v["total_count"], 10);
    }

    #[test]
    fn entry_points_payload_reports_page_and_total() {
        // A 2-item page out of a 7-match total.
        let v = entry_points_payload(&["a".to_owned(), "b".to_owned()], 7);
        assert_eq!(v["entry_points"], serde_json::json!(["a", "b"]));
        assert_eq!(v["count"], 2);
        assert_eq!(v["total_count"], 7);
    }

    // ── RFC-0116 Phase 2: safe_to_edit_payload ────────────────────────────────

    #[test]
    fn safe_to_edit_payload_not_found_returns_not_found_verdict() {
        let store = Store::new();
        let v = safe_to_edit_payload(&store, "src/missing.rs>Missing>method");
        assert_eq!(v["verdict"], "NOT_FOUND");
        assert_eq!(v["blast_radius"], 0);
        assert_eq!(v["direct_callers"], 0);
    }

    #[test]
    fn safe_to_edit_payload_leaf_symbol_is_safe() {
        let mut store = Store::new();
        let _leaf = store.upsert_node(p("src/a.rs>A>leaf"));
        let v = safe_to_edit_payload(&store, "src/a.rs>A>leaf");
        assert_eq!(v["verdict"], "SAFE");
        assert_eq!(v["blast_radius"], 0);
        assert_eq!(v["direct_callers"], 0);
        assert!(v["checklist"].as_array().is_some_and(Vec::is_empty));
    }

    #[test]
    fn safe_to_edit_payload_caution_blast_radius() {
        // 3 symbols call target → blast_radius=3 after reachable_to → CAUTION
        let mut store = Store::new();
        let target = store.upsert_node(p("src/b.rs>B>target"));
        for i in 0..3u32 {
            let caller = store.upsert_node(p(&format!("src/c{i}.rs>C>call")));
            store.upsert_edge(EdgeKind::Calls, caller, target);
        }
        let v = safe_to_edit_payload(&store, "src/b.rs>B>target");
        assert_eq!(v["verdict"], "CAUTION");
        assert_eq!(v["blast_radius"], 3);
        assert!(v["direct_callers"].as_u64().unwrap() > 0);
    }

    #[test]
    fn safe_to_edit_payload_review_blast_radius() {
        let mut store = Store::new();
        let target = store.upsert_node(p("src/core.rs>Core>fn"));
        for i in 0..12u32 {
            let caller = store.upsert_node(p(&format!("src/u{i}.rs>U>call")));
            store.upsert_edge(EdgeKind::Calls, caller, target);
        }
        let v = safe_to_edit_payload(&store, "src/core.rs>Core>fn");
        assert_eq!(v["verdict"], "REVIEW");
    }

    #[test]
    fn safe_to_edit_payload_unsafe_blast_radius() {
        let mut store = Store::new();
        let target = store.upsert_node(p("src/base.rs>Base>hot"));
        for i in 0..25u32 {
            let caller = store.upsert_node(p(&format!("src/d{i}.rs>D>call")));
            store.upsert_edge(EdgeKind::Calls, caller, target);
        }
        let v = safe_to_edit_payload(&store, "src/base.rs>Base>hot");
        assert_eq!(v["verdict"], "UNSAFE");
        assert!(!v["checklist"].as_array().unwrap().is_empty());
    }

    #[test]
    fn safe_to_edit_payload_shape_has_required_fields() {
        let mut store = Store::new();
        let _ = store.upsert_node(p("src/x.rs>X>method"));
        let v = safe_to_edit_payload(&store, "src/x.rs>X>method");
        assert!(v.get("verdict").is_some());
        assert!(v.get("reasons").is_some());
        assert!(v.get("checklist").is_some());
        assert!(v.get("blast_radius").is_some());
        assert!(v.get("direct_callers").is_some());
    }

    // ── RFC-0115 Phase 2: parse_coverage_json + test_gap_payload ─────────────

    #[test]
    fn parse_coverage_json_parses_executed_lines() {
        let json = r#"{"files":{"src/main.py":{"executed_lines":[5,6,7]}}}"#;
        let facts = parse_coverage_json(json).expect("should parse");
        let lines = facts
            .executed_lines
            .get("src/main.py")
            .expect("file present");
        assert!(lines.contains(&5));
        assert!(lines.contains(&7));
        assert!(!lines.contains(&10));
    }

    #[test]
    fn parse_coverage_json_empty_files() {
        let json = r#"{"files":{}}"#;
        let facts = parse_coverage_json(json).expect("should parse");
        assert!(facts.executed_lines.is_empty());
    }

    // ─────────────────────────────────── RFC-0117 Phase 2 ──────────────────────

    #[test]
    fn check_architecture_no_config_is_empty() {
        let store = Store::new();
        let result = check_architecture_payload(&store, std::path::Path::new("/nonexistent/dir"));
        assert_eq!(result["violation_count"].as_u64().unwrap(), 0);
        assert_eq!(result["error_count"].as_u64().unwrap(), 0);
        assert_eq!(result["violations"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn check_architecture_detects_forbidden_import_edge() {
        use std::io::Write as _;
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".mycelium")).unwrap();
        let mut f = std::fs::File::create(tmp.path().join(".mycelium/constraints.yml")).unwrap();
        writeln!(
            f,
            "version: 1\nconstraints:\n  - id: ui-no-db\n    from: src/ui/**\n    to: \
             src/db/**\n    rule: forbid\n"
        )
        .unwrap();
        drop(f);

        let mut store = Store::new();
        let ui = store.upsert_node(p("src/ui/page.rs>Page>render"));
        let db = store.upsert_node(p("src/db/pool.rs>Pool>get"));
        store.upsert_edge(crate::types::EdgeKind::Calls, ui, db);

        let result = check_architecture_payload(&store, tmp.path());
        assert_eq!(result["violation_count"].as_u64().unwrap(), 1);
        assert_eq!(result["error_count"].as_u64().unwrap(), 1);
        let v = &result["violations"].as_array().unwrap()[0];
        assert_eq!(v["rule_id"].as_str().unwrap(), "ui-no-db");
        assert_eq!(v["severity"].as_str().unwrap(), "error");
        assert_eq!(v["kind"].as_str().unwrap(), "calls");
    }

    #[test]
    fn test_gap_payload_shape_has_required_fields() {
        let store = Store::new();
        let facts = crate::test_gap::CoverageFacts::default();
        let v = test_gap_payload(&store, &facts, None, "coverage.json");
        assert!(v.get("gaps").is_some());
        assert!(v.get("gap_count").is_some());
        assert!(v.get("total_symbols").is_some());
        assert!(v.get("coverage_source").is_some());
        assert!(v.get("truncated").is_some());
    }

    #[test]
    fn test_gap_payload_empty_store_no_gaps() {
        let store = Store::new();
        let facts = crate::test_gap::CoverageFacts::default();
        let v = test_gap_payload(&store, &facts, None, "coverage.json");
        assert_eq!(v["gap_count"], 0);
        assert_eq!(v["total_symbols"], 0);
        let gaps = v["gaps"].as_array().unwrap();
        assert!(gaps.is_empty());
    }

    #[test]
    fn test_gap_payload_uncovered_symbol_appears_as_gap() {
        let mut store = Store::new();
        let _ = store.upsert_node(p("src/a.py>A>method"));
        let facts = crate::test_gap::CoverageFacts::default(); // nothing executed
        let v = test_gap_payload(&store, &facts, None, "cov.json");
        assert_eq!(v["gap_count"], 1);
        let gaps = v["gaps"].as_array().unwrap();
        assert_eq!(gaps[0]["name"], "src/a.py>A>method");
    }

    #[test]
    fn test_gap_payload_top_limits_gaps() {
        let mut store = Store::new();
        for i in 0..5u32 {
            let _ = store.upsert_node(p(&format!("src/f{i}.py>C>m{i}")));
        }
        let facts = crate::test_gap::CoverageFacts::default();
        let v = test_gap_payload(&store, &facts, Some(2), "cov.json");
        let gaps = v["gaps"].as_array().unwrap();
        assert_eq!(gaps.len(), 2);
        assert_eq!(v["truncated"], true);
    }

    // ── RFC-0113 Phase 2: language dispatch for TypeScript/JS callers ─────────
    // Codex P1/P2 fix: callees_payload must dispatch to the TS classifier for
    // .ts/.js callers instead of always using the Python classifier.

    #[test]
    fn callees_payload_ts_builtin_classified_for_ts_caller() {
        // TypeScript caller → bare stub "parseInt" must be "builtin" (TS global).
        // Before the dispatch fix the Python classifier returns "unknown".
        let mut store = Store::new();
        let src = store.upsert_node(p("src/app.ts>App>run"));
        let stub = store.upsert_node(p("parseInt"));
        store.upsert_edge(EdgeKind::Calls, src, stub);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().expect("callees must be an array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["path"], "parseInt");
        assert_eq!(entries[0]["class"], "builtin");
    }

    #[test]
    fn callees_payload_ts_stdlib_with_fs_import_is_stdlib() {
        // TypeScript caller + `import { readFileSync } from 'fs'` → "stdlib".
        use crate::types::NodeKind;
        let mut store = Store::new();
        let file = store.upsert_node_with_kind(p("src/app.ts"), NodeKind::File);
        let src = store.upsert_node(p("src/app.ts>App>run"));
        let stub = store.upsert_node(p("readFileSync"));
        store.upsert_edge(EdgeKind::Calls, src, stub);
        let fs_mod = store.upsert_node_with_kind(p("fs"), NodeKind::Module);
        store.upsert_edge(EdgeKind::Imports, file, fs_mod);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().expect("callees must be an array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["path"], "readFileSync");
        assert_eq!(entries[0]["class"], "stdlib");
    }

    #[test]
    fn callees_payload_is_integer_is_unknown_for_ts_caller() {
        // `isInteger` is NOT a JS/TS global (Number.isInteger is qualified).
        // Codex P2: after removing from TS_GLOBAL_BUILTINS → "unknown".
        let mut store = Store::new();
        let src = store.upsert_node(p("src/app.ts>App>run"));
        let stub = store.upsert_node(p("isInteger"));
        store.upsert_edge(EdgeKind::Calls, src, stub);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().expect("callees must be an array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["path"], "isInteger");
        assert_eq!(entries[0]["class"], "unknown");
    }

    #[test]
    fn callees_payload_js_file_also_uses_ts_classifier() {
        // .js extension also routes to TypeScript classifier.
        let mut store = Store::new();
        let src = store.upsert_node(p("src/util.js>util>helper"));
        let stub = store.upsert_node(p("setTimeout"));
        store.upsert_edge(EdgeKind::Calls, src, stub);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().expect("callees must be an array");
        assert_eq!(entries[0]["class"], "builtin");
    }

    // ── RFC-0113 Phase 3b: Go qualified stdlib call classification ────────────

    #[test]
    fn callees_payload_go_qualified_fmt_println_is_stdlib() {
        // `fmt>Println` in a .go caller must be `stdlib`, not `project`.
        use crate::types::NodeKind;
        let mut store = Store::new();
        let src = store.upsert_node(p("src/main.go>main"));
        let println = store.upsert_node_with_kind(p("fmt>Println"), NodeKind::Unresolved);
        store.upsert_edge(EdgeKind::Calls, src, println);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["path"], "fmt>Println");
        assert_eq!(
            entries[0]["class"], "stdlib",
            "Go qualified stdlib callee `fmt>Println` must be `stdlib`, not `project`"
        );
    }

    #[test]
    fn callees_payload_go_qualified_http_get_is_stdlib() {
        // Full import-path form: `net/http>Get` must classify as stdlib.
        // (The extractor stores the full path after the Codex P2 fix.)
        use crate::types::NodeKind;
        let mut store = Store::new();
        let src = store.upsert_node(p("fetch.go>fetch"));
        let get = store.upsert_node_with_kind(p("net/http>Get"), NodeKind::Unresolved);
        store.upsert_edge(EdgeKind::Calls, src, get);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().unwrap();
        assert_eq!(
            entries[0]["class"], "stdlib",
            "`net/http>Get` callee for .go file must be `stdlib`"
        );
    }

    #[test]
    fn callees_payload_go_third_party_domain_pkg_stays_project() {
        // Codex P2: `github.com/acme/http>Get` must NOT be classified as stdlib.
        // Domain prefix (dot in first component) identifies third-party packages.
        use crate::types::NodeKind;
        let mut store = Store::new();
        let src = store.upsert_node(p("main.go>main"));
        let get = store.upsert_node_with_kind(p("github.com/acme/http>Get"), NodeKind::Unresolved);
        store.upsert_edge(EdgeKind::Calls, src, get);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().unwrap();
        assert_eq!(
            entries[0]["class"], "project",
            "`github.com/acme/http>Get` must be `project`, not `stdlib` (third-party domain)"
        );
    }

    #[test]
    fn callees_payload_go_qualified_project_path_stays_project() {
        // A .go callee whose receiver is not a stdlib package name must stay `project`.
        let mut store = Store::new();
        let src = store.upsert_node(p("src/main.go>handleRequest"));
        let run = store.upsert_node(p("src/server.go>Server>Run"));
        store.upsert_edge(EdgeKind::Calls, src, run);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0]["class"], "project",
            "Go project callee `src/server.go>Server>Run` must remain `project`"
        );
    }

    // ── RFC-0113 Phase 4: Rust stdlib classification ──────────────────────────

    #[test]
    fn callees_payload_rust_macro_builtin_classified() {
        // Rust caller with bare stub "println" — builtin macro, no import needed.
        let mut store = Store::new();
        let src = store.upsert_node(p("src/main.rs>main"));
        let stub = store.upsert_node(p("println"));
        store.upsert_edge(EdgeKind::Calls, src, stub);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().expect("callees must be an array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["path"], "println");
        assert_eq!(
            entries[0]["class"], "builtin",
            "Rust macro `println` must be `builtin` for .rs caller"
        );
    }

    #[test]
    fn callees_payload_rust_drop_builtin_classified() {
        // `drop(x)` is always available, no import required.
        let mut store = Store::new();
        let src = store.upsert_node(p("src/lib.rs>Module>method"));
        let stub = store.upsert_node(p("drop"));
        store.upsert_edge(EdgeKind::Calls, src, stub);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().expect("callees must be an array");
        assert_eq!(entries[0]["path"], "drop");
        assert_eq!(
            entries[0]["class"], "builtin",
            "Rust `drop` must be `builtin` for .rs caller"
        );
    }

    #[test]
    fn callees_payload_rust_stdlib_module_with_import_is_stdlib() {
        // Rust caller + `use std::fs` import → "fs" bare stub → stdlib.
        use crate::types::NodeKind;
        let mut store = Store::new();
        let file = store.upsert_node_with_kind(p("src/main.rs"), NodeKind::File);
        let src = store.upsert_node(p("src/main.rs>main"));
        let stub = store.upsert_node(p("fs"));
        store.upsert_edge(EdgeKind::Calls, src, stub);
        let fs_mod = store.upsert_node_with_kind(p("std::fs"), NodeKind::Module);
        store.upsert_edge(EdgeKind::Imports, file, fs_mod);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().expect("callees must be an array");
        assert_eq!(entries[0]["path"], "fs");
        assert_eq!(
            entries[0]["class"], "stdlib",
            "Rust `fs` with `use std::fs` import must be `stdlib`"
        );
    }

    #[test]
    fn callees_payload_rust_stdlib_module_without_import_is_unknown() {
        // No `use std::fs` → bare `fs` stub stays unknown.
        let mut store = Store::new();
        let src = store.upsert_node(p("src/main.rs>main"));
        let stub = store.upsert_node(p("fs"));
        store.upsert_edge(EdgeKind::Calls, src, stub);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().expect("callees must be an array");
        assert_eq!(
            entries[0]["class"], "unknown",
            "Rust `fs` without import must be `unknown`"
        );
    }

    #[test]
    fn callees_payload_rust_qualified_fs_is_stdlib() {
        // Rust caller with `fs>read_to_string` qualified path → stdlib.
        use crate::types::NodeKind;
        let mut store = Store::new();
        let src = store.upsert_node(p("src/main.rs>main"));
        let method = store.upsert_node_with_kind(p("fs>read_to_string"), NodeKind::Unresolved);
        store.upsert_edge(EdgeKind::Calls, src, method);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().unwrap();
        assert_eq!(entries[0]["path"], "fs>read_to_string");
        assert_eq!(
            entries[0]["class"], "stdlib",
            "Rust `fs>read_to_string` must be `stdlib` for .rs caller"
        );
    }

    #[test]
    fn callees_payload_rust_project_path_stays_project() {
        // Rust project callee path stays "project".
        let mut store = Store::new();
        let src = store.upsert_node(p("src/main.rs>main"));
        let proj = store.upsert_node(p("src/lib.rs>MyStruct>method"));
        store.upsert_edge(EdgeKind::Calls, src, proj);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().unwrap();
        assert_eq!(
            entries[0]["class"], "project",
            "Rust project callee must remain `project`"
        );
    }

    #[test]
    fn callees_payload_rust_qualified_unknown_receiver_stays_project() {
        // Unknown receiver for .rs callee stays "project" (no stdlib match).
        use crate::types::NodeKind;
        let mut store = Store::new();
        let src = store.upsert_node(p("src/main.rs>main"));
        let method = store.upsert_node_with_kind(p("my_crate>MyType>method"), NodeKind::Unresolved);
        store.upsert_edge(EdgeKind::Calls, src, method);

        let v = callees_payload(&store, src, EdgeKind::Calls);
        let entries = v["callees"].as_array().unwrap();
        assert_eq!(
            entries[0]["class"], "project",
            "Unknown qualified Rust receiver must fall back to `project`"
        );
    }
}
