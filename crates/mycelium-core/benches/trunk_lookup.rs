//! Trunk lookup microbenchmark.
//!
//! Charter §2 SLA: cold small query < 5 ms on a 100k-node graph.
//!
//! This benchmark is a starting point. The v0.1 spike uses `HashMap`; the
//! real radix-trie target lands as an optimization PR.
//!
//! Run with:
//!
//! ```text
//! cargo bench --bench trunk_lookup
//! ```

#![allow(missing_docs)]

use std::hint::black_box;
use std::time::Instant;

use mycelium_core::trunk::{Trunk, TrunkPath};

fn make_trunk(n: usize) -> (Trunk, Vec<String>) {
    let mut trunk = Trunk::new();
    let mut paths = Vec::with_capacity(n);
    for i in 0..n {
        let file_idx = i / 100;
        let class_idx = i / 10;
        let path = format!("src/file_{file_idx}.rs>Class_{class_idx}>method_{i}");
        trunk.upsert(TrunkPath::parse(&path).unwrap());
        paths.push(path);
    }
    (trunk, paths)
}

fn main() {
    for &n in &[1_000_usize, 10_000, 100_000] {
        let (trunk, paths) = make_trunk(n);

        // Warmup
        for p in paths.iter().take(100) {
            black_box(trunk.lookup_path(p));
        }

        let start = Instant::now();
        let iterations = 10_000;
        for i in 0..iterations {
            let p = &paths[i % paths.len()];
            black_box(trunk.lookup_path(p));
        }
        let elapsed = start.elapsed();
        let per_op_ns = elapsed.as_nanos() / iterations as u128;

        println!("trunk_lookup  n={n:>7}  total={elapsed:>10?}  per_op={per_op_ns:>6} ns");
    }
}
