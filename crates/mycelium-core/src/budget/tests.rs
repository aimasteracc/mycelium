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
