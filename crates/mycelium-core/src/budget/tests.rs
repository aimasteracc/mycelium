//! Tests for the shared output budget — written RED-first (Charter §5.1).

use super::{OutputBudget, apply_budget};
use serde_json::json;

#[test]
fn for_project_tiers() {
    assert_eq!(OutputBudget::for_project(100).max_nodes, 15);
    assert_eq!(OutputBudget::for_project(100).max_edges, 30);
    assert_eq!(OutputBudget::for_project(1_000).max_nodes, 30);
    assert_eq!(OutputBudget::for_project(1_000).max_edges, 60);
    assert_eq!(OutputBudget::for_project(50_000).max_nodes, 50);
    assert_eq!(OutputBudget::for_project(50_000).max_edges, 100);
}

#[test]
fn truncates_nodes_and_marks_total_available() {
    let mut v = json!({ "nodes": (0..40).collect::<Vec<_>>() });
    apply_budget(&mut v, &OutputBudget::for_project(100)); // max_nodes = 15
    assert_eq!(v["nodes"].as_array().unwrap().len(), 15);
    assert_eq!(v["truncated"], true);
    assert_eq!(v["total_available"], 40);
}

#[test]
fn truncates_edges_at_edge_cap() {
    let mut v = json!({ "edges": (0..200).collect::<Vec<_>>() });
    apply_budget(&mut v, &OutputBudget::for_project(100)); // max_edges = 30
    assert_eq!(v["edges"].as_array().unwrap().len(), 30);
    assert_eq!(v["truncated"], true);
}

#[test]
fn truncates_entry_points_at_node_cap() {
    // Codex P2 on PR #689: `entry_points` (emitted by mycelium_get_entry_points)
    // must be a budgeted node-shaped key, else a no-limit / default-auto call on
    // a large repo still returns the full multi-10K-token array.
    let mut v = json!({ "entry_points": (0..40).collect::<Vec<_>>() });
    apply_budget(&mut v, &OutputBudget::for_project(100)); // max_nodes = 15
    assert_eq!(v["entry_points"].as_array().unwrap().len(), 15);
    assert_eq!(v["truncated"], true);
    assert_eq!(v["total_available"], 40);
}

#[test]
fn no_truncation_when_under_limit() {
    let mut v = json!({ "nodes": [1, 2, 3], "edges": [1, 2] });
    apply_budget(&mut v, &OutputBudget::for_project(100));
    assert_eq!(v["nodes"].as_array().unwrap().len(), 3);
    assert!(v.get("truncated").is_none());
}

#[test]
fn absent_keys_are_ignored() {
    let mut v = json!({ "verdict": "INFO" });
    apply_budget(&mut v, &OutputBudget::for_project(100));
    assert!(v.get("truncated").is_none());
    assert_eq!(v["verdict"], "INFO");
}

// ---- RFC-0102 pending piece (2): nested `budget {}` response object ----

use super::BudgetMode;

#[test]
fn for_project_tags_mode() {
    assert_eq!(OutputBudget::for_project(100).mode, BudgetMode::Small);
    assert_eq!(OutputBudget::for_project(1_000).mode, BudgetMode::Medium);
    assert_eq!(OutputBudget::for_project(50_000).mode, BudgetMode::Large);
}

#[test]
fn budget_mode_serializes_lowercase() {
    assert_eq!(BudgetMode::Small.as_str(), "small");
    assert_eq!(BudgetMode::Medium.as_str(), "medium");
    assert_eq!(BudgetMode::Large.as_str(), "large");
}

