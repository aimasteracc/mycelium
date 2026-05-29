//! Synapse 3-hop traversal benchmark.
//!
//! Charter §2 SLA: 3-hop graph traversal (callers, depth 3) < 1 ms on 100k-node graph.

#![allow(missing_docs)]

use std::collections::VecDeque;
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use mycelium_core::store::Store;
use mycelium_core::trunk::TrunkPath;
use mycelium_core::types::EdgeKind;

/// Build a store with `n` symbol nodes arranged in a random sparse graph.
/// Each node gets ~3 outgoing `Calls` edges to random targets.
/// Returns `(store, root_id)` where `root_id` is node 0.
fn build_sparse_graph(n: usize) -> (Store, mycelium_core::types::NodeId) {
    let mut store = Store::new();
    let ids: Vec<_> = (0..n)
        .map(|i| {
            let path = format!("src/f{}.rs>sym_{i}", i / 100);
            store.upsert_node(TrunkPath::parse(&path).unwrap())
        })
        .collect();

    // Deterministic pseudo-random via LCG (no rand dependency needed).
    let mut lcg = 12_345u64;
    for i in 0..n {
        for _ in 0..3 {
            lcg = lcg
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let j = (lcg >> 33) as usize % n;
            if i != j {
                store.upsert_edge(EdgeKind::Calls, ids[i], ids[j]);
            }
        }
    }
    (store, ids[0])
}

/// BFS 3-hop traversal: count all nodes reachable within 3 hops.
fn bfs_3hop(store: &Store, root: mycelium_core::types::NodeId) -> usize {
    let mut visited = std::collections::HashSet::new();
    visited.insert(root);
    let mut queue = VecDeque::new();
    queue.push_back((root, 0u8));
    let mut count = 0usize;
    while let Some((node, depth)) = queue.pop_front() {
        if depth >= 3 {
            continue;
        }
        for &next in store.outgoing(node, EdgeKind::Calls) {
            if visited.insert(next) {
                count += 1;
                queue.push_back((next, depth + 1));
            }
        }
    }
    count
}

fn bench_3hop(c: &mut Criterion) {
    let mut group = c.benchmark_group("synapse/3hop_callers");
    group.sample_size(50);
    for n in [1_000usize, 10_000, 100_000] {
        let (store, root) = build_sparse_graph(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(bfs_3hop(&store, root)));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_3hop);
criterion_main!(benches);
