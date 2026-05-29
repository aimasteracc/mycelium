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

use std::collections::{HashSet, VecDeque};
use std::io::{BufReader, BufWriter};
use std::path::Path;

use std::collections::BTreeMap;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use hashbrown::HashMap;

use crate::synapse::Synapse;
use crate::trunk::{Trunk, TrunkPath};
use crate::types::{EdgeKind, NodeId, NodeKind, SourceSpan};

/// A node in the callee tree returned by [`Store::callee_tree`].
///
/// Represents a symbol and its direct callees (recursively up to the
/// configured `max_depth`).  Cycles are represented as leaf nodes
/// (`children` is empty) rather than causing infinite recursion.
#[derive(Debug, Clone, PartialEq)]
pub struct CalleeNode {
    /// The node this tree entry represents.
    pub id: NodeId,
    /// Callee subtrees, one per outgoing `Calls` edge, up to `max_depth`.
    pub children: Vec<Self>,
}

/// A node in the caller tree returned by [`Store::caller_tree`].
///
/// Represents a symbol and its direct callers (recursively up to the
/// configured `max_depth`).  Cycles are represented as leaf nodes
/// (`callers` is empty) rather than causing infinite recursion.
#[derive(Debug, Clone, PartialEq)]
pub struct CallerNode {
    /// The node this tree entry represents.
    pub id: NodeId,
    /// Caller subtrees, one per incoming `Calls` edge, up to `max_depth`.
    pub callers: Vec<Self>,
}

/// A node in the import tree returned by [`Store::import_tree`].
///
/// Represents a module/file and its direct imports (recursively up to the
/// configured `max_depth`).  Cycles are represented as leaf nodes
/// (`imports` is empty) rather than causing infinite recursion.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportNode {
    /// The node this tree entry represents.
    pub id: NodeId,
    /// Import subtrees, one per outgoing `Imports` edge, up to `max_depth`.
    pub imports: Vec<Self>,
}

/// A node in the importers tree returned by [`Store::importers_tree`].
///
/// Represents a module and the modules that directly import it (recursively
/// up to the configured `max_depth`).  Cycles are represented as leaf nodes
/// (`importers` is empty) rather than causing infinite recursion.
#[derive(Debug, Clone, PartialEq)]
pub struct ImporterNode {
    /// The node this tree entry represents.
    pub id: NodeId,
    /// Importer subtrees, one per incoming `Imports` edge, up to `max_depth`.
    pub importers: Vec<Self>,
}

/// A node in the extends tree returned by [`Store::extends_tree`].
///
/// Represents a class and its direct superclasses (recursively up to the
/// configured `max_depth`).  Cycles are represented as leaf nodes
/// (`parents` is empty) rather than causing infinite recursion.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtendsNode {
    /// The node this tree entry represents.
    pub id: NodeId,
    /// Parent (superclass) subtrees, one per outgoing `Extends` edge, up to `max_depth`.
    pub parents: Vec<Self>,
}

/// A node in the implements tree returned by [`Store::implements_tree`].
///
/// Represents a class and the interfaces it directly implements (recursively
/// up to the configured `max_depth`).  Cycles are represented as leaf nodes
/// (`interfaces` is empty) rather than causing infinite recursion.
#[derive(Debug, Clone, PartialEq)]
pub struct ImplementsNode {
    /// The node this tree entry represents.
    pub id: NodeId,
    /// Interface subtrees, one per outgoing `Implements` edge, up to `max_depth`.
    pub interfaces: Vec<Self>,
}

/// A node in the subclasses tree returned by [`Store::subclasses_tree`].
///
/// Represents a class and its direct subclasses (recursively up to the
/// configured `max_depth`).  Cycles are represented as leaf nodes
/// (`subclasses` is empty) rather than causing infinite recursion.
#[derive(Debug, Clone, PartialEq)]
pub struct SubclassNode {
    /// The node this tree entry represents.
    pub id: NodeId,
    /// Subclass subtrees, one per incoming `Extends` edge, up to `max_depth`.
    pub subclasses: Vec<Self>,
}

/// A node in the implementors tree returned by [`Store::implementors_tree`].
///
/// Represents an interface and the classes that directly implement it
/// (recursively up to the configured `max_depth`).  Cycles are represented as
/// leaf nodes (`implementors` is empty) rather than causing infinite recursion.
#[derive(Debug, Clone, PartialEq)]
pub struct ImplementorNode {
    /// The node this tree entry represents.
    pub id: NodeId,
    /// Implementor subtrees, one per incoming `Implements` edge, up to `max_depth`.
    pub implementors: Vec<Self>,
}

