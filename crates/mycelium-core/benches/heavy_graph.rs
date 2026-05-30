//! Heavy-graph algorithm benchmarks — issue #153.
//!
//! Charter §2 SLA (new rows added for v0.1.4): on a 1 K-node / ~3 K-edge graph
//! all six tools must complete in < 2 s wall-clock. These benchmarks provide
//! the regression baseline for those SLA rows.

#![allow(missing_docs)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use mycelium_core::store::Store;
use mycelium_core::trunk::TrunkPath;
use mycelium_core::types::EdgeKind;

/// Deterministic sparse call graph. `n` symbol nodes, ~3 outgoing Calls edges each.
fn build_graph(n: usize) -> (Store, Vec<mycelium_core::types::NodeId>) {
    let mut store = Store::new();
    let ids: Vec<_> = (0..n)
        .map(|i| {
            let p = format!("src/f{}.rs>sym_{i}", i / 100);
            store.upsert_node(TrunkPath::parse(&p).unwrap())
        })
        .collect();
    let mut lcg = 0x1234_5678_9abc_def0u64;
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
    (store, ids)
}

fn bench_leaf_symbols(c: &mut Criterion) {
    let mut g = c.benchmark_group("leaf_symbols");
    for &n in &[1_000usize, 10_000] {
        let (store, _) = build_graph(n);
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(store.leaf_symbols(EdgeKind::Calls, 100)));
        });
    }
    g.finish();
}

fn bench_degree_histogram(c: &mut Criterion) {
    let mut g = c.benchmark_group("degree_histogram");
    for &n in &[1_000usize, 10_000] {
        let (store, _) = build_graph(n);
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(store.degree_histogram(EdgeKind::Calls)));
        });
    }
    g.finish();
}

fn bench_graph_metrics(c: &mut Criterion) {
    let mut g = c.benchmark_group("graph_metrics");
    for &n in &[1_000usize, 10_000] {
        let (store, _) = build_graph(n);
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(store.graph_metrics(EdgeKind::Calls)));
        });
    }
    g.finish();
}

fn bench_page_rank(c: &mut Criterion) {
    let mut g = c.benchmark_group("page_rank");
    for &n in &[1_000usize, 10_000] {
        let (store, _) = build_graph(n);
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(store.page_rank(EdgeKind::Calls, 0.85, 20)));
        });
    }
    g.finish();
}

fn bench_wcc(c: &mut Criterion) {
    let mut g = c.benchmark_group("wcc");
    for &n in &[1_000usize, 10_000] {
        let (store, _) = build_graph(n);
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(store.weakly_connected_components(EdgeKind::Calls)));
        });
    }
    g.finish();
}

fn bench_find_call_path(c: &mut Criterion) {
    let mut g = c.benchmark_group("find_call_path");
    for &n in &[1_000usize, 10_000] {
        let (store, ids) = build_graph(n);
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(store.find_call_path(ids[0], ids[n - 1], 10)));
        });
    }
    g.finish();
}

criterion_group!(
    benches,
    bench_leaf_symbols,
    bench_degree_histogram,
    bench_graph_metrics,
    bench_page_rank,
    bench_wcc,
    bench_find_call_path,
);
criterion_main!(benches);
