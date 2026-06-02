//! # cold-reader — spawned as a fresh process by the spike
//!
//! argv[1] = path to the redb database file
//! argv[2] = comma-separated list of NodeIds (u64) to look up
//!
//! Opens the database for the first time in a brand-new process,
//! then immediately measures the first N point lookups and first 3-hop
//! traversals WITHOUT any warmup. Those first-N numbers are the "cold
//! first-query" penalty we want to quantify.
//!
//! CAVEAT: The OS page cache is NOT purged before this binary runs.
//! Numbers measure fresh mmap setup + redb metadata reads + B-tree root
//! traversal, but NOT off-disk I/O latency.

use std::time::Instant;

use redb::{Database, TableDefinition};
use serde::{Deserialize, Serialize};

const NODES: TableDefinition<u64, &[u8]> = TableDefinition::new("nodes");
const ADJ: TableDefinition<(u8, u64), &[u8]> = TableDefinition::new("adj");

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NodeValue {
    path: String,
    kind: u8,
    start_line: u32,
    end_line: u32,
}

fn median_and_p99(mut samples: Vec<u64>) -> (u64, u64) {
    if samples.is_empty() { return (0, 0); }
    samples.sort_unstable();
    let median = samples[samples.len() / 2];
    let p99_idx = (samples.len() as f64 * 0.99) as usize;
    let p99 = samples[p99_idx.min(samples.len() - 1)];
    (median, p99)
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: cold-reader <db-path> <comma-sep-u64-ids>");
        std::process::exit(1);
    }
    let db_path = &args[1];
    let ids: Vec<u64> = args[2]
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<u64>().expect("valid u64"))
        .collect();

    let t_open = Instant::now();
    let db = Database::open(db_path)?;
    let open_dur = t_open.elapsed();
    println!("fresh_open_us={}", open_dur.as_micros());

    let txn = db.begin_read()?;
    let node_tbl = txn.open_table(NODES)?;

    let mut lookup_lats = Vec::with_capacity(ids.len());
    for &id in &ids {
        let t0 = Instant::now();
        let got = node_tbl.get(&id)?;
        let elapsed = t0.elapsed().as_nanos() as u64;
        if let Some(v) = got {
            let _: NodeValue = rmp_serde::from_slice(v.value())?;
        }
        lookup_lats.push(elapsed);
    }

    let adj_tbl = txn.open_table(ADJ)?;
    let mut hop_lats = Vec::with_capacity(ids.len());
    for &start in &ids {
        let t0 = Instant::now();
        let mut frontier: Vec<u64> = vec![start];
        let mut visited: std::collections::HashSet<u64> = std::collections::HashSet::new();
        visited.insert(start);
        for _hop in 0..3 {
            let mut next_frontier = Vec::new();
            for node in &frontier {
                if let Some(encoded) = adj_tbl.get(&(0u8, *node))? {
                    let dsts: Vec<u64> = rmp_serde::from_slice(encoded.value())?;
                    for dst in dsts {
                        if visited.insert(dst) { next_frontier.push(dst); }
                    }
                }
            }
            frontier = next_frontier;
            if frontier.is_empty() { break; }
        }
        let elapsed = t0.elapsed().as_nanos() as u64;
        let _ = visited.len();
        hop_lats.push(elapsed);
    }

    let n = lookup_lats.len();
    let first_lookup_us = lookup_lats.first().copied().unwrap_or(0) as f64 / 1000.0;
    let first_hop_us = hop_lats.first().copied().unwrap_or(0) as f64 / 1000.0;
    let (lk_med, lk_p99) = median_and_p99(lookup_lats);
    let (hop_med, hop_p99) = median_and_p99(hop_lats);

    println!("cold_samples={n}");
    println!(
        "cold_lookup_first_us={:.2}  cold_lookup_median_us={:.2}  cold_lookup_p99_us={:.2}",
        first_lookup_us, lk_med as f64 / 1000.0, lk_p99 as f64 / 1000.0,
    );
    println!(
        "cold_3hop_first_us={:.2}  cold_3hop_median_us={:.2}  cold_3hop_p99_us={:.2}",
        first_hop_us, hop_med as f64 / 1000.0, hop_p99 as f64 / 1000.0,
    );
    println!(
        "cold_lookup_vs_sla: {} (p99={:.2}µs, sla=5000µs)",
        if lk_p99 as f64 / 1000.0 < 5000.0 { "PASS" } else { "FAIL" },
        lk_p99 as f64 / 1000.0,
    );
    println!(
        "cold_3hop_vs_sla: {} (p99={:.2}µs, sla=1000µs)",
        if hop_p99 as f64 / 1000.0 < 1000.0 { "PASS" } else { "FAIL" },
        hop_p99 as f64 / 1000.0,
    );

    Ok(())
}
