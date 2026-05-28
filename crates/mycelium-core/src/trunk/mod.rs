//! Trunk — the containment-tree storage layer.
//!
//! Code containment is naturally a tree: a file contains classes, which
//! contain methods, which contain local items. Trunk stores this tree
//! such that:
//!
//! - **Exact lookup** by path is `O(length-of-path)`.
//! - **All descendants** of a node enumerate in `O(K)` where K is the
//!   output size (not the whole graph).
//! - **All ancestors** of a node enumerate in `O(depth)`.
//!
//! Per [RFC-0001](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0001-trunk-and-synapse.md),
//! the eventual data structure is a **Materialized-Path Radix Trie** with
//! HAMT structural sharing for free time-travel snapshots.
//!
//! ## v0.1 spike (this module right now)
//!
//! For the v0.1 spike, we use a **`HashMap`-backed implementation** that
//! preserves the same external semantics. This is honest TDD: it passes
//! the tests, demonstrates the API surface, and unblocks downstream
//! Synapse/Cortex work — without prematurely optimizing the trie before
//! we have a benchmark target nailed.
//!
//! The radix-trie optimization is tracked as RFC-0001 Open Question #2
//! and will become its own PR with `cargo bench` deltas posted.
//!
//! ## Quick example
//!
//! ```
//! use mycelium_core::trunk::{Trunk, TrunkPath};
//!
//! let mut trunk = Trunk::new();
//!
//! let auth_service = trunk.upsert(TrunkPath::parse("src/auth.rs>AuthService").unwrap());
//! let login = trunk.upsert(TrunkPath::parse("src/auth.rs>AuthService>login").unwrap());
//!
//! // Exact lookup
//! assert_eq!(trunk.lookup_path("src/auth.rs>AuthService>login"), Some(login));
//!
//! // Ancestors of `login` include `AuthService`
//! let ancestors: Vec<_> = trunk.ancestors(login).collect();
//! assert_eq!(ancestors, vec![auth_service]);
//!
//! // Descendants of `AuthService` include `login`
//! let descendants: Vec<_> = trunk.descendants(auth_service).collect();
//! assert_eq!(descendants, vec![login]);
//! ```

mod path;
#[cfg(test)]
mod tests;

pub use path::TrunkPath;

use hashbrown::HashMap;

use crate::types::NodeId;

/// The Trunk storage layer.
///
/// See the module-level docs for the data structure rationale and the
/// v0.1 spike caveat.
#[derive(Clone, Debug, Default)]
pub struct Trunk {
    /// path string → `NodeId`.
    by_path: HashMap<String, NodeId>,
    /// `NodeId` → owned path string. Reverse of `by_path`.
    by_id: HashMap<NodeId, String>,
}

impl Trunk {
    /// Create an empty trunk.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of nodes currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// True if no nodes are stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Insert a path, returning its (stable) [`NodeId`].
    ///
    /// Idempotent: inserting the same path twice yields the same id and
    /// does not duplicate state.
    ///
    /// **Note (v0.1)**: ancestor paths are not auto-materialized. If you
    /// want `src/auth.rs>AuthService` to be addressable as a node, you
    /// must `upsert` it explicitly. This matches how extraction works:
    /// the extractor knows when a class is a node vs a structural
    /// container.
    pub fn upsert(&mut self, path: TrunkPath) -> NodeId {
        let s = path.into_string();
        let id = NodeId(path_to_id(&s));
        self.by_path.entry(s.clone()).or_insert(id);
        self.by_id.entry(id).or_insert(s);
        id
    }

    /// Find the id for an exact path. `None` if not present.
    ///
    /// **Distinguishes exact match from prefix match.** Looking up an
    /// ancestor path that was never `upsert`ed returns `None`, even if
    /// descendant paths exist.
    #[must_use]
    pub fn lookup_path(&self, path: &str) -> Option<NodeId> {
        self.by_path.get(path).copied()
    }

    /// Return the path string a [`NodeId`] was assigned. `None` if the
    /// id is not in this trunk.
    #[must_use]
    pub fn path_of(&self, id: NodeId) -> Option<&str> {
        self.by_id.get(&id).map(String::as_str)
    }

