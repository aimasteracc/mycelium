//! # Spike T1 — redb cold vs warm query latency
//!
//! RFC-0100 de-risk spike. This binary:
//! 1. Populates a redb database with a synthetic graph at two sizes (10K / 100K nodes).
//! 2. Measures WARM point-lookup and 3-hop adjacency traversal latency (median + p99).
//! 3. Spawns a fresh subprocess (`cold-reader`) to approximate COLD latency
//!    (first queries after a fresh process open, before any OS page-cache warmup).
//!
//! ## What "cold" means here
//!
//! macOS does NOT expose `posix_fadvise(FADV_DONTNEED)`, so we cannot forcibly
//! evict the page cache between runs in-process. Instead we:
//!   a) Write the DB, close it fully, then spawn a *fresh OS process* (`cold-reader`)
//!      which opens the DB for the first time and immediately measures the first N
//!      queries before any warming loops.
//!   b) The fresh process inherits no mmap state; the OS page cache may still hold
//!      the data if written and re-opened immediately on the same host. On a hot
//!      bench machine (< 4 GB DB, plenty of RAM) that means the "cold" numbers are
//!      optimistic — they measure first-open overhead (mmap setup, redb metadata
//!      page reads, B-tree root loading) but NOT disk I/O latency.
//!   c) For real "cold-off-disk" you would need `sudo purge` (macOS) or
//!      `echo 3 > /proc/sys/vm/drop_caches` (Linux) between runs. This spike notes
//!      that explicitly and provides the timing for OS-page-cache-warm cold open.
//!
//! ## Schema
//!
//! Two redb tables:
//!   `NODES`: `u64` (NodeId)  → `Vec<u8>` (rmp-serde encoded NodeValue)
//!   `ADJ`:   `(u8, u64)` (EdgeKind + src NodeId) → `Vec<u8>` (rmp-serde Vec<u64> dsts)
//!
//! This mirrors the Mycelium Synapse adjacency (kind, src) -> [dst] shape.
//!
//! ## Graph shape
//!
//! A realistic call graph has an out-degree distribution resembling a power law:
//! ~80% of nodes have 0-2 out-edges ("leaf symbols"), ~15% have 3-10, ~5% "hub"
//! nodes have 10-50. We use a simple approximation: for each node, sample
//! out-degree from {0,1,2,3,5,10,20} with weights matching that profile.

use std::{
    io::Write as _,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use rand::{Rng, SeedableRng, rngs::StdRng};
use redb::{Database, TableDefinition};
use serde::{Deserialize, Serialize};

// ── table definitions ─────────────────────────────────────────────────────────

/// NodeId (u64) → serialised NodeValue
const NODES: TableDefinition<u64, &[u8]> = TableDefinition::new("nodes");

/// (EdgeKind u8, src u64) → serialised Vec<u64> destinations
const ADJ: TableDefinition<(u8, u64), &[u8]> = TableDefinition::new("adj");

// ── data types ────────────────────────────────────────────────────────────────

/// Simplified node metadata (same fields as Mycelium NodeKind / SourceSpan).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NodeValue {
    /// Simulated fully-qualified path string (e.g. "src/auth.rs>AuthService>login")
    path: String,
    kind: u8, // 0=function, 1=class, 2=module, 3=variable
    start_line: u32,
    end_line: u32,
}

/// Edge kinds matching Mycelium EdgeKind ordinals.
#[repr(u8)]
#[derive(Clone, Copy)]
enum EdgeKind {
    Calls = 0,
    Imports = 1,
    Extends = 2,
    Implements = 3,
}

// ── graph generation ──────────────────────────────────────────────────────────

struct SyntheticGraph {
    nodes: Vec<(u64, NodeValue)>,
    /// adjacency: (kind, src, [dst])
    edges: Vec<(u8, u64, Vec<u64>)>,
}