#[test]
fn truncation_emits_nested_budget_object() {
    let mut v = json!({
        "nodes": (0..40).collect::<Vec<_>>(),
        "edges": (0..200).collect::<Vec<_>>(),
    });
    apply_budget(&mut v, &OutputBudget::for_project(100)); // small: 15 nodes / 30 edges

    // Flat fields preserved for backward compatibility.
    assert_eq!(v["truncated"], true);
    assert_eq!(v["total_available"], 40);

    // New nested object per RFC-0102 §"Response metadata".
    let b = &v["budget"];
    assert_eq!(b["mode"], "small");
    assert_eq!(b["truncated"], true);
    // truncated_fields lists every capped key, in deterministic (node-then-edge) order.
    assert_eq!(b["truncated_fields"], json!(["nodes", "edges"]));
    // total_available is a per-field map (not the single flat number).
    assert_eq!(b["total_available"]["nodes"], 40);
    assert_eq!(b["total_available"]["edges"], 200);
    // limits echo the budget caps that were applied.
    assert_eq!(b["limits"]["max_nodes"], 15);
    assert_eq!(b["limits"]["max_edges"], 30);
}

#[test]
fn nested_budget_truncated_fields_only_lists_capped_keys() {
    // Only edges overflow; nodes are under the cap.
    let mut v = json!({
        "nodes": [1, 2, 3],
        "edges": (0..200).collect::<Vec<_>>(),
    });
    apply_budget(&mut v, &OutputBudget::for_project(100));
    let b = &v["budget"];
    assert_eq!(b["truncated_fields"], json!(["edges"]));
    assert_eq!(b["total_available"]["edges"], 200);
    assert!(
        b["total_available"].get("nodes").is_none(),
        "uncapped fields must not appear in total_available"
    );
}

#[test]
fn no_nested_budget_object_when_nothing_truncated() {
    // Increment 1 scope: nested object mirrors flat-field semantics — only
    // present on truncation. (Always-on metadata ships with the request knob.)
    let mut v = json!({ "nodes": [1, 2, 3], "edges": [1, 2] });
    apply_budget(&mut v, &OutputBudget::for_project(100));
    assert!(v.get("budget").is_none());
    assert!(v.get("truncated").is_none());
}

// ---- RFC-0102 pending piece (1): per-call `budget` request override knob ----

use super::BudgetOverride;

#[test]
fn resolve_none_and_auto_track_project_size() {
    // No override (and explicit Auto) follow the project-size tier.
    assert_eq!(
        OutputBudget::resolve(None, 100),
        OutputBudget::for_project(100)
    );
    assert_eq!(
        OutputBudget::resolve(Some(BudgetOverride::Auto), 50_000),
        OutputBudget::for_project(50_000)
    );
}

#[test]
fn resolve_explicit_tiers_ignore_project_size() {
    // An explicit tier pins the caps regardless of how big the project is.
    let small = OutputBudget::resolve(Some(BudgetOverride::Small), 50_000);
    assert_eq!(small.mode, BudgetMode::Small);
    assert_eq!((small.max_nodes, small.max_edges), (15, 30));

    let medium = OutputBudget::resolve(Some(BudgetOverride::Medium), 10);
    assert_eq!(medium.mode, BudgetMode::Medium);
    assert_eq!((medium.max_nodes, medium.max_edges), (30, 60));

    let large = OutputBudget::resolve(Some(BudgetOverride::Large), 10);
    assert_eq!(large.mode, BudgetMode::Large);
    assert_eq!((large.max_nodes, large.max_edges), (50, 100));
}

#[test]
fn resolve_disabled_imposes_no_caps_and_never_truncates() {
    let b = OutputBudget::resolve(Some(BudgetOverride::Disabled), 50_000);
    assert_eq!(b.mode, BudgetMode::Disabled);
    let mut v = json!({ "nodes": (0..10_000).collect::<Vec<_>>() });
    apply_budget(&mut v, &b);
    assert_eq!(v["nodes"].as_array().unwrap().len(), 10_000);
    assert!(v.get("truncated").is_none());
    assert!(v.get("budget").is_none());
}

