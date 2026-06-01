//! RED-first TDD tests for the `StorageBackend` trait and `InMemoryBackend`.
//!
//! These tests must FAIL (compile error or assertion failure) before the
//! implementation exists, then pass GREEN after P1-T06 + P1-T07 are implemented.
//!
//! Charter §5.1: tests written before implementation. Dev≠QA rule.

use mycelium_core::store::backend::{StorageBackend, StorageError};
use mycelium_core::store::in_memory::InMemoryBackend;
use mycelium_core::types::{EdgeKind, NodeKind, SourceSpan};

// ── helper ───────────────────────────────────────────────────────────────────

fn make() -> InMemoryBackend {
    InMemoryBackend::new()
}

// ── object-safety ─────────────────────────────────────────────────────────────

/// `StorageBackend` must be usable as a trait object.
#[test]
fn backend_is_object_safe() {
    let b: Box<dyn StorageBackend> = Box::new(InMemoryBackend::new());
    drop(b);
}

/// `StorageError` must implement `std::error::Error`.
#[test]
fn storage_error_is_std_error() {
    let e: &dyn std::error::Error = &StorageError::NotFound;
    let _ = e;
}

// ── node primitives ──────────────────────────────────────────────────────────

#[test]
fn upsert_node_returns_stable_id() {
    let mut b = make();
    let id1 = b.upsert_node("src/lib.rs>main");
    let id2 = b.upsert_node("src/lib.rs>main");
    assert_eq!(id1, id2, "upsert is idempotent");
}

#[test]
fn lookup_path_returns_same_id_as_upsert() {
    let mut b = make();
    let id = b.upsert_node("src/auth.rs>login");
    assert_eq!(b.lookup_path("src/auth.rs>login"), Some(id));
}

#[test]
fn lookup_path_unknown_returns_none() {
    let b = make();
    assert_eq!(b.lookup_path("does/not>exist"), None);
}

#[test]
fn path_of_upserted_node_round_trips() {
    let mut b = make();
    let id = b.upsert_node("src/utils.rs>helper");
    assert_eq!(b.path_of(id), Some("src/utils.rs>helper".to_owned()));
}

#[test]
fn path_of_unknown_id_returns_none() {
    let b = make();
    assert_eq!(b.path_of(mycelium_core::types::NodeId(0xDEAD)), None);
}

#[test]
fn node_count_increments_on_new_nodes() {
    let mut b = make();
    assert_eq!(b.node_count(), 0);
    b.upsert_node("a>x");
    assert_eq!(b.node_count(), 1);
    b.upsert_node("a>y");
    assert_eq!(b.node_count(), 2);
    b.upsert_node("a>x"); // duplicate — no change
    assert_eq!(b.node_count(), 2);
}

#[test]
fn all_paths_returns_all_materialized_paths() {
    let mut b = make();
    b.upsert_node("src/a.rs>f");
    b.upsert_node("src/b.rs>g");
    let mut paths = b.all_paths();
    paths.sort_unstable();
    assert_eq!(paths, ["src/a.rs>f", "src/b.rs>g"]);
}

#[test]
fn remove_node_makes_it_invisible() {
    let mut b = make();
    let id = b.upsert_node("src/lib.rs>target");
    b.remove_node(id);
    assert_eq!(b.lookup_path("src/lib.rs>target"), None);
    assert_eq!(b.path_of(id), None);
}

#[test]
fn remove_node_also_removes_its_edges() {
    let mut b = make();
    let src = b.upsert_node("src/a.rs>caller");
    let dst = b.upsert_node("src/b.rs>callee");
    b.upsert_edge(EdgeKind::Calls, src, dst);
    b.remove_node(src);
    assert!(
        b.outgoing(src, EdgeKind::Calls).is_empty(),
        "edges from removed node must be gone"
    );
    assert!(
        b.incoming(dst, EdgeKind::Calls).is_empty(),
        "reverse edges must also be gone"
    );
}

// ── kind / span ──────────────────────────────────────────────────────────────

#[test]
fn kind_roundtrips() {
    let mut b = make();
    let id = b.upsert_node("src/lib.rs>MyClass");
    b.set_kind(id, NodeKind::Class);
    assert_eq!(b.kind_of(id), Some(NodeKind::Class));
}