fn generate_graph(n_nodes: usize, seed: u64) -> SyntheticGraph {
    let mut rng = StdRng::seed_from_u64(seed);
    let kinds = [
        EdgeKind::Calls as u8,
        EdgeKind::Imports as u8,
        EdgeKind::Extends as u8,
        EdgeKind::Implements as u8,
    ];

    // Generate node IDs (simulate BLAKE3-derived u64 by using sequential hashes)
    let ids: Vec<u64> = (0..n_nodes as u64)
        .map(|i| {
            // Simple hash mix to spread IDs non-sequentially (like BLAKE3 would)
            let mut x = i.wrapping_mul(0x9e3779b97f4a7c15);
            x ^= x >> 30;
            x = x.wrapping_mul(0xbf58476d1ce4e5b9);
            x ^= x >> 27;
            x = x.wrapping_mul(0x94d049bb133111eb);
            x ^ (x >> 31)
        })
        .collect();

    let mut nodes = Vec::with_capacity(n_nodes);
    for (i, &id) in ids.iter().enumerate() {
        let kind = rng.gen_range(0u8..4u8);
        let start_line = rng.gen_range(1u32..5000u32);
        nodes.push((
            id,
            NodeValue {
                path: format!("src/module_{}/file_{}.rs>Symbol_{}", i / 100, i / 10, i),
                kind,
                start_line,
                end_line: start_line + rng.gen_range(1u32..200u32),
            },
        ));
    }

    // Out-degree distribution: power-law approximation
    // weights for degrees [0, 1, 2, 3, 5, 10, 20]:
    //   0 → 35%, 1 → 25%, 2 → 20%, 3 → 10%, 5 → 5%, 10 → 3%, 20 → 2%
    let degree_choices: &[(u32, u32)] = &[
        (0, 35),
        (1, 25),
        (2, 20),
        (3, 10),
        (5, 5),
        (10, 3),
        (20, 2),
    ];
    let total_weight: u32 = degree_choices.iter().map(|(_, w)| w).sum();

    let mut edges = Vec::new();
    for &(src_id, _) in &nodes {
        let ek = kinds[rng.gen_range(0..kinds.len())];
        let roll = rng.gen_range(0..total_weight);
        let mut cum = 0u32;
        let mut out_degree = 0u32;
        for &(deg, w) in degree_choices {
            cum += w;
            if roll < cum {
                out_degree = deg;
                break;
            }
        }
        if out_degree == 0 {
            continue;
        }
        let dsts: Vec<u64> = (0..out_degree)
            .map(|_| {
                let idx = rng.gen_range(0..n_nodes);
                ids[idx]
            })
            .collect();
        edges.push((ek, src_id, dsts));
    }

    SyntheticGraph { nodes, edges }
}

// ── database population ───────────────────────────────────────────────────────

fn populate(db: &Database, graph: &SyntheticGraph) -> anyhow::Result<()> {
    let txn = db.begin_write()?;
    {
        let mut node_tbl = txn.open_table(NODES)?;
        for (id, val) in &graph.nodes {
            let encoded = rmp_serde::to_vec(val)?;
            node_tbl.insert(id, encoded.as_slice())?;
        }
    }
    {
        let mut adj_tbl = txn.open_table(ADJ)?;
        for (ek, src, dsts) in &graph.edges {
            let encoded = rmp_serde::to_vec(dsts)?;
            adj_tbl.insert((*ek, *src), encoded.as_slice())?;
        }
    }
    txn.commit()?;
    Ok(())
}

// ── timing helpers ────────────────────────────────────────────────────────────

fn median_and_p99(mut samples: Vec<u64>) -> (u64, u64) {
    samples.sort_unstable();
    let median = samples[samples.len() / 2];
    let p99_idx = (samples.len() as f64 * 0.99) as usize;
    let p99 = samples[p99_idx.min(samples.len() - 1)];
    (median, p99)
}

// ── warm benchmark ────────────────────────────────────────────────────────────

