//! Tree-aware output budgeting for nested call-tree payloads (RFC-0102).
//!
//! `apply_budget` caps *flat* arrays by key, so the nested
//! `get_callee_tree` / `get_caller_tree` payloads escaped budgeting entirely
//! (live QA measured a single default call at ~373 KB). This module closes
//! that hole: it walks the `{ "root": { … } }` payload **breadth-first**,
//! keeps the first `max_nodes` nodes in BFS order, and rebuilds the tree.
//!
//! Traversal-order choice: breadth-first (not depth-first). A budgeted tree
//! is an *overview* — BFS guarantees every direct callee/caller appears
//! before any grandchild, so the near-root structure survives and only the
//! deep tails are cut. Depth-first would instead keep one leftmost spine at
//! full depth and drop entire sibling branches, which reads as "this symbol
//! only calls one thing".
//!
//! Per-node honesty: every kept node whose direct children were cut gains a
//! `children_truncated: K` field (mirroring the ADR-0013 `unresolved_callees`
//! style). The payload root gains the standard `truncated` /
//! `total_available` / `budget {}` metadata, byte-identical in shape to what
//! `apply_budget` emits for flat arrays.

use std::collections::VecDeque;

use serde_json::Value;

use super::{OutputBudget, write_budget_metadata};

/// Count the nodes of a `{ <children_key>: [...] }` tree.
///
/// Recursion depth is bounded by the tools' `max_depth` cap (≤ 10), so this
/// cannot overflow the stack.
fn count_nodes(node: &Value, children_key: &str) -> usize {
    1 + node
        .get(children_key)
        .and_then(Value::as_array)
        .map_or(0, |children| {
            children
                .iter()
                .map(|child| count_nodes(child, children_key))
                .sum()
        })
}

/// Rebuild `root` keeping only the first `max_nodes` nodes in BFS order.
///
/// Every kept node whose direct children were (partially or fully) cut gains
/// a `children_truncated: <count of cut direct children>` field; the field is
/// absent when nothing was cut at that node.
fn truncate_bfs(root: &Value, children_key: &str, max_nodes: usize) -> Value {
    // Pass 1 — BFS over the original tree. `kept[i]` is the i-th kept node in
    // BFS order; `parents[i]` its parent's index in `kept` (unused for root).
    let mut kept: Vec<&Value> = Vec::with_capacity(max_nodes);
    let mut parents: Vec<usize> = Vec::with_capacity(max_nodes);
    kept.push(root);
    parents.push(0);
    let mut queue: VecDeque<usize> = VecDeque::from([0usize]);
    'bfs: while let Some(i) = queue.pop_front() {
        let Some(children) = kept[i].get(children_key).and_then(Value::as_array) else {
            continue;
        };
        for child in children {
            if kept.len() >= max_nodes {
                break 'bfs; // capacity exhausted — all remaining nodes are cut
            }
            kept.push(child);
            parents.push(i);
            queue.push_back(kept.len() - 1);
        }
    }

    // Pass 2 — rebuild. Each new node is the original's scalar fields with an
    // empty children array; children are re-attached bottom-up (a node's
    // index is always greater than its parent's in BFS order, so iterating in
    // reverse completes every subtree before it is attached).
    let mut kept_children_count = vec![0usize; kept.len()];
    for &p in parents.iter().skip(1) {
        kept_children_count[p] += 1;
    }
    let mut rebuilt: Vec<Value> = kept
        .iter()
        .zip(kept_children_count.iter())
        .map(|(orig, &kept_count)| {
            let mut obj = serde_json::Map::new();
            if let Some(map) = orig.as_object() {
                for (k, v) in map {
                    if k != children_key {
                        obj.insert(k.clone(), v.clone());
                    }
                }
            }
            obj.insert(children_key.to_owned(), Value::Array(Vec::new()));
            let original_count = orig
                .get(children_key)
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            if original_count > kept_count {
                obj.insert(
                    "children_truncated".to_owned(),
                    Value::Number((original_count - kept_count).into()),
                );
            }
            Value::Object(obj)
        })
        .collect();
    for i in (1..rebuilt.len()).rev() {
        let node = rebuilt[i].take();
        if let Some(arr) = rebuilt[parents[i]]
            .get_mut(children_key)
            .and_then(Value::as_array_mut)
        {
            // Reverse iteration attaches later siblings first; insert at the
            // front to restore original sibling order. Bounded by
            // `max_nodes` (≤ 50), so the O(n²) inserts are negligible.
            arr.insert(0, node);
        }
    }
    rebuilt.swap_remove(0)
}

/// Truncate a nested `{ "root": { … } }` tree payload to `budget.max_nodes`
/// nodes (breadth-first), in place.
///
/// `children_key` names the per-node subtree array (`"children"` for callee
/// trees, `"callers"` for caller trees). When the tree fits the budget — or
/// the budget is `Disabled` — the payload is left byte-identical. When it is
/// cut, the root gains `truncated: true`, `total_available: <full node
/// count>`, and the standard nested `budget {}` object (same shape as
/// [`super::apply_budget`]); each node with cut direct children gains
/// `children_truncated: K`.
pub fn apply_tree_budget(value: &mut Value, children_key: &str, budget: &OutputBudget) {
    if budget.max_nodes == usize::MAX {
        return; // Disabled — no caps.
    }
    let Some(root) = value.get("root") else {
        return; // Not a tree payload (e.g. an { error } envelope).
    };
    let total = count_nodes(root, children_key);
    if total <= budget.max_nodes {
        return;
    }
    let truncated_root = truncate_bfs(root, children_key, budget.max_nodes);
    value["root"] = truncated_root;
    write_budget_metadata(value, budget, &[("root", total)]);
}
