//! Store — unified read/write surface for Trunk + Synapse.
//!
//! RFC-0001 §3 specifies the Store as the single entry point into the
//! storage layer. This v0.1 implementation wires [`Trunk`] and [`Synapse`]
//! together; the deferred features land in later PRs:
//!
//! | Feature | Status | Tracked |
//! |---|---|---|
//! | In-memory Trunk + Synapse API | **this module** | — |
//! | `.myc` persistence (`open`/`create`/`checkpoint`) | deferred | P4 |
//! | Arrow attribute columns (`AttrStore`) | deferred | RFC-0001 §Trunk node payload |
//! | Name index (`name → NodeId[]`) | deferred | RFC-0001 §id-index |
//!
//! ## Quick example
//!
//! ```
//! use mycelium_core::store::Store;
//! use mycelium_core::trunk::TrunkPath;
//! use mycelium_core::types::EdgeKind;
//!
//! let mut store = Store::new();
//!
//! let auth = store.upsert_node(TrunkPath::parse("src/auth.rs>AuthService").unwrap());
//! let login = store.upsert_node(TrunkPath::parse("src/auth.rs>AuthService>login").unwrap());
//! let utils = store.upsert_node(TrunkPath::parse("src/utils.rs>validate").unwrap());
//!
//! store.upsert_edge(EdgeKind::Calls, login, utils);
//!
//! assert_eq!(store.outgoing(login, EdgeKind::Calls), &[utils]);
//! assert_eq!(store.incoming(utils, EdgeKind::Calls), &[login]);
//! ```

#[cfg(test)]
mod tests;

use std::io::{BufReader, BufWriter};
use std::path::Path;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use crate::synapse::Synapse;
use crate::trunk::{Trunk, TrunkPath};
use crate::types::{EdgeKind, NodeId};

/// The unified storage surface for a single codebase graph.
///
/// Coordinates [`Trunk`] (containment tree) and [`Synapse`] (cross-cutting
/// edges) so that mutations stay consistent across both stores.
///
/// See the [module-level docs](self) for deferred features and the
/// [RFC-0001](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0001-trunk-and-synapse.md)
/// for the full design.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Store {
    trunk: Trunk,
    synapse: Synapse,
}

impl Store {
    /// Create an empty in-memory store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // ── persistence ─────────────────────────────────────────────────────