fn bench_warm_point_lookup(db: &Database, ids: &[u64], n_samples: usize, rng: &mut StdRng) -> anyhow::Result<Vec<u64>> {
    let mut latencies = Vec::with_capacity(n_samples);
    let txn = db.begin_read()?;
    let tbl = txn.open_table(NODES)?;

    // Warmup: 200 reads to fill mmap / B-tree pages
    for _ in 0..200 {
        let idx = rng.gen_range(0..ids.len());
        let _ = tbl.get(&ids[idx])?;
    }

    for _ in 0..n_samples {
        let idx = rng.gen_range(0..ids.len());
        let id = ids[idx];
        let t0 = Instant::now();
        let got = tbl.get(&id)?;
        let elapsed = t0.elapsed().as_nanos() as u64;
        // Decode to avoid being optimised away
        if let Some(v) = got {
            let _: NodeValue = rmp_serde::from_slice(v.value())?;
        }
        latencies.push(elapsed);
    }
    Ok(latencies)
}

/// 3-hop traversal: starting from a random node, follow Calls edges 3 hops,
/// collecting the set of reachable nodes. Returns Vec of per-traversal latencies.
fn bench_warm_3hop(db: &Database, ids: &[u64], n_samples: usize, rng: &mut StdRng) -> anyhow::Result<Vec<u64>> {
    let mut latencies = Vec::with_capacity(n_samples);
    let txn = db.begin_read()?;
    let adj_tbl = txn.open_table(ADJ)?;

    // Warmup
    for _ in 0..50 {
        let idx = rng.gen_range(0..ids.len());
        let _ = adj_tbl.get(&(EdgeKind::Calls as u8, ids[idx]))?;
    }

    for _ in 0..n_samples {
        let start_idx = rng.gen_range(0..ids.len());
        let start = ids[start_idx];

        let t0 = Instant::now();
        let mut frontier: Vec<u64> = vec![start];
        let mut visited: std::collections::HashSet<u64> = std::collections::HashSet::new();
        visited.insert(start);

        for _hop in 0..3 {
            let mut next_frontier = Vec::new();
            for node in &frontier {
                if let Some(encoded) = adj_tbl.get(&(EdgeKind::Calls as u8, *node))? {
                    let dsts: Vec<u64> = rmp_serde::from_slice(encoded.value())?;
                    for dst in dsts {
                        if visited.insert(dst) {
                            next_frontier.push(dst);
                        }
                    }
                }
            }
            frontier = next_frontier;
            if frontier.is_empty() {
                break;
            }
        }
        let elapsed = t0.elapsed().as_nanos() as u64;
        // prevent optimiser from eliding
        let _ = visited.len();
        latencies.push(elapsed);
    }
    Ok(latencies)
}

// ── main ──────────────────────────────────────────────────────────────────────

