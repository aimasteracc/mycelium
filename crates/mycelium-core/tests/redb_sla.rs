//! RFC-0100 P2-T06 — redb SLA checks for the incremental storage path.
//!
//! These tests are Linux-gated timing checks for the redb backend after the
//! file-scoped persistence path landed. Criterion carries the detailed
//! before/after numbers; these tests keep the basic lookup and bounded traversal
//! targets from silently regressing in CI.

#![cfg(feature = "redb-backend")]

#[cfg(target_os = "linux")]
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

use mycelium_core::store::backend::StorageBackend;
use mycelium_core::store::redb_backend::{FileEdge, FileNode, RedbBackend};
use mycelium_core::trunk::path_to_node_id;
#[cfg(target_os = "linux")]
use mycelium_core::types::NodeId;
use mycelium_core::types::{EdgeKind, NodeKind};

const SYMBOLS_PER_FILE: usize = 100;
#[cfg(target_os = "linux")]
const SLA_NODES: usize = 10_000;
#[cfg(target_os = "linux")]
const LOOKUP_SLA: Duration = Duration::from_millis(5);
#[cfg(target_os = "linux")]
const THREE_HOP_SLA: Duration = Duration::from_millis(1);

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

fn seed_redb(total_symbols: usize) -> (tempfile::TempDir, PathBuf, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("redb-sla.redb");
    {
        let mut backend = RedbBackend::open(&path).expect("open redb");
        let files = total_symbols.div_ceil(SYMBOLS_PER_FILE);
        for file_idx in 0..files {
            let (file, nodes, edges) = file_payload(file_idx, total_symbols, 0);
            backend
                .replace_file(&file, &nodes, &edges)
                .expect("seed redb file");
        }
        backend.flush().expect("flush redb");
    }
    (dir, path, "src/mod_00000.rs>sym_0_v0".to_owned())
}

#[cfg(target_os = "linux")]
fn three_hop_count(backend: &RedbBackend, root: NodeId) -> usize {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    visited.insert(root);
    queue.push_back((root, 0u8));
    let mut count = 0usize;

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= 3 {
            continue;
        }
        for next in backend.outgoing(node, EdgeKind::Calls) {
            if visited.insert(next) {
                count += 1;
                queue.push_back((next, depth + 1));
            }
        }
    }

    count
}

#[cfg(target_os = "linux")]
#[test]
fn redb_lookup_sla_10k_under_5ms() {
    let (_dir, path, root_symbol) = seed_redb(SLA_NODES);
    let backend = RedbBackend::open_existing(&path).expect("reopen redb");

    let started = Instant::now();
    let found = backend.lookup_path(&root_symbol);
    let elapsed = started.elapsed();

    assert_eq!(found, Some(path_to_node_id(&root_symbol)));
    assert!(
        elapsed < LOOKUP_SLA,
        "redb cold-ish exact lookup exceeded SLA: elapsed={elapsed:?}, sla={LOOKUP_SLA:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn redb_three_hop_sla_10k_under_1ms() {
    let (_dir, path, root_symbol) = seed_redb(SLA_NODES);
    let backend = RedbBackend::open_existing(&path).expect("reopen redb");
    let root = backend.lookup_path(&root_symbol).expect("root symbol");

    let started = Instant::now();
    let reached = three_hop_count(&backend, root);
    let elapsed = started.elapsed();

    assert!(
        reached > 0,
        "fixture must have non-empty 3-hop neighborhood"
    );
    assert!(
        elapsed < THREE_HOP_SLA,
        "redb 3-hop traversal exceeded SLA: reached={reached}, elapsed={elapsed:?}, sla={THREE_HOP_SLA:?}"
    );
}

// ── 100k-node gate (Charter §2 mandated scale) ──────────────────────────────
//
// Charter §2 specifies the latency SLAs on a **100k-node** graph, but the checks
// above only exercise 10k — leaving the redb path unproven at the contract's
// scale. Seeding 100k nodes through `replace_file` (~1000 file transactions) is
// slow, so these run only when `MYCELIUM_REDB_BENCH_100K=1` (the nightly job sets
// it). They are the redb-path proof that the §2 cold-lookup / 3-hop targets hold
// at the mandated scale, and the guard that the path cannot regress invisibly.

#[cfg(target_os = "linux")]
const SLA_NODES_100K: usize = 100_000;

#[cfg(target_os = "linux")]
fn bench_100k_enabled() -> bool {
    std::env::var("MYCELIUM_REDB_BENCH_100K").is_ok_and(|v| v == "1" || v == "true")
}

#[cfg(target_os = "linux")]
#[test]
fn redb_lookup_sla_100k_under_5ms() {
    if !bench_100k_enabled() {
        eprintln!("skipping 100k lookup SLA — set MYCELIUM_REDB_BENCH_100K=1 to run");
        return;
    }
    let (_dir, path, root_symbol) = seed_redb(SLA_NODES_100K);
    let backend = RedbBackend::open_existing(&path).expect("reopen redb");

    let started = Instant::now();
    let found = backend.lookup_path(&root_symbol);
    let elapsed = started.elapsed();

    assert_eq!(found, Some(path_to_node_id(&root_symbol)));
    assert!(
        elapsed < LOOKUP_SLA,
        "redb 100k exact lookup exceeded Charter §2 SLA: elapsed={elapsed:?}, sla={LOOKUP_SLA:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn redb_three_hop_sla_100k_under_1ms() {
    if !bench_100k_enabled() {
        eprintln!("skipping 100k 3-hop SLA — set MYCELIUM_REDB_BENCH_100K=1 to run");
        return;
    }
    let (_dir, path, root_symbol) = seed_redb(SLA_NODES_100K);
    let backend = RedbBackend::open_existing(&path).expect("reopen redb");
    let root = backend.lookup_path(&root_symbol).expect("root symbol");

    let started = Instant::now();
    let reached = three_hop_count(&backend, root);
    let elapsed = started.elapsed();

    assert!(
        reached > 0,
        "fixture must have non-empty 3-hop neighborhood"
    );
    assert!(
        elapsed < THREE_HOP_SLA,
        "redb 100k 3-hop traversal exceeded Charter §2 SLA: reached={reached}, elapsed={elapsed:?}, sla={THREE_HOP_SLA:?}"
    );
}

#[cfg(not(target_os = "linux"))]
#[test]
fn redb_sla_checks_are_linux_gated() {
    let (_dir, path, root_symbol) = seed_redb(1_000);
    let backend = RedbBackend::open_existing(&path).expect("reopen redb");
    assert_eq!(
        backend.lookup_path(&root_symbol),
        Some(path_to_node_id(&root_symbol))
    );
}