#[test]
fn budget_override_parses_case_insensitively() {
    assert_eq!(
        "auto".parse::<BudgetOverride>().unwrap(),
        BudgetOverride::Auto
    );
    assert_eq!(
        "Small".parse::<BudgetOverride>().unwrap(),
        BudgetOverride::Small
    );
    assert_eq!(
        "MEDIUM".parse::<BudgetOverride>().unwrap(),
        BudgetOverride::Medium
    );
    assert_eq!(
        "large".parse::<BudgetOverride>().unwrap(),
        BudgetOverride::Large
    );
    assert_eq!(
        "disabled".parse::<BudgetOverride>().unwrap(),
        BudgetOverride::Disabled
    );
}

#[test]
fn budget_override_rejects_unknown_value() {
    let err = "huge".parse::<BudgetOverride>().unwrap_err();
    assert!(
        err.contains("huge"),
        "error should name the bad value: {err}"
    );
}

#[test]
fn disabled_mode_wire_token() {
    assert_eq!(BudgetMode::Disabled.as_str(), "disabled");
}

// ---- RFC-0102 key coverage: tools emit `callee_paths` / `caller_paths` /
//      `dead_symbols` / `isolated_symbols`, which were NOT in the cap list, so
//      `apply_budget` silently no-opped for get_callees/get_callers/
//      get_dead_symbols/get_isolated_symbols. These caps close that gap. ----

#[test]
fn caps_callee_and_caller_paths_at_edge_cap() {
    for key in ["callee_paths", "caller_paths"] {
        let mut v = json!({ key: (0..200).collect::<Vec<_>>() });
        apply_budget(&mut v, &OutputBudget::for_project(100)); // max_edges = 30
        assert_eq!(
            v[key].as_array().unwrap().len(),
            30,
            "{key} should be capped at max_edges"
        );
        assert_eq!(v["truncated"], true);
        assert_eq!(v["budget"]["truncated_fields"], json!([key]));
        assert_eq!(v["budget"]["total_available"][key], 200);
    }
}

// ---- Post-budget `count` consistency: a paginated payload carries both
//      `count` (page size) and `total_count` (full total). When the budget
//      truncates the page array, `count` must follow the array so an agent
//      never reads a `count` that contradicts the array it iterates.
//      Payloads WITHOUT `total_count` (dead/isolated/reachable) document
//      `count` as the full pre-budget total — those must stay untouched. ----

#[test]
fn truncation_updates_count_when_total_count_sibling_present() {
    // Mirrors the live bug: get-entry-points with no limit on a 2425-entry
    // repo — page == full set, count == 2425, budget truncates to 30 but the
    // old code left count at 2425, contradicting the 30-element array.
    for key in ["entry_points", "symbols"] {
        let mut v = json!({
            key: (0..40).collect::<Vec<_>>(),
            "count": 40,
            "total_count": 2425,
        });
        apply_budget(&mut v, &OutputBudget::for_project(100)); // max_nodes = 15
        assert_eq!(v[key].as_array().unwrap().len(), 15);
        assert_eq!(
            v["count"], 15,
            "{key}: count must equal the returned array length after budget truncation"
        );
        assert_eq!(
            v["total_count"], 2425,
            "{key}: total_count carries the full total and must not change"
        );
        assert_eq!(v["budget"]["total_available"][key], 40);
    }
}

#[test]
fn truncation_leaves_count_alone_without_total_count_sibling() {
    // dead_symbols/isolated_symbols/reachable payloads have no total_count;
    // their `count` is documented as the full pre-budget total. Don't touch it.
    let mut v = json!({ "dead_symbols": (0..40).collect::<Vec<_>>(), "count": 40 });
    apply_budget(&mut v, &OutputBudget::for_project(100)); // max_nodes = 15
    assert_eq!(v["dead_symbols"].as_array().unwrap().len(), 15);
    assert_eq!(
        v["count"], 40,
        "without a total_count sibling, count is the only full-total carrier"
    );
}

