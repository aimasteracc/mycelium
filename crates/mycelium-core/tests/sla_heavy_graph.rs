//! Charter §2 SLA assertions for heavy-graph algorithms on a 100 K-node Store.
//!
//! Charter §2 SLA row: Heavy-graph tools (`leaf_symbols`, `degree_histogram`,
//! `graph_metrics`, `page_rank`, `wcc`, `find_call_path`) on a 100 K-node
//! graph must complete in < 30 s.
//!
//! Graph fixture: 100 000 symbol nodes, ~300 000 directed `Calls` edges (3 per
//! node, deterministic LCG shuffle). Sparse graph matching real codebases
//! (average fan-out ≈ 3, no isolated nodes).
//!
//! `page_rank` uses 5 iterations (real usage is 10–20, but 5 is sufficient to
//! stress the O(iter × N) loop without a 60-second timeout).

use std::hint::black_box;
use std::time::{Duration, Instant};

use mycelium_core::store::Store;
use mycelium_core::trunk::TrunkPath;
use mycelium_core::types::{EdgeKind, NodeId};

const N: usize = 100_000;
/// Upper bound per algorithm — generous to avoid flakiness on slow CI.
const SLA_HEAVY: Duration = Duration::from_secs(30);

fn build_store_100k() -> (Store, Vec<NodeId>) {
    let mut store = Store::new();
    let ids: Vec<NodeId> = (0..N)
        .map(|i| {
            let path = format!("src/f{}.rs>sym_{i}", i / 100);
            store.upsert_node(TrunkPath::parse(&path).unwrap())
        })
        .collect();
    // Deterministic pseudo-random edges via LCG (no rand dependency).
    let mut lcg = 42_u64;
    for i in 0..N {
        for _ in 0..3 {
            lcg = lcg
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let j = (lcg >> 33) as usize % N;
            if i != j {
                store.upsert_edge(EdgeKind::Calls, ids[i], ids[j]);
            }
        }
    }
    (store, ids)
}

#[test]
fn sla_leaf_symbols_100k() {
    let (store, _) = build_store_100k();
    let t0 = Instant::now();
    let leaves = black_box(store.leaf_symbols(EdgeKind::Calls, N));
    let elapsed = t0.elapsed();
    // In a random graph with 3 outgoing edges per node, true leaf count may be
    // 0 — the assertion is purely a timing SLA gate, not a result check.
    let _ = leaves;
    assert!(
        elapsed < SLA_HEAVY,
        "SLA breach: leaf_symbols on 100k graph took {elapsed:?}, limit is {SLA_HEAVY:?}",
    );
}

#[test]
fn sla_degree_histogram_100k() {
    let (store, _) = build_store_100k();
    let t0 = Instant::now();
    let _hist = black_box(store.degree_histogram(EdgeKind::Calls));
    let elapsed = t0.elapsed();
    assert!(
        elapsed < SLA_HEAVY,
        "SLA breach: degree_histogram on 100k graph took {elapsed:?}, limit is {SLA_HEAVY:?}",
    );
}

#[test]
fn sla_graph_metrics_100k() {
    let (store, _) = build_store_100k();
    let t0 = Instant::now();
    let metrics = black_box(store.graph_metrics(EdgeKind::Calls));
    let elapsed = t0.elapsed();
    assert!(
        metrics.symbol_count > 0,
        "graph_metrics returned 0 symbols on 100k graph"
    );
    assert!(
        elapsed < SLA_HEAVY,
        "SLA breach: graph_metrics on 100k graph took {elapsed:?}, limit is {SLA_HEAVY:?}",
    );
}

#[test]
fn sla_page_rank_100k() {
    let (store, _) = build_store_100k();
    let t0 = Instant::now();
    let pr = black_box(store.page_rank(EdgeKind::Calls, 0.85, 5));
    let elapsed = t0.elapsed();
    assert!(!pr.is_empty(), "page_rank returned empty on 100k graph");
    assert!(
        elapsed < SLA_HEAVY,
        "SLA breach: page_rank (5 iter) on 100k graph took {elapsed:?}, limit is {SLA_HEAVY:?}",
    );
}

#[test]
fn sla_wcc_100k() {
    let (store, _) = build_store_100k();
    let t0 = Instant::now();
    let components = black_box(store.weakly_connected_components(EdgeKind::Calls));
    let elapsed = t0.elapsed();
    assert!(
        !components.is_empty(),
        "weakly_connected_components returned empty on 100k graph"
    );
    assert!(
        elapsed < SLA_HEAVY,
        "SLA breach: wcc on 100k graph took {elapsed:?}, limit is {SLA_HEAVY:?}",
    );
}

#[test]
fn sla_find_call_path_100k() {
    let (store, ids) = build_store_100k();
    // Find a path from node 0 to node N/2 with max_depth 10.
    // In a sparse random graph with avg degree 3, BFS at depth 10
    // can reach ≈ 3^10 = 59049 nodes — likely to find a path.
    let from = ids[0];
    let to = ids[N / 2];
    let t0 = Instant::now();
    let _path = black_box(store.find_call_path(from, to, 10));
    let elapsed = t0.elapsed();
    assert!(
        elapsed < SLA_HEAVY,
        "SLA breach: find_call_path on 100k graph took {elapsed:?}, limit is {SLA_HEAVY:?}",
    );
}
