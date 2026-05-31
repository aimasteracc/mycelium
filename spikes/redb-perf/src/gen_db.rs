//! QA helper (not part of author's spike): writes a persistent DB and prints
//! some node IDs, so we can independently drive cold-reader against a file that
//! is NOT deleted, and test cache-cold behavior.
use rand::{Rng, SeedableRng, rngs::StdRng};
use redb::{Database, TableDefinition};
use serde::{Deserialize, Serialize};

const NODES: TableDefinition<u64, &[u8]> = TableDefinition::new("nodes");
const ADJ: TableDefinition<(u8, u64), &[u8]> = TableDefinition::new("adj");

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NodeValue { path: String, kind: u8, start_line: u32, end_line: u32 }

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];
    let n: usize = args[2].parse()?;
    let mut rng = StdRng::seed_from_u64(0xdeadbeef);
    let ids: Vec<u64> = (0..n as u64).map(|i| {
        let mut x = i.wrapping_mul(0x9e3779b97f4a7c15);
        x ^= x >> 30; x = x.wrapping_mul(0xbf58476d1ce4e5b9);
        x ^= x >> 27; x = x.wrapping_mul(0x94d049bb133111eb);
        x ^ (x >> 31)
    }).collect();
    let _ = std::fs::remove_file(path);
    let db = Database::create(path)?;
    let txn = db.begin_write()?;
    {
        let mut t = txn.open_table(NODES)?;
        for (i,&id) in ids.iter().enumerate() {
            let v = NodeValue { path: format!("src/module_{}/file_{}.rs>Symbol_{}", i/100, i/10, i), kind: rng.gen_range(0..4), start_line: rng.gen_range(1..5000), end_line: 100 };
            t.insert(&id, rmp_serde::to_vec(&v)?.as_slice())?;
        }
    }
    {
        let mut t = txn.open_table(ADJ)?;
        for &id in &ids {
            let deg = rng.gen_range(0..3);
            if deg>0 {
                let dsts: Vec<u64> = (0..deg).map(|_| ids[rng.gen_range(0..n)]).collect();
                t.insert(&(0u8, id), rmp_serde::to_vec(&dsts)?.as_slice())?;
            }
        }
    }
    txn.commit()?;
    // print 50 sample ids
    let sample: Vec<String> = (0..50).map(|_| ids[rng.gen_range(0..n)].to_string()).collect();
    println!("{}", sample.join(","));
    Ok(())
}