    /// Serialize the store to a `MessagePack` snapshot at `path`.
    ///
    /// Creates parent directories if they do not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the parent directories cannot be created, the file
    /// cannot be written, or serialization fails.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating snapshot dir {}", parent.display()))?;
        }
        let file = std::fs::File::create(path)
            .with_context(|| format!("creating snapshot file {}", path.display()))?;
        let mut writer = BufWriter::new(file);
        rmp_serde::encode::write(&mut writer, self)
            .with_context(|| format!("serializing store to {}", path.display()))?;
        Ok(())
    }

    /// Deserialize a `Store` from a `MessagePack` snapshot at `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened or deserialization fails.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("opening snapshot file {}", path.display()))?;
        let reader = BufReader::new(file);
        rmp_serde::decode::from_read(reader)
            .with_context(|| format!("deserializing store from {}", path.display()))
    }

    // ── writes ──────────────────────────────────────────────────────────

    /// Insert a node by path, returning its stable [`NodeId`].
    ///
    /// Idempotent: inserting the same path twice yields the same id and
    /// does not duplicate state.
    pub fn upsert_node(&mut self, path: TrunkPath) -> NodeId {
        self.trunk.upsert(path)
    }

    /// Insert a directed edge of `kind` from `src` to `dst`.
    ///
    /// Idempotent: inserting the same edge multiple times has no effect.
    pub fn upsert_edge(&mut self, kind: EdgeKind, src: NodeId, dst: NodeId) {
        self.synapse.add(kind, src, dst);
    }

    /// Remove the node `id` from both Trunk and Synapse.
    ///
    /// **Non-cascading**: descendants remain in the Trunk. Use
    /// [`Self::remove_file`] to remove an entire file subtree including
    /// all its descendants.
    pub fn remove_node(&mut self, id: NodeId) {
        self.trunk.remove(id);
        self.synapse.remove_node(id);
    }

    /// Remove all nodes whose path is equal to or descended from
    /// `file_path`, along with all Synapse edges involving any of those
    /// nodes.
    ///
    /// `file_path` must be the exact path of the file root node (e.g.
    /// `"src/auth.rs"`). If `file_path` is not materialized, this is a
    /// no-op.
    pub fn remove_file(&mut self, file_path: &str) {
        let Some(root_id) = self.trunk.lookup_path(file_path) else {
            return;
        };
        // Collect the full subtree before mutating either store.
        let mut ids: Vec<NodeId> = self.trunk.descendants(root_id).collect();
        ids.push(root_id);
        for id in ids {
            self.trunk.remove(id);
            self.synapse.remove_node(id);
        }
    }

    // ── queries ─────────────────────────────────────────────────────────

    /// Find the [`NodeId`] for an exact path. `None` if not materialized.
    #[must_use]
    pub fn lookup(&self, qpath: &str) -> Option<NodeId> {
        self.trunk.lookup_path(qpath)
    }

    /// Return the number of materialized nodes in the store.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.trunk.len()
    }

    /// Total number of directed synapse edges across all edge kinds.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.synapse.edge_count()
    }

    /// Iterate all materialized path strings (delegates to the inner Trunk).
    pub fn all_paths(&self) -> impl Iterator<Item = &str> + '_ {
        self.trunk.all_paths()
    }

    /// Return the path string for a node id, if present.
    #[must_use]
    pub fn path_of(&self, id: NodeId) -> Option<&str> {
        self.trunk.path_of(id)
    }

    /// Search symbols by case-insensitive substring match on the **name
    /// segment** (last `>`-separated segment of the trunk path). Returns
    /// up to `limit` results sorted lexicographically.
    ///
    /// # Example
    ///
    /// ```
    /// use mycelium_core::store::Store;
    /// use mycelium_core::trunk::TrunkPath;
    ///
    /// let mut store = Store::new();
    /// store.upsert_node(TrunkPath::parse("src/auth.rs>AuthService").unwrap());
    /// let hits = store.search_symbol("auth", 10);
    /// assert!(hits.contains(&"src/auth.rs>AuthService".to_string()));
    /// ```
    #[must_use]
    pub fn search_symbol(&self, query: &str, limit: usize) -> Vec<String> {
        let q = query.to_lowercase();
        let mut results: Vec<String> = self
            .trunk
            .all_paths()
            .filter(|p| {
                p.split('>')
                    .next_back()
                    .is_some_and(|seg| seg.to_lowercase().contains(&q))
            })
            .map(str::to_owned)
            .collect();
        results.sort_unstable();
        results.truncate(limit);
        results
    }

    /// Return ancestor path strings for `path` in child-to-root order.
    ///
    /// Returns `None` if `path` is not materialized. Returns an empty
    /// `Vec` if `path` is a root node (no materialized ancestors).
    ///
    /// # Example
    ///
    /// ```
    /// use mycelium_core::store::Store;
    /// use mycelium_core::trunk::TrunkPath;
    ///
    /// let mut store = Store::new();
    /// store.upsert_node(TrunkPath::parse("src/lib.rs").unwrap());
    /// store.upsert_node(TrunkPath::parse("src/lib.rs>Foo").unwrap());
    /// store.upsert_node(TrunkPath::parse("src/lib.rs>Foo>bar").unwrap());
    ///
    /// let ancestors = store.ancestors_of_path("src/lib.rs>Foo>bar").unwrap();
    /// assert_eq!(ancestors[0], "src/lib.rs>Foo");
    /// assert_eq!(ancestors[1], "src/lib.rs");
    /// ```
    #[must_use]
    pub fn ancestors_of_path(&self, path: &str) -> Option<Vec<String>> {
        let id = self.trunk.lookup_path(path)?;
        Some(
            self.trunk
                .ancestors(id)
                .filter_map(|aid| self.trunk.path_of(aid).map(str::to_owned))
                .collect(),
        )
    }

    /// Return descendant path strings for `path` in unspecified order.
    ///
    /// Returns `None` if `path` is not materialized. Returns an empty
    /// `Vec` if `path` is a leaf node (no materialized descendants).
    ///
    /// # Example
    ///
    /// ```
    /// use mycelium_core::store::Store;
    /// use mycelium_core::trunk::TrunkPath;
    ///
    /// let mut store = Store::new();
    /// store.upsert_node(TrunkPath::parse("src/lib.rs").unwrap());
    /// store.upsert_node(TrunkPath::parse("src/lib.rs>Foo").unwrap());
    ///
    /// let desc = store.descendants_of_path("src/lib.rs").unwrap();
    /// assert!(desc.contains(&"src/lib.rs>Foo".to_string()));
    /// ```
    #[must_use]
    pub fn descendants_of_path(&self, path: &str) -> Option<Vec<String>> {
        let id = self.trunk.lookup_path(path)?;
        Some(
            self.trunk
                .descendants(id)
                .filter_map(|did| self.trunk.path_of(did).map(str::to_owned))
                .collect(),
        )
    }

    /// Iterate all materialized ancestors of `id` in child-to-root order.
    pub fn ancestors(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.trunk.ancestors(id)
    }

    /// Iterate all materialized descendants of `id` in unspecified order.
    pub fn descendants(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.trunk.descendants(id)
    }

    /// Resolve unambiguous bare call stubs to their definition nodes.
    ///
    /// After a workspace index, callee names that could not be found
    /// in the same file are stored as "bare" stub nodes (path has no
    /// `>` separator, e.g. `"bar"`). This pass scans all bare nodes and
    /// for each stub whose simple name matches **exactly one** definition
    /// in the store, redirects all `Calls` edges to that definition and
    /// removes the stub node.
    ///
    /// Stubs with 0 or ≥2 matching definitions are left unchanged.
    ///
    /// Returns the count of stubs successfully resolved.
    pub fn resolve_bare_call_stubs(&mut self) -> usize {
        // Snapshot all paths to avoid borrow conflicts during mutation.
        let all_paths: Vec<String> = self.trunk.all_paths().map(str::to_owned).collect();

        // Bare stubs: paths with no `>` separator.
        let stubs: Vec<(NodeId, String)> = all_paths
            .iter()
            .filter(|p| !p.contains('>'))
            .filter_map(|p| self.trunk.lookup_path(p).map(|id| (id, p.clone())))
            .collect();

        let mut resolved = 0;
        for (stub_id, stub_name) in stubs {
            let suffix = format!(">{stub_name}");
            let matches: Vec<NodeId> = all_paths
                .iter()
                .filter(|p| p.ends_with(&suffix) && p.contains('>'))
                .filter_map(|p| self.trunk.lookup_path(p))
                .collect();

            if matches.len() == 1 {
                let def_id = matches[0];
                self.synapse.redirect_node(stub_id, def_id);
                self.trunk.remove(stub_id);
                resolved += 1;
            }
        }
        resolved
    }

    /// Return all targets of edges of `kind` outgoing from `id`.
    #[must_use]
    pub fn outgoing(&self, id: NodeId, kind: EdgeKind) -> &[NodeId] {
        self.synapse.outgoing(id, kind)
    }

    /// Return all sources of edges of `kind` incoming to `id`.
    #[must_use]
    pub fn incoming(&self, id: NodeId, kind: EdgeKind) -> &[NodeId] {
        self.synapse.incoming(id, kind)
    }
}