fn run_size(label: &str, n_nodes: usize, db_path: &str) -> anyhow::Result<()> {
    println!("\n=== {label} ({n_nodes} nodes) ===");

    // 1. Generate graph
    print!("  Generating synthetic graph... ");
    std::io::stdout().flush().ok();
    let graph = generate_graph(n_nodes, 0xdeadbeef);
    let n_edges: usize = graph.edges.iter().map(|(_, _, dsts)| dsts.len()).sum();
    println!("done ({n_nodes} nodes, {n_edges} edges)");

    // 2. Write DB
    print!("  Writing redb database ({db_path})... ");
    std::io::stdout().flush().ok();
    let t_write = Instant::now();
    let _ = std::fs::remove_file(db_path); // clean slate
    let db = Database::create(db_path)?;
    populate(&db, &graph)?;
    let write_dur = t_write.elapsed();
    let db_size = std::fs::metadata(db_path)?.len();
    println!(
        "done in {:.2}s  ({:.1} MB on disk)",
        write_dur.as_secs_f64(),
        db_size as f64 / 1_048_576.0
    );

    // 3. WARM measurements — DB is still open, pages in mmap / OS cache
    println!("  --- WARM (DB still open, mmap pages hot) ---");
    let ids: Vec<u64> = graph.nodes.iter().map(|(id, _)| *id).collect();
    let mut rng = StdRng::seed_from_u64(0xcafe);

    let n_lookup_samples = 2000;
    let lookup_lats = bench_warm_point_lookup(&db, &ids, n_lookup_samples, &mut rng)?;
    let (lk_med, lk_p99) = median_and_p99(lookup_lats);
    println!(
        "  Point lookup   ({n_lookup_samples} samples): median={lk_med}ns  p99={lk_p99}ns  ({:.3}µs / {:.3}µs)",
        lk_med as f64 / 1000.0,
        lk_p99 as f64 / 1000.0,
    );
    let sla_lookup_ns = 5_000_000u64; // 5ms
    let pass_lk = if lk_p99 < sla_lookup_ns { "PASS" } else { "FAIL" };
    println!("  SLA <5ms warm lookup: {pass_lk} (p99={:.3}ms)", lk_p99 as f64 / 1_000_000.0);

    let n_hop_samples = 500;
    let hop_lats = bench_warm_3hop(&db, &ids, n_hop_samples, &mut rng)?;
    let (hop_med, hop_p99) = median_and_p99(hop_lats);
    println!(
        "  3-hop traversal ({n_hop_samples} samples): median={hop_med}ns  p99={hop_p99}ns  ({:.3}µs / {:.3}µs)",
        hop_med as f64 / 1000.0,
        hop_p99 as f64 / 1000.0,
    );
    let sla_hop_ns = 1_000_000u64; // 1ms
    let pass_hop = if hop_p99 < sla_hop_ns { "PASS" } else { "FAIL" };
    println!("  SLA <1ms warm 3-hop: {pass_hop} (p99={:.3}ms)", hop_p99 as f64 / 1_000_000.0);

    // Close the DB fully before cold measurement
    drop(db);
    std::thread::sleep(Duration::from_millis(100));

    // 4. COLD measurement — spawn fresh subprocess
    println!("  --- COLD (fresh process open, page-cache NOT purged — see caveat) ---");
    println!("  CAVEAT: On macOS, posix_fadvise(FADV_DONTNEED) is unavailable.");
    println!("  'Cold' here = first N queries in a brand-new process (no prior mmap).");
    println!("  The OS page cache may still hold DB pages (optimistic cold number).");
    println!("  For true cold-off-disk, run: sudo purge && ./cold-reader <db> <n>");

    let n_cold = 50;
    let cold_ids: Vec<u64> = {
        let mut r = StdRng::seed_from_u64(0xbeef);
        (0..n_cold).map(|_| ids[r.gen_range(0..ids.len())]).collect()
    };
    let id_list = cold_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let self_path = std::env::current_exe()?;
    let bin_dir = self_path.parent().expect("binary has parent");
    let cold_reader = bin_dir.join("cold-reader");

    if !cold_reader.exists() {
        println!("  cold-reader binary not found at {cold_reader:?} — skipping cold measurement");
        println!("  (Build with: cargo build --release --bin cold-reader)");
    } else {
        let output = Command::new(&cold_reader)
            .arg(db_path)
            .arg(&id_list)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stdout.lines() {
            println!("  [cold-reader] {line}");
        }
        if !stderr.is_empty() {
            for line in stderr.lines() {
                eprintln!("  [cold-reader ERR] {line}");
            }
        }
    }

    let _ = std::fs::remove_file(db_path);

    Ok(())
}

fn main() -> anyhow::Result<()> {
    println!("=================================================================");
    println!(" Spike T1 — redb cold vs warm query latency");
    println!(" RFC-0100 de-risk | Charter SLA: lookup <5ms warm, 3-hop <1ms warm");
    println!("=================================================================");
    println!();
    println!("redb version: {}", env!("CARGO_PKG_VERSION"));
    println!("(redb crate version pinned in spike Cargo.toml: 2.x)");
    println!();

    let tmp = std::env::temp_dir();
    let db_10k = tmp.join("spike_redb_10k.redb").to_string_lossy().into_owned();
    let db_100k = tmp.join("spike_redb_100k.redb").to_string_lossy().into_owned();

    run_size("10K-node graph", 10_000, &db_10k)?;
    run_size("100K-node graph", 100_000, &db_100k)?;

    println!();
    println!("=================================================================");
    println!(" Summary comparison against Charter §2 SLA targets:");
    println!("   Warm point lookup   SLA: < 5 ms");
    println!("   Warm 3-hop          SLA: < 1 ms");
    println!("   Warm reactive re-q  SLA: < 10 ms (not directly measured here)");
    println!(" See per-size PASS/FAIL lines above.");
    println!("=================================================================");

    Ok(())
}
