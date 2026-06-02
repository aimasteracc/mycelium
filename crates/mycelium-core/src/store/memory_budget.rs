//! Memory budget and LRU eviction for bounded Store residency (RFC-0100 R3).
//!
//! When the `memory-bound` feature is enabled, Store tracks file-level access
//! timestamps and can evict cold file subtrees to keep peak RSS under a
//! configurable cap. The default in-memory path is completely unchanged when
//! the feature is off.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::store::Store;
use crate::trunk::TrunkPath;
use crate::types::NodeId;

static GLOBAL_ACCESS_CLOCK: AtomicU64 = AtomicU64::new(0);

fn tick() -> u64 {
    GLOBAL_ACCESS_CLOCK.fetch_add(1, Ordering::Relaxed)
}

/// Configuration for memory-bounded Store operation.
#[derive(Debug, Clone)]
pub struct MemoryBudget {
    /// Maximum number of file-level nodes before eviction triggers.
    pub max_file_nodes: usize,
    /// Number of cold files to evict per eviction pass.
    pub eviction_batch: usize,
}

impl Default for MemoryBudget {
    fn default() -> Self {
        Self {
            max_file_nodes: 10_000,
            eviction_batch: 100,
        }
    }
}

impl MemoryBudget {
    /// Create a new budget with the given file-node cap.
    #[must_use]
    pub const fn new(max_file_nodes: usize) -> Self {
        Self {
            max_file_nodes,
            eviction_batch: 100,
        }
    }

    /// Builder: set eviction batch size.
    #[must_use]
    pub const fn with_eviction_batch(mut self, batch: usize) -> Self {
        self.eviction_batch = batch;
        self
    }
}

/// Per-file access tracking for LRU eviction.
#[derive(Debug, Default)]
struct FileAccessTracker {
    access_tick: HashMap<String, u64>,
}

impl FileAccessTracker {
    fn touch(&mut self, file_path: &str) {
        self.access_tick.insert(file_path.to_string(), tick());
    }

    fn coldest(&self, count: usize) -> Vec<String> {
        let mut entries: Vec<_> = self.access_tick.iter().collect();
        entries.sort_by_key(|&(_, &t)| t);
        entries
            .into_iter()
            .take(count)
            .map(|(p, _)| p.clone())
            .collect()
    }

    fn remove(&mut self, file_path: &str) {
        self.access_tick.remove(file_path);
    }

    #[must_use]
    fn file_count(&self) -> usize {
        self.access_tick.len()
    }
}

