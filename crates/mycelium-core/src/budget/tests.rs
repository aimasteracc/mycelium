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
