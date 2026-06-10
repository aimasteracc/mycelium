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
use crate::types::{EdgeKind, NodeId};

/// Build the `{ "callee_paths": [...], "callees": [{path, class}, ...] }` payload
/// for `get_callees` (RFC-0109 Option A shape + RFC-0113 Phase 2 class field).
///
/// `callee_paths` is kept for backward compatibility. `callees` is the additive
/// array where each entry carries the trunk path **and** a static classification:
/// - Paths containing `>` are project-defined symbols → `"project"`.
/// - Bare stub paths (no `>`) are classified against the Python stdlib/builtin/
///   external allowlists ([`crate::classify::classify_python`]). Callers must
///   apply the project-ownership shadow first (only unresolved stubs reach here),
///   which `resolve_bare_call_stubs` already ensures.
///
/// Both arrays are sorted lexicographically by path and deduplicated.
#[must_use]
pub fn callees_payload(store: &Store, id: NodeId, kind: EdgeKind) -> Value {
    use crate::classify::{CalleeClass, classify_python_import_gated};
    use std::collections::HashSet;

    // RFC-0113 Phase 3: build the caller file's import-module set for gating.
    // Extract the file prefix of the caller's path, look up its NodeId, then
    // collect the module-name stems of all its Imports edges.
    let caller_imports: HashSet<String> = store
        .path_of(id)
        .map(|p| p.split('>').next().unwrap_or(p))
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

    let mut entries: Vec<(String, &'static str)> = store
        .outgoing(id, kind)
        .iter()
        .filter_map(|&dst| {
            store.path_of(dst).map(|path| {
                let class = if path.contains('>') {
                    CalleeClass::Project.as_str()
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
/// `count` is the page length (pre-budget); `total_count` is the full match
/// count before `limit`/`offset`. Budgeting (if any) is applied by the caller
/// *after* this, capping the page — so `count`/`total_count` always report the
/// true sizes (RFC-0109: budget caps the selected page).
#[must_use]
pub fn all_symbols_payload(page: &[String], total_count: usize) -> Value {
    json!({ "symbols": page, "count": page.len(), "total_count": total_count })
}

/// Build the `{ "entry_points": [...], "count": N, "total_count": M }` payload
/// for `get_entry_points` from an already-paginated `page` and the
/// pre-pagination `total_count`.
///
/// `count` is the page length (pre-budget); `total_count` is the full match
/// count before `limit`/`offset`. Budgeting (if any) is applied by the caller
/// *after* this, capping the page — so `count`/`total_count` always report the
/// true sizes (mirrors [`all_symbols_payload`]; RFC-0109: budget caps the page).
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
}