    /// Iterate every node id whose path is a strict ancestor of `id`'s
    /// path. Returns in *child-to-root* order (the immediate parent first).
    ///
    /// "Ancestor" here means *materialized ancestor* — only paths that
    /// were also `upsert`ed are returned. Unmaterialized structural
    /// containers are skipped silently.
    pub fn ancestors(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let path = self.by_id.get(&id).cloned();
        AncestorIter {
            trunk: self,
            remaining: path,
        }
    }

    /// Iterate every node id whose path is a strict descendant of `id`'s
    /// path. Returns in unspecified order (callers should sort if order matters).
    ///
    /// O(N) over the trunk; will become O(K) once the radix trie lands.
    pub fn descendants(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let prefix = self
            .by_id
            .get(&id)
            .map(|p| {
                let mut owned = p.clone();
                owned.push(path::SEPARATOR);
                owned
            })
            .unwrap_or_default();

        let want_any = !prefix.is_empty();
        self.by_path.iter().filter_map(move |(p, &nid)| {
            if want_any && p.starts_with(&prefix) {
                Some(nid)
            } else {
                None
            }
        })
    }

    /// Remove `id` from the trunk. Does not cascade — descendants and
    /// edges remain. Returns `true` if the node was present.
    ///
    /// Use [`Self::remove_subtree`] to remove a node and everything below it.
    pub fn remove(&mut self, id: NodeId) -> bool {
        if let Some(path) = self.by_id.remove(&id) {
            self.by_path.remove(&path);
            true
        } else {
            false
        }
    }

    /// Iterate every materialized path string in unspecified order.
    pub fn all_paths(&self) -> impl Iterator<Item = &str> + '_ {
        self.by_path.keys().map(String::as_str)
    }

    /// Remove `id` and all of its descendants. Returns the count removed.
    pub fn remove_subtree(&mut self, id: NodeId) -> usize {
        let Some(prefix) = self.by_id.get(&id).cloned() else {
            return 0;
        };
        let mut prefix_sep = prefix.clone();
        prefix_sep.push(path::SEPARATOR);

        let to_remove: Vec<NodeId> = self
            .by_path
            .iter()
            .filter_map(|(p, &nid)| {
                if p == &prefix || p.starts_with(&prefix_sep) {
                    Some(nid)
                } else {
                    None
                }
            })
            .collect();

        let count = to_remove.len();
        for nid in to_remove {
            if let Some(path) = self.by_id.remove(&nid) {
                self.by_path.remove(&path);
            }
        }
        count
    }
}

/// Iterator yielded by [`Trunk::ancestors`].
struct AncestorIter<'a> {
    trunk: &'a Trunk,
    remaining: Option<String>,
}

impl Iterator for AncestorIter<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        loop {
            let path = self.remaining.take()?;
            let parent = path::parent(&path)?.to_owned();
            self.remaining = Some(parent.clone());
            if let Some(&id) = self.trunk.by_path.get(&parent) {
                return Some(id);
            }
            // skip unmaterialized ancestors and keep walking up
        }
    }
}

/// Derive a stable [`NodeId`] from a path string.
///
/// Uses BLAKE3 truncated to 64 bits. Per RFC-0001 §detailed-design we
/// reserve the low 8 bits as a shard tag (currently always 0), so the
/// effective namespace is 56 bits. At realistic codebase sizes
/// (< 10⁸ paths) collision probability is well below 10⁻⁶ and resolved
/// at the resolver layer by tagging both candidates.
fn path_to_id(path: &str) -> u64 {
    let hash = blake3::hash(path.as_bytes());
    let bytes: [u8; 8] = hash.as_bytes()[..8].try_into().expect("blake3 ≥ 8 bytes");
    let raw = u64::from_le_bytes(bytes);
    // Mask the low 8 bits to reserve shard tag; leave shard = 0 for v0.1.
    raw & 0xFFFF_FFFF_FFFF_FF00
}