/// All incoming edge references for a single symbol, grouped by edge kind.
///
/// Returned by [`Store::cross_refs`].  All lists are sorted lexicographically.
/// Empty lists are always present (never omitted).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CrossRefs {
    /// Symbols that call this node (`EdgeKind::Calls` incoming).
    pub callers: Vec<String>,
    /// Symbols that import this node (`EdgeKind::Imports` incoming).
    pub importers: Vec<String>,
    /// Symbols that extend this node (`EdgeKind::Extends` incoming).
    pub extended_by: Vec<String>,
    /// Symbols that implement this node (`EdgeKind::Implements` incoming).
    pub implemented_by: Vec<String>,
}

/// All outgoing edge references from a single symbol, grouped by edge kind.
///
/// Symmetric complement to [`CrossRefs`] (incoming edges).
/// Returned by [`Store::outgoing_refs`].  All lists are sorted
/// lexicographically.  Empty lists are always present (never omitted).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutgoingRefs {
    /// Symbols this node calls (`EdgeKind::Calls` outgoing).
    pub callees: Vec<String>,
    /// Symbols this node imports (`EdgeKind::Imports` outgoing).
    pub imports: Vec<String>,
    /// Symbols this node extends (`EdgeKind::Extends` outgoing).
    pub extends: Vec<String>,
    /// Symbols this node implements (`EdgeKind::Implements` outgoing).
    pub implements: Vec<String>,
}

