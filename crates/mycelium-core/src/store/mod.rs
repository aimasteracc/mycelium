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

/// The ego-graph of a symbol for a single `EdgeKind`.
///
/// Contains the symbol's own path plus its direct incoming and outgoing
/// neighbours for the requested edge kind. Both lists are sorted ascending.
/// Returned by [`Store::symbol_neighborhood`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SymbolNeighborhood {
    /// The symbol's own path. Empty string if the node was not found.
    pub path: String,
    /// Direct incoming neighbours for the requested edge kind, sorted ascending.
    pub incoming: Vec<String>,
    /// Direct outgoing neighbours for the requested edge kind, sorted ascending.
    pub outgoing: Vec<String>,
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

/// Per-node edge-count summary returned by [`Store::node_degree`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NodeDegree {
    /// Incoming `Calls` edge count.
    pub in_calls: usize,
    /// Outgoing `Calls` edge count.
    pub out_calls: usize,
    /// Incoming `Imports` edge count.
    pub in_imports: usize,
    /// Outgoing `Imports` edge count.
    pub out_imports: usize,
    /// Incoming `Extends` edge count.
    pub in_extends: usize,
    /// Outgoing `Extends` edge count.
    pub out_extends: usize,
    /// Incoming `Implements` edge count.
    pub in_implements: usize,
    /// Outgoing `Implements` edge count.
    pub out_implements: usize,
}

/// Result of [`Store::topological_sort`].
#[derive(Debug, Clone, Default)]
pub struct TopologicalOrder {
    /// Symbol paths in dependency order: each symbol appears after all its
    /// predecessors for the queried `EdgeKind`.  Sources come first.
    pub order: Vec<String>,
    /// Symbol paths that could not be placed in `order` because they
    /// participate in a directed cycle.  Sorted ascending.
    pub cycle_members: Vec<String>,
}

const AP_UNVISITED: usize = usize::MAX;

fn uf_find(parent: &mut Vec<usize>, x: usize) -> usize {
    if parent[x] != x {
        parent[x] = uf_find(parent, parent[x]);
    }
    parent[x]
}

