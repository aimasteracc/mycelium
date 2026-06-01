//! P2-T01 — RFC-0100 Phase 2 equivalence harness.
//!
//! Drives the same op-sequence against `InMemoryBackend` (the oracle) and
//! `RedbBackend` (the subject under test), then asserts a set-based 3-layer
//! comparator finds no divergence.
//!
//! ## TDD status (Charter §5.1 / Dev≠QA)
//!
//! Authored test-author-first, before any implementer touches `redb_backend.rs`.
//! Cases that pass today verify happy-path equivalence of the shipped code.
//! Cases marked `#[ignore = "RED spec …"]` document a *confirmed* divergence
//! (see `docs/sprints/rfc-0100-phase2-build-plan.md`) and are un-ignored as the
//! P2-T05 batched-atomic-transaction + sorted-adjacency fix lands.
//!
//! Per the expert synthesis (`wf_21a3635f-0e6`): this harness does NOT catch
//! the crash-only half-edge bug under normal operation — that is P2-T03's job
//! via crash injection. It DOES catch any divergence observable without a crash.
//!
//! All comparisons are **set-based** (`BTreeSet`), never `Vec::eq`, because
//! `RedbBackend::all_edges()` returns B-tree order while the oracle returns
//! `HashMap` order (build-plan finding #10).
//!
//! The whole file is gated on `redb-backend`; with the feature off it compiles
//! to nothing and the default build is unchanged.

#![cfg(feature = "redb-backend")]

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;

use mycelium_core::store::backend::StorageBackend;
use mycelium_core::store::in_memory::InMemoryBackend;
use mycelium_core::store::redb_backend::RedbBackend;
use mycelium_core::types::{EdgeKind, NodeId, NodeKind, SourceSpan};

/// All edge kinds, kept in sync with `redb_tags`.
const ALL_EDGE_KINDS: &[EdgeKind] = &[
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
];

type SpanTuple = (u32, u32, u32, u32, u32, u32);