/// Comprehensive statistics about the indexed symbol graph.
///
/// Returned by [`Store::graph_stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphStats {
    /// Total number of nodes in the graph.
    pub total_nodes: usize,
    /// Total number of directed edges across all edge kinds.
    pub total_edges: usize,
    /// Node count broken down by [`NodeKind`] wire string.  Kinds with zero
    /// nodes are omitted.  Nodes without a recorded kind are counted in
    /// `total_nodes` but do not appear here.
    pub nodes_by_kind: BTreeMap<String, usize>,
    /// Edge count broken down by [`EdgeKind`] wire string.  Kinds with zero
    /// edges are omitted.
    pub edges_by_kind: BTreeMap<String, usize>,
}

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
    /// Per-node kind metadata, populated by the extractor.
    kind_map: HashMap<NodeId, NodeKind>,
    /// Per-node source span (file + line/col/byte), populated by the extractor.
    span_map: HashMap<NodeId, SourceSpan>,
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

    /// Record the [`NodeKind`] for an already-upserted node.
    ///
    /// Overwrites any previous value for the same `id`.
    pub fn set_kind(&mut self, id: NodeId, kind: NodeKind) {
        self.kind_map.insert(id, kind);
    }

    /// Return the [`NodeKind`] for `id`, or `None` if not recorded.
    #[must_use]
    pub fn kind_of(&self, id: NodeId) -> Option<NodeKind> {
        self.kind_map.get(&id).copied()
    }

    /// Record the [`SourceSpan`] for an already-upserted node.
    ///
    /// Overwrites any previous value for the same `id`.
    pub fn set_span(&mut self, id: NodeId, span: SourceSpan) {
        self.span_map.insert(id, span);
    }

    /// Return the [`SourceSpan`] for `id`, or `None` if not recorded.
    #[must_use]
    pub fn span_of(&self, id: NodeId) -> Option<SourceSpan> {
        self.span_map.get(&id).copied()
    }

    /// Return all materialized symbol paths whose recorded kind equals `kind`.
    ///
    /// If `prefix` is given, only paths starting with that string are returned.
    /// Results are sorted lexicographically.
    #[must_use]
    pub fn symbols_of_kind(&self, kind: NodeKind, prefix: Option<&str>) -> Vec<String> {
        let mut result: Vec<String> = self
            .kind_map
            .iter()
            .filter(|&(_, &k)| k == kind)
            .filter_map(|(&id, _)| self.trunk.path_of(id))
            .filter(|p| prefix.is_none_or(|pfx| p.starts_with(pfx)))
            .map(str::to_owned)
            .collect();
        result.sort_unstable();
        result
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
        self.kind_map.remove(&id);
        self.span_map.remove(&id);
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
            self.kind_map.remove(&id);
            self.span_map.remove(&id);
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

    /// Return the top `limit` symbols ranked by incoming `Calls` edge count,
    /// sorted by caller count descending (ties broken by path ascending).
    ///
    /// Only symbols with at least one caller are included.
    ///
    /// # Example
    ///
    /// ```
    /// use mycelium_core::store::Store;
    /// use mycelium_core::trunk::TrunkPath;
    /// use mycelium_core::types::EdgeKind;
    ///
    /// let mut store = Store::new();
    /// let a = store.upsert_node(TrunkPath::parse("a.rs>a").unwrap());
    /// let b = store.upsert_node(TrunkPath::parse("b.rs>b").unwrap());
    /// let c = store.upsert_node(TrunkPath::parse("c.rs>c").unwrap());
    /// store.upsert_edge(EdgeKind::Calls, a, b);
    /// store.upsert_edge(EdgeKind::Calls, c, b);
    /// store.upsert_edge(EdgeKind::Calls, a, c);
    ///
    /// let ranked = store.top_callee_symbols(10);
    /// assert_eq!(ranked[0], ("b.rs>b".to_string(), 2));
    /// assert_eq!(ranked[1], ("c.rs>c".to_string(), 1));
    /// ```
    #[must_use]
    pub fn top_callee_symbols(&self, limit: usize) -> Vec<(String, usize)> {
        let mut ranked: Vec<(String, usize)> = self
            .trunk
            .all_paths()
            .filter_map(|p| {
                let id = self.trunk.lookup_path(p)?;
                let count = self.synapse.incoming(id, EdgeKind::Calls).len();
                if count > 0 {
                    Some((p.to_owned(), count))
                } else {
                    None
                }
            })
            .collect();
        ranked.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        ranked.truncate(limit);
        ranked
    }

    /// Return all file-level paths (trunk paths with no `>` separator),
    /// sorted lexicographically.
    ///
    /// # Example
    ///
    /// ```
    /// use mycelium_core::store::Store;
    /// use mycelium_core::trunk::TrunkPath;
    ///
    /// let mut store = Store::new();
    /// store.upsert_node(TrunkPath::parse("src/auth.rs").unwrap());
    /// store.upsert_node(TrunkPath::parse("src/auth.rs>login").unwrap());
    /// store.upsert_node(TrunkPath::parse("src/main.rs").unwrap());
    ///
    /// let files = store.all_file_paths();
    /// assert_eq!(files, vec!["src/auth.rs", "src/main.rs"]);
    /// ```
    #[must_use]
    pub fn all_file_paths(&self) -> Vec<String> {
        let mut files: Vec<String> = self
            .trunk
            .all_paths()
            .filter(|p| !p.contains('>'))
            .map(str::to_owned)
            .collect();
        files.sort_unstable();
        files
    }

    /// Return all symbol paths (paths containing `>`) that have zero
    /// incoming `Calls` edges, sorted lexicographically.
    ///
    /// File-level nodes (no `>`) are excluded — they have no callers by
    /// definition and would create noise.  Pass `prefix` to restrict
    /// results to symbols whose path starts with that string.
    #[must_use]
    pub fn entry_points(&self, prefix: Option<&str>) -> Vec<String> {
        let mut result: Vec<String> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter(|p| prefix.is_none_or(|pfx| p.starts_with(pfx)))
            .filter(|p| {
                self.trunk
                    .lookup_path(p)
                    .is_some_and(|id| self.synapse.incoming(id, EdgeKind::Calls).is_empty())
            })
            .map(str::to_owned)
            .collect();
        result.sort_unstable();
        result
    }

    /// Return all paths that `id` imports (outgoing `Imports` edges), sorted.
    #[must_use]
    pub fn imports_of(&self, id: NodeId) -> Vec<String> {
        let mut result: Vec<String> = self
            .synapse
            .outgoing(id, EdgeKind::Imports)
            .iter()
            .filter_map(|&dep| self.trunk.path_of(dep).map(str::to_owned))
            .collect();
        result.sort_unstable();
        result
    }

    /// Return all paths that import `id` (incoming `Imports` edges), sorted.
    #[must_use]
    pub fn imported_by(&self, id: NodeId) -> Vec<String> {
        let mut result: Vec<String> = self
            .synapse
            .incoming(id, EdgeKind::Imports)
            .iter()
            .filter_map(|&src| self.trunk.path_of(src).map(str::to_owned))
            .collect();
        result.sort_unstable();
        result
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

    /// Find the shortest call chain from `from` to `to` using BFS.
    ///
    /// Returns `Some(path)` where `path` is a `Vec<NodeId>` including both
    /// endpoints (`path[0] == from`, `path.last() == to`), or `None` if
    /// `to` is not reachable from `from` within `max_depth` hops.
    ///
    /// `max_depth` limits the number of edges traversed (hops = path.len()-1).
    /// Cycles are handled safely via a visited set.
    ///
    /// If `from == to`, returns `Some(vec![from])` immediately (0 hops).
    #[must_use]
    pub fn find_call_path(
        &self,
        from: NodeId,
        to: NodeId,
        max_depth: usize,
    ) -> Option<Vec<NodeId>> {
        if from == to {
            return Some(vec![from]);
        }
        // BFS queue: (current_node, path_so_far)
        let mut queue: VecDeque<(NodeId, Vec<NodeId>)> = VecDeque::new();
        let mut visited: HashSet<NodeId> = HashSet::new();
        queue.push_back((from, vec![from]));
        visited.insert(from);
        while let Some((cur, path)) = queue.pop_front() {
            if path.len() > max_depth {
                continue;
            }
            for &next in self.synapse.outgoing(cur, EdgeKind::Calls) {
                if next == to {
                    let mut result = path;
                    result.push(next);
                    return Some(result);
                }
                if !visited.contains(&next) && path.len() < max_depth {
                    visited.insert(next);
                    let mut new_path = path.clone();
                    new_path.push(next);
                    queue.push_back((next, new_path));
                }
            }
        }
        None
    }

    /// BFS shortest import-dependency path from `from` to `to`.
    ///
    /// Returns `Some(path)` including both endpoints, or `None` if `to` is
    /// unreachable within `max_depth` hops. `max_depth` limits the number of
    /// edges traversed, not the path length. Cycle-safe via visited set.
    #[must_use]
    pub fn find_import_path(
        &self,
        from: NodeId,
        to: NodeId,
        max_depth: usize,
    ) -> Option<Vec<NodeId>> {
        if from == to {
            return Some(vec![from]);
        }
        let mut queue: VecDeque<(NodeId, Vec<NodeId>)> = VecDeque::new();
        let mut visited: HashSet<NodeId> = HashSet::new();
        queue.push_back((from, vec![from]));
        visited.insert(from);
        while let Some((cur, path)) = queue.pop_front() {
            if path.len() > max_depth {
                continue;
            }
            for &next in self.synapse.outgoing(cur, EdgeKind::Imports) {
                if next == to {
                    let mut result = path;
                    result.push(next);
                    return Some(result);
                }
                if !visited.contains(&next) && path.len() < max_depth {
                    visited.insert(next);
                    let mut new_path = path.clone();
                    new_path.push(next);
                    queue.push_back((next, new_path));
                }
            }
        }
        None
    }

    /// BFS shortest extends-chain from `from` to `to`.
    ///
    /// Returns `Some(path)` including both endpoints, or `None` if `to` is
    /// unreachable within `max_depth` hops. Cycle-safe via visited set.
    #[must_use]
    pub fn find_extends_path(
        &self,
        from: NodeId,
        to: NodeId,
        max_depth: usize,
    ) -> Option<Vec<NodeId>> {
        if from == to {
            return Some(vec![from]);
        }
        let mut queue: VecDeque<(NodeId, Vec<NodeId>)> = VecDeque::new();
        let mut visited: HashSet<NodeId> = HashSet::new();
        queue.push_back((from, vec![from]));
        visited.insert(from);
        while let Some((cur, path)) = queue.pop_front() {
            if path.len() > max_depth {
                continue;
            }
            for &next in self.synapse.outgoing(cur, EdgeKind::Extends) {
                if next == to {
                    let mut result = path;
                    result.push(next);
                    return Some(result);
                }
                if !visited.contains(&next) && path.len() < max_depth {
                    visited.insert(next);
                    let mut new_path = path.clone();
                    new_path.push(next);
                    queue.push_back((next, new_path));
                }
            }
        }
        None
    }

    /// BFS shortest path from `from` to `to` over `Implements` edges.
    ///
    /// Returns `Some(path)` (including both endpoints) if reachable within
    /// `max_depth` hops, otherwise `None`.  Cycle-safe via visited set.
    #[must_use]
    pub fn find_implements_path(
        &self,
        from: NodeId,
        to: NodeId,
        max_depth: usize,
    ) -> Option<Vec<NodeId>> {
        if from == to {
            return Some(vec![from]);
        }
        let mut queue: VecDeque<(NodeId, Vec<NodeId>)> = VecDeque::new();
        let mut visited: HashSet<NodeId> = HashSet::new();
        queue.push_back((from, vec![from]));
        visited.insert(from);
        while let Some((cur, path)) = queue.pop_front() {
            if path.len() > max_depth {
                continue;
            }
            for &next in self.synapse.outgoing(cur, EdgeKind::Implements) {
                if next == to {
                    let mut result = path;
                    result.push(next);
                    return Some(result);
                }
                if !visited.contains(&next) && path.len() < max_depth {
                    visited.insert(next);
                    let mut new_path = path.clone();
                    new_path.push(next);
                    queue.push_back((next, new_path));
                }
            }
        }
        None
    }

    /// Return the transitive callee tree rooted at `id`, up to `max_depth` hops.
    ///
    /// Cycles are broken via a visited set: a node already in the current
    /// traversal path is returned as a leaf with no children.
    ///
    /// `max_depth = 0` returns a leaf (no children) regardless of edges.
    #[must_use]
    pub fn callee_tree(&self, id: NodeId, max_depth: usize) -> CalleeNode {
        let mut visited = HashSet::new();
        self.callee_tree_inner(id, max_depth, &mut visited)
    }

    fn callee_tree_inner(
        &self,
        id: NodeId,
        depth_remaining: usize,
        visited: &mut HashSet<NodeId>,
    ) -> CalleeNode {
        if depth_remaining == 0 || !visited.insert(id) {
            return CalleeNode {
                id,
                children: vec![],
            };
        }
        let children: Vec<CalleeNode> = self
            .synapse
            .outgoing(id, EdgeKind::Calls)
            .iter()
            .map(|&child_id| self.callee_tree_inner(child_id, depth_remaining - 1, visited))
            .collect();
        visited.remove(&id);
        CalleeNode { id, children }
    }

    /// Return a depth-limited tree of all transitive callers of `id`.
    ///
    /// Traverses incoming `Calls` edges up the call graph.  `max_depth`
    /// controls how many hops to follow; 0 returns a leaf immediately.
    ///
    /// Cycles are broken via a visited set: a node already in the current
    /// traversal path is returned as a leaf with no callers.
    ///
    /// `max_depth = 0` returns a leaf (no callers) regardless of edges.
    #[must_use]
    pub fn caller_tree(&self, id: NodeId, max_depth: usize) -> CallerNode {
        let mut visited = HashSet::new();
        self.caller_tree_inner(id, max_depth, &mut visited)
    }

    fn caller_tree_inner(
        &self,
        id: NodeId,
        depth_remaining: usize,
        visited: &mut HashSet<NodeId>,
    ) -> CallerNode {
        if depth_remaining == 0 || !visited.insert(id) {
            return CallerNode {
                id,
                callers: vec![],
            };
        }
        let callers: Vec<CallerNode> = self
            .synapse
            .incoming(id, EdgeKind::Calls)
            .iter()
            .map(|&caller_id| self.caller_tree_inner(caller_id, depth_remaining - 1, visited))
            .collect();
        visited.remove(&id);
        CallerNode { id, callers }
    }

    /// Return a depth-limited tree of all transitive imports of `id`.
    ///
    /// Traverses outgoing `Imports` edges.  `max_depth` controls how many
    /// hops to follow; 0 returns a leaf immediately.
    ///
    /// Cycles are broken via a visited set: a node already in the current
    /// traversal path is returned as a leaf with no imports.
    #[must_use]
    pub fn import_tree(&self, id: NodeId, max_depth: usize) -> ImportNode {
        let mut visited = HashSet::new();
        self.import_tree_inner(id, max_depth, &mut visited)
    }

    fn import_tree_inner(
        &self,
        id: NodeId,
        depth_remaining: usize,
        visited: &mut HashSet<NodeId>,
    ) -> ImportNode {
        if depth_remaining == 0 || !visited.insert(id) {
            return ImportNode {
                id,
                imports: vec![],
            };
        }
        let imports: Vec<ImportNode> = self
            .synapse
            .outgoing(id, EdgeKind::Imports)
            .iter()
            .map(|&dep_id| self.import_tree_inner(dep_id, depth_remaining - 1, visited))
            .collect();
        visited.remove(&id);
        ImportNode { id, imports }
    }

    /// Return the transitive superclass tree rooted at `id`, up to `max_depth` hops.
    ///
    /// Cycles are broken via a visited set: a node already in the current
    /// traversal path is returned as a leaf with no parents.
    ///
    /// `max_depth = 0` returns a leaf (no parents) regardless of edges.
    #[must_use]
    pub fn extends_tree(&self, id: NodeId, max_depth: usize) -> ExtendsNode {
        let mut visited = HashSet::new();
        self.extends_tree_inner(id, max_depth, &mut visited)
    }

    fn extends_tree_inner(
        &self,
        id: NodeId,
        depth_remaining: usize,
        visited: &mut HashSet<NodeId>,
    ) -> ExtendsNode {
        if depth_remaining == 0 || !visited.insert(id) {
            return ExtendsNode {
                id,
                parents: vec![],
            };
        }
        let parents: Vec<ExtendsNode> = self
            .synapse
            .outgoing(id, EdgeKind::Extends)
            .iter()
            .map(|&parent_id| self.extends_tree_inner(parent_id, depth_remaining - 1, visited))
            .collect();
        visited.remove(&id);
        ExtendsNode { id, parents }
    }

    /// Return the subclass forest rooted at `id` via incoming `Extends` edges.
    ///
    /// DFS over incoming `Extends` edges up to `max_depth` hops.  Cycles produce
    /// leaf nodes (no infinite recursion).
    #[must_use]
    pub fn subclasses_tree(&self, id: NodeId, max_depth: usize) -> SubclassNode {
        let mut visited = HashSet::new();
        self.subclasses_tree_inner(id, max_depth, &mut visited)
    }

    fn subclasses_tree_inner(
        &self,
        id: NodeId,
        depth_remaining: usize,
        visited: &mut HashSet<NodeId>,
    ) -> SubclassNode {
        if depth_remaining == 0 || !visited.insert(id) {
            return SubclassNode {
                id,
                subclasses: vec![],
            };
        }
        let subclasses: Vec<SubclassNode> = self
            .synapse
            .incoming(id, EdgeKind::Extends)
            .iter()
            .map(|&child_id| self.subclasses_tree_inner(child_id, depth_remaining - 1, visited))
            .collect();
        visited.remove(&id);
        SubclassNode { id, subclasses }
    }

    /// Return the interface hierarchy implemented by `id`, up to `max_depth` hops.
    ///
    /// DFS over outgoing `Implements` edges.  Cycles produce leaf nodes.
    #[must_use]
    pub fn implements_tree(&self, id: NodeId, max_depth: usize) -> ImplementsNode {
        let mut visited = HashSet::new();
        self.implements_tree_inner(id, max_depth, &mut visited)
    }

    fn implements_tree_inner(
        &self,
        id: NodeId,
        depth_remaining: usize,
        visited: &mut HashSet<NodeId>,
    ) -> ImplementsNode {
        if depth_remaining == 0 || !visited.insert(id) {
            return ImplementsNode {
                id,
                interfaces: vec![],
            };
        }
        let interfaces: Vec<ImplementsNode> = self
            .synapse
            .outgoing(id, EdgeKind::Implements)
            .iter()
            .map(|&iface_id| self.implements_tree_inner(iface_id, depth_remaining - 1, visited))
            .collect();
        visited.remove(&id);
        ImplementsNode { id, interfaces }
    }

    /// Return the implementor forest rooted at `id` via incoming `Implements` edges.
    ///
    /// DFS over incoming `Implements` edges up to `max_depth` hops.  Cycles produce
    /// leaf nodes (no infinite recursion).
    #[must_use]
    pub fn implementors_tree(&self, id: NodeId, max_depth: usize) -> ImplementorNode {
        let mut visited = HashSet::new();
        self.implementors_tree_inner(id, max_depth, &mut visited)
    }

    fn implementors_tree_inner(
        &self,
        id: NodeId,
        depth_remaining: usize,
        visited: &mut HashSet<NodeId>,
    ) -> ImplementorNode {
        if depth_remaining == 0 || !visited.insert(id) {
            return ImplementorNode {
                id,
                implementors: vec![],
            };
        }
        let implementors: Vec<ImplementorNode> = self
            .synapse
            .incoming(id, EdgeKind::Implements)
            .iter()
            .map(|&cls_id| self.implementors_tree_inner(cls_id, depth_remaining - 1, visited))
            .collect();
        visited.remove(&id);
        ImplementorNode { id, implementors }
    }

    /// Return the reverse-dependency forest rooted at `id` via incoming `Imports` edges.
    ///
    /// DFS over incoming `Imports` edges up to `max_depth` hops.  Cycles produce
    /// leaf nodes (no infinite recursion).
    #[must_use]
    pub fn importers_tree(&self, id: NodeId, max_depth: usize) -> ImporterNode {
        let mut visited = HashSet::new();
        self.importers_tree_inner(id, max_depth, &mut visited)
    }

    fn importers_tree_inner(
        &self,
        id: NodeId,
        depth_remaining: usize,
        visited: &mut HashSet<NodeId>,
    ) -> ImporterNode {
        if depth_remaining == 0 || !visited.insert(id) {
            return ImporterNode {
                id,
                importers: vec![],
            };
        }
        let importers: Vec<ImporterNode> = self
            .synapse
            .incoming(id, EdgeKind::Imports)
            .iter()
            .map(|&dep_id| self.importers_tree_inner(dep_id, depth_remaining - 1, visited))
            .collect();
        visited.remove(&id);
        ImporterNode { id, importers }
    }

    /// Return comprehensive per-kind statistics about the indexed graph.
    ///
    /// Nodes without a recorded [`NodeKind`] are included in `total_nodes`
    /// but do not appear in `nodes_by_kind`.  Edge kinds with zero edges are
    /// omitted from `edges_by_kind`.
    #[must_use]
    pub fn graph_stats(&self) -> GraphStats {
        let total_nodes = self.node_count();
        let total_edges = self.edge_count();

        let mut nodes_by_kind: BTreeMap<String, usize> = BTreeMap::new();
        for &kind in self.kind_map.values() {
            *nodes_by_kind.entry(kind.as_str().to_owned()).or_insert(0) += 1;
        }

        let mut edges_by_kind: BTreeMap<String, usize> = BTreeMap::new();
        for (kind, count) in self.synapse.edge_counts_by_kind() {
            edges_by_kind.insert(kind.as_str().to_owned(), count);
        }

        GraphStats {
            total_nodes,
            total_edges,
            nodes_by_kind,
            edges_by_kind,
        }
    }

    /// Return all paths whose nodes participate in at least one cycle of
    /// `edge_kind` edges, sorted lexicographically.
    ///
    /// Uses iterative DFS with an `in_stack` tracking set.  Nodes that appear
    /// on an active DFS stack path more than once are cycle members.  Optional
    /// `prefix` restricts which paths are returned (the cycle detection still
    /// covers the full graph; only the output is filtered).
    #[must_use]
    pub fn nodes_in_cycles(&self, edge_kind: EdgeKind, prefix: Option<&str>) -> Vec<String> {
        use std::collections::HashSet;

        let all_ids: Vec<NodeId> = self
            .trunk
            .all_paths()
            .filter_map(|p| self.trunk.lookup_path(p))
            .collect();

        let mut visited: HashSet<NodeId> = HashSet::new();
        let mut cycle_members: HashSet<NodeId> = HashSet::new();

        for &start in &all_ids {
            if visited.contains(&start) {
                continue;
            }
            // Iterative DFS: stack items are (node, neighbor_index, stack_set)
            // We track the DFS path stack as a Vec to detect back-edges.
            let mut dfs_stack: Vec<NodeId> = Vec::new();
            let mut in_stack: HashSet<NodeId> = HashSet::new();
            let mut stack: Vec<(NodeId, usize)> = vec![(start, 0)];

            while let Some((node, idx)) = stack.last_mut() {
                let node = *node;
                if *idx == 0 {
                    // Entering node for the first time
                    if visited.contains(&node) {
                        stack.pop();
                        continue;
                    }
                    dfs_stack.push(node);
                    in_stack.insert(node);
                }
                let neighbors = self.synapse.outgoing(node, edge_kind);
                if *idx < neighbors.len() {
                    let neighbor = neighbors[*idx];
                    *idx += 1;
                    if in_stack.contains(&neighbor) {
                        // Back-edge found: mark cycle members from neighbor to node
                        let cycle_start =
                            dfs_stack.iter().rposition(|&n| n == neighbor).unwrap_or(0);
                        for &member in &dfs_stack[cycle_start..] {
                            cycle_members.insert(member);
                        }
                    } else if !visited.contains(&neighbor) {
                        stack.push((neighbor, 0));
                    }
                } else {
                    // Leaving node
                    visited.insert(node);
                    dfs_stack.pop();
                    in_stack.remove(&node);
                    stack.pop();
                }
            }
        }

        let mut result: Vec<String> = cycle_members
            .iter()
            .filter_map(|&id| self.path_of(id))
            .filter(|p| prefix.is_none_or(|pfx| p.starts_with(pfx)))
            .map(str::to_owned)
            .collect();
        result.sort_unstable();
        result
    }

    /// Return all incoming edge references to `id`, grouped by edge kind.
    ///
    /// Each list in the returned [`CrossRefs`] is sorted lexicographically.
    /// Unknown `id` values (nodes removed from the graph) return an empty
    /// `CrossRefs`.
    #[must_use]
    pub fn cross_refs(&self, id: NodeId) -> CrossRefs {
        let resolve = |ids: &[NodeId]| -> Vec<String> {
            let mut paths: Vec<String> = ids
                .iter()
                .filter_map(|&nid| self.path_of(nid).map(str::to_owned))
                .collect();
            paths.sort_unstable();
            paths
        };

        CrossRefs {
            callers: resolve(self.synapse.incoming(id, EdgeKind::Calls)),
            importers: resolve(self.synapse.incoming(id, EdgeKind::Imports)),
            extended_by: resolve(self.synapse.incoming(id, EdgeKind::Extends)),
            implemented_by: resolve(self.synapse.incoming(id, EdgeKind::Implements)),
        }
    }

    /// Return all outgoing edge references from `id`, grouped by edge kind.
    ///
    /// Symmetric complement to [`Self::cross_refs`].  Each list in the
    /// returned [`OutgoingRefs`] is sorted lexicographically.
    #[must_use]
    pub fn outgoing_refs(&self, id: NodeId) -> OutgoingRefs {
        let resolve = |ids: &[NodeId]| -> Vec<String> {
            let mut paths: Vec<String> = ids
                .iter()
                .filter_map(|&nid| self.path_of(nid).map(str::to_owned))
                .collect();
            paths.sort_unstable();
            paths
        };

        OutgoingRefs {
            callees: resolve(self.synapse.outgoing(id, EdgeKind::Calls)),
            imports: resolve(self.synapse.outgoing(id, EdgeKind::Imports)),
            extends: resolve(self.synapse.outgoing(id, EdgeKind::Extends)),
            implements: resolve(self.synapse.outgoing(id, EdgeKind::Implements)),
        }
    }

    /// Return all symbol paths (path contains `>`) with zero incoming `Calls`
    /// edges and zero incoming `Imports` edges, sorted lexicographically.
    ///
    /// File-level nodes (no `>`) are excluded — they are containers, not
    /// callable/importable symbols.  Pass `prefix` to restrict results to
    /// symbols whose path starts with that string.
    #[must_use]
    pub fn dead_symbols(&self, prefix: Option<&str>) -> Vec<String> {
        let mut result: Vec<String> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter(|p| prefix.is_none_or(|pfx| p.starts_with(pfx)))
            .filter(|p| {
                self.trunk.lookup_path(p).is_some_and(|id| {
                    self.synapse.incoming(id, EdgeKind::Calls).is_empty()
                        && self.synapse.incoming(id, EdgeKind::Imports).is_empty()
                })
            })
            .map(str::to_owned)
            .collect();
        result.sort_unstable();
        result
    }

    /// Return all symbol paths (paths containing `>`) sorted lexicographically.
    ///
    /// File-level nodes (no `>`) are excluded.  Optionally filter by path
    /// prefix and/or `NodeKind`.
    #[must_use]
    pub fn all_symbols(&self, prefix: Option<&str>, kind: Option<NodeKind>) -> Vec<String> {
        let mut result: Vec<String> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter(|p| prefix.is_none_or(|pfx| p.starts_with(pfx)))
            .filter(|p| {
                kind.is_none_or(|k| {
                    self.trunk
                        .lookup_path(p)
                        .and_then(|id| self.kind_map.get(&id).copied())
                        == Some(k)
                })
            })
            .map(str::to_owned)
            .collect();
        result.sort_unstable();
        result
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
