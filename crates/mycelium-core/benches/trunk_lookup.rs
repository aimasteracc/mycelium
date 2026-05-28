//! Trunk criterion benchmarks.
//!
//! Charter §2 SLA: cold small query < 5 ms on a 100k-node graph.
//!
//! Run:
//! ```text
//! cargo bench --bench trunk_lookup
//! ```
//!
//! HTML reports (when `html_reports` feature is active) are written to
//! `target/criterion/`. The baseline is recorded automatically on the
//! first run; subsequent runs compare against it and fail CI if a
//! regression exceeds the configured threshold.

#![allow(missing_docs)]

use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use mycelium_core::trunk::{Trunk, TrunkPath};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Build a flat trunk with `n` depth-3 leaf nodes.
///
/// Path template: `src/file_F.rs>Class_C>method_M`
/// `file_idx = i / 100`, `class_idx = i / 10`.
fn build_flat(n: usize) -> (Trunk, Vec<String>) {
    let mut trunk = Trunk::new();
    let mut paths = Vec::with_capacity(n);
    for i in 0..n {
        let file_idx = i / 100;
        let class_idx = i / 10;
        let p = format!("src/file_{file_idx}.rs>Class_{class_idx}>method_{i}");
        trunk.upsert(TrunkPath::parse(&p).unwrap());
        paths.push(p);
    }
    (trunk, paths)
}

/// Build a trunk with explicit parent nodes inserted alongside leaves.
///
/// Used for `bench_descendants` so the root node has real children.
/// The root is `src/file_0.rs`; it has all `method_*` descendants whose
/// `file_idx == 0` (i.e. `i < 100`).
fn build_with_parents(n: usize) -> (Trunk, String) {
    let mut trunk = Trunk::new();
    let root = "src/file_0.rs".to_owned();
    trunk.upsert(TrunkPath::parse(&root).unwrap());
    for i in 0..n {
        let file_idx = i / 100;
        let class_idx = i / 10;
        let p = format!("src/file_{file_idx}.rs>Class_{class_idx}>method_{i}");
        trunk.upsert(TrunkPath::parse(&p).unwrap());
    }
    (trunk, root)
}

// ── benches ──────────────────────────────────────────────────────────────────

/// How fast can we bulk-insert N paths into a fresh trunk?
fn bench_upsert(c: &mut Criterion) {
    let mut group = c.benchmark_group("trunk/upsert");
    group.sample_size(20); // inserting 100k paths is slow; reduce sample count
    for n in [1_000_usize, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched(
                || {
                    (0..n)
                        .map(|i| {
                            let fi = i / 100;
                            let ci = i / 10;
                            TrunkPath::parse(&format!("src/file_{fi}.rs>Class_{ci}>method_{i}"))
                                .unwrap()
                        })
                        .collect::<Vec<_>>()
                },
                |paths| {
                    let mut trunk = Trunk::new();
                    for p in paths {
                        black_box(trunk.upsert(p));
                    }
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// Single exact-match lookup at various trunk sizes.
///
/// This is the primary Charter §2 SLA gate: must stay well under 5 ms
/// even at 100k nodes.
fn bench_lookup_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("trunk/lookup_path");
    for n in [1_000_usize, 10_000, 100_000] {
        let (trunk, paths) = build_flat(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            let mut idx = 0usize;
            b.iter(|| {
                let p = &paths[idx % paths.len()];
                idx = idx.wrapping_add(1);
                black_box(trunk.lookup_path(p))
            });
        });
    }
    group.finish();
}

/// Walk the ancestor chain from a mid-trunk leaf.
///
/// With depth-3 paths there are exactly 2 materialized ancestors
/// (only the leaf is stored; parent/grandparent are not), so this
/// measures the per-level overhead rather than chain length.
fn bench_ancestors(c: &mut Criterion) {
    let mut group = c.benchmark_group("trunk/ancestors");
    for n in [1_000_usize, 10_000, 100_000] {
        let (trunk, paths) = build_flat(n);
        let leaf_id = trunk.lookup_path(&paths[n / 2]).unwrap();
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(trunk.ancestors(leaf_id).count()));
        });
    }
    group.finish();
}

/// Enumerate all descendants of a root node.
///
/// v0.1 (`HashMap`) is O(N) — the full table is scanned. This benchmark
/// records the baseline so the radix-trie PR can prove the O(K) claim
/// with a measured delta.
fn bench_descendants(c: &mut Criterion) {
    let mut group = c.benchmark_group("trunk/descendants");
    group.sample_size(20);
    for n in [1_000_usize, 10_000, 100_000] {
        let (trunk, root_path) = build_with_parents(n);
        let root_id = trunk.lookup_path(&root_path).unwrap();
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(trunk.descendants(root_id).count()));
        });
    }
    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default();
    targets = bench_upsert, bench_lookup_path, bench_ancestors, bench_descendants
);
criterion_main!(benches);
