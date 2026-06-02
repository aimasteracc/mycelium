//! Resident-memory **measurement** tooling (RFC-0099 Phase 0, carried forward
//! by RFC-0100).
//!
//! RFC-0099 is *Superseded by RFC-0100*; only its measurement phase lives on
//! here. RFC-0100 adopted redb, whose mmap + OS page cache bound resident RAM — so the
//! hand-built LRU segment-eviction cache that once lived here (`BoundedStore`)
//! was the "reinvent what a memory-mapped B-tree already gives you" anti-pattern
//! RFC-0100 explicitly retires, and has been removed. What *lives on* is the
//! measurement half: an RSS probe and a store-size estimate used to produce the
//! RSS-vs-node-count evidence the Charter §2 SLA work needs.
//!
//! Gated behind the `memory-bound` feature only because the macOS RSS probe
//! needs `libc`; the default build is unaffected.

use crate::store::Store;

/// Estimate the memory usage of a [`Store`] in bytes.
///
/// Heuristic based on node and edge counts: each node ≈ 200 bytes
/// (path string + `HashMap` entry + kind + span), each edge ≈ 40 bytes
/// (forward + reverse `Vec` entries).
#[must_use]
pub fn estimate_store_bytes(store: &Store) -> usize {
    let node_bytes = store.node_count() * 200;
    let edge_bytes = store.edge_count() * 40;
    node_bytes + edge_bytes
}

/// Measure actual RSS of the current process in bytes (macOS / Linux).
///
/// Returns `None` if the platform is unsupported.
#[must_use]
pub fn measure_rss() -> Option<usize> {
    #[cfg(target_os = "macos")]
    {
        #[allow(deprecated, unsafe_code)]
        let rss: usize = unsafe {
            let mut info: std::mem::MaybeUninit<libc::mach_task_basic_info> =
                std::mem::MaybeUninit::uninit();
            let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
            let ptr = info.as_mut_ptr();
            let result = libc::task_info(
                libc::mach_task_self(),
                libc::MACH_TASK_BASIC_INFO,
                ptr.cast(),
                &raw mut count,
            );
            if result != libc::KERN_SUCCESS {
                return None;
            }
            usize::try_from((*ptr).resident_size).ok()?
        };
        Some(rss)
    }
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let kb: usize = line.split_whitespace().nth(1)?.parse().ok()?;
                return Some(kb * 1024);
            }
        }
        None
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trunk::TrunkPath;

    fn path(s: &str) -> TrunkPath {
        TrunkPath::parse(s).unwrap()
    }

    #[test]
    fn estimate_store_bytes_positive() {
        let mut store = Store::new();
        store.upsert_node(path("lib.rs>App"));
        let bytes = estimate_store_bytes(&store);
        assert!(bytes > 0, "estimate should be positive, got {bytes}");
    }

    #[test]
    fn measure_rss_returns_some() {
        let rss = measure_rss();
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            assert!(rss.is_none(), "RSS should be None on unsupported platforms");
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            assert!(
                rss.is_some(),
                "RSS measurement should work on this platform"
            );
            assert!(rss.unwrap() > 0, "RSS should be positive");
        }
    }
}
