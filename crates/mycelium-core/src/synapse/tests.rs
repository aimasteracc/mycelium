//! Synapse tests — written before implementation per Charter §5.1.

use super::{AdjacencyList, Synapse};
use crate::types::{EdgeKind, NodeId};

fn n(x: u64) -> NodeId {
    NodeId(x << 8) // honor the reserved shard-tag byte
}

// ──────────────────────────────────────────────────────────────────────
// AdjacencyList
// ──────────────────────────────────────────────────────────────────────

#[test]
fn adjacency_forward_outgoing_returns_targets() {
    let mut adj = AdjacencyList::new();
    adj.add(n(1), n(2));
    adj.add(n(1), n(3));
    let mut out = adj.outgoing(n(1)).to_vec();
    out.sort();
    let mut want = vec![n(2), n(3)];
    want.sort();
    assert_eq!(out, want);
}

#[test]
fn adjacency_reverse_incoming_returns_sources() {
    let mut adj = AdjacencyList::new();
    adj.add(n(1), n(10));
    adj.add(n(2), n(10));
    let mut inc = adj.incoming(n(10)).to_vec();
    inc.sort();
    let mut want = vec![n(1), n(2)];
    want.sort();
    assert_eq!(inc, want);
}

#[test]
fn adjacency_unknown_node_returns_empty_slices() {
    let adj = AdjacencyList::new();
    assert!(adj.outgoing(n(99)).is_empty());
    assert!(adj.incoming(n(99)).is_empty());
}

#[test]
fn adjacency_add_is_idempotent() {
    let mut adj = AdjacencyList::new();
    adj.add(n(1), n(2));
    adj.add(n(1), n(2));
    adj.add(n(1), n(2));
    assert_eq!(adj.outgoing(n(1)), &[n(2)]);
    assert_eq!(adj.incoming(n(2)), &[n(1)]);
    assert_eq!(adj.edge_count(), 1);
}

#[test]
fn adjacency_remove_node_drops_all_involving_edges() {
    let mut adj = AdjacencyList::new();
    adj.add(n(1), n(2));
    adj.add(n(2), n(3));
    adj.add(n(4), n(2));

    adj.remove_node(n(2));

    assert!(adj.outgoing(n(2)).is_empty());
    assert!(adj.incoming(n(2)).is_empty());
    assert!(adj.outgoing(n(1)).is_empty(), "1->2 should be gone");
    assert!(adj.outgoing(n(4)).is_empty(), "4->2 should be gone");
    assert!(adj.incoming(n(3)).is_empty(), "2->3 should be gone");
}

// ──────────────────────────────────────────────────────────────────────
// Synapse (multi-kind)
// ──────────────────────────────────────────────────────────────────────

#[test]
fn synapse_edge_kinds_are_isolated() {
    // RFC-0001 §3.2: each EdgeKind has its own adjacency list.
    let mut syn = Synapse::new();
    syn.add(EdgeKind::Calls, n(1), n(2));
    syn.add(EdgeKind::Extends, n(1), n(3));

    assert_eq!(syn.outgoing(n(1), EdgeKind::Calls), &[n(2)]);
    assert_eq!(syn.outgoing(n(1), EdgeKind::Extends), &[n(3)]);
    assert!(syn.outgoing(n(1), EdgeKind::Implements).is_empty());
}

#[test]
fn synapse_forward_and_reverse_both_materialize() {
    let mut syn = Synapse::new();
    syn.add(EdgeKind::Calls, n(1), n(2));
    assert_eq!(syn.outgoing(n(1), EdgeKind::Calls), &[n(2)]);
    assert_eq!(syn.incoming(n(2), EdgeKind::Calls), &[n(1)]);
}

#[test]
fn synapse_remove_node_drops_across_all_kinds() {
    let mut syn = Synapse::new();
    syn.add(EdgeKind::Calls, n(1), n(2));
    syn.add(EdgeKind::Extends, n(1), n(3));
    syn.add(EdgeKind::Implements, n(4), n(1));

    syn.remove_node(n(1));

    assert!(syn.outgoing(n(1), EdgeKind::Calls).is_empty());
    assert!(syn.outgoing(n(1), EdgeKind::Extends).is_empty());
    assert!(syn.incoming(n(1), EdgeKind::Implements).is_empty());
    assert!(syn.outgoing(n(4), EdgeKind::Implements).is_empty());
}