#[test]
fn truncation_leaves_mismatched_count_alone() {
    // Conservative guard: if `count` did not equal the pre-truncation array
    // length, it means something else — leave it as-is.
    let mut v = json!({
        "entry_points": (0..40).collect::<Vec<_>>(),
        "count": 7,
        "total_count": 99,
    });
    apply_budget(&mut v, &OutputBudget::for_project(100));
    assert_eq!(v["count"], 7);
}

#[test]
fn caps_dead_and_isolated_symbols_at_node_cap() {
    for key in ["dead_symbols", "isolated_symbols"] {
        let mut v = json!({ key: (0..40).collect::<Vec<_>>() });
        apply_budget(&mut v, &OutputBudget::for_project(100)); // max_nodes = 15
        assert_eq!(
            v[key].as_array().unwrap().len(),
            15,
            "{key} should be capped at max_nodes"
        );
        assert_eq!(v["truncated"], true);
        assert_eq!(v["budget"]["total_available"][key], 40);
    }
}

// ---- Budget holes (live QA, HIGH): `mycelium_query` emits `matches`,
//      `get_cross_refs` emits `importers`/`extended_by`/`implemented_by` —
//      none were in the cap lists, so those tools dumped unbounded output
//      while their siblings truncated. These caps close the flat-array gap. ----

#[test]
fn caps_query_matches_at_node_cap() {
    let mut v = json!({
        "matches": (0..40).collect::<Vec<_>>(),
        "count": 40,
        "total_count": 40,
    });
    apply_budget(&mut v, &OutputBudget::for_project(100)); // max_nodes = 15
    assert_eq!(v["matches"].as_array().unwrap().len(), 15);
    assert_eq!(
        v["count"], 15,
        "count must equal the returned array length after budget truncation"
    );
    assert_eq!(v["total_count"], 40, "total_count carries the full total");
    assert_eq!(v["truncated"], true);
    assert_eq!(v["budget"]["total_available"]["matches"], 40);
}

#[test]
fn caps_cross_ref_groups_at_edge_cap() {
    let mut v = json!({
        "callers": (0..200).collect::<Vec<_>>(),
        "importers": (0..50).collect::<Vec<_>>(),
        "extended_by": [1, 2],
        "implemented_by": (0..31).collect::<Vec<_>>(),
    });
    apply_budget(&mut v, &OutputBudget::for_project(100)); // max_edges = 30
    assert_eq!(v["callers"].as_array().unwrap().len(), 30);
    assert_eq!(v["importers"].as_array().unwrap().len(), 30);
    assert_eq!(
        v["extended_by"].as_array().unwrap().len(),
        2,
        "groups under the cap stay whole"
    );
    assert_eq!(v["implemented_by"].as_array().unwrap().len(), 30);
    assert_eq!(v["truncated"], true);
    assert_eq!(
        v["budget"]["truncated_fields"],
        json!(["callers", "importers", "implemented_by"])
    );
    assert_eq!(v["budget"]["total_available"]["callers"], 200);
    assert_eq!(v["budget"]["total_available"]["importers"], 50);
    assert_eq!(v["budget"]["total_available"]["implemented_by"], 31);
}

// ---- Tree-aware budgeting (`apply_tree_budget`): callee/caller trees are
//      nested, so the flat-key `apply_budget` cannot cap them. The tree
//      variant walks the `{ "root": {...} }` payload breadth-first, keeps the
//      first `max_nodes` nodes in BFS order, marks every node whose direct
//      children were cut with `children_truncated: K`, and emits the standard
//      root metadata (`truncated` / `total_available` / `budget {}`). ----

use super::apply_tree_budget;

/// Count nodes of a `{path, <children_key>: [...]}` tree.
fn count_tree_nodes(node: &serde_json::Value, children_key: &str) -> usize {
    1 + node[children_key].as_array().map_or(0, |c| {
        c.iter()
            .map(|child| count_tree_nodes(child, children_key))
            .sum()
    })
}

