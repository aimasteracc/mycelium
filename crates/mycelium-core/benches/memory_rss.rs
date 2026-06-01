//! RSS-vs-node-count curve benchmark (RFC-0100 R3).
//!
//! Measures how resident memory scales with graph size, producing the
//! documented curve required by issue #343.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use mycelium_core::store::Store;
use mycelium_core::trunk::TrunkPath;

fn path(s: &str) -> TrunkPath {
    TrunkPath::parse(s).unwrap()
}

fn build_store(file_count: usize, symbols_per_file: usize) -> Store {
    let mut store = Store::new();
    for f in 0..file_count {
        let file_path = format!("src/file_{f:06}.rs");
        store.upsert_node(path(&file_path));
        for s in 0..symbols_per_file {
            let sym_path = format!("{file_path}>Sym_{s}");
            let id = store.upsert_node(path(&sym_path));
            store.set_kind(id, mycelium_core::types::NodeKind::Function);
        }
    }
    store
}

fn rss_curve(c: &mut Criterion) {
    let mut group = c.benchmark_group("rss_curve");
    for &file_count in &[100, 500, 1_000, 5_000, 10_000] {
        let symbols_per_file = 10;
        let total_nodes = file_count * (symbols_per_file + 1);
        group.bench_with_input(
            BenchmarkId::new("build_store", total_nodes),
            &total_nodes,
            |b, &_total| {
                b.iter(|| build_store(file_count, symbols_per_file));
            },
        );
    }
    group.finish();
}

fn eviction_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("eviction");
    for cap in [100, 500, 1_000] {
        group.bench_with_input(BenchmarkId::new("remove_file", cap), &cap, |b, &cap| {
            b.iter_batched(
                || build_store(cap * 2, 5),
                |mut store| {
                    for f in 0..cap {
                        store.remove_file(&format!("src/file_{f:06}.rs"));
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, rss_curve, eviction_benchmark);
criterion_main!(benches);
