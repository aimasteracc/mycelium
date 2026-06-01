//! RFC-0100 P2-T06 — incremental redb persistence benchmarks.
//!
//! Compares the legacy full-graph `MessagePack` snapshot write against the
//! redb file-scoped replacement unit used by MCP watch persistence.
//!
//! The default redb run measures 10K nodes. Set
//! `MYCELIUM_REDB_BENCH_100K=1` to include the slower 100K redb replacement
//! scenario when collecting release-grade numbers.

#![allow(missing_docs)]
#![cfg(feature = "redb-backend")]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use mycelium_core::store::Store;
use mycelium_core::store::backend::StorageBackend;
use mycelium_core::store::redb_backend::{FileEdge, FileNode, RedbBackend};
use mycelium_core::trunk::{TrunkPath, path_to_node_id};
use mycelium_core::types::{EdgeKind, NodeKind};

const SYMBOLS_PER_FILE: usize = 100;

fn file_path(file_idx: usize) -> String {
    format!("src/mod_{file_idx:05}.rs")
}

fn symbol_path(symbol_idx: usize) -> String {
    let file_idx = symbol_idx / SYMBOLS_PER_FILE;
    format!("{}>sym_{symbol_idx}", file_path(file_idx))
}

const fn node(path: String, kind: NodeKind) -> FileNode {
    FileNode {
        path,
        kind: Some(kind),
        span: None,
    }
}

fn edge(kind: EdgeKind, src: &str, dst: &str) -> FileEdge {
    FileEdge {
        kind,
        src: path_to_node_id(src),
        dst: path_to_node_id(dst),
    }
}

fn file_payload(
    file_idx: usize,
    total_symbols: usize,
    version: usize,
) -> (String, Vec<FileNode>, Vec<FileEdge>) {
    let file = file_path(file_idx);
    let file_id = path_to_node_id(&file);
    let start = file_idx * SYMBOLS_PER_FILE;
    let end = (start + SYMBOLS_PER_FILE).min(total_symbols);
    let mut nodes = Vec::with_capacity(end.saturating_sub(start) + 1);
    let mut edges = Vec::with_capacity(end.saturating_sub(start) * 3);

    nodes.push(node(file.clone(), NodeKind::File));
    for symbol_idx in start..end {
        let symbol = if file_idx == 0 && symbol_idx == start {
            format!("{file}>sym_{symbol_idx}_v{version}")
        } else {
            symbol_path(symbol_idx)
        };
        nodes.push(node(symbol.clone(), NodeKind::Function));
        edges.push(FileEdge {
            kind: EdgeKind::Contains,
            src: file_id,
            dst: path_to_node_id(&symbol),
        });

        for step in 1..=2 {
            let target = symbol_path((symbol_idx + step) % total_symbols);
            edges.push(edge(EdgeKind::Calls, &symbol, &target));
        }
    }

    (file, nodes, edges)
}

fn insert_file_into_store(
    store: &mut Store,
    file_idx: usize,
    total_symbols: usize,
    version: usize,
) {
    let (_file, nodes, edges) = file_payload(file_idx, total_symbols, version);
    for node in nodes {
        let id = store.upsert_node(TrunkPath::parse(&node.path).expect("valid trunk path"));
        if let Some(kind) = node.kind {
            store.set_kind(id, kind);
        }
    }
    for edge in edges {
        store.upsert_edge(edge.kind, edge.src, edge.dst);
    }
}

fn build_store(total_symbols: usize) -> Store {
    let mut store = Store::new();
    let files = total_symbols.div_ceil(SYMBOLS_PER_FILE);
    for file_idx in 0..files {
        insert_file_into_store(&mut store, file_idx, total_symbols, 0);
    }
    store
}

fn build_redb(total_symbols: usize) -> (tempfile::TempDir, RedbBackend) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("redb-incremental.redb");
    let mut backend = RedbBackend::open(&path).expect("open redb");
    let files = total_symbols.div_ceil(SYMBOLS_PER_FILE);
    for file_idx in 0..files {
        let (file, nodes, edges) = file_payload(file_idx, total_symbols, 0);
        backend
            .replace_file(&file, &nodes, &edges)
            .expect("seed redb file");
    }
    backend.flush().expect("flush redb");
    (dir, backend)
}

fn bench_full_snapshot(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistence/full_snapshot_rmp");
    group.sample_size(10);
    for total_symbols in [10_000usize, 100_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(total_symbols),
            &total_symbols,
            |b, &total_symbols| {
                let dir = tempfile::tempdir().expect("tempdir");
                let snap = dir.path().join("index.rmp");
                let mut store = build_store(total_symbols);
                let mut version = 0usize;
                b.iter(|| {
                    version += 1;
                    store.remove_file("src/mod_00000.rs");
                    insert_file_into_store(&mut store, 0, total_symbols, version);
                    store.save(&snap).expect("save full snapshot");
                    black_box(store.node_count())
                });
            },
        );
    }
    group.finish();
}

fn bench_redb_replace_file(c: &mut Criterion) {
    let mut group = c.benchmark_group("persistence/redb_replace_file");
    group.sample_size(10);
    let scales: &[usize] = if std::env::var_os("MYCELIUM_REDB_BENCH_100K").is_some() {
        &[10_000, 100_000]
    } else {
        &[10_000]
    };
    for &total_symbols in scales {
        group.bench_with_input(
            BenchmarkId::from_parameter(total_symbols),
            &total_symbols,
            |b, &total_symbols| {
                let (_dir, mut backend) = build_redb(total_symbols);
                let mut version = 0usize;
                b.iter(|| {
                    version += 1;
                    let (file, nodes, edges) = file_payload(0, total_symbols, version);
                    backend
                        .replace_file(&file, &nodes, &edges)
                        .expect("replace one redb file");
                    black_box(backend.node_count())
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_full_snapshot, bench_redb_replace_file);
criterion_main!(benches);
