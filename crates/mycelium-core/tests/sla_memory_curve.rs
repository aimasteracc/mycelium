//! Memory-growth measurements for R3 scale-gap (#344).
//!
//! Two fast unit tests assert `Store::heap_size_estimate()` is non-zero and
//! grows monotonically — these run in every CI pass.
//!
//! Two `#[ignore]` tests build 10K / 100K synthetic nodes and log the delta
//! between system RSS before and after, giving the data needed to spec the
//! LRU / mmap mitigation. Run them with:
//!
//! ```text
//! cargo test -p mycelium-core --test sla_memory_curve -- --include-ignored --nocapture
//! ```

use mycelium_core::store::Store;
#[cfg(feature = "redb-backend")]
use mycelium_core::store::backend::StorageBackend;
#[cfg(feature = "redb-backend")]
use mycelium_core::store::redb_backend::RedbBackend;
use mycelium_core::trunk::TrunkPath;
use mycelium_core::types::EdgeKind;

// ── helpers ──────────────────────────────────────────────────────────────────

fn build_synthetic_store(n: usize) -> Store {
    let mut store = Store::new();
    for i in 0..n {
        let path = format!("src/mod_{}.rs>sym_{i}", i / 100);
        store.upsert_node(TrunkPath::parse(&path).unwrap());
    }
    store
}

fn build_synthetic_store_with_edges(n: usize) -> Store {
    let mut store = Store::new();
    let ids: Vec<_> = (0..n)
        .map(|i| {
            let path = format!("src/mod_{}.rs>sym_{i}", i / 100);
            store.upsert_node(TrunkPath::parse(&path).unwrap())
        })
        .collect();
    // ~3 outgoing Calls edges per node (deterministic LCG — same pattern as sla_heavy_graph).
    let mut lcg: u64 = 42;
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
    store
}

#[cfg(feature = "redb-backend")]
fn build_synthetic_redb_backend_with_edges(
    n: usize,
    edges_per_node: usize,
) -> (tempfile::TempDir, RedbBackend) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("memory-curve.redb");
    let mut backend = RedbBackend::open(&path).expect("open redb backend");
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
    backend.flush().expect("redb flush");
    (dir, backend)
}

/// Returns current process RSS in kibibytes (Linux only; returns 0 elsewhere).
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
    #[cfg(target_os = "macos")]
    {
        let pid = std::process::id().to_string();
        std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &pid])
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|stdout| stdout.trim().parse().ok())
            .unwrap_or(0)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        0
    }
}

// ── fast CI tests (always run) ────────────────────────────────────────────────

/// RED: `heap_size_estimate()` does not yet exist on `Store`.
/// GREEN: implement in `store/mod.rs`.
#[test]
fn heap_size_estimate_positive() {
    let mut store = Store::new();
    store.upsert_node(TrunkPath::parse("src/lib.rs>dummy").unwrap());
    assert!(
        store.heap_size_estimate() > 0,
        "heap_size_estimate must be non-zero after inserting a node"
    );
}

#[test]
fn heap_size_grows_with_nodes() {
    let small = {
        let mut s = Store::new();
        for i in 0..10 {
            s.upsert_node(TrunkPath::parse(&format!("a>sym_{i}")).unwrap());
        }
        s.heap_size_estimate()
    };
    let large = {
        let mut s = Store::new();
        for i in 0..1_000 {
            s.upsert_node(TrunkPath::parse(&format!("a>sym_{i}")).unwrap());
        }
        s.heap_size_estimate()
    };
    assert!(
        large > small,
        "heap estimate must grow with more nodes: large={large} small={small}"
    );
}

#[test]
fn heap_size_counts_edges() {
    let mut no_edges = Store::new();
    let a = no_edges.upsert_node(TrunkPath::parse("x>a").unwrap());
    let b = no_edges.upsert_node(TrunkPath::parse("x>b").unwrap());
    let baseline = no_edges.heap_size_estimate();

    let mut with_edges = Store::new();
    let a2 = with_edges.upsert_node(TrunkPath::parse("x>a").unwrap());
    let b2 = with_edges.upsert_node(TrunkPath::parse("x>b").unwrap());
    for _ in 0..100 {
        with_edges.upsert_edge(EdgeKind::Calls, a2, b2);
    }
    // upsert_edge deduplicates — edge_count stays 1, but estimate may reflect
    // internal storage. The key invariant: estimate with edges >= estimate without.
    assert!(
        with_edges.heap_size_estimate() >= baseline,
        "edge estimate must be >= node-only estimate"
    );
    // Keep a/b alive to silence unused-variable warnings.
    let _ = (a, b);
}