#[test]
fn kind_of_unknown_node_returns_none() {
    let b = make();
    assert_eq!(b.kind_of(mycelium_core::types::NodeId(999)), None);
}

#[test]
fn span_roundtrips() {
    let mut b = make();
    let id = b.upsert_node("src/lib.rs>fn_a");
    let span = SourceSpan {
        start_line: 10,
        start_col: 0,
        end_line: 15,
        end_col: 1,
        start_byte: 200,
        end_byte: 350,
    };
    b.set_span(id, span);
    assert_eq!(b.span_of(id), Some(span));
}

#[test]
fn span_of_unknown_node_returns_none() {
    let b = make();
    assert_eq!(b.span_of(mycelium_core::types::NodeId(42)), None);
}

// ── edge primitives ──────────────────────────────────────────────────────────

#[test]
fn upsert_edge_is_idempotent() {
    let mut b = make();
    let a = b.upsert_node("src/a.rs>A");
    let c = b.upsert_node("src/b.rs>C");
    b.upsert_edge(EdgeKind::Calls, a, c);
    b.upsert_edge(EdgeKind::Calls, a, c);
    assert_eq!(b.outgoing(a, EdgeKind::Calls).len(), 1);
}

#[test]
fn outgoing_and_incoming_are_symmetric() {
    let mut b = make();
    let src_node = b.upsert_node("src/a.rs>caller");
    let dst_node = b.upsert_node("src/b.rs>callee");
    b.upsert_edge(EdgeKind::Calls, src_node, dst_node);
    assert!(b.outgoing(src_node, EdgeKind::Calls).contains(&dst_node));
    assert!(b.incoming(dst_node, EdgeKind::Calls).contains(&src_node));
}

#[test]
fn edge_kinds_are_independent() {
    let mut b = make();
    let a = b.upsert_node("src/a.rs>A");
    let c = b.upsert_node("src/b.rs>C");
    b.upsert_edge(EdgeKind::Calls, a, c);
    assert!(b.outgoing(a, EdgeKind::Imports).is_empty());
    assert!(b.incoming(c, EdgeKind::Imports).is_empty());
}

#[test]
fn edge_count_grows_with_unique_edges() {
    let mut b = make();
    let a = b.upsert_node("src/a.rs>A");
    let bnode = b.upsert_node("src/b.rs>B");
    let c = b.upsert_node("src/c.rs>C");
    assert_eq!(b.edge_count(), 0);
    b.upsert_edge(EdgeKind::Calls, a, bnode);
    assert_eq!(b.edge_count(), 1);
    b.upsert_edge(EdgeKind::Calls, a, c);
    assert_eq!(b.edge_count(), 2);
    b.upsert_edge(EdgeKind::Calls, a, bnode); // duplicate
    assert_eq!(b.edge_count(), 2);
}

#[test]
fn all_edges_returns_every_edge() {
    let mut b = make();
    let a = b.upsert_node("x>a");
    let bnode = b.upsert_node("x>b");
    let c = b.upsert_node("x>c");
    b.upsert_edge(EdgeKind::Calls, a, bnode);
    b.upsert_edge(EdgeKind::Imports, a, c);
    let edges = b.all_edges();
    assert_eq!(edges.len(), 2);
    assert!(edges.contains(&(EdgeKind::Calls, a, bnode)));
    assert!(edges.contains(&(EdgeKind::Imports, a, c)));
}

// ── heap estimate ─────────────────────────────────────────────────────────────

#[test]
fn heap_estimate_grows_with_data() {
    let mut small = make();
    small.upsert_node("a>x");
    let small_est = small.heap_size_estimate();

    let mut large = make();
    for i in 0..100usize {
        large.upsert_node(&format!("src/mod.rs>sym_{i}"));
    }
    let large_est = large.heap_size_estimate();
    assert!(
        large_est > small_est,
        "estimate must grow: large={large_est} small={small_est}"
    );
}

// ── flush (no-op for InMemory) ────────────────────────────────────────────────

#[test]
fn flush_is_ok_for_in_memory() {
    let mut b = make();
    b.upsert_node("src/lib.rs>main");
    assert!(b.flush().is_ok());
}
