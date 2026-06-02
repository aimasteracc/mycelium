//! P2-T04a — opt-in child-process RSS comparison scaffold for Issue #344.
//!
//! This is intentionally **not** a #344-closing gate yet. It records the
//! InMemory-vs-Redb measurement shape in isolated child processes so the later
//! Phase 2 proof can add a hard Linux RSS cap without inheriting parent test
//! process allocator noise.

#![cfg(feature = "redb-backend")]

use mycelium_core::store::backend::StorageBackend;
use mycelium_core::store::in_memory::InMemoryBackend;
use mycelium_core::store::redb_backend::RedbBackend;
use mycelium_core::types::EdgeKind;
#[cfg(target_os = "linux")]
use std::process::Command;

const CHILD_ENV: &str = "MYCELIUM_MEMORY_CEILING_CHILD";
const NODES_ENV: &str = "MYCELIUM_MEMORY_CEILING_NODES";
const DEFAULT_NODES: usize = 1_000;

/// Returns current process RSS in kibibytes.
#[allow(clippy::missing_const_for_fn)]
fn resident_set_kb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/self/status")
            .unwrap_or_default()
            .lines()
            .find_map(|l| {
                l.strip_prefix("VmRSS:").map(|rest| {
                    rest.split_whitespace()
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0)
                })
            })
            .unwrap_or(0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

fn populate_backend(backend: &mut dyn StorageBackend, n: usize, edges_per_node: usize) {
    let ids: Vec<_> = (0..n)
        .map(|i| {
            let path = format!("src/mod_{}.rs>sym_{i}", i / 100);
            backend.upsert_node(&path)
        })
        .collect();

    let mut lcg: u64 = 42;
    for i in 0..n {
        for _ in 0..edges_per_node {
            lcg = lcg
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let j = (lcg >> 33) as usize % n;
            if i != j {
                backend.upsert_edge(EdgeKind::Calls, ids[i], ids[j]);
            }
        }
    }
    backend.flush().expect("backend flush");
}

#[test]
#[ignore = "helper: run by compare_child_process_rss_for_inmemory_and_redb"]
fn memory_ceiling_child_entrypoint() {
    let Ok(scenario) = std::env::var(CHILD_ENV) else {
        return;
    };
    let nodes = std::env::var(NODES_ENV)
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(DEFAULT_NODES);
    let before = resident_set_kb();

    let (node_count, edge_count, footprint_bytes) = match scenario.as_str() {
        "inmemory" => {
            let mut backend = InMemoryBackend::new();
            populate_backend(&mut backend, nodes, 3);
            (
                backend.node_count(),
                backend.edge_count(),
                backend.heap_size_estimate(),
            )
        }
        "redb" => {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("memory-ceiling.redb");
            let mut backend = RedbBackend::open(&path).expect("open redb");
            populate_backend(&mut backend, nodes, 3);
            let result = (
                backend.node_count(),
                backend.edge_count(),
                backend.heap_size_estimate(),
            );
            drop(backend);
            drop(dir);
            result
        }
        other => panic!("unknown memory ceiling child scenario: {other}"),
    };

    let after = resident_set_kb();
    let rss_delta_kb = after.saturating_sub(before);
    println!(
        "MYCELIUM_MEMORY_RESULT scenario={scenario} nodes={node_count} edges={edge_count} footprint_bytes={footprint_bytes} rss_delta_kb={rss_delta_kb}"
    );
}

#[cfg(target_os = "linux")]
#[derive(Debug)]
struct Measurement {
    nodes: usize,
    edges: usize,
    footprint_bytes: usize,
    rss_delta_kb: u64,
}

#[cfg(target_os = "linux")]
fn parse_measurement(stdout: &str, scenario: &str) -> Measurement {
    let Some(line) = stdout
        .lines()
        .find(|line| line.starts_with("MYCELIUM_MEMORY_RESULT "))
    else {
        panic!("missing memory result for {scenario}; stdout:\n{stdout}");
    };
    let mut nodes = None;
    let mut edges = None;
    let mut footprint_bytes = None;
    let mut rss_delta_kb = None;

    for part in line.split_whitespace().skip(1) {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        match key {
            "nodes" => nodes = value.parse().ok(),
            "edges" => edges = value.parse().ok(),
            "footprint_bytes" => footprint_bytes = value.parse().ok(),
            "rss_delta_kb" => rss_delta_kb = value.parse().ok(),
            _ => {}
        }
    }

    Measurement {
        nodes: nodes.expect("nodes in memory result"),
        edges: edges.expect("edges in memory result"),
        footprint_bytes: footprint_bytes.expect("footprint_bytes in memory result"),
        rss_delta_kb: rss_delta_kb.expect("rss_delta_kb in memory result"),
    }
}

#[cfg(target_os = "linux")]
fn run_child_measurement(scenario: &str, nodes: usize) -> Measurement {
    let output = Command::new(std::env::current_exe().expect("current test exe"))
        .args([
            "memory_ceiling_child_entrypoint",
            "--ignored",
            "--exact",
            "--nocapture",
        ])
        .env(CHILD_ENV, scenario)
        .env(NODES_ENV, nodes.to_string())
        .output()
        .expect("run memory ceiling child");

    assert!(
        output.status.success(),
        "{scenario} child failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_measurement(&String::from_utf8_lossy(&output.stdout), scenario)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "slow/informational: Linux child-process RSS comparison for R3 (#344)"]
fn compare_child_process_rss_for_inmemory_and_redb() {
    let nodes = std::env::var(NODES_ENV)
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(DEFAULT_NODES);

    let mem = run_child_measurement("inmemory", nodes);
    let redb = run_child_measurement("redb", nodes);

    assert_eq!(mem.nodes, nodes);
    assert_eq!(redb.nodes, nodes);
    assert!(mem.edges > 0);
    assert!(redb.edges > 0);
    assert!(mem.footprint_bytes > 0);
    assert!(redb.footprint_bytes > 0);

    eprintln!(
        "R3 child RSS comparison n={nodes}: inmemory footprint={}B rss_delta={}KB; redb footprint={}B rss_delta={}KB",
        mem.footprint_bytes, mem.rss_delta_kb, redb.footprint_bytes, redb.rss_delta_kb
    );
}

#[cfg(not(target_os = "linux"))]
#[test]
#[ignore = "Linux-only RSS comparison scaffold for R3 (#344)"]
fn compare_child_process_rss_for_inmemory_and_redb() {
    eprintln!("R3 child-process RSS comparison is Linux-only");
}