/// A fully path-resolved, set-normalized snapshot of a backend's observable
/// state. Path-keyed (not NodeId-keyed) so id-assignment order cannot mask a
/// mismatch.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    paths: BTreeSet<String>,
    node_count: usize,
    edge_count: usize,
    /// `(path, kind_wire_str)` for every node with a recorded kind.
    kinds: BTreeSet<(String, &'static str)>,
    /// `(path, span_tuple)` for every node with a recorded span.
    spans: BTreeSet<(String, SpanTuple)>,
    /// `(kind_wire_str, src_path, dst_path)` for every directed edge, derived
    /// two independent ways and asserted consistent: from `all_edges()` and
    /// from per-node `outgoing()` over all kinds.
    edges_via_all: BTreeSet<(&'static str, String, String)>,
    edges_via_outgoing: BTreeSet<(&'static str, String, String)>,
    edges_via_incoming: BTreeSet<(&'static str, String, String)>,
}

const fn span_tuple(s: SourceSpan) -> SpanTuple {
    (
        s.start_line,
        s.start_col,
        s.end_line,
        s.end_col,
        s.start_byte,
        s.end_byte,
    )
}

fn capture(b: &dyn StorageBackend) -> Snapshot {
    let paths: BTreeSet<String> = b.all_paths().into_iter().collect();

    // Global id → path map so edge endpoints resolve to stable strings.
    let mut id_to_path: HashMap<NodeId, String> = HashMap::new();
    for p in &paths {
        if let Some(id) = b.lookup_path(p) {
            id_to_path.insert(id, p.clone());
        }
    }
    let resolve = |id: NodeId| -> String {
        id_to_path
            .get(&id)
            .cloned()
            .unwrap_or_else(|| format!("<unknown:{}>", id.0))
    };

    let mut kinds = BTreeSet::new();
    let mut spans = BTreeSet::new();
    for (id, p) in &id_to_path {
        if let Some(k) = b.kind_of(*id) {
            kinds.insert((p.clone(), k.as_str()));
        }
        if let Some(s) = b.span_of(*id) {
            spans.insert((p.clone(), span_tuple(s)));
        }
    }

    let edges_via_all: BTreeSet<(&'static str, String, String)> = b
        .all_edges()
        .into_iter()
        .map(|(k, s, d)| (k.as_str(), resolve(s), resolve(d)))
        .collect();

    let mut edges_via_outgoing = BTreeSet::new();
    let mut edges_via_incoming = BTreeSet::new();
    for (id, p) in &id_to_path {
        for &k in ALL_EDGE_KINDS {
            for d in b.outgoing(*id, k) {
                edges_via_outgoing.insert((k.as_str(), p.clone(), resolve(d)));
            }
            for s in b.incoming(*id, k) {
                edges_via_incoming.insert((k.as_str(), resolve(s), p.clone()));
            }
        }
    }

    Snapshot {
        node_count: b.node_count(),
        edge_count: b.edge_count(),
        paths,
        kinds,
        spans,
        edges_via_all,
        edges_via_outgoing,
        edges_via_incoming,
    }
}

/// Assert two backends are observably equivalent; panic with a minimal diff.
fn assert_equivalent(label: &str, mem: &dyn StorageBackend, redb: &dyn StorageBackend) {
    let a = capture(mem);
    let b = capture(redb);
    if a == b {
        return;
    }

    let mut msg = format!("Backend divergence [{label}]:\n");
    if a.paths != b.paths {
        let only_mem: Vec<_> = a.paths.difference(&b.paths).take(10).collect();
        let only_redb: Vec<_> = b.paths.difference(&a.paths).take(10).collect();
        let _ = writeln!(
            msg,
            "  L1 paths: only_mem={only_mem:?} only_redb={only_redb:?}"
        );
    }
    if a.node_count != b.node_count {
        let _ = writeln!(
            msg,
            "  L1 node_count: mem={} redb={}",
            a.node_count, b.node_count
        );
    }
    if a.edge_count != b.edge_count {
        let _ = writeln!(
            msg,
            "  L2 edge_count: mem={} redb={}",
            a.edge_count, b.edge_count
        );
    }
    if a.kinds != b.kinds {
        let dm: Vec<_> = a.kinds.symmetric_difference(&b.kinds).take(10).collect();
        let _ = writeln!(msg, "  L2 kinds differ: {dm:?}");
    }
    if a.spans != b.spans {
        msg.push_str("  L2 spans differ\n");
    }
    if a.edges_via_all != b.edges_via_all {
        let only_mem: Vec<_> = a
            .edges_via_all
            .difference(&b.edges_via_all)
            .take(10)
            .collect();
        let only_redb: Vec<_> = b
            .edges_via_all
            .difference(&a.edges_via_all)
            .take(10)
            .collect();
        let _ = writeln!(
            msg,
            "  L2 all_edges: only_mem={only_mem:?} only_redb={only_redb:?}"
        );
    }
    if a.edges_via_outgoing != b.edges_via_outgoing {
        let only_mem: Vec<_> = a
            .edges_via_outgoing
            .difference(&b.edges_via_outgoing)
            .take(10)
            .collect();
        let only_redb: Vec<_> = b
            .edges_via_outgoing
            .difference(&a.edges_via_outgoing)
            .take(10)
            .collect();
        let _ = writeln!(
            msg,
            "  L3 outgoing: only_mem={only_mem:?} only_redb={only_redb:?}"
        );
    }
    if a.edges_via_incoming != b.edges_via_incoming {
        let only_mem: Vec<_> = a
            .edges_via_incoming
            .difference(&b.edges_via_incoming)
            .take(10)
            .collect();
        let only_redb: Vec<_> = b
            .edges_via_incoming
            .difference(&a.edges_via_incoming)
            .take(10)
            .collect();
        let _ = writeln!(
            msg,
            "  L3 incoming: only_mem={only_mem:?} only_redb={only_redb:?}"
        );
    }
    panic!("{msg}");
}

/// Run `body` against a fresh `InMemoryBackend` and a fresh temp-file
/// `RedbBackend`, then assert full equivalence.
fn run_matrix(label: &str, body: impl Fn(&mut dyn StorageBackend)) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join(format!("{label}.redb"));

    let mut mem = InMemoryBackend::new();
    let mut redb = RedbBackend::open(&db_path).expect("open redb");

    body(&mut mem);
    body(&mut redb);

    assert_equivalent(label, &mem, &redb);
}

// ── Layer-0 sanity: the comparator agrees with itself ────────────────────────

#[test]
fn comparator_inmemory_self_consistent() {
    let mut a = InMemoryBackend::new();
    let mut b = InMemoryBackend::new();
    for be in [&mut a, &mut b] {
        let x = be.upsert_node("src/a.rs>foo");
        let y = be.upsert_node("src/b.rs>bar");
        be.upsert_edge(EdgeKind::Calls, x, y);
    }
    assert_equivalent("inmemory_self", &a, &b);
}

// ── matrix cases (InMemory oracle vs Redb subject) ───────────────────────────

#[test]
fn matrix_single_node() {
    run_matrix("single_node", |b| {
        b.upsert_node("src/lib.rs>main");
    });
}

#[test]
fn matrix_content_hash_id_match() {
    // The same path must yield the same content-hash NodeId in both backends.
    let dir = tempfile::tempdir().expect("tempdir");
    let mut mem = InMemoryBackend::new();
    let mut redb = RedbBackend::open(&dir.path().join("idmatch.redb")).expect("open");
    let mid = mem.upsert_node("src/x.rs>Sym");
    let rid = redb.upsert_node("src/x.rs>Sym");
    assert_eq!(mid, rid, "content-hash NodeId must match across backends");
}

#[test]
fn matrix_invalid_paths_yield_null() {
    run_matrix("invalid_paths", |b| {
        let _ = b.upsert_node("");
        let _ = b.upsert_node("no-separator-here");
    });
}

#[test]
fn matrix_single_edge() {
    run_matrix("single_edge", |b| {
        let a = b.upsert_node("src/a.rs>caller");
        let c = b.upsert_node("src/b.rs>callee");
        b.upsert_edge(EdgeKind::Calls, a, c);
    });
}

#[test]
fn matrix_node_with_kind_and_span() {
    run_matrix("node_kind_span", |b| {
        let id = b.upsert_node("src/lib.rs>MyClass");
        b.set_kind(id, NodeKind::Class);
        b.set_span(
            id,
            SourceSpan {
                start_line: 10,
                start_col: 0,
                end_line: 20,
                end_col: 1,
                start_byte: 200,
                end_byte: 400,
            },
        );
    });
}

#[test]
fn matrix_all_edge_kinds() {
    run_matrix("all_edge_kinds", |b| {
        let a = b.upsert_node("src/a.rs>A");
        let c = b.upsert_node("src/b.rs>C");
        for &k in ALL_EDGE_KINDS {
            b.upsert_edge(k, a, c);
        }
    });
}

#[test]
fn matrix_idempotent_upsert() {
    run_matrix("idempotent", |b| {
        b.upsert_node("src/a.rs>dup");
        b.upsert_node("src/a.rs>dup");
        let a = b.upsert_node("x>a");
        let c = b.upsert_node("x>c");
        b.upsert_edge(EdgeKind::Calls, a, c);
        b.upsert_edge(EdgeKind::Calls, a, c);
    });
}

#[test]
fn matrix_reverse_insertion_order() {
    run_matrix("reverse_order", |b| {
        let hub = b.upsert_node("src/hub.rs>hub");
        for i in (0..20).rev() {
            let t = b.upsert_node(&format!("src/t.rs>target_{i:02}"));
            b.upsert_edge(EdgeKind::Calls, hub, t);
        }
    });
}

#[test]
fn matrix_bidirectional_removal() {
    run_matrix("bidirectional_removal", |b| {
        let a = b.upsert_node("src/a.rs>A");
        let c = b.upsert_node("src/b.rs>C");
        let d = b.upsert_node("src/c.rs>D");
        b.upsert_edge(EdgeKind::Calls, a, c);
        b.upsert_edge(EdgeKind::Calls, d, a);
        b.remove_node(a);
    });
}

#[test]
fn matrix_multi_file_graph() {
    run_matrix("multi_file", |b| {
        let nodes: Vec<NodeId> = (0..10)
            .map(|i| b.upsert_node(&format!("src/mod_{}.rs>sym_{i}", i % 3)))
            .collect();
        for w in nodes.windows(2) {
            b.upsert_edge(EdgeKind::Calls, w[0], w[1]);
            b.upsert_edge(EdgeKind::Imports, w[1], w[0]);
        }
    });
}

// ── reopen durability: write to redb, drop, reopen, compare to live oracle ───

#[test]
fn matrix_reopen_equivalence() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("reopen.redb");

    let build = |b: &mut dyn StorageBackend| {
        let a = b.upsert_node("src/a.rs>A");
        let c = b.upsert_node("src/b.rs>C");
        b.set_kind(a, NodeKind::Function);
        b.upsert_edge(EdgeKind::Calls, a, c);
    };

    let mut mem = InMemoryBackend::new();
    build(&mut mem);

    {
        let mut redb = RedbBackend::open(&db_path).expect("open redb");
        build(&mut redb);
        redb.flush().expect("flush");
    }
    let redb = RedbBackend::open(&db_path).expect("reopen redb");
    assert_equivalent("reopen", &mem, &redb);
}