/// A root with `width` children, each having one grandchild:
/// `1 + width + width` nodes total.
fn wide_tree(width: usize, children_key: &str) -> serde_json::Value {
    let children: Vec<serde_json::Value> = (0..width)
        .map(|i| {
            json!({
                "path": format!("src/lib.rs>child{i}"),
                children_key: [ { "path": format!("src/lib.rs>grandchild{i}"), children_key: [] } ],
            })
        })
        .collect();
    json!({ "root": { "path": "src/lib.rs>root", children_key: children } })
}

#[test]
fn tree_budget_caps_node_count_breadth_first() {
    // 1 root + 20 children + 20 grandchildren = 41 nodes; small cap = 15.
    let mut v = wide_tree(20, "children");
    apply_tree_budget(&mut v, "children", &OutputBudget::for_project(100));

    assert_eq!(
        count_tree_nodes(&v["root"], "children"),
        15,
        "serialized node count must equal max_nodes: {v}"
    );
    // BFS keeps the near-root overview: root + its first 14 children; no
    // grandchild is kept while a direct child was cut.
    let children = v["root"]["children"].as_array().unwrap();
    assert_eq!(children.len(), 14);
    for child in children {
        assert!(
            child["children"].as_array().unwrap().is_empty(),
            "BFS must cut grandchildren before direct children: {child}"
        );
        assert_eq!(
            child["children_truncated"], 1,
            "each kept child lost its 1 grandchild: {child}"
        );
    }
    assert_eq!(
        v["root"]["children_truncated"], 6,
        "root had 20 children, 14 kept"
    );
    // Standard root metadata, same shape as apply_budget.
    assert_eq!(v["truncated"], true);
    assert_eq!(v["total_available"], 41);
    assert_eq!(v["budget"]["mode"], "small");
    assert_eq!(v["budget"]["truncated"], true);
    assert_eq!(v["budget"]["truncated_fields"], json!(["root"]));
    assert_eq!(v["budget"]["total_available"]["root"], 41);
    assert_eq!(v["budget"]["limits"]["max_nodes"], 15);
}

#[test]
fn tree_budget_works_for_caller_trees_children_key() {
    // Caller trees nest under "callers", not "children".
    let mut v = wide_tree(20, "callers");
    apply_tree_budget(&mut v, "callers", &OutputBudget::for_project(100));
    assert_eq!(count_tree_nodes(&v["root"], "callers"), 15);
    assert_eq!(v["truncated"], true);
    assert_eq!(v["total_available"], 41);
}

#[test]
fn tree_budget_no_op_under_cap() {
    let mut v = wide_tree(3, "children"); // 7 nodes < 15
    let before = v.clone();
    apply_tree_budget(&mut v, "children", &OutputBudget::for_project(100));
    assert_eq!(v, before, "payload under the cap must be byte-identical");
}

#[test]
fn tree_budget_disabled_is_a_no_op() {
    let mut v = wide_tree(50, "children"); // 101 nodes
    let before = v.clone();
    apply_tree_budget(
        &mut v,
        "children",
        &OutputBudget::resolve(Some(BudgetOverride::Disabled), 50_000),
    );
    assert_eq!(v, before, "budget=disabled must return the full tree");
}

#[test]
fn tree_budget_preserves_unresolved_callees_on_kept_nodes() {
    // ADR-0013 counts on kept nodes survive truncation untouched.
    let mut v = json!({
        "root": {
            "path": "src/lib.rs>root",
            "unresolved_callees": 3,
            "children": (0..20).map(|i| json!({
                "path": format!("src/lib.rs>c{i}"),
                "unresolved_callees": 1,
                "children": [],
            })).collect::<Vec<_>>(),
        }
    });
    apply_tree_budget(&mut v, "children", &OutputBudget::for_project(100));
    assert_eq!(v["root"]["unresolved_callees"], 3);
    assert_eq!(v["root"]["children"][0]["unresolved_callees"], 1);
    assert_eq!(v["root"]["children_truncated"], 6, "20 children, 14 kept");
}