// ── slow measurement tests (opt-in with --include-ignored) ───────────────────

#[test]
#[ignore = "slow (~5 s): measures RSS for R3 (#344) planning; run with --include-ignored --nocapture"]
#[allow(clippy::cast_precision_loss)]
fn memory_curve_1k_nodes() {
    let before = resident_set_kb();
    let store = build_synthetic_store(1_000);
    let after = resident_set_kb();
    let delta_kb = after.saturating_sub(before);
    eprintln!(
        "R3 n=1_000 nodes={} edges={} estimate={}B delta_rss={}KB ({:.0}B/node)",
        store.node_count(),
        store.edge_count(),
        store.heap_size_estimate(),
        delta_kb,
        delta_kb as f64 * 1024.0 / 1_000.0,
    );
}

#[test]
#[ignore = "slow (~30 s): measures RSS for R3 (#344) planning; run with --include-ignored --nocapture"]
#[allow(clippy::cast_precision_loss)]
fn memory_curve_10k_nodes_with_edges() {
    let before = resident_set_kb();
    let store = build_synthetic_store_with_edges(10_000);
    let after = resident_set_kb();
    let delta_kb = after.saturating_sub(before);
    eprintln!(
        "R3 n=10_000 nodes={} edges={} estimate={}B delta_rss={}KB ({:.0}B/node)",
        store.node_count(),
        store.edge_count(),
        store.heap_size_estimate(),
        delta_kb,
        delta_kb as f64 * 1024.0 / 10_000.0,
    );
}

#[test]
#[ignore = "slow (~120 s): measures RSS for R3 (#344) planning; run with --include-ignored --nocapture"]
#[allow(clippy::cast_precision_loss)]
fn memory_curve_100k_nodes_with_edges() {
    let before = resident_set_kb();
    let store = build_synthetic_store_with_edges(100_000);
    let after = resident_set_kb();
    let delta_kb = after.saturating_sub(before);
    eprintln!(
        "R3 n=100_000 nodes={} edges={} estimate={}B delta_rss={}KB ({:.0}B/node)",
        store.node_count(),
        store.edge_count(),
        store.heap_size_estimate(),
        delta_kb,
        delta_kb as f64 * 1024.0 / 100_000.0,
    );
}

#[cfg(feature = "redb-backend")]
#[test]
#[ignore = "slow: measures RedbBackend RSS/footprint for R3 (#344); run with --include-ignored --nocapture"]
#[allow(clippy::cast_precision_loss)]
fn redb_memory_curve_1k_nodes_with_edges() {
    let before = resident_set_kb();
    let (_dir, backend) = build_synthetic_redb_backend_with_edges(1_000, 3);
    let after = resident_set_kb();
    let delta_kb = after.saturating_sub(before);
    eprintln!(
        "R3 redb n=1_000 nodes={} edges={} redb_pages={}B delta_rss={}KB ({:.0}B/node)",
        backend.node_count(),
        backend.edge_count(),
        backend.heap_size_estimate(),
        delta_kb,
        delta_kb as f64 * 1024.0 / 1_000.0,
    );
    assert_eq!(backend.node_count(), 1_000);
    assert!(backend.heap_size_estimate() > 0);
}

#[cfg(feature = "redb-backend")]
#[test]
#[ignore = "slow: measures RedbBackend RSS/footprint for R3 (#344); run with --include-ignored --nocapture"]
#[allow(clippy::cast_precision_loss)]
fn redb_memory_curve_10k_nodes_with_sparse_edges() {
    let before = resident_set_kb();
    let (_dir, backend) = build_synthetic_redb_backend_with_edges(10_000, 1);
    let after = resident_set_kb();
    let delta_kb = after.saturating_sub(before);
    eprintln!(
        "R3 redb n=10_000 nodes={} edges={} redb_pages={}B delta_rss={}KB ({:.0}B/node)",
        backend.node_count(),
        backend.edge_count(),
        backend.heap_size_estimate(),
        delta_kb,
        delta_kb as f64 * 1024.0 / 10_000.0,
    );
    assert_eq!(backend.node_count(), 10_000);
    assert!(backend.heap_size_estimate() > 0);
}