/// Estimate the memory usage of a [`Store`] in bytes.
///
/// Heuristic based on node and edge counts: each node ≈ 200 bytes
/// (path string + `HashMap` entry + kind + span), each edge ≈ 40 bytes
/// (forward + reverse `Vec` entries).
#[must_use]
pub fn estimate_store_bytes(store: &Store) -> usize {
    let node_count = store.node_count();
    let node_bytes = node_count * 200;
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

/// Bounded [`Store`] wrapper that evicts cold files when the budget is exceeded.
///
/// Wrap a `Store` with a [`MemoryBudget`] and call [`BoundedStore::after_upsert`]
/// after each batch of mutations.
pub struct BoundedStore {
    /// The underlying store.
    pub store: Store,
    /// The budget configuration.
    pub budget: MemoryBudget,
    tracker: FileAccessTracker,
}

impl BoundedStore {
    /// Create a new bounded store with the given budget.
    #[must_use]
    pub fn new(store: Store, budget: MemoryBudget) -> Self {
        let mut tracker = FileAccessTracker::default();
        for file_path in store.all_file_paths() {
            tracker.touch(&file_path);
        }
        Self {
            store,
            budget,
            tracker,
        }
    }

    /// Upsert a node and track file-level access.
    pub fn upsert_node(&mut self, path: TrunkPath) -> NodeId {
        let file = file_part(path.as_str());
        if let Some(f) = file {
            self.tracker.touch(f);
        }
        self.store.upsert_node(path)
    }

    /// Call after a batch of mutations to trigger eviction if needed.
    ///
    /// Returns the number of files evicted.
    pub fn after_upsert(&mut self) -> usize {
        if self.tracker.file_count() <= self.budget.max_file_nodes {
            return 0;
        }
        let to_evict = self.tracker.coldest(self.budget.eviction_batch);
        let count = to_evict.len();
        for file_path in &to_evict {
            self.store.remove_file(file_path);
            self.tracker.remove(file_path);
        }
        count
    }

    /// Current estimated memory usage in bytes.
    #[must_use]
    pub fn estimated_bytes(&self) -> usize {
        estimate_store_bytes(&self.store)
    }

    /// Number of tracked files.
    #[must_use]
    pub fn tracked_file_count(&self) -> usize {
        self.tracker.file_count()
    }
}

fn file_part(path: &str) -> Option<&str> {
    path.split('>').next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trunk::TrunkPath;

    fn path(s: &str) -> TrunkPath {
        TrunkPath::parse(s).unwrap()
    }

    #[test]
    fn file_access_tracker_touch_orders_correctly() {
        let mut tracker = FileAccessTracker::default();
        tracker.touch("a.rs");
        tracker.touch("b.rs");
        tracker.touch("c.rs");
        tracker.touch("b.rs");

        let coldest = tracker.coldest(2);
        assert_eq!(coldest, vec!["a.rs", "c.rs"]);
    }

    #[test]
    fn bounded_store_evicts_coldest_files() {
        let budget = MemoryBudget::new(3).with_eviction_batch(1);
        let mut bs = BoundedStore::new(Store::new(), budget);

        for f in ["src/a.rs", "src/b.rs", "src/c.rs", "src/d.rs"] {
            bs.upsert_node(path(f));
        }
        bs.upsert_node(path("src/a.rs>Foo"));
        bs.store.upsert_node(path("src/b.rs>Bar"));
        bs.store.upsert_node(path("src/c.rs>Baz"));
        bs.store.upsert_node(path("src/d.rs>Qux"));

        let evicted = bs.after_upsert();
        assert!(evicted >= 1, "should evict at least 1 cold file");
        assert!(
            bs.store.lookup("src/a.rs>Foo").is_some(),
            "a.rs>Foo should survive (recently touched)"
        );
    }

    #[test]
    fn bounded_store_seeds_tracker_from_existing_store() {
        let mut store = Store::new();
        store.upsert_node(path("src/a.rs"));
        store.upsert_node(path("src/a.rs>Foo"));
        store.upsert_node(path("src/b.rs"));
        store.upsert_node(path("src/b.rs>Bar"));
        store.upsert_node(path("src/c.rs"));
        store.upsert_node(path("src/c.rs>Baz"));
        store.upsert_node(path("src/d.rs"));
        store.upsert_node(path("src/d.rs>Qux"));

        let budget = MemoryBudget::new(3).with_eviction_batch(2);
        let mut bs = BoundedStore::new(store, budget);
        assert_eq!(
            bs.tracked_file_count(),
            4,
            "tracker should be seeded with all 4 existing files"
        );

        let evicted = bs.after_upsert();
        assert!(evicted >= 1, "should evict pre-existing files above budget");
    }

    #[test]
    fn estimate_store_bytes_positive() {
        let mut store = Store::new();
        store.upsert_node(path("lib.rs>App"));
        let bytes = estimate_store_bytes(&store);
        assert!(bytes > 0, "estimate should be positive, got {bytes}");
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn measure_rss_returns_some_on_supported_platform() {
        let rss = measure_rss();
        assert!(rss.is_some(), "RSS measurement must work on macOS/Linux");
        assert!(rss.unwrap() > 0, "RSS must be positive");
    }

    #[test]
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn measure_rss_returns_none_on_unsupported_platform() {
        // Windows and other platforms are not yet implemented; None is the documented contract.
        assert!(
            measure_rss().is_none(),
            "measure_rss must return None on unsupported platforms"
        );
    }
}