fn uf_union(parent: &mut Vec<usize>, x: usize, y: usize) {
    let rx = uf_find(parent, x);
    let ry = uf_find(parent, y);
    if rx != ry {
        parent[rx] = ry;
    }
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

    /// Insert or retrieve the node for `path` and immediately record its `kind`.
    /// Convenience wrapper over `upsert_node` + `set_kind`.
    pub fn upsert_node_with_kind(&mut self, path: TrunkPath, kind: NodeKind) -> NodeId {
        let id = self.upsert_node(path);
        self.kind_map.insert(id, kind);
        id
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

    /// Tarjan's SCC — groups of symbol nodes that are mutually reachable via
    /// `kind` edges (size ≥ 2). Sorted by group size descending, then by
    /// first path ascending. Paths within each group sorted ascending.
    ///
    /// # Panics
    ///
    /// Does not panic in practice; internal unwraps are invariant-guarded by
    /// the Tarjan algorithm (a node's lowlink and stack entry always exist when
    /// the algorithm accesses them).
    #[must_use]
    pub fn scc_groups(&self, kind: EdgeKind) -> Vec<Vec<String>> {
        // Collect symbol node IDs only.
        let sym_ids: Vec<NodeId> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter_map(|p| self.trunk.lookup_path(p))
            .collect();

        // Tarjan's iterative SCC.
        let mut index_counter: u32 = 0;
        let mut index: HashMap<NodeId, u32> = HashMap::new();
        let mut lowlink: HashMap<NodeId, u32> = HashMap::new();
        let mut on_stack: HashSet<NodeId> = HashSet::new();
        let mut tarjan_stack: Vec<NodeId> = Vec::new();
        let mut sccs: Vec<Vec<NodeId>> = Vec::new();

        // Iterative frame: (node, neighbour_index)
        for &start in &sym_ids {
            if index.contains_key(&start) {
                continue;
            }
            let mut call_stack: Vec<(NodeId, usize)> = vec![(start, 0)];
            while let Some((v, i)) = call_stack.last_mut() {
                let v = *v;
                if *i == 0 {
                    // First visit: assign index and push to tarjan_stack.
                    index.insert(v, index_counter);
                    lowlink.insert(v, index_counter);
                    index_counter += 1;
                    on_stack.insert(v);
                    tarjan_stack.push(v);
                }
                let neighbors = self.synapse.outgoing(v, kind);
                let mut pushed = false;
                while *i < neighbors.len() {
                    let w = neighbors[*i];
                    *i += 1;
                    // Only follow edges to symbol nodes.
                    if !sym_ids.contains(&w) {
                        continue;
                    }
                    if !index.contains_key(&w) {
                        // Recurse into w.
                        call_stack.push((w, 0));
                        pushed = true;
                        break;
                    } else if on_stack.contains(&w) {
                        // w is on the stack — update lowlink.
                        let w_idx = index[&w];
                        let v_ll = lowlink.get_mut(&v).unwrap();
                        if w_idx < *v_ll {
                            *v_ll = w_idx;
                        }
                    }
                }
                if pushed {
                    continue;
                }
                // Pop v: propagate lowlink to parent and check SCC root.
                call_stack.pop();
                if let Some(&(parent, _)) = call_stack.last() {
                    let v_ll = lowlink[&v];
                    let p_ll = lowlink.get_mut(&parent).unwrap();
                    if v_ll < *p_ll {
                        *p_ll = v_ll;
                    }
                }
                if lowlink[&v] == index[&v] {
                    // v is the root of an SCC — pop from tarjan_stack.
                    let mut scc: Vec<NodeId> = Vec::new();
                    loop {
                        let w = tarjan_stack.pop().unwrap();
                        on_stack.remove(&w);
                        scc.push(w);
                        if w == v {
                            break;
                        }
                    }
                    if scc.len() >= 2 {
                        sccs.push(scc);
                    }
                }
            }
        }

        // Resolve NodeIds to paths and sort.
        let mut groups: Vec<Vec<String>> = sccs
            .into_iter()
            .map(|scc| {
                let mut paths: Vec<String> = scc
                    .iter()
                    .filter_map(|&id| self.path_of(id).map(str::to_owned))
                    .collect();
                paths.sort_unstable();
                paths
            })
            .filter(|g| g.len() >= 2)
            .collect();
        // Sort: largest group first, ties by first path ascending.
        groups.sort_unstable_by(|a, b| b.len().cmp(&a.len()).then_with(|| a[0].cmp(&b[0])));
        groups
    }

    /// Symbols reachable from `id` in exactly 2 outgoing steps for `kind`.
    ///
    /// Excludes `id` itself and all direct (1-hop) outgoing neighbours.
    /// Includes only symbol nodes (paths containing `>`).
    /// Results sorted ascending. Returns empty `Vec` for unknown `id` or
    /// nodes with no outgoing edges.
    #[must_use]
    pub fn two_hop_neighbors(&self, id: NodeId, kind: EdgeKind) -> Vec<String> {
        let direct: HashSet<NodeId> = self.synapse.outgoing(id, kind).iter().copied().collect();

        let mut two_hop: HashSet<NodeId> = HashSet::new();
        for &hop1 in &direct {
            for &hop2 in self.synapse.outgoing(hop1, kind) {
                if hop2 != id
                    && !direct.contains(&hop2)
                    && self.path_of(hop2).is_some_and(|p| p.contains('>'))
                {
                    two_hop.insert(hop2);
                }
            }
        }

        let mut result: Vec<String> = two_hop
            .iter()
            .filter_map(|&nid| self.path_of(nid).map(str::to_owned))
            .collect();
        result.sort_unstable();
        result
    }

    /// Ego-graph of `id` for `kind`: path + direct incoming + direct outgoing.
    ///
    /// Both lists are sorted ascending. All node types included (no file-node
    /// filter). Returns a default empty [`SymbolNeighborhood`] for unknown `id`.
    #[must_use]
    pub fn symbol_neighborhood(&self, id: NodeId, kind: EdgeKind) -> SymbolNeighborhood {
        let Some(own_path) = self.path_of(id) else {
            return SymbolNeighborhood::default();
        };
        let resolve_sorted = |ids: &[NodeId]| -> Vec<String> {
            let mut paths: Vec<String> = ids
                .iter()
                .filter_map(|&nid| self.path_of(nid).map(str::to_owned))
                .collect();
            paths.sort_unstable();
            paths
        };
        SymbolNeighborhood {
            path: own_path.to_owned(),
            incoming: resolve_sorted(self.synapse.incoming(id, kind)),
            outgoing: resolve_sorted(self.synapse.outgoing(id, kind)),
        }
    }

    /// Symbols with both in-degree ≥ `min_in` AND out-degree ≥ `min_out` for `kind`.
    ///
    /// Returns `(path, in_degree, out_degree)` tuples, sorted by
    /// `in_degree + out_degree` descending; ties broken by path ascending.
    /// File nodes excluded. `limit` capped at 100.
    #[must_use]
    pub fn hub_symbols(
        &self,
        kind: EdgeKind,
        min_in: usize,
        min_out: usize,
        limit: usize,
    ) -> Vec<(String, usize, usize)> {
        let limit = limit.min(100);
        let mut entries: Vec<(String, usize, usize)> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter_map(|p| {
                self.trunk.lookup_path(p).map(|id| {
                    let in_deg = self.synapse.incoming(id, kind).len();
                    let out_deg = self.synapse.outgoing(id, kind).len();
                    (p.to_owned(), in_deg, out_deg)
                })
            })
            .filter(|(_, in_deg, out_deg)| *in_deg >= min_in && *out_deg >= min_out)
            .collect();
        entries.sort_unstable_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)).then_with(|| a.0.cmp(&b.0)));
        entries.truncate(limit);
        entries
    }

    /// K-core decomposition of the symbol graph for `kind`.
    ///
    /// Returns the maximal induced subgraph where every node has total
    /// degree (in + out within the subgraph) ≥ k.  `k = 0` returns all
    /// symbol nodes.  Results sorted ascending.  File nodes excluded.
    #[must_use]
    pub fn k_core(&self, kind: EdgeKind, k: usize) -> Vec<String> {
        let sym_ids: Vec<NodeId> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter_map(|p| self.trunk.lookup_path(p))
            .collect();
        if sym_ids.is_empty() {
            return Vec::new();
        }
        let sym_set: HashSet<NodeId> = sym_ids.iter().copied().collect();
        if k == 0 {
            let mut result: Vec<String> = sym_ids
                .iter()
                .filter_map(|&id| self.path_of(id).map(str::to_owned))
                .collect();
            result.sort_unstable();
            return result;
        }
        // Degree within the symbol subgraph only.
        let mut degree: HashMap<NodeId, usize> = HashMap::new();
        for &id in &sym_ids {
            let in_d = self
                .synapse
                .incoming(id, kind)
                .iter()
                .filter(|&&n| sym_set.contains(&n))
                .count();
            let out_d = self
                .synapse
                .outgoing(id, kind)
                .iter()
                .filter(|&&n| sym_set.contains(&n))
                .count();
            degree.insert(id, in_d + out_d);
        }
        let mut active: HashSet<NodeId> = sym_ids.iter().copied().collect();
        let mut queue: Vec<NodeId> = active.iter().copied().filter(|id| degree[id] < k).collect();
        while let Some(u) = queue.pop() {
            if !active.remove(&u) {
                continue;
            }
            // Update neighbours that are still active.
            let neighbors: Vec<NodeId> = self
                .synapse
                .incoming(u, kind)
                .iter()
                .chain(self.synapse.outgoing(u, kind).iter())
                .copied()
                .filter(|n| active.contains(n))
                .collect();
            for v in neighbors {
                if let Some(d) = degree.get_mut(&v) {
                    if *d > 0 {
                        *d -= 1;
                    }
                    if *d < k {
                        queue.push(v);
                    }
                }
            }
        }
        let mut result: Vec<String> = active
            .iter()
            .filter_map(|&id| self.path_of(id).map(str::to_owned))
            .collect();
        result.sort_unstable();
        result
    }

    /// Symbols with exactly one incoming edge for `kind`.
    ///
    /// Returns `(symbol_path, sole_referencing_path)` pairs sorted by symbol path
    /// ascending; limit capped at 100; file nodes excluded from results.
    #[must_use]
    pub fn singly_referenced(&self, kind: EdgeKind, limit: usize) -> Vec<(String, String)> {
        let limit = limit.min(100);
        let mut entries: Vec<(String, String)> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter_map(|p| {
                let id = self.trunk.lookup_path(p)?;
                let inc = self.synapse.incoming(id, kind);
                if inc.len() != 1 {
                    return None;
                }
                let ref_path = self.path_of(inc[0])?.to_owned();
                Some((p.to_owned(), ref_path))
            })
            .collect();
        entries.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        entries.truncate(limit);
        entries
    }

    /// Kahn's BFS topological dependency layering for symbol nodes.
    ///
    /// Layer 0 = utility/leaf symbols (zero outgoing edges for `kind` within
    /// the symbol-only subgraph). Layer k+1 = symbols all of whose outgoing
    /// symbol neighbours are in layers 0..=k. Symbols in cycles are excluded.
    ///
    /// Paths within each layer sorted ascending. Empty layers omitted.
    /// Returns an empty `Vec` if there are no symbol nodes.
    ///
    /// # Panics
    ///
    /// Does not panic in practice; the subtraction of `out_remaining` is
    /// guarded by the invariant that each edge is processed exactly once.
    #[must_use]
    pub fn dependency_layers(&self, kind: EdgeKind) -> Vec<Vec<String>> {
        // Collect symbol NodeIds only (paths containing '>').
        let sym_ids: Vec<NodeId> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter_map(|p| self.trunk.lookup_path(p))
            .collect();

        if sym_ids.is_empty() {
            return Vec::new();
        }

        let sym_set: HashSet<NodeId> = sym_ids.iter().copied().collect();

        // Build `out_remaining[u]` = count of outgoing symbol-subgraph neighbours.
        // Build `incoming_sym[v]` = callers of v within the symbol subgraph.
        let mut out_remaining: HashMap<NodeId, usize> = HashMap::new();
        let mut incoming_sym: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

        for &id in &sym_ids {
            let sym_out: Vec<NodeId> = self
                .synapse
                .outgoing(id, kind)
                .iter()
                .copied()
                .filter(|t| sym_set.contains(t))
                .collect();
            out_remaining.insert(id, sym_out.len());
            for &target in &sym_out {
                incoming_sym.entry(target).or_default().push(id);
            }
        }

        // Kahn's BFS layering.
        let mut layers: Vec<Vec<String>> = Vec::new();
        let mut current: Vec<NodeId> = sym_ids
            .iter()
            .copied()
            .filter(|id| out_remaining[id] == 0)
            .collect();

        while !current.is_empty() {
            let mut layer_paths: Vec<String> = current
                .iter()
                .filter_map(|&id| self.path_of(id).map(str::to_owned))
                .collect();
            layer_paths.sort_unstable();

            let mut next: Vec<NodeId> = Vec::new();
            for &id in &current {
                if let Some(callers) = incoming_sym.get(&id) {
                    for &caller in callers {
                        if let Some(rem) = out_remaining.get_mut(&caller) {
                            *rem -= 1;
                            if *rem == 0 {
                                next.push(caller);
                            }
                        }
                    }
                }
            }

            layers.push(layer_paths);
            current = next;
        }

        layers
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

    /// Return all paths reachable from `id` via outgoing `kind` edges, BFS up
    /// to `max_depth` hops.  The starting node is excluded.  Each node is
    /// visited at most once (cycle-safe).  `max_depth` is capped at 20.
    /// Results sorted lexicographically.
    #[must_use]
    pub fn reachable_from(&self, id: NodeId, kind: EdgeKind, max_depth: usize) -> Vec<String> {
        let max_depth = max_depth.min(20);
        let mut visited: HashSet<NodeId> = HashSet::new();
        visited.insert(id);
        let mut frontier: Vec<NodeId> = vec![id];
        let mut result: Vec<String> = Vec::new();
        for _ in 0..max_depth {
            if frontier.is_empty() {
                break;
            }
            let mut next_frontier: Vec<NodeId> = Vec::new();
            for node in frontier {
                for &neighbor in self.synapse.outgoing(node, kind) {
                    if visited.insert(neighbor) {
                        if let Some(p) = self.path_of(neighbor) {
                            result.push(p.to_owned());
                        }
                        next_frontier.push(neighbor);
                    }
                }
            }
            frontier = next_frontier;
        }
        result.sort_unstable();
        result
    }

    /// Return all paths that can reach `id` via incoming `kind` edges, BFS up
    /// to `max_depth` hops.  The starting node is excluded.  Each node is
    /// visited at most once (cycle-safe).  `max_depth` is capped at 20.
    /// Results sorted lexicographically.
    #[must_use]
    pub fn reachable_to(&self, id: NodeId, kind: EdgeKind, max_depth: usize) -> Vec<String> {
        let max_depth = max_depth.min(20);
        let mut visited: HashSet<NodeId> = HashSet::new();
        visited.insert(id);
        let mut frontier: Vec<NodeId> = vec![id];
        let mut result: Vec<String> = Vec::new();
        for _ in 0..max_depth {
            if frontier.is_empty() {
                break;
            }
            let mut next_frontier: Vec<NodeId> = Vec::new();
            for node in frontier {
                for &neighbor in self.synapse.incoming(node, kind) {
                    if visited.insert(neighbor) {
                        if let Some(p) = self.path_of(neighbor) {
                            result.push(p.to_owned());
                        }
                        next_frontier.push(neighbor);
                    }
                }
            }
            frontier = next_frontier;
        }
        result.sort_unstable();
        result
    }

    /// Union of `reachable_to` for each id in `ids`.
    ///
    /// Returns all symbols that can transitively reach *any* input node via
    /// incoming `kind` edges, deduplicated, input nodes excluded, sorted ascending.
    /// `max_depth` capped at 20.
    #[must_use]
    pub fn batch_reachable_to(
        &self,
        ids: &[NodeId],
        kind: EdgeKind,
        max_depth: usize,
    ) -> Vec<String> {
        let max_depth = max_depth.min(20);
        let input_set: HashSet<NodeId> = ids.iter().copied().collect();
        let mut visited: HashSet<NodeId> = input_set.clone();
        let mut result_set: HashSet<NodeId> = HashSet::new();
        let mut frontier: Vec<NodeId> = ids.to_vec();
        for _ in 0..max_depth {
            if frontier.is_empty() {
                break;
            }
            let mut next_frontier: Vec<NodeId> = Vec::new();
            for node in frontier {
                for &neighbor in self.synapse.incoming(node, kind) {
                    if visited.insert(neighbor) {
                        if !input_set.contains(&neighbor) {
                            result_set.insert(neighbor);
                        }
                        next_frontier.push(neighbor);
                    }
                }
            }
            frontier = next_frontier;
        }
        let mut result: Vec<String> = result_set
            .iter()
            .filter_map(|&nid| self.path_of(nid).map(str::to_owned))
            .collect();
        result.sort_unstable();
        result
    }

    /// Union of `reachable_from` for each id in `ids`.
    ///
    /// Returns all symbols transitively reachable from *any* input node via
    /// outgoing `kind` edges, deduplicated, input nodes excluded, sorted ascending.
    /// `max_depth` capped at 20.
    #[must_use]
    pub fn batch_reachable_from(
        &self,
        ids: &[NodeId],
        kind: EdgeKind,
        max_depth: usize,
    ) -> Vec<String> {
        let max_depth = max_depth.min(20);
        let input_set: HashSet<NodeId> = ids.iter().copied().collect();
        let mut visited: HashSet<NodeId> = input_set.clone();
        let mut result_set: HashSet<NodeId> = HashSet::new();
        let mut frontier: Vec<NodeId> = ids.to_vec();
        for _ in 0..max_depth {
            if frontier.is_empty() {
                break;
            }
            let mut next_frontier: Vec<NodeId> = Vec::new();
            for node in frontier {
                for &neighbor in self.synapse.outgoing(node, kind) {
                    if visited.insert(neighbor) {
                        if !input_set.contains(&neighbor) {
                            result_set.insert(neighbor);
                        }
                        next_frontier.push(neighbor);
                    }
                }
            }
            frontier = next_frontier;
        }
        let mut result: Vec<String> = result_set
            .iter()
            .filter_map(|&nid| self.path_of(nid).map(str::to_owned))
            .collect();
        result.sort_unstable();
        result
    }

    /// Return all sibling paths — direct children of the same parent container,
    /// excluding `id` itself.  Root nodes (no `>` in path) return empty `Vec`.
    /// Results sorted lexicographically.
    #[must_use]
    pub fn siblings(&self, id: NodeId) -> Vec<String> {
        let Some(path) = self.path_of(id) else {
            return Vec::new();
        };
        let Some(idx) = path.rfind('>') else {
            return Vec::new();
        };
        let parent_path = &path[..idx];
        let prefix = format!("{parent_path}>");
        let mut result: Vec<String> = self
            .trunk
            .all_paths()
            .filter(|p| p.starts_with(prefix.as_str()))
            .filter(|p| !p[prefix.len()..].contains('>'))
            .filter(|p| *p != path)
            .map(str::to_owned)
            .collect();
        result.sort_unstable();
        result
    }

    /// Return in/out edge counts for all four `EdgeKind`s.  O(1) per kind.
    #[must_use]
    pub fn node_degree(&self, id: NodeId) -> NodeDegree {
        NodeDegree {
            in_calls: self.synapse.incoming(id, EdgeKind::Calls).len(),
            out_calls: self.synapse.outgoing(id, EdgeKind::Calls).len(),
            in_imports: self.synapse.incoming(id, EdgeKind::Imports).len(),
            out_imports: self.synapse.outgoing(id, EdgeKind::Imports).len(),
            in_extends: self.synapse.incoming(id, EdgeKind::Extends).len(),
            out_extends: self.synapse.outgoing(id, EdgeKind::Extends).len(),
            in_implements: self.synapse.incoming(id, EdgeKind::Implements).len(),
            out_implements: self.synapse.outgoing(id, EdgeKind::Implements).len(),
        }
    }

    /// Batch degree query — returns one `NodeDegree` per id, in input order.
    ///
    /// IDs absent from the synapse return `NodeDegree::default()` (all zeros).
    #[must_use]
    pub fn batch_node_degree(&self, ids: &[NodeId]) -> Vec<NodeDegree> {
        ids.iter().map(|&id| self.node_degree(id)).collect()
    }

    /// Symbol nodes that are articulation points (cut vertices) in the undirected
    /// version of the symbol graph for `kind`.
    ///
    /// Uses Tarjan's iterative DFS (discovery time + low-link values), O(V+E).
    /// Edges treated as undirected.  File nodes and singleton nodes excluded.
    /// Results sorted ascending.
    #[must_use]
    pub fn articulation_points(&self, kind: EdgeKind) -> Vec<String> {
        let sym_ids: Vec<NodeId> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter_map(|p| self.trunk.lookup_path(p))
            .collect();

        let n = sym_ids.len();
        if n == 0 {
            return Vec::new();
        }

        let id_to_idx: HashMap<NodeId, usize> = sym_ids
            .iter()
            .copied()
            .enumerate()
            .map(|(i, id)| (id, i))
            .collect();

        let sym_set: HashSet<NodeId> = sym_ids.iter().copied().collect();

        // Build undirected adjacency (deduplicated).
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (idx, &id) in sym_ids.iter().enumerate() {
            for &nb in self.synapse.outgoing(id, kind) {
                if sym_set.contains(&nb) && nb != id {
                    let nb_idx = id_to_idx[&nb];
                    adj[idx].push(nb_idx);
                    adj[nb_idx].push(idx);
                }
            }
        }
        // Dedup adjacency lists to avoid multi-edge issues.
        for list in &mut adj {
            list.sort_unstable();
            list.dedup();
        }

        let mut disc = vec![AP_UNVISITED; n];
        let mut low = vec![0usize; n];
        let mut parent = vec![AP_UNVISITED; n];
        let mut is_ap = vec![false; n];
        let mut timer = 0usize;

        for start in 0..n {
            if disc[start] != AP_UNVISITED {
                continue;
            }

            // Iterative DFS using explicit stack.
            // Each entry: (node, index into adj[node] to process next).
            let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
            disc[start] = timer;
            low[start] = timer;
            timer += 1;
            let mut root_dfs_children = 0usize;

            while let Some((u, ei)) = stack.last_mut() {
                let u = *u;
                if *ei < adj[u].len() {
                    let v = adj[u][*ei];
                    *ei += 1;
                    if disc[v] == AP_UNVISITED {
                        // Tree edge.
                        if u == start {
                            root_dfs_children += 1;
                        }
                        parent[v] = u;
                        disc[v] = timer;
                        low[v] = timer;
                        timer += 1;
                        stack.push((v, 0));
                    } else if v != parent[u] {
                        // Back edge: update low.
                        low[u] = low[u].min(disc[v]);
                    }
                } else {
                    stack.pop();
                    if let Some(&(pu, _)) = stack.last() {
                        low[pu] = low[pu].min(low[u]);
                        // Non-root AP condition: low[child] >= disc[parent].
                        // Clippy flags low[u]/disc[pu] as suspicious grouping; it is intentional.
                        #[allow(clippy::suspicious_operation_groupings)]
                        if pu != start && low[u] >= disc[pu] {
                            is_ap[pu] = true;
                        }
                    }
                }
            }

            // Root AP condition: root has > 1 DFS tree children.
            if root_dfs_children > 1 {
                is_ap[start] = true;
            }
        }

        let mut result: Vec<String> = sym_ids
            .iter()
            .enumerate()
            .filter(|&(idx, _)| is_ap[idx])
            .filter_map(|(_, &id)| self.path_of(id).map(str::to_owned))
            .collect();
        result.sort_unstable();
        result
    }

    /// Bridge edges (cut edges) in the undirected symbol graph for `kind`.
    ///
    /// A bridge is an edge whose removal disconnects its weakly-connected
    /// component.  Uses Tarjan's iterative bridge-finding DFS (O(V+E)).
    /// Edges are treated as undirected; file nodes are excluded.
    ///
    /// Returns `(from_path, to_path)` pairs with `from_path ≤ to_path`
    /// (canonical order), sorted ascending by `(from, to)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use mycelium_core::store::Store;
    /// use mycelium_core::trunk::TrunkPath;
    /// use mycelium_core::types::EdgeKind;
    ///
    /// let mut store = Store::new();
    /// let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
    /// let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
    /// store.upsert_edge(EdgeKind::Calls, a, b);
    /// let bridges = store.bridge_edges(EdgeKind::Calls);
    /// assert_eq!(bridges, vec![("src/a.rs>a".to_owned(), "src/b.rs>b".to_owned())]);
    /// ```
    #[must_use]
    pub fn bridge_edges(&self, kind: EdgeKind) -> Vec<(String, String)> {
        let sym_ids: Vec<NodeId> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter_map(|p| self.trunk.lookup_path(p))
            .collect();
        let n = sym_ids.len();
        if n == 0 {
            return Vec::new();
        }
        let id_to_idx: HashMap<NodeId, usize> = sym_ids
            .iter()
            .copied()
            .enumerate()
            .map(|(i, id)| (id, i))
            .collect();
        let sym_set: HashSet<NodeId> = sym_ids.iter().copied().collect();

        // Build undirected adjacency with edge-multiplicity count.
        // We need to know if (u, v) has multiplicity > 1 to detect non-bridges.
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut edge_count: HashMap<(usize, usize), usize> = HashMap::new();
        for (idx, &id) in sym_ids.iter().enumerate() {
            for &nb in self.synapse.outgoing(id, kind) {
                if sym_set.contains(&nb) && nb != id {
                    let nb_idx = id_to_idx[&nb];
                    let key = if idx < nb_idx {
                        (idx, nb_idx)
                    } else {
                        (nb_idx, idx)
                    };
                    *edge_count.entry(key).or_insert(0) += 1;
                    adj[idx].push(nb_idx);
                    adj[nb_idx].push(idx);
                }
            }
        }
        for list in &mut adj {
            list.sort_unstable();
            list.dedup();
        }

        let mut disc = vec![AP_UNVISITED; n];
        let mut low = vec![0usize; n];
        let mut parent = vec![AP_UNVISITED; n];
        let mut is_bridge: HashSet<(usize, usize)> = HashSet::new();
        let mut timer = 0usize;

        for start in 0..n {
            if disc[start] != AP_UNVISITED {
                continue;
            }
            let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
            disc[start] = timer;
            low[start] = timer;
            timer += 1;

            while let Some((u, ei)) = stack.last_mut() {
                let u = *u;
                if *ei < adj[u].len() {
                    let v = adj[u][*ei];
                    *ei += 1;
                    if disc[v] == AP_UNVISITED {
                        parent[v] = u;
                        disc[v] = timer;
                        low[v] = timer;
                        timer += 1;
                        stack.push((v, 0));
                    } else if v != parent[u] {
                        low[u] = low[u].min(disc[v]);
                    }
                } else {
                    stack.pop();
                    if let Some(&(pu, _)) = stack.last() {
                        low[pu] = low[pu].min(low[u]);
                        // Bridge condition: low[child] > disc[parent] (strict).
                        // Also check multiplicity: parallel edges mean not a bridge.
                        let key = if pu < u { (pu, u) } else { (u, pu) };
                        if low[u] > disc[pu] && edge_count.get(&key).copied().unwrap_or(0) == 1 {
                            is_bridge.insert(key);
                        }
                    }
                }
            }
        }

        let mut result: Vec<(String, String)> = is_bridge
            .iter()
            .filter_map(|&(a, b)| {
                let pa = self.path_of(sym_ids[a])?.to_owned();
                let pb = self.path_of(sym_ids[b])?.to_owned();
                let pair = if pa <= pb { (pa, pb) } else { (pb, pa) };
                Some(pair)
            })
            .collect();
        result.sort_unstable();
        result
    }

    /// Topological ordering of the symbol graph for `kind` via Kahn's algorithm.
    ///
    /// Returns a [`TopologicalOrder`] with:
    /// - `order`: symbols in dependency order (sources first); ties broken by
    ///   path ascending for determinism.
    /// - `cycle_members`: symbols that are part of a directed cycle and could
    ///   not be placed in the linear order; sorted ascending.
    ///
    /// File nodes excluded.
    #[must_use]
    pub fn topological_sort(&self, kind: EdgeKind) -> TopologicalOrder {
        // Collect symbol ids and build adjacency + in-degree structures.
        let sym_ids: Vec<NodeId> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter_map(|p| self.trunk.lookup_path(p))
            .collect();

        if sym_ids.is_empty() {
            return TopologicalOrder::default();
        }

        let sym_set: HashSet<NodeId> = sym_ids.iter().copied().collect();
        let id_to_idx: HashMap<NodeId, usize> = sym_ids
            .iter()
            .copied()
            .enumerate()
            .map(|(i, id)| (id, i))
            .collect();

        let n = sym_ids.len();
        let mut in_degree = vec![0usize; n];
        let mut successors: Vec<Vec<usize>> = vec![Vec::new(); n];

        for (idx, &id) in sym_ids.iter().enumerate() {
            for &nb in self.synapse.outgoing(id, kind) {
                if sym_set.contains(&nb) {
                    let nb_idx = id_to_idx[&nb];
                    successors[idx].push(nb_idx);
                    in_degree[nb_idx] += 1;
                }
            }
        }

        // Kahn's BFS: start with all zero-in-degree nodes, sorted by path for determinism.
        let mut queue: std::collections::BinaryHeap<std::cmp::Reverse<(String, usize)>> = sym_ids
            .iter()
            .enumerate()
            .filter(|&(idx, _)| in_degree[idx] == 0)
            .filter_map(|(idx, &id)| {
                self.path_of(id)
                    .map(|p| std::cmp::Reverse((p.to_owned(), idx)))
            })
            .collect();

        let mut order: Vec<String> = Vec::with_capacity(n);

        while let Some(std::cmp::Reverse((path, idx))) = queue.pop() {
            order.push(path);
            // Sort successors by path for determinism.
            let mut next: Vec<(String, usize)> = successors[idx]
                .iter()
                .copied()
                .filter_map(|nb| {
                    in_degree[nb] -= 1;
                    if in_degree[nb] == 0 {
                        self.path_of(sym_ids[nb]).map(|p| (p.to_owned(), nb))
                    } else {
                        None
                    }
                })
                .collect();
            next.sort_unstable_by(|a, b| a.0.cmp(&b.0));
            for (p, nb) in next {
                queue.push(std::cmp::Reverse((p, nb)));
            }
        }

        // Remaining nodes with in_degree > 0 are cycle members.
        let mut cycle_members: Vec<String> = sym_ids
            .iter()
            .enumerate()
            .filter(|&(idx, _)| in_degree[idx] > 0)
            .filter_map(|(_, &id)| self.path_of(id).map(str::to_owned))
            .collect();
        cycle_members.sort_unstable();

        TopologicalOrder {
            order,
            cycle_members,
        }
    }

    /// Groups symbol nodes into weakly-connected components for `kind`, treating edges
    /// as undirected.  Uses path-compressed Union-Find (O(α(V) · E)).
    ///
    /// Returns one `Vec<String>` per component (symbols sorted ascending).
    /// Components sorted by size descending; ties broken by first element ascending.
    /// File nodes excluded.
    #[must_use]
    pub fn weakly_connected_components(&self, kind: EdgeKind) -> Vec<Vec<String>> {
        let sym_ids: Vec<NodeId> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter_map(|p| self.trunk.lookup_path(p))
            .collect();

        let n = sym_ids.len();
        if n == 0 {
            return Vec::new();
        }

        let id_to_idx: HashMap<NodeId, usize> = sym_ids
            .iter()
            .copied()
            .enumerate()
            .map(|(i, id)| (id, i))
            .collect();

        // Path-compressed Union-Find.
        let mut parent: Vec<usize> = (0..n).collect();

        let sym_set: HashSet<NodeId> = sym_ids.iter().copied().collect();
        for (idx, &id) in sym_ids.iter().enumerate() {
            for &nb in self.synapse.outgoing(id, kind) {
                if sym_set.contains(&nb) {
                    let nb_idx = id_to_idx[&nb];
                    uf_union(&mut parent, idx, nb_idx);
                }
            }
        }

        // Flatten path compression and group by root.
        let mut groups: HashMap<usize, Vec<String>> = HashMap::new();
        for (idx, &id) in sym_ids.iter().enumerate() {
            let root = uf_find(&mut parent, idx);
            if let Some(p) = self.path_of(id) {
                groups.entry(root).or_default().push(p.to_owned());
            }
        }

        let mut result: Vec<Vec<String>> = groups
            .into_values()
            .map(|mut comp| {
                comp.sort_unstable();
                comp
            })
            .collect();

        result.sort_unstable_by(|a, b| b.len().cmp(&a.len()).then_with(|| a[0].cmp(&b[0])));
        result
    }

    /// Symbol nodes that participate in at least one directed cycle for `kind`.
    ///
    /// Uses Kosaraju's two-pass SCC algorithm.  Any node whose SCC has size ≥ 2
    /// is a cycle member.  File nodes are excluded.  Results sorted ascending.
    #[must_use]
    pub fn cycle_members(&self, kind: EdgeKind) -> Vec<String> {
        // Collect symbol node ids and build forward/reverse adjacency lists.
        let sym_ids: Vec<NodeId> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter_map(|p| self.trunk.lookup_path(p))
            .collect();

        let sym_set: HashSet<NodeId> = sym_ids.iter().copied().collect();

        // Map NodeId → dense index for the arrays.
        let id_to_idx: HashMap<NodeId, usize> = sym_ids
            .iter()
            .copied()
            .enumerate()
            .map(|(i, id)| (id, i))
            .collect();

        let n = sym_ids.len();
        let mut fwd: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut rev: Vec<Vec<usize>> = vec![Vec::new(); n];

        for (idx, &id) in sym_ids.iter().enumerate() {
            for &nb in self.synapse.outgoing(id, kind) {
                if sym_set.contains(&nb) {
                    let nb_idx = id_to_idx[&nb];
                    fwd[idx].push(nb_idx);
                    rev[nb_idx].push(idx);
                }
            }
        }

        // Kosaraju pass 1: DFS on forward graph, record finish order.
        let mut visited = vec![false; n];
        let mut finish_order: Vec<usize> = Vec::with_capacity(n);
        for start in 0..n {
            if !visited[start] {
                // Iterative DFS to avoid stack overflow on large graphs.
                let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
                visited[start] = true;
                while let Some((node, edge_idx)) = stack.last_mut() {
                    if *edge_idx < fwd[*node].len() {
                        let nb = fwd[*node][*edge_idx];
                        *edge_idx += 1;
                        if !visited[nb] {
                            visited[nb] = true;
                            stack.push((nb, 0));
                        }
                    } else {
                        let node = *node;
                        stack.pop();
                        finish_order.push(node);
                    }
                }
            }
        }

        // Kosaraju pass 2: DFS on reverse graph in reverse finish order.
        let mut comp = vec![usize::MAX; n];
        let mut comp_id = 0usize;
        for &start in finish_order.iter().rev() {
            if comp[start] != usize::MAX {
                continue;
            }
            let mut stack: Vec<usize> = vec![start];
            comp[start] = comp_id;
            while let Some(node) = stack.pop() {
                for &nb in &rev[node] {
                    if comp[nb] == usize::MAX {
                        comp[nb] = comp_id;
                        stack.push(nb);
                    }
                }
            }
            comp_id += 1;
        }

        // Count component sizes.
        let mut comp_size = vec![0usize; comp_id];
        for &c in &comp {
            if c != usize::MAX {
                comp_size[c] += 1;
            }
        }

        // Collect nodes in SCCs with size ≥ 2.
        let mut result: Vec<String> = sym_ids
            .iter()
            .enumerate()
            .filter(|&(idx, _)| comp[idx] != usize::MAX && comp_size[comp[idx]] >= 2)
            .filter_map(|(_, &id)| self.path_of(id).map(str::to_owned))
            .collect();
        result.sort_unstable();
        result
    }

    /// Symbol nodes with zero connectivity across all four `EdgeKind`s.
    /// File nodes excluded. Optional prefix filter. Results sorted alphabetically.
    #[must_use]
    pub fn isolated_symbols(&self, prefix: Option<&str>) -> Vec<String> {
        let mut result: Vec<String> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter(|p| prefix.is_none_or(|pfx| p.starts_with(pfx)))
            .filter(|p| {
                self.trunk.lookup_path(p).is_some_and(|id| {
                    let d = self.node_degree(id);
                    d.in_calls == 0
                        && d.out_calls == 0
                        && d.in_imports == 0
                        && d.out_imports == 0
                        && d.in_extends == 0
                        && d.out_extends == 0
                        && d.in_implements == 0
                        && d.out_implements == 0
                })
            })
            .map(str::to_owned)
            .collect();
        result.sort_unstable();
        result
    }

    /// Return top-`limit` files (nodes without `>`) ranked by direct child
    /// symbol count, descending.  Ties broken by path ascending.  Files with
    /// zero direct children are excluded.  `limit` is capped at 100.
    #[must_use]
    pub fn top_files(&self, limit: usize) -> Vec<(String, usize)> {
        let limit = limit.min(100);
        let mut counts: Vec<(String, usize)> = self
            .trunk
            .all_paths()
            .filter(|p| !p.contains('>'))
            .map(|file_path| {
                let prefix = format!("{file_path}>");
                let count = self
                    .trunk
                    .all_paths()
                    .filter(|p| p.starts_with(prefix.as_str()) && !p[prefix.len()..].contains('>'))
                    .count();
                (file_path.to_owned(), count)
            })
            .filter(|(_, count)| *count > 0)
            .collect();
        counts.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        counts.truncate(limit);
        counts
    }

    /// Return top-`limit` symbol nodes (paths containing `>`) ranked by total
    /// degree (in + out) for `kind`, descending.  Ties broken by path ascending.
    /// Nodes with degree 0 and file-level nodes are excluded.  Capped at 100.
    #[must_use]
    pub fn most_connected(&self, limit: usize, kind: EdgeKind) -> Vec<(String, usize)> {
        let limit = limit.min(100);
        let mut entries: Vec<(String, usize)> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter_map(|p| {
                self.trunk.lookup_path(p).map(|id| {
                    let degree = self.synapse.incoming(id, kind).len()
                        + self.synapse.outgoing(id, kind).len();
                    (p.to_owned(), degree)
                })
            })
            .filter(|(_, degree)| *degree > 0)
            .collect();
        entries.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        entries.truncate(limit);
        entries
    }

    /// Return symbol nodes (paths containing `>`) with out-degree 0 for `kind`.
    /// Symmetric complement to `entry_points` (which uses in-degree 0 for Calls).
    /// Results are sorted alphabetically; `limit` is capped at 100.
    #[must_use]
    pub fn leaf_symbols(&self, kind: EdgeKind, limit: usize) -> Vec<String> {
        let limit = limit.min(100);
        let mut result: Vec<String> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter(|p| {
                self.trunk
                    .lookup_path(p)
                    .is_some_and(|id| self.synapse.outgoing(id, kind).is_empty())
            })
            .map(str::to_owned)
            .collect();
        result.sort_unstable();
        result.truncate(limit);
        result
    }

    /// BFS shortest path from `from` to `to` following outgoing `kind` edges.
    /// Returns `Some(path)` where each element is a node path string (endpoints
    /// included), or `None` if no path exists.  `from == to` returns a
    /// single-element vec.  Cycle-safe via visited set.
    #[must_use]
    pub fn shortest_path(&self, from: NodeId, to: NodeId, kind: EdgeKind) -> Option<Vec<String>> {
        if from == to {
            return self.path_of(from).map(|p| vec![p.to_owned()]);
        }
        // BFS: queue holds (current_node, path_so_far)
        let mut visited: HashSet<NodeId> = HashSet::new();
        visited.insert(from);
        let mut queue: VecDeque<(NodeId, Vec<NodeId>)> = VecDeque::new();
        queue.push_back((from, vec![from]));
        while let Some((node, node_path)) = queue.pop_front() {
            for &neighbor in self.synapse.outgoing(node, kind) {
                if neighbor == to {
                    let mut full_path = node_path;
                    full_path.push(to);
                    return full_path
                        .iter()
                        .map(|&id| self.path_of(id).map(str::to_owned))
                        .collect();
                }
                if visited.insert(neighbor) {
                    let mut new_path = node_path.clone();
                    new_path.push(neighbor);
                    queue.push_back((neighbor, new_path));
                }
            }
        }
        None
    }

    /// Count all indexed symbols grouped by `NodeKind` wire string.
    /// Only nodes present in `kind_map` are counted.  Returns `(kind, count)`
    /// pairs sorted alphabetically by kind name; kinds with count 0 are excluded.
    #[must_use]
    pub fn symbol_count_by_kind(&self) -> Vec<(String, usize)> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for &kind in self.kind_map.values() {
            *counts.entry(kind.as_str().to_owned()).or_insert(0) += 1;
        }
        counts.into_iter().collect()
    }

    /// Return paths of nodes that are incoming neighbours for ALL of `target_ids`
    /// via `kind` edges (set intersection of each target's in-neighbour set).
    /// Empty `target_ids` returns empty `Vec`.  Results sorted alphabetically.
    #[must_use]
    pub fn common_callers(&self, target_ids: &[NodeId], kind: EdgeKind) -> Vec<String> {
        let mut iter = target_ids.iter();
        let Some(&first) = iter.next() else {
            return Vec::new();
        };
        let mut common: HashSet<NodeId> =
            self.synapse.incoming(first, kind).iter().copied().collect();
        for &target in iter {
            let incoming: HashSet<NodeId> = self
                .synapse
                .incoming(target, kind)
                .iter()
                .copied()
                .collect();
            common.retain(|id| incoming.contains(id));
            if common.is_empty() {
                return Vec::new();
            }
        }
        let mut result: Vec<String> = common
            .iter()
            .filter_map(|&id| self.path_of(id).map(str::to_owned))
            .collect();
        result.sort_unstable();
        result
    }

    /// Symbols that appear as an outgoing neighbour for ALL sources for `kind`.
    /// Empty `source_ids` returns empty `Vec`. Results sorted alphabetically.
    #[must_use]
    pub fn common_callees(&self, source_ids: &[NodeId], kind: EdgeKind) -> Vec<String> {
        let mut iter = source_ids.iter();
        let Some(&first) = iter.next() else {
            return Vec::new();
        };
        let mut common: HashSet<NodeId> =
            self.synapse.outgoing(first, kind).iter().copied().collect();
        for &source in iter {
            let outgoing: HashSet<NodeId> = self
                .synapse
                .outgoing(source, kind)
                .iter()
                .copied()
                .collect();
            common.retain(|id| outgoing.contains(id));
            if common.is_empty() {
                return Vec::new();
            }
        }
        let mut result: Vec<String> = common
            .iter()
            .filter_map(|&id| self.path_of(id).map(str::to_owned))
            .collect();
        result.sort_unstable();
        result
    }

    /// Top-N symbol nodes ranked by out-degree (outgoing edge count) for `kind`.
    /// Excludes file nodes and nodes with out-degree 0.  Sorted descending by
    /// out-degree; ties broken alphabetically.  `limit` capped at 100.
    #[must_use]
    pub fn fan_out_rank(&self, kind: EdgeKind, limit: usize) -> Vec<(String, usize)> {
        let limit = limit.min(100);
        let mut entries: Vec<(String, usize)> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter_map(|p| {
                self.trunk.lookup_path(p).map(|id| {
                    let deg = self.synapse.outgoing(id, kind).len();
                    (p.to_owned(), deg)
                })
            })
            .filter(|(_, deg)| *deg > 0)
            .collect();
        entries.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        entries.truncate(limit);
        entries
    }

    /// Top-N symbol nodes ranked by in-degree for `kind`. Symmetric complement to `fan_out_rank`.
    #[must_use]
    pub fn fan_in_rank(&self, kind: EdgeKind, limit: usize) -> Vec<(String, usize)> {
        let limit = limit.min(100);
        let mut entries: Vec<(String, usize)> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter_map(|p| {
                self.trunk.lookup_path(p).map(|id| {
                    let deg = self.synapse.incoming(id, kind).len();
                    (p.to_owned(), deg)
                })
            })
            .filter(|(_, deg)| *deg > 0)
            .collect();
        entries.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        entries.truncate(limit);
        entries
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
