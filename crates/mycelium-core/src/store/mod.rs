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

pub mod backend;
pub mod in_memory;
pub mod journal;
#[cfg(feature = "memory-bound")]
pub mod memory_budget;
#[cfg(feature = "redb-backend")]
pub mod redb_backend;
#[cfg(feature = "redb-backend")]
mod redb_codec;
#[cfg(feature = "redb-backend")]
pub mod redb_keys;
#[cfg(feature = "redb-backend")]
pub mod redb_tags;

use std::collections::{HashSet, VecDeque};
use std::io::{BufReader, BufWriter};
use std::path::Path;

use std::collections::BTreeMap;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use hashbrown::HashMap;

use crate::resolver::receiver::{self, Candidate, ReceiverContext, Resolution};
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

/// In- and out-degree frequency distribution for [`Store::degree_histogram`].
#[derive(Debug, Clone, Default)]
pub struct DegreeHistogram {
    /// `(in_degree, symbol_count)` pairs, sorted by degree ascending.
    pub in_degrees: Vec<(u64, u64)>,
    /// `(out_degree, symbol_count)` pairs, sorted by degree ascending.
    pub out_degrees: Vec<(u64, u64)>,
}

/// Structural summary metrics for [`Store::graph_metrics`].
#[derive(Debug, Clone, Default)]
pub struct EdgeKindMetrics {
    /// Number of symbol nodes (file nodes excluded).
    pub symbol_count: usize,
    /// Total directed edges for the queried `EdgeKind`.
    pub directed_edge_count: usize,
    /// `E / (V*(V-1))` — directed graph density; 0.0 for V < 2.
    pub density: f64,
    /// `directed_edge_count / symbol_count`; 0.0 for empty graph.
    pub avg_degree: f64,
    /// Highest in-degree seen across all symbol nodes.
    pub max_in_degree: usize,
    /// Highest out-degree seen across all symbol nodes.
    pub max_out_degree: usize,
}

/// One entry in the result of [`Store::page_rank`].
#[derive(Debug, Clone)]
pub struct PageRankEntry {
    /// Materialized path of the symbol node.
    pub path: String,
    /// `PageRank` score (unnormalized; sum ≈ 1.0 over all symbol nodes).
    pub score: f64,
}

/// One entry in the result of [`Store::betweenness_centrality`].
#[derive(Debug, Clone)]
pub struct BetweennessEntry {
    /// Materialized path of the symbol node.
    pub path: String,
    /// Normalized betweenness centrality score ∈ [0.0, 1.0].
    pub score: f64,
}

/// One strongly connected component from [`Store::strongly_connected_components`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SccEntry {
    /// Materialized paths of symbol nodes in this component, sorted alphabetically.
    pub members: Vec<String>,
    /// Number of members; equals `members.len()`.
    pub size: usize,
}

/// One entry in the result of [`Store::degree_centrality`].
#[derive(Debug, Clone)]
pub struct DegreeCentralityEntry {
    /// Materialized path of the symbol node.
    pub path: String,
    /// Raw in-degree (number of incoming edges of the given kind).
    pub in_degree: usize,
    /// Raw out-degree (number of outgoing edges of the given kind).
    pub out_degree: usize,
    /// Normalized in-degree: `in_degree / (n-1)` ∈ [0.0, 1.0].
    pub in_centrality: f64,
    /// Normalized out-degree: `out_degree / (n-1)` ∈ [0.0, 1.0].
    pub out_centrality: f64,
}

/// One entry in the result of [`Store::closeness_centrality`].
#[derive(Debug, Clone)]
pub struct ClosenessCentralityEntry {
    /// Materialized path of the symbol node.
    pub path: String,
    /// Wasserman-Faust normalized closeness centrality score ∈ [0.0, 1.0].
    pub score: f64,
}

/// Result of [`Store::mutual_reachability`]: forward/backward BFS distances
/// and derived reachability flags for two symbol nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutualReachability {
    /// `true` if `id1` can reach `id2`.
    pub forward: bool,
    /// `true` if `id2` can reach `id1`.
    pub backward: bool,
    /// `true` if both directions are reachable.
    pub mutual: bool,
    /// BFS hop count `id1 → id2`; `None` if unreachable.
    pub forward_distance: Option<usize>,
    /// BFS hop count `id2 → id1`; `None` if unreachable.
    pub backward_distance: Option<usize>,
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

/// Pop edges from `edge_stack` until (and including) the canonical edge
/// `(min(pu,u), max(pu,u))`, collect the node indices into a set, and push
/// the set into `raw_comps` if it contains ≥ 2 distinct nodes.
fn bcc_pop_component(
    pu: usize,
    u: usize,
    edge_stack: &mut Vec<(usize, usize)>,
    raw_comps: &mut Vec<Vec<usize>>,
) {
    let target = if pu < u { (pu, u) } else { (u, pu) };
    let mut comp_nodes: std::collections::HashSet<usize> = std::collections::HashSet::new();
    while let Some(e) = edge_stack.pop() {
        comp_nodes.insert(e.0);
        comp_nodes.insert(e.1);
        if e == target {
            break;
        }
    }
    if comp_nodes.len() >= 2 {
        raw_comps.push(comp_nodes.into_iter().collect());
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
    /// Per-call-site receiver context captured by the extractor (RFC-0118 Part B).
    ///
    /// Transient: `#[serde(skip)]` — captured during extraction, drained by the
    /// post-merge [`Store::resolve_call_site_contexts`] pass, and never persisted
    /// (so the snapshot/redb wire format is unchanged). Empty in a loaded store.
    #[serde(default, skip)]
    call_site_contexts: Vec<CallSiteContext>,
}

/// One captured method call site awaiting post-merge disambiguation (RFC-0118 B).
///
/// The extractor records this when it cannot statically bind a method call to a
/// definition and falls back to the shared `Unresolved` stub.
#[derive(Clone, Debug)]
pub struct CallSiteContext {
    /// The calling symbol (source of the conservative `Calls` edge).
    pub caller_id: NodeId,
    /// The `Unresolved` placeholder the extractor minted for this call.
    pub stub_id: NodeId,
    /// The per-call-site receiver facts for [`receiver::infer_receiver_type`].
    pub receiver_ctx: ReceiverContext,
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

    /// Deserialize a `Store` from a snapshot at `path`.
    ///
    /// With the `redb-backend` feature enabled, the file format is auto-detected:
    /// a redb database is loaded via [`crate::store::redb_backend::RedbBackend`];
    /// a legacy `MessagePack` snapshot falls back to `rmp_serde`.  Without the
    /// feature, only `MessagePack` is attempted.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened, the format is unrecognised,
    /// or deserialization fails.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        #[cfg(feature = "redb-backend")]
        if let Some(store) = Self::try_load_redb(path) {
            return Ok(store);
        }

        let file = std::fs::File::open(path)
            .with_context(|| format!("opening snapshot file {}", path.display()))?;
        let reader = BufReader::new(file);
        rmp_serde::decode::from_read(reader)
            .with_context(|| format!("deserializing store from {}", path.display()))
    }

    /// Try to open `path` as a redb database and convert it to a [`Store`].
    ///
    /// Returns `Some(store)` on success, `None` if the file is absent or not a
    /// redb database (caller falls back to the next format reader).
    #[cfg(feature = "redb-backend")]
    fn try_load_redb(path: &Path) -> Option<Self> {
        use crate::store::backend::StorageBackend as _;
        use crate::store::redb_backend::RedbBackend;

        let backend = RedbBackend::open_existing(path).ok()?;

        let mut store = Self::default();
        for path_str in backend.all_paths() {
            if let Ok(tp) = TrunkPath::parse(&path_str) {
                let id = store.trunk.upsert(tp);
                if let Some(kind) = backend.kind_of(id) {
                    store.kind_map.insert(id, kind);
                }
                if let Some(span) = backend.span_of(id) {
                    store.span_map.insert(id, span);
                }
            }
        }
        for (kind, src, dst) in backend.all_edges() {
            store.synapse.add(kind, src, dst);
        }
        Some(store)
    }

    /// Serialize a sub-store to a `Base64`-encoded `MessagePack` blob for journal deltas.
    #[must_use]
    pub fn serialize_delta(store: &Self) -> String {
        use base64::Engine;
        let mut buf = Vec::new();
        rmp_serde::encode::write(&mut buf, store).unwrap_or_default();
        base64::engine::general_purpose::STANDARD.encode(&buf)
    }

    /// Deserialize a sub-store from a `Base64`-encoded `MessagePack` blob.
    ///
    /// # Errors
    /// Returns an error if the input is not valid `Base64` or `MessagePack`.
    pub fn deserialize_delta(encoded: &str) -> anyhow::Result<Self> {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .context("decoding base64 delta")?;
        rmp_serde::decode::from_slice(&bytes).context("deserializing delta store")
    }

    /// Load the store from a base snapshot, then replay any journal deltas.
    ///
    /// If no journal exists, this is equivalent to `Store::load()`.
    ///
    /// # Errors
    /// Returns an error if the base snapshot cannot be loaded or journal replay fails.
    pub fn load_with_journal(path: &Path) -> anyhow::Result<Self> {
        let mut store = Self::load(path)?;
        let dot = Path::new(".");
        let journal_path = path.parent().unwrap_or(dot).join("journal.jsonl");
        if journal_path.exists() {
            let journal = crate::store::journal::Journal::open(path)?;
            journal.replay(&mut store)?;
        }
        Ok(store)
    }

    /// Extract a sub-Store containing only the nodes and edges belonging to
    /// `file_path` and its descendants. Used for incremental journal deltas.
    #[must_use]
    pub fn extract_file_substore(&self, file_path: &str) -> Self {
        let mut sub = Self::default();
        let Some(root_id) = self.trunk.lookup_path(file_path) else {
            return sub;
        };
        let ids: Vec<NodeId> = self
            .trunk
            .descendants(root_id)
            .chain(std::iter::once(root_id))
            .collect();
        for &id in &ids {
            if let Some(p) = self.trunk.path_of(id) {
                if let Ok(tp) = TrunkPath::parse(p) {
                    let new_id = sub.trunk.upsert(tp);
                    if let Some(&kind) = self.kind_map.get(&id) {
                        sub.kind_map.insert(new_id, kind);
                    }
                    if let Some(&span) = self.span_map.get(&id) {
                        sub.span_map.insert(new_id, span);
                    }
                }
            }
        }
        let edge_kinds: &[EdgeKind] = &[
            EdgeKind::Contains,
            EdgeKind::Calls,
            EdgeKind::Imports,
            EdgeKind::TypeImports,
            EdgeKind::Exports,
            EdgeKind::Extends,
            EdgeKind::Implements,
            EdgeKind::References,
            EdgeKind::TypeOf,
            EdgeKind::Returns,
            EdgeKind::Instantiates,
            EdgeKind::Overrides,
            EdgeKind::Decorates,
            EdgeKind::Aggregates,
            EdgeKind::Composes,
            EdgeKind::Uses,
        ];
        let id_set: HashSet<NodeId> = ids.iter().copied().collect();
        for &id in &ids {
            for &ek in edge_kinds {
                for &dst in self.synapse.outgoing(id, ek) {
                    let Some(sp) = self.trunk.path_of(id) else {
                        continue;
                    };
                    let Some(dp) = self.trunk.path_of(dst) else {
                        continue;
                    };
                    let (Ok(stp), Ok(dtp)) = (TrunkPath::parse(sp), TrunkPath::parse(dp)) else {
                        continue;
                    };
                    let s = sub.trunk.upsert(stp);
                    let d = sub.trunk.upsert(dtp);
                    sub.synapse.add(ek, s, d);
                    if !id_set.contains(&dst) {
                        if let Some(&kind) = self.kind_map.get(&dst) {
                            sub.kind_map.insert(d, kind);
                        }
                        if let Some(&span) = self.span_map.get(&dst) {
                            sub.span_map.insert(d, span);
                        }
                    }
                }
            }
        }
        sub
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

    /// Whether `id` is a real indexed symbol — i.e. NOT an unresolved-callee
    /// placeholder (RFC-0118 Part A).
    ///
    /// The resolver mints phantom nodes for calls whose receiver/callee cannot
    /// be statically resolved (e.g. `unwrap`, `Db>upsert_node`) and links a
    /// `Calls` edge to them so the caller is not falsely "dead"; those phantoms
    /// carry [`NodeKind::Unresolved`]. Everything else — real kinded symbols,
    /// AND kind-less nodes from programmatic/test stores that predate the kind
    /// contract — counts as real. This is a *negative* marker (exclude if
    /// `Unresolved`), so it needs no presence-gate: a store that never set kinds
    /// has no `Unresolved` nodes and is therefore unchanged (back-compatible).
    #[must_use]
    pub fn is_real_symbol(&self, id: NodeId) -> bool {
        self.kind_of(id) != Some(NodeKind::Unresolved)
    }

    /// Whether `id` is a navigable symbol an agent can jump to.
    ///
    /// Stricter than [`Store::is_real_symbol`]: a node is *searchable* only if it has a
    /// recorded definition kind that is not the resolver's `Unresolved` phantom
    /// marker. This additionally excludes KIND-LESS nodes — the import-target
    /// stubs the extractor mints via a bare `upsert_node` (e.g. `anyhow::Context`,
    /// `std::collections::HashMap`), which carry no span and point nowhere — that
    /// `is_real_symbol` (a pure negative gate) lets through. Callers should apply
    /// this only when the store is kind-annotated (see [`Store::search_symbol`]),
    /// so legacy programmatic stores that never set kinds keep their contract.
    #[must_use]
    pub fn is_searchable_symbol(&self, id: NodeId) -> bool {
        self.kind_of(id).is_some_and(|k| k != NodeKind::Unresolved)
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

    /// Remove the single directed edge `kind: src → dst`. No-op if absent.
    pub fn remove_edge(&mut self, kind: EdgeKind, src: NodeId, dst: NodeId) {
        self.synapse.remove_edge(kind, src, dst);
    }

    /// Union `other` into `self` (Issue #342 / R1 — parallel indexing).
    ///
    /// Every node, kind, span, and edge in `other` is added to `self`.
    /// Because [`NodeId`] is a content hash of the path (BLAKE3, see
    /// [`crate::trunk`]), the same path always maps to the same id in both
    /// stores, so the merge is a deterministic, **order-independent** union:
    /// `a.merge(&b)` then `a.merge(&c)` yields the same store as building all
    /// three sets of files into one store in any order. Both node and edge
    /// insertion are idempotent, so merging overlapping stores is safe.
    ///
    /// This is the reduce step that lets the indexer extract files in parallel
    /// into per-thread sub-stores and combine them without a shared lock.
    ///
    /// Kind/span metadata from `other` overwrites `self`'s for shared nodes;
    /// since both are derived from the same source file content, they agree.
    pub fn merge(&mut self, other: &Self) {
        // 1. Union nodes + per-node metadata. Re-upserting `other`'s paths
        //    reproduces identical NodeIds (content-hash), so `other`'s id-keyed
        //    metadata maps onto the freshly-upserted nodes verbatim.
        for path in other.trunk.all_paths() {
            if let Ok(tp) = TrunkPath::parse(path) {
                let id = self.trunk.upsert(tp);
                if let Some(&kind) = other.kind_map.get(&id) {
                    self.kind_map.insert(id, kind);
                }
                if let Some(&span) = other.span_map.get(&id) {
                    self.span_map.insert(id, span);
                }
            }
        }
        // 2. Union edges. NodeIds are global, so endpoints are valid in `self`
        //    once step 1 has upserted every node `other` knew about.
        for (kind, src, dst) in other.synapse.all_edges() {
            self.synapse.add(kind, src, dst);
        }
        // 3. Carry the captured call-site contexts forward (RFC-0118 Part B).
        //    Parallel indexing records contexts into per-thread sub-stores; if
        //    merge dropped them, the post-merge disambiguation pass would see an
        //    empty table and silently no-op in parallel mode. NodeIds are
        //    content-hashes (identity-stable across sub-stores), so the captured
        //    caller_id/stub_id remain valid in `self` with no remapping.
        self.call_site_contexts
            .extend(other.call_site_contexts.iter().cloned());
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

    /// Conservative lower-bound estimate of bytes held by this store's
    /// data structures. Intended for diagnostics and the R3 memory-bound
    /// investigation (#344) — not a precise allocator report.
    ///
    /// The estimate uses structural heuristics:
    /// - Patricia-trie nodes: ~256 bytes each (key fragment + child map + id).
    /// - CSR synapse edges: ~24 bytes each (kind tag + src `NodeId` + dst `NodeId`).
    #[must_use]
    pub fn heap_size_estimate(&self) -> usize {
        self.node_count() * 256 + self.edge_count() * 24
    }

    /// Iterate all materialized path strings (delegates to the inner Trunk).
    pub fn all_paths(&self) -> impl Iterator<Item = &str> + '_ {
        self.trunk.all_paths()
    }

    /// Iterate all symbol nodes (paths with `>`). Yields `(NodeId, &str)`.
    ///
    /// O(V) — no trie navigation. Replaces `all_paths() + filter + lookup_path()` loops.
    pub fn symbol_nodes(&self) -> impl Iterator<Item = (NodeId, &str)> + '_ {
        self.trunk.symbol_nodes()
    }

    /// The canonical **real-symbol node universe** for graph-theory queries
    /// (RFC-0118 Part A.2).
    ///
    /// Returns every `>`-qualified symbol node whose [`Store::is_real_symbol`]
    /// holds — i.e. all materialized symbols **except** the resolver's
    /// [`NodeKind::Unresolved`] phantoms (the synthetic callee/receiver stubs
    /// minted for calls that cannot be statically resolved, e.g. `unwrap`,
    /// `Db>upsert_node`). File-level nodes (no `>`) are excluded, as are
    /// phantoms.
    ///
    /// This is the *single source of truth* for the node set fed to the
    /// centrality / cycle / connectivity / k-core / layering queries. Those
    /// algorithms additionally restrict edge traversal to this set (the
    /// real-symbol *induced subgraph*), so a phantom can neither appear in a
    /// result nor inflate a real node's degree or sit on a shortest path.
    ///
    /// Back-compatible: a store that never recorded kinds has no `Unresolved`
    /// nodes, so this is exactly its `>`-qualified symbol set.
    #[must_use]
    pub fn symbol_universe(&self) -> Vec<NodeId> {
        self.symbol_nodes()
            .filter(|(id, _)| self.is_real_symbol(*id))
            .map(|(id, _)| id)
            .collect()
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
                // RFC-0118 Part A: never rank unresolved-callee phantoms.
                if !self.is_real_symbol(id) {
                    return None;
                }
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

    /// Top-N symbols by incoming-edge count for the given edge kind.
    /// Symbols with zero incoming edges of `kind` are excluded.
    /// Results are sorted descending by count, then alphabetically by path.
    #[must_use]
    pub fn top_symbols_by_incoming(&self, kind: EdgeKind, limit: usize) -> Vec<(String, usize)> {
        let mut ranked: Vec<(String, usize)> = self
            .trunk
            .all_paths()
            .filter_map(|p| {
                let id = self.trunk.lookup_path(p)?;
                // RFC-0118 Part A: never rank unresolved-callee phantoms.
                if !self.is_real_symbol(id) {
                    return None;
                }
                let count = self.synapse.incoming(id, kind).len();
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
    /// use mycelium_core::types::NodeKind;
    ///
    /// let mut store = Store::new();
    /// // File nodes carry NodeKind::File (set by the extractor at index time).
    /// store.upsert_node_with_kind(TrunkPath::parse("src/auth.rs").unwrap(), NodeKind::File);
    /// store.upsert_node(TrunkPath::parse("src/auth.rs>login").unwrap());
    /// store.upsert_node_with_kind(TrunkPath::parse("src/main.rs").unwrap(), NodeKind::File);
    /// // A kind-less bare stub (e.g. an unresolved callee) is NOT a file.
    /// store.upsert_node(TrunkPath::parse("unwrap").unwrap());
    ///
    /// let files = store.all_file_paths();
    /// assert_eq!(files, vec!["src/auth.rs", "src/main.rs"]);
    /// ```
    #[must_use]
    pub fn all_file_paths(&self) -> Vec<String> {
        // Prefer the authoritative `NodeKind::File` (set by the extractor at
        // index time) over the old `!p.contains('>')` string heuristic. The
        // resolver mints kind-less stub nodes for unresolved callees (`unwrap`)
        // and import targets (`std::collections::HashMap`) — none contain `>`,
        // so the heuristic reported them all as fake files (dogfood F1: 671 of
        // 786 get-files entries were such junk).
        //
        // Presence-gated for backward compatibility: only trust the kind when
        // the store is actually kind-annotated (≥1 `File` node, i.e. it was
        // built by the extractor). A purely programmatic / test store that
        // never set kinds keeps the historical RFC-0018 "no `>`" contract, so
        // it does not silently return an empty file list.
        let kind_annotated = self.kind_map.values().any(|k| *k == NodeKind::File);
        let mut files: Vec<String> = self
            .trunk
            .all_paths()
            .filter(|p| {
                if kind_annotated {
                    self.trunk.lookup_path(p).and_then(|id| self.kind_of(id))
                        == Some(NodeKind::File)
                } else {
                    !p.contains('>')
                }
            })
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
        // De-noise (dogfood-new): when the store is kind-annotated (built by the
        // extractor — has ≥1 `File` node), drop unnavigable nodes so every result
        // is a real, jump-to-able symbol: `is_searchable_symbol` excludes both
        // `NodeKind::Unresolved` phantoms and the kind-less import-target stubs the
        // extractor mints (24–48% of raw results on the dogfood corpus). A purely
        // programmatic store that never set kinds keeps the historical contract.
        let kind_annotated = self.kind_map.values().any(|k| *k == NodeKind::File);
        let mut results: Vec<String> = self
            .trunk
            .all_paths()
            .filter(|p| {
                p.split('>')
                    .next_back()
                    .is_some_and(|seg| seg.to_lowercase().contains(&q))
            })
            .filter(|p| {
                !kind_annotated
                    || self
                        .trunk
                        .lookup_path(p)
                        .is_some_and(|id| self.is_searchable_symbol(id))
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

    /// Return the trunk paths of every symbol *under* the given file path,
    /// sorted lexicographically. The file path itself is **excluded**.
    ///
    /// Returns an empty `Vec` if `file_rel` is not present in the trunk —
    /// callers diff against the result so the empty case is the well-defined
    /// "OLD set" of a newly-created file (RFC-0107 §5).
    #[must_use]
    pub fn symbols_in_file(&self, file_rel: &str) -> Vec<String> {
        let mut v = self.descendants_of_path(file_rel).unwrap_or_default();
        v.sort();
        v
    }

    /// Return callers of `path` that are reachable only via virtual dispatch.
    ///
    /// When a typed variable (e.g. `plugin: AbstractBase`) is used to invoke
    /// a method, the call edge points to the **base class** method, not to any
    /// concrete override.  This method finds those callers by:
    ///
    /// 1. Extracting the method name (last `>` segment) from `path`.
    /// 2. Walking Extends edges from the class that owns `path` to each base class.
    /// 3. For each base class, looking up `BaseClass>method_name` and collecting
    ///    its incoming Calls edges.
    ///
    /// Returns `None` when `path` cannot be found in the index.
    pub fn virtual_dispatch_callers_of_path(&self, path: &str) -> Option<Vec<String>> {
        let _ = self.trunk.lookup_path(path)?;
        let method_name = path.split('>').next_back()?;
        let class_path_end = path.rfind('>')?;
        let class_path = &path[..class_path_end];
        let class_id = self.trunk.lookup_path(class_path)?;

        let mut result: Vec<String> = Vec::new();
        for &base_id in self.outgoing(class_id, EdgeKind::Extends) {
            let Some(base_path) = self.trunk.path_of(base_id) else {
                continue;
            };
            let base_method_path = format!("{base_path}>{method_name}");
            let Some(base_method_id) = self.trunk.lookup_path(&base_method_path) else {
                continue;
            };
            for &caller_id in self.incoming(base_method_id, EdgeKind::Calls) {
                if let Some(p) = self.trunk.path_of(caller_id).map(str::to_owned) {
                    result.push(p);
                }
            }
        }
        result.sort();
        result.dedup();
        Some(result)
    }

    /// Return methods inherited from base classes via `Extends` edges.
    ///
    /// For each base class reachable through Extends, collect the descendants
    /// of the base that have a **method name** (last `>` segment) NOT already
    /// present among the direct descendants of `id` (i.e., overridden methods
    /// are excluded). Returns `None` if `path` is not found.
    ///
    /// Each entry in the returned vec is `(path, declaring_class_path)`.
    pub fn inherited_descendants_of_path(&self, path: &str) -> Option<Vec<(String, String)>> {
        let id = self.trunk.lookup_path(path)?;
        let direct_names: std::collections::HashSet<String> = self
            .trunk
            .descendants(id)
            .filter_map(|did| self.trunk.path_of(did))
            .filter(|p| p.contains('>'))
            .filter_map(|p| p.split('>').next_back().map(str::to_owned))
            .collect();

        let mut result: Vec<(String, String)> = Vec::new();
        for &base_id in self.outgoing(id, EdgeKind::Extends) {
            let Some(base_path) = self.trunk.path_of(base_id).map(str::to_owned) else {
                continue;
            };
            for did in self.trunk.descendants(base_id) {
                let Some(p) = self.trunk.path_of(did) else {
                    continue;
                };
                if !p.contains('>') {
                    continue;
                }
                let method_name = p.split('>').next_back().unwrap_or("");
                if !direct_names.contains(method_name) {
                    result.push((p.to_owned(), base_path.clone()));
                }
            }
        }
        Some(result)
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
        let base = self.resolve_bare_call_stubs_simple();
        let aware = self.resolve_import_aware_stubs();
        let extends_aware = self.resolve_import_aware_extends_stubs();
        // RFC-0118 Part B: receiver-type disambiguation runs LAST, after the
        // single-match passes above have removed every unambiguous stub. It only
        // resolves the multi-match method calls those passes decline (the F5
        // `store.upsert_node()` case), using captured per-call-site context.
        let by_receiver = self.resolve_call_site_contexts();
        base + aware + extends_aware + by_receiver
    }

    /// Record a method call site for post-merge receiver disambiguation
    /// (RFC-0118 Part B). Called by the extractor when a method call cannot be
    /// statically bound and falls back to the shared `Unresolved` stub.
    pub fn record_call_site(
        &mut self,
        caller_id: NodeId,
        stub_id: NodeId,
        receiver_ctx: ReceiverContext,
    ) {
        self.call_site_contexts.push(CallSiteContext {
            caller_id,
            stub_id,
            receiver_ctx,
        });
    }

    /// Post-merge pass (RFC-0118 Part B): for each captured call site, infer the
    /// receiver type statically and, when it disambiguates to exactly one
    /// definition, **add** the precise `Calls(caller → …>Type>method)` edge.
    /// Order-independent: runs against the fully merged graph, so a receiver type
    /// defined in any file resolves. Returns the number of call sites bound.
    ///
    /// **Does NOT remove the conservative `caller → stub` edge** (Codex P2 #633).
    /// Synapse stores a single deduplicated `caller → stub` edge that may stand
    /// for *multiple* calls — including ones with no recorded context (e.g. a bare
    /// `m()` alongside `s.m()`). Removing it from the recorded contexts alone
    /// could drop an edge a still-unresolved call needs. The stub carries
    /// `NodeKind::Unresolved`, so Part A already excludes it from the symbol/rank
    /// universe; the precise edge added here is a pure gain. Safe stub-edge
    /// removal is deferred to when the extractor records *every* unresolved call.
    #[must_use]
    pub fn resolve_call_site_contexts(&mut self) -> usize {
        let contexts = std::mem::take(&mut self.call_site_contexts);
        if contexts.is_empty() {
            return 0;
        }
        // Snapshot all real-symbol paths once for candidate lookup.
        let all_paths: Vec<String> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter(|p| {
                self.trunk
                    .lookup_path(p)
                    .is_some_and(|id| self.is_real_symbol(id))
            })
            .map(str::to_owned)
            .collect();

        let mut resolved = 0usize;
        for ctx in &contexts {
            // Skip a context whose stub an earlier single-match pass already
            // resolved + removed (the call is bound; re-binding would inflate the
            // count). A stub still in the trunk is one the prior passes declined.
            if self.trunk.path_of(ctx.stub_id).is_none() {
                continue;
            }
            let suffix = format!(">{}", ctx.receiver_ctx.method);
            let candidates: Vec<Candidate> = all_paths
                .iter()
                .filter(|p| p.ends_with(&suffix))
                .map(|p| Candidate {
                    node_path: p.clone(),
                })
                .collect();

            let inferred = receiver::infer_receiver_type(&ctx.receiver_ctx);
            if let Resolution::Unique(path) = receiver::disambiguate(inferred, &candidates) {
                if let Some(def_id) = self.trunk.lookup_path(&path) {
                    // Don't bind a call to itself. (A stub can't be a candidate:
                    // candidates come from all_paths filtered to contain '>',
                    // and stubs are bare names without '>'.)
                    if def_id != ctx.caller_id {
                        self.synapse.add(EdgeKind::Calls, ctx.caller_id, def_id);
                        resolved += 1;
                    }
                }
            }
        }
        resolved
    }

    fn resolve_bare_call_stubs_simple(&mut self) -> usize {
        let all_paths: Vec<String> = self.trunk.all_paths().map(str::to_owned).collect();

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
                // RFC-0118 Part C: route through remove_node so kind_map + span_map
                // are cleaned alongside trunk. redirect_node already cleared synapse,
                // so the synapse.remove_node call inside is a no-op.
                self.remove_node(stub_id);
                resolved += 1;
            }
        }
        resolved
    }

    /// Second-pass resolution using import edges to disambiguate.
    ///
    /// For each remaining bare stub, examine its callers. If a caller's file
    /// imports a module that defines the symbol, resolve the stub to that
    /// definition. This handles cases where multiple files define the same
    /// symbol name but only one is imported by the caller.
    fn resolve_import_aware_stubs(&mut self) -> usize {
        let all_paths: Vec<String> = self.trunk.all_paths().map(str::to_owned).collect();

        let stubs: Vec<(NodeId, String)> = all_paths
            .iter()
            .filter(|p| !p.contains('>'))
            .filter_map(|p| self.trunk.lookup_path(p).map(|id| (id, p.clone())))
            .collect();

        if stubs.is_empty() {
            return 0;
        }

        let suffix_map: hashbrown::HashMap<String, Vec<NodeId>> = {
            let mut m: HashMap<String, Vec<NodeId>> = HashMap::new();
            for path in &all_paths {
                if let Some(gt) = path.rfind('>') {
                    let suffix = &path[gt..];
                    if let Some(id) = self.trunk.lookup_path(path) {
                        m.entry(suffix.to_owned()).or_default().push(id);
                    }
                }
            }
            m
        };

        let mut resolved = 0;
        for (stub_id, stub_name) in stubs {
            let suffix = format!(">{stub_name}");
            let Some(defs) = suffix_map.get(&suffix) else {
                continue;
            };

            let callers: Vec<NodeId> = self.synapse.incoming(stub_id, EdgeKind::Calls).to_vec();
            if callers.is_empty() {
                continue;
            }

            let mut best_def: Option<NodeId> = None;
            let mut best_count = 0usize;

            for &def_id in defs {
                let Some(def_path) = self.trunk.path_of(def_id) else {
                    continue;
                };
                let def_path = def_path.to_owned();
                let Some(file_path) = def_path.split('>').next() else {
                    continue;
                };
                let Some(file_id) = self.trunk.lookup_path(file_path) else {
                    continue;
                };

                let mut match_count = 0usize;
                for &caller_id in &callers {
                    let Some(caller_path) = self.trunk.path_of(caller_id) else {
                        continue;
                    };
                    let caller_path = caller_path.to_owned();
                    let Some(caller_file) = caller_path.split('>').next() else {
                        continue;
                    };
                    let Some(caller_file_id) = self.trunk.lookup_path(caller_file) else {
                        continue;
                    };

                    let imports = self.synapse.outgoing(caller_file_id, EdgeKind::Imports);
                    if imports.contains(&file_id) || imports.contains(&def_id) {
                        match_count += 1;
                    }
                }

                if match_count > best_count {
                    best_count = match_count;
                    best_def = Some(def_id);
                }
            }

            if let Some(def_id) = best_def {
                self.synapse.redirect_node(stub_id, def_id);
                // RFC-0118 Part C: kind_map + span_map hygiene (same as simple pass).
                self.remove_node(stub_id);
                resolved += 1;
            }
        }
        resolved
    }

    /// RFC-0103: resolve ambiguous bare `Extends` target stubs using import
    /// evidence.
    ///
    /// The simple pass already redirects a bare inheritance target (e.g.
    /// `LanguagePlugin`) when its name has exactly one definition. When the
    /// name is defined in *several* files the simple pass leaves the stub, and
    /// the call-aware pass ignores it because an `Extends`-only stub has no
    /// `Calls` callers. This pass closes that gap: for each such stub it
    /// inspects the subclasses (incoming `Extends` edges) and favours the
    /// candidate definition whose file is imported by the subclass's file.
    ///
    /// Conservative by RFC-0103 mandate, and safe for the whole-node redirect:
    /// Per-edge import-aware Extends stub resolution (RFC-0103 follow-up, Issue #555).
    ///
    /// For each `(subclass → stub)` Extends edge, independently find which
    /// candidate definition the subclass's file imports. If exactly one
    /// candidate matches, rewire that single edge: add `(subclass → def)` and
    /// remove `(subclass → stub)`. After all subclasses are processed, if the
    /// stub's incoming Extends degree reaches zero it is removed from trunk.
    ///
    /// Mixed-import sites (different subclasses importing different defs) are
    /// now handled correctly — each edge is resolved to its own target. Ties
    /// (subclass imports ≥2 candidates) and no-match edges are left unchanged.
    fn resolve_import_aware_extends_stubs(&mut self) -> usize {
        let all_paths: Vec<String> = self.trunk.all_paths().map(str::to_owned).collect();

        let stubs: Vec<(NodeId, String)> = all_paths
            .iter()
            .filter(|p| !p.contains('>'))
            .filter_map(|p| self.trunk.lookup_path(p).map(|id| (id, p.clone())))
            .collect();

        if stubs.is_empty() {
            return 0;
        }

        let suffix_map: HashMap<String, Vec<NodeId>> = {
            let mut m: HashMap<String, Vec<NodeId>> = HashMap::new();
            for path in &all_paths {
                if let Some(gt) = path.rfind('>') {
                    if let Some(id) = self.trunk.lookup_path(path) {
                        m.entry(path[gt..].to_owned()).or_default().push(id);
                    }
                }
            }
            m
        };

        let mut resolved = 0;
        for (stub_id, stub_name) in stubs {
            let suffix = format!(">{stub_name}");
            let Some(defs) = suffix_map.get(&suffix) else {
                continue;
            };

            let subclasses: Vec<NodeId> =
                self.synapse.incoming(stub_id, EdgeKind::Extends).to_vec();
            if subclasses.is_empty() {
                continue;
            }

            // Build (file_id → def_id) lookup once per stub to avoid
            // re-deriving def file paths for every subclass.
            let def_by_file: Vec<(NodeId, NodeId)> = defs
                .iter()
                .filter_map(|&def_id| {
                    let def_path = self.trunk.path_of(def_id)?.to_owned();
                    let file_path = def_path.split('>').next()?.to_owned();
                    let file_id = self.trunk.lookup_path(&file_path)?;
                    Some((file_id, def_id))
                })
                .collect();

            // Per-edge: for each subclass independently find which def it imports.
            for &sub_id in &subclasses {
                let Some(sub_path) = self.trunk.path_of(sub_id).map(str::to_owned) else {
                    continue;
                };
                let Some(sub_file) = sub_path.split('>').next() else {
                    continue;
                };
                let Some(sub_file_id) = self.trunk.lookup_path(sub_file) else {
                    continue;
                };

                // Clone to avoid holding a borrow on self.synapse during mutation.
                let imports: Vec<NodeId> = self
                    .synapse
                    .outgoing(sub_file_id, EdgeKind::Imports)
                    .to_vec();

                let mut matched_def: Option<NodeId> = None;
                let mut match_count = 0usize;
                for &(file_id, def_id) in &def_by_file {
                    if imports.contains(&file_id) || imports.contains(&def_id) {
                        match_count += 1;
                        matched_def = Some(def_id);
                    }
                }

                // Resolve only when exactly one candidate matches (no ties).
                if match_count == 1 {
                    if let Some(def_id) = matched_def {
                        self.synapse.add(EdgeKind::Extends, sub_id, def_id);
                        self.synapse.remove_edge(EdgeKind::Extends, sub_id, stub_id);
                    }
                }
            }

            // Remove stub only when it has no remaining edges of ANY kind.
            // Checking only Extends-incoming would corrupt the graph when the
            // same bare name also has Calls/References edges (Codex P2, PR #572).
            if self.synapse.is_isolated(stub_id) {
                // RFC-0118 Part C: kind_map + span_map hygiene. Stub is already
                // isolated so synapse.remove_node inside is a no-op.
                self.remove_node(stub_id);
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
        if max_depth == 0 {
            return None;
        }
        // Parent-map BFS — O(V) space vs O(V·D) for path-clone BFS.
        // `parent[v] = Some(u)` means `u` is v's predecessor on the BFS tree.
        // `parent[from] = None` is the root sentinel.
        let mut parent: HashMap<NodeId, Option<NodeId>> = HashMap::new();
        parent.insert(from, None);
        let mut queue: VecDeque<(NodeId, usize)> = VecDeque::new(); // (node, edge-depth)
        queue.push_back((from, 0));
        while let Some((cur, edge_depth)) = queue.pop_front() {
            for &next in self.synapse.outgoing(cur, EdgeKind::Calls) {
                if parent.contains_key(&next) {
                    continue;
                }
                parent.insert(next, Some(cur));
                if next == to {
                    // Reconstruct path via parent links.
                    let mut path = vec![to];
                    let mut node = to;
                    while let Some(&Some(p)) = parent.get(&node) {
                        path.push(p);
                        node = p;
                    }
                    path.reverse();
                    return Some(path);
                }
                if edge_depth + 1 < max_depth {
                    queue.push_back((next, edge_depth + 1));
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

        // RFC-0118 Part A.2: operate on the real-symbol induced subgraph — both
        // the DFS roots and every traversed edge are restricted to real symbols,
        // so a phantom can neither be a cycle member nor close a cycle through a
        // real node.
        let real: HashSet<NodeId> = self.symbol_universe().into_iter().collect();
        let all_ids: Vec<NodeId> = real.iter().copied().collect();

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
                    // Induced subgraph: ignore edges to non-real (phantom/file) nodes.
                    if !real.contains(&neighbor) {
                        continue;
                    }
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
        // RFC-0118 Part A.2: real-symbol induced subgraph — phantoms excluded as
        // nodes; the existing `sym_ids.contains(&w)` edge guard excludes phantom
        // edges.
        let sym_ids: Vec<NodeId> = self.symbol_universe();

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
        // RFC-0118 Part A.2: operate on the real-symbol induced subgraph — count
        // only edges whose other endpoint is a *real* symbol, and never report a
        // phantom node.
        let real: HashSet<NodeId> = self.symbol_universe().into_iter().collect();
        let mut entries: Vec<(String, usize, usize)> = real
            .iter()
            .filter_map(|&id| {
                let in_deg = self
                    .synapse
                    .incoming(id, kind)
                    .iter()
                    .filter(|n| real.contains(n))
                    .count();
                let out_deg = self
                    .synapse
                    .outgoing(id, kind)
                    .iter()
                    .filter(|n| real.contains(n))
                    .count();
                self.path_of(id).map(|p| (p.to_owned(), in_deg, out_deg))
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
        // RFC-0118 Part A.2: real-symbol induced subgraph — phantoms excluded as
        // nodes; the existing sym_set restriction excludes phantom edges.
        let sym_ids: Vec<NodeId> = self.symbol_universe();
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
        // RFC-0118 Part A.2: operate on the real-symbol induced subgraph — count
        // only *real* incoming references, and never report a phantom node.
        let real: HashSet<NodeId> = self.symbol_universe().into_iter().collect();
        let mut entries: Vec<(String, String)> = real
            .iter()
            .filter_map(|&id| {
                let real_callers: Vec<NodeId> = self
                    .synapse
                    .incoming(id, kind)
                    .iter()
                    .copied()
                    .filter(|n| real.contains(n))
                    .collect();
                if real_callers.len() != 1 {
                    return None;
                }
                let ref_path = self.path_of(real_callers[0])?.to_owned();
                Some((self.path_of(id)?.to_owned(), ref_path))
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
        // RFC-0118 Part A.2: real-symbol induced subgraph — phantoms excluded as
        // nodes; the existing sym_set restriction excludes phantom edges.
        let sym_ids: Vec<NodeId> = self.symbol_universe();

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

    /// Return symbol paths (paths containing `>`) that have zero incoming edges
    /// of the given `kind`, sorted lexicographically.
    /// File-level nodes (no `>`) are excluded. Pass `prefix` to restrict results.
    #[must_use]
    pub fn dead_symbols_for_kind(&self, kind: EdgeKind, prefix: Option<&str>) -> Vec<String> {
        let mut result: Vec<String> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter(|p| prefix.is_none_or(|pfx| p.starts_with(pfx)))
            .filter(|p| {
                self.trunk
                    .lookup_path(p)
                    .is_some_and(|id| self.synapse.incoming(id, kind).is_empty())
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
            // RFC-0118 Part A: drop unresolved-callee phantoms (NodeKind::Unresolved).
            .filter(|p| {
                self.trunk
                    .lookup_path(p)
                    .is_none_or(|id| self.is_real_symbol(id))
            })
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
        // RFC-0118 Part A.2: real-symbol induced subgraph — phantoms excluded as
        // nodes; the existing sym_set undirected-adjacency guard excludes phantom
        // edges.
        let sym_ids: Vec<NodeId> = self.symbol_universe();

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
        // RFC-0118 Part A.2: real-symbol induced subgraph — phantoms excluded as
        // nodes; the existing sym_set undirected-adjacency guard excludes phantom
        // edges.
        let sym_ids: Vec<NodeId> = self.symbol_universe();
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

    /// Biconnected components of the undirected symbol graph for `kind`.
    ///
    /// A biconnected component (BCC) is a maximal subgraph with no
    /// articulation point: removing any single vertex leaves it connected.
    /// Uses Tarjan's iterative BCC detection via an edge stack, O(V+E).
    /// Edges treated as undirected; file nodes excluded; singletons excluded.
    ///
    /// Returns groups of symbol node paths, sorted ascending within each group.
    /// Groups sorted by size descending, ties broken by first element ascending.
    #[must_use]
    pub fn biconnected_components(&self, kind: EdgeKind) -> Vec<Vec<String>> {
        // RFC-0118 Part A.2: real-symbol induced subgraph — phantoms excluded as
        // nodes; the existing sym_set undirected-adjacency guard excludes phantom
        // edges.
        let sym_ids: Vec<NodeId> = self.symbol_universe();
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
        for list in &mut adj {
            list.sort_unstable();
            list.dedup();
        }

        let mut disc = vec![AP_UNVISITED; n];
        let mut low = vec![0usize; n];
        let mut parent = vec![AP_UNVISITED; n];
        let mut timer = 0usize;
        // Edge stack: (u, v) with u < v (canonical)
        let mut edge_stack: Vec<(usize, usize)> = Vec::new();
        let mut raw_comps: Vec<Vec<usize>> = Vec::new();

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
                        let e = if u < v { (u, v) } else { (v, u) };
                        edge_stack.push(e);
                        stack.push((v, 0));
                    } else if v != parent[u] && disc[v] < disc[u] {
                        // Back edge: only push once (when disc[v] < disc[u])
                        low[u] = low[u].min(disc[v]);
                        let e = if u < v { (u, v) } else { (v, u) };
                        edge_stack.push(e);
                    }
                } else {
                    stack.pop();
                    if let Some(&(pu, _)) = stack.last() {
                        low[pu] = low[pu].min(low[u]);
                        // BCC root condition: low[u] >= disc[pu]
                        #[allow(clippy::suspicious_operation_groupings)]
                        if low[u] >= disc[pu] {
                            bcc_pop_component(pu, u, &mut edge_stack, &mut raw_comps);
                        }
                    }
                }
            }
        }

        let mut result: Vec<Vec<String>> = raw_comps
            .into_iter()
            .map(|mut idx_group| {
                idx_group.sort_unstable();
                idx_group.dedup();
                let mut paths: Vec<String> = idx_group
                    .into_iter()
                    .filter_map(|i| self.path_of(sym_ids[i]).map(str::to_owned))
                    .collect();
                paths.sort_unstable();
                paths
            })
            .filter(|g| g.len() >= 2)
            .collect();

        result.sort_unstable_by(|a, b| b.len().cmp(&a.len()).then_with(|| a[0].cmp(&b[0])));
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
        // RFC-0118 Part A.2: real-symbol induced subgraph — phantoms excluded as
        // nodes; the existing sym_set successor guard excludes phantom edges.
        let sym_ids: Vec<NodeId> = self.symbol_universe();

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
        // RFC-0118 Part A.2: real-symbol induced subgraph — exclude phantoms from
        // the node set; the existing sym_set union-find guard excludes phantom
        // edges.
        let sym_nodes: Vec<(NodeId, String)> = self
            .symbol_nodes()
            .filter(|(id, _)| self.is_real_symbol(*id))
            .map(|(id, p)| (id, p.to_owned()))
            .collect();

        let n = sym_nodes.len();
        if n == 0 {
            return Vec::new();
        }

        let id_to_idx: HashMap<NodeId, usize> = sym_nodes
            .iter()
            .enumerate()
            .map(|(i, (id, _))| (*id, i))
            .collect();

        // Path-compressed Union-Find.
        let mut parent: Vec<usize> = (0..n).collect();

        let sym_set: HashSet<NodeId> = sym_nodes.iter().map(|(id, _)| *id).collect();
        for (idx, (id, _)) in sym_nodes.iter().enumerate() {
            for &nb in self.synapse.outgoing(*id, kind) {
                if sym_set.contains(&nb) {
                    let nb_idx = id_to_idx[&nb];
                    uf_union(&mut parent, idx, nb_idx);
                }
            }
        }

        // Flatten path compression and group by root.
        let mut groups: HashMap<usize, Vec<String>> = HashMap::new();
        for (idx, (_, p)) in sym_nodes.iter().enumerate() {
            let root = uf_find(&mut parent, idx);
            groups.entry(root).or_default().push(p.clone());
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
        // RFC-0118 Part A.2: real-symbol induced subgraph — phantoms excluded as
        // nodes; the existing sym_set adjacency guard excludes phantom edges.
        let sym_ids: Vec<NodeId> = self.symbol_universe();

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
        // RFC-0118 Part A.2: operate on the real-symbol induced subgraph — a node
        // is isolated iff it has no edge (any kind, either direction) to another
        // *real* symbol. Edges to/from Unresolved phantoms do not count, and
        // phantoms are never themselves reported.
        let real: HashSet<NodeId> = self.symbol_universe().into_iter().collect();
        let has_real_neighbor = |id: NodeId| -> bool {
            [
                EdgeKind::Calls,
                EdgeKind::Imports,
                EdgeKind::Extends,
                EdgeKind::Implements,
            ]
            .iter()
            .any(|&k| {
                self.synapse
                    .incoming(id, k)
                    .iter()
                    .any(|n| real.contains(n))
                    || self
                        .synapse
                        .outgoing(id, k)
                        .iter()
                        .any(|n| real.contains(n))
            })
        };
        let mut result: Vec<String> = real
            .iter()
            .filter(|&&id| !has_real_neighbor(id))
            .filter_map(|&id| self.path_of(id).map(str::to_owned))
            .filter(|p| prefix.is_none_or(|pfx| p.starts_with(pfx)))
            .collect();
        result.sort_unstable();
        result
    }

    /// Compute a graph-native A–F health grade for the indexed project (RFC-0114).
    ///
    /// Fills [`crate::health::HealthMetrics`] from the in-memory graph (no I/O,
    /// no subprocess) and delegates to the pure scorer. Empty stores fail closed
    /// (grade F, score 0). The CLI+MCP surface (`project-health` /
    /// `mycelium_project_health`) calls this method.
    #[must_use]
    pub fn health(&self) -> crate::health::HealthReport {
        let metrics = crate::health::HealthMetrics {
            total_symbols: self.symbol_nodes().count(),
            dead_count: self.dead_symbols(None).len(),
            isolated_count: self.isolated_symbols(None).len(),
            edge_count: self.edge_count(),
        };
        crate::health::score(&metrics)
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
        // RFC-0118 Part A.2: operate on the real-symbol induced subgraph — total
        // degree counts only edges to/from *real* symbols, phantoms excluded.
        let real: HashSet<NodeId> = self.symbol_universe().into_iter().collect();
        let mut entries: Vec<(String, usize)> = real
            .iter()
            .filter_map(|&id| {
                let degree = self
                    .synapse
                    .incoming(id, kind)
                    .iter()
                    .filter(|n| real.contains(n))
                    .count()
                    + self
                        .synapse
                        .outgoing(id, kind)
                        .iter()
                        .filter(|n| real.contains(n))
                        .count();
                self.path_of(id).map(|p| (p.to_owned(), degree))
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
        // RFC-0118 Part A.2: operate on the real-symbol induced subgraph — a node
        // is a leaf iff it has no outgoing edge to a *real* symbol.
        let real: HashSet<NodeId> = self.symbol_universe().into_iter().collect();
        let mut result: Vec<String> = real
            .iter()
            .filter(|&&id| {
                !self
                    .synapse
                    .outgoing(id, kind)
                    .iter()
                    .any(|n| real.contains(n))
            })
            .filter_map(|&id| self.path_of(id).map(str::to_owned))
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

    /// Degree histogram for the symbol graph for `kind`.
    ///
    /// Returns frequency distributions of in- and out-degree across all
    /// symbol nodes (file nodes excluded).  Degree 0 is included.
    /// Each distribution is sorted ascending by degree.
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
    /// let h = store.degree_histogram(EdgeKind::Calls);
    /// assert_eq!(h.in_degrees.len(), 2); // degree 0 and degree 1
    /// ```
    #[must_use]
    pub fn degree_histogram(&self, kind: EdgeKind) -> DegreeHistogram {
        let mut in_counts: HashMap<u64, u64> = HashMap::new();
        let mut out_counts: HashMap<u64, u64> = HashMap::new();
        for (id, _) in self.symbol_nodes() {
            let in_deg = self.synapse.incoming(id, kind).len() as u64;
            let out_deg = self.synapse.outgoing(id, kind).len() as u64;
            *in_counts.entry(in_deg).or_insert(0) += 1;
            *out_counts.entry(out_deg).or_insert(0) += 1;
        }
        let mut in_degrees: Vec<(u64, u64)> = in_counts.into_iter().collect();
        let mut out_degrees: Vec<(u64, u64)> = out_counts.into_iter().collect();
        in_degrees.sort_unstable_by_key(|&(d, _)| d);
        out_degrees.sort_unstable_by_key(|&(d, _)| d);
        DegreeHistogram {
            in_degrees,
            out_degrees,
        }
    }

    /// Structural summary metrics for the symbol graph for `kind`.
    ///
    /// Returns [`EdgeKindMetrics`] with density, average degree, and max
    /// in/out degree.  File nodes excluded.  O(V + E).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn graph_metrics(&self, kind: EdgeKind) -> EdgeKindMetrics {
        let mut symbol_count = 0usize;
        let mut directed_edge_count = 0usize;
        let mut max_in = 0usize;
        let mut max_out = 0usize;

        for (id, _) in self.symbol_nodes() {
            symbol_count += 1;
            let out = self.synapse.outgoing(id, kind).len();
            let inc = self.synapse.incoming(id, kind).len();
            directed_edge_count += out;
            if out > max_out {
                max_out = out;
            }
            if inc > max_in {
                max_in = inc;
            }
        }

        let sc = symbol_count as u64;
        let ec = directed_edge_count as u64;
        let density = if sc < 2 {
            0.0
        } else {
            ec as f64 / (sc as f64 * (sc as f64 - 1.0))
        };
        let avg_degree = if sc == 0 { 0.0 } else { ec as f64 / sc as f64 };

        EdgeKindMetrics {
            symbol_count,
            directed_edge_count,
            density,
            avg_degree,
            max_in_degree: max_in,
            max_out_degree: max_out,
        }
    }

    /// Jaccard similarity between the combined neighbor sets of `id1` and `id2`
    /// for `kind`.
    ///
    /// `N(x)` = union of outgoing and incoming neighbors of `x`.
    /// Returns `|N(id1) ∩ N(id2)| / |N(id1) ∪ N(id2)|` ∈ [0.0, 1.0].
    ///
    /// Two nodes with no neighbors both return 0.0 (undefined similarity).
    #[must_use]
    /// Returns `(similarity, shared_count, total_union_count)`.
    ///
    /// Shared = |N(id1) ∩ N(id2)|, total = |N(id1) ∪ N(id2)|,
    /// similarity = shared / total (Jaccard).  Both isolated → (0.0, 0, 0).
    pub fn neighbor_similarity_stats(
        &self,
        id1: NodeId,
        id2: NodeId,
        kind: EdgeKind,
    ) -> (f64, usize, usize) {
        let neighbors = |id: NodeId| -> HashSet<NodeId> {
            let mut set: HashSet<NodeId> =
                self.synapse.outgoing(id, kind).iter().copied().collect();
            set.extend(self.synapse.incoming(id, kind).iter().copied());
            set.remove(&id);
            set
        };
        let n1 = neighbors(id1);
        let n2 = neighbors(id2);
        if n1.is_empty() && n2.is_empty() {
            return (0.0, 0, 0);
        }
        let shared = n1.intersection(&n2).count();
        let total = n1.union(&n2).count();
        // Code graphs never approach 2^52 nodes; precision loss is acceptable.
        #[allow(clippy::cast_precision_loss)]
        let similarity = if total == 0 {
            0.0
        } else {
            shared as f64 / total as f64
        };
        (similarity, shared, total)
    }

    /// Jaccard similarity ∈ [0.0, 1.0] between the neighbor sets of two nodes.
    ///
    /// Both isolated nodes → 0.0 (undefined, not 1.0).
    #[must_use]
    pub fn neighbor_similarity(&self, id1: NodeId, id2: NodeId, kind: EdgeKind) -> f64 {
        self.neighbor_similarity_stats(id1, id2, kind).0
    }

    /// Returns `(coefficient, neighbor_count, neighbor_edge_count)`.
    ///
    /// `coefficient` = directed edges among N(id) / (|N(id)| * (|N(id)|-1)).
    /// N(id) = outgoing ∪ incoming, self and file nodes excluded.
    /// `|N(id)| < 2` → (0.0, |N|, 0).
    #[must_use]
    pub fn clustering_coefficient_stats(&self, id: NodeId, kind: EdgeKind) -> (f64, usize, usize) {
        let is_file =
            |nid: NodeId| -> bool { self.trunk.path_of(nid).is_some_and(|p| !p.contains('>')) };
        let neighbors: HashSet<NodeId> = {
            let mut set: HashSet<NodeId> =
                self.synapse.outgoing(id, kind).iter().copied().collect();
            set.extend(self.synapse.incoming(id, kind).iter().copied());
            set.remove(&id);
            set.retain(|&n| !is_file(n));
            set
        };
        let k = neighbors.len();
        if k < 2 {
            return (0.0, k, 0);
        }
        let mut edge_count: usize = 0;
        for &src in &neighbors {
            for neighbor_of_src in self.synapse.outgoing(src, kind) {
                if neighbors.contains(neighbor_of_src) {
                    edge_count += 1;
                }
            }
        }
        // k*(k-1) is the number of ordered directed pairs; never exceeds 2^52 in practice.
        #[allow(clippy::cast_precision_loss)]
        let coeff = edge_count as f64 / (k * (k - 1)) as f64;
        (coeff, k, edge_count)
    }

    /// Local clustering coefficient ∈ [0.0, 1.0] for a symbol node.
    ///
    /// Measures what fraction of pairs among the node's neighbors are
    /// themselves connected (directed).  Returns 0.0 for fewer than 2
    /// neighbors.
    #[must_use]
    pub fn clustering_coefficient(&self, id: NodeId, kind: EdgeKind) -> f64 {
        self.clustering_coefficient_stats(id, kind).0
    }

    /// Returns `(eccentricity, reachable_count)` for a symbol node.
    ///
    /// `eccentricity` = maximum BFS distance from `id` to any reachable
    /// symbol node (file nodes excluded); 0 when no symbol is reachable.
    /// `reachable_count` = number of distinct symbol nodes reached.
    #[must_use]
    pub fn eccentricity_stats(&self, id: NodeId, kind: EdgeKind) -> (usize, usize) {
        let is_symbol =
            |nid: NodeId| -> bool { self.trunk.path_of(nid).is_some_and(|p| p.contains('>')) };
        let mut visited: HashMap<NodeId, usize> = HashMap::new();
        let mut queue: VecDeque<(NodeId, usize)> = VecDeque::new();
        queue.push_back((id, 0));
        visited.insert(id, 0);
        let mut max_dist: usize = 0;
        let mut reachable_count: usize = 0;
        while let Some((cur, dist)) = queue.pop_front() {
            for &next in self.synapse.outgoing(cur, kind) {
                if visited.contains_key(&next) {
                    continue;
                }
                visited.insert(next, dist + 1);
                if is_symbol(next) {
                    let new_dist = dist + 1;
                    if new_dist > max_dist {
                        max_dist = new_dist;
                    }
                    reachable_count += 1;
                    queue.push_back((next, new_dist));
                }
                // file nodes: mark visited but don't traverse further
            }
        }
        (max_dist, reachable_count)
    }

    /// Maximum BFS distance from `id` to any reachable symbol node.
    ///
    /// File nodes excluded from traversal.  Returns 0 for isolated nodes.
    #[must_use]
    pub fn eccentricity(&self, id: NodeId, kind: EdgeKind) -> usize {
        self.eccentricity_stats(id, kind).0
    }

    /// Returns `(harmonic_centrality, reachable_count, symbol_count)`.
    ///
    /// `harmonic_centrality` = (1/(n-1)) × Σ_{v reachable} (1/d(id,v)),
    /// where n = total symbol count.  Returns (0.0, 0, n) for isolated nodes.
    /// File nodes excluded from BFS and from n.
    #[must_use]
    pub fn harmonic_centrality_stats(&self, id: NodeId, kind: EdgeKind) -> (f64, usize, usize) {
        let is_symbol =
            |nid: NodeId| -> bool { self.trunk.path_of(nid).is_some_and(|p| p.contains('>')) };
        // Count total symbols (file nodes excluded).
        let symbol_count = self.trunk.all_paths().filter(|p| p.contains('>')).count();
        if symbol_count < 2 {
            return (0.0, 0, symbol_count);
        }
        // BFS to collect distances to all reachable symbol nodes.
        let mut visited: HashMap<NodeId, usize> = HashMap::new();
        let mut queue: VecDeque<(NodeId, usize)> = VecDeque::new();
        queue.push_back((id, 0));
        visited.insert(id, 0);
        let mut harmonic_sum = 0.0_f64;
        let mut reachable_count: usize = 0;
        while let Some((cur, dist)) = queue.pop_front() {
            for &next in self.synapse.outgoing(cur, kind) {
                if visited.contains_key(&next) {
                    continue;
                }
                visited.insert(next, dist + 1);
                if is_symbol(next) {
                    let new_dist = dist + 1;
                    // Code graphs never exceed 2^52 nodes.
                    #[allow(clippy::cast_precision_loss)]
                    {
                        harmonic_sum += 1.0 / new_dist as f64;
                    }
                    reachable_count += 1;
                    queue.push_back((next, new_dist));
                }
            }
        }
        if reachable_count == 0 {
            return (0.0, 0, symbol_count);
        }
        // Code graphs never exceed 2^52 nodes.
        #[allow(clippy::cast_precision_loss)]
        let centrality = harmonic_sum / (symbol_count - 1) as f64;
        (centrality, reachable_count, symbol_count)
    }

    /// Harmonic centrality ∈ [0.0, 1.0] for a symbol node.
    ///
    /// = (1/(n-1)) × Σ_{v reachable} (1/d(id,v)).  Unreachable nodes
    /// contribute 0.  Returns 0.0 for isolated nodes.  File nodes excluded.
    #[must_use]
    pub fn harmonic_centrality(&self, id: NodeId, kind: EdgeKind) -> f64 {
        self.harmonic_centrality_stats(id, kind).0
    }

    /// BFS hop count from `from` to `to` following `kind` edges, excluding file
    /// nodes.  Returns `None` if unreachable or `from == to` (caller handles
    /// the same-node special case).
    fn bfs_distance(&self, from: NodeId, to: NodeId, kind: EdgeKind) -> Option<usize> {
        let is_file =
            |nid: NodeId| -> bool { self.trunk.path_of(nid).is_some_and(|p| !p.contains('>')) };
        let mut visited: HashSet<NodeId> = HashSet::new();
        visited.insert(from);
        let mut queue: VecDeque<(NodeId, usize)> = VecDeque::new();
        queue.push_back((from, 0));
        while let Some((node, dist)) = queue.pop_front() {
            for &neighbor in self.synapse.outgoing(node, kind) {
                if neighbor == to {
                    return Some(dist + 1);
                }
                if !is_file(neighbor) && visited.insert(neighbor) {
                    queue.push_back((neighbor, dist + 1));
                }
            }
        }
        None
    }

    /// Bidirectional reachability between two symbol nodes for a given
    /// [`EdgeKind`].
    ///
    /// Returns a [`MutualReachability`] containing forward/backward BFS
    /// distances and derived flags.  `id1 == id2` short-circuits with both
    /// directions true and distances `Some(0)`.  File nodes are excluded from
    /// traversal.
    #[must_use]
    pub fn mutual_reachability(
        &self,
        id1: NodeId,
        id2: NodeId,
        kind: EdgeKind,
    ) -> MutualReachability {
        if id1 == id2 {
            return MutualReachability {
                forward: true,
                backward: true,
                mutual: true,
                forward_distance: Some(0),
                backward_distance: Some(0),
            };
        }
        let forward_distance = self.bfs_distance(id1, id2, kind);
        let backward_distance = self.bfs_distance(id2, id1, kind);
        let forward = forward_distance.is_some();
        let backward = backward_distance.is_some();
        MutualReachability {
            forward,
            backward,
            mutual: forward && backward,
            forward_distance,
            backward_distance,
        }
    }

    /// Returns all symbol paths transitively reachable from `id` via `kind`
    /// edges, sorted alphabetically.  `id` itself is not included.
    /// File nodes are excluded from traversal and results.  O(V + E).
    #[must_use]
    pub fn reachable_set(&self, id: NodeId, kind: EdgeKind) -> Vec<String> {
        let is_file =
            |nid: NodeId| -> bool { self.trunk.path_of(nid).is_some_and(|p| !p.contains('>')) };
        let mut visited: HashSet<NodeId> = HashSet::new();
        visited.insert(id);
        let mut queue: VecDeque<NodeId> = VecDeque::new();
        queue.push_back(id);
        while let Some(node) = queue.pop_front() {
            for &neighbor in self.synapse.outgoing(node, kind) {
                if !is_file(neighbor) && visited.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }
        visited.remove(&id);
        let mut paths: Vec<String> = visited
            .into_iter()
            .filter_map(|nid| self.trunk.path_of(nid).map(str::to_owned))
            .collect();
        paths.sort();
        paths
    }

    /// Returns all symbol paths that can transitively reach `id` via `kind`
    /// edges (reverse BFS transitive closure), sorted alphabetically.
    /// `id` itself is not included.  File nodes excluded.  O(V + E).
    ///
    /// Answers "what symbols transitively call/import/extend this one?".
    #[must_use]
    pub fn reaches_into(&self, id: NodeId, kind: EdgeKind) -> Vec<String> {
        let is_file =
            |nid: NodeId| -> bool { self.trunk.path_of(nid).is_some_and(|p| !p.contains('>')) };
        let mut visited: HashSet<NodeId> = HashSet::new();
        visited.insert(id);
        let mut queue: VecDeque<NodeId> = VecDeque::new();
        queue.push_back(id);
        while let Some(node) = queue.pop_front() {
            for &predecessor in self.synapse.incoming(node, kind) {
                if !is_file(predecessor) && visited.insert(predecessor) {
                    queue.push_back(predecessor);
                }
            }
        }
        visited.remove(&id);
        let mut paths: Vec<String> = visited
            .into_iter()
            .filter_map(|nid| self.trunk.path_of(nid).map(str::to_owned))
            .collect();
        paths.sort();
        paths
    }

    /// Compute `PageRank` scores for all symbol nodes using the iterative
    /// power method.
    ///
    /// `damping` is clamped to `[0.0, 1.0]`.  `iterations == 0` returns
    /// uniform scores.  File nodes are excluded.  Returns entries sorted
    /// descending by score.
    #[must_use]
    pub fn page_rank(&self, kind: EdgeKind, damping: f64, iterations: usize) -> Vec<PageRankEntry> {
        let damping = damping.clamp(0.0, 1.0);

        // Collect all symbol NodeIds in a stable order (no trie navigation).
        // RFC-0118 Part A: exclude unresolved-callee phantoms from the rank universe.
        let symbols: Vec<NodeId> = self
            .symbol_nodes()
            .filter(|(id, _)| self.is_real_symbol(*id))
            .map(|(id, _)| id)
            .collect();
        let n = symbols.len();
        if n == 0 {
            return Vec::new();
        }

        // Map NodeId → index for O(1) lookup.
        let idx: HashMap<NodeId, usize> =
            symbols.iter().enumerate().map(|(i, &id)| (id, i)).collect();

        // Build out-edges restricted to symbol nodes.
        let out_edges: Vec<Vec<usize>> = symbols
            .iter()
            .map(|&src| {
                self.synapse
                    .outgoing(src, kind)
                    .iter()
                    .filter_map(|&dst| idx.get(&dst).copied())
                    .collect()
            })
            .collect();

        #[allow(clippy::cast_precision_loss)]
        let n_f = n as f64;
        let teleport = (1.0 - damping) / n_f;

        let mut scores = vec![1.0 / n_f; n];

        for _ in 0..iterations {
            // Dangling mass: sum of scores of nodes with out-degree 0.
            let dangling_mass: f64 = scores
                .iter()
                .zip(out_edges.iter())
                .filter(|(_, oe)| oe.is_empty())
                .map(|(s, _)| s)
                .sum::<f64>()
                / n_f;

            let mut new_scores = vec![teleport + damping * dangling_mass; n];
            for (src_idx, out) in out_edges.iter().enumerate() {
                if out.is_empty() {
                    continue;
                }
                #[allow(clippy::cast_precision_loss)]
                let share = damping * scores[src_idx] / out.len() as f64;
                for &dst_idx in out {
                    new_scores[dst_idx] += share;
                }
            }
            scores = new_scores;
        }

        let mut entries: Vec<PageRankEntry> = symbols
            .into_iter()
            .zip(scores)
            .filter_map(|(nid, score)| {
                self.trunk.path_of(nid).map(|p| PageRankEntry {
                    path: p.to_owned(),
                    score,
                })
            })
            .collect();
        entries.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        entries
    }

    /// Returns the sorted intersection of the transitive reachable sets of
    /// `id1` and `id2` via `kind` edges.
    ///
    /// Answers "what symbols do both nodes transitively depend on?".
    /// `id1` and `id2` are excluded from the result.
    /// `id1 == id2` returns the same result as [`Self::reachable_set`].
    /// File nodes excluded.  O(V + E).
    #[must_use]
    pub fn common_reachable(&self, id1: NodeId, id2: NodeId, kind: EdgeKind) -> Vec<String> {
        let set1: HashSet<String> = self.reachable_set(id1, kind).into_iter().collect();
        if set1.is_empty() {
            return Vec::new();
        }
        let set2: HashSet<String> = self.reachable_set(id2, kind).into_iter().collect();
        let mut common: Vec<String> = set1.intersection(&set2).cloned().collect();
        common.sort();
        common
    }

    /// Returns all symbol paths reachable from `id` in **exactly** `k` BFS
    /// hops via `kind` edges, sorted alphabetically.
    ///
    /// `k == 0` → empty.  Nodes first reached at depth < k are excluded.
    /// `id` itself is excluded even if a cycle brings it back at hop k.
    /// File nodes excluded.  O(V + E).
    #[must_use]
    pub fn k_hop_neighbors(&self, id: NodeId, kind: EdgeKind, k: usize) -> Vec<String> {
        if k == 0 {
            return Vec::new();
        }
        let is_file =
            |nid: NodeId| -> bool { self.trunk.path_of(nid).is_some_and(|p| !p.contains('>')) };
        // BFS tracking the current frontier depth.
        let mut visited: HashSet<NodeId> = HashSet::new();
        visited.insert(id);
        let mut frontier: Vec<NodeId> = vec![id];
        for depth in 1..=k {
            let mut next_frontier: Vec<NodeId> = Vec::new();
            for node in &frontier {
                for &neighbor in self.synapse.outgoing(*node, kind) {
                    if !is_file(neighbor) && visited.insert(neighbor) {
                        next_frontier.push(neighbor);
                    }
                }
            }
            if depth == k {
                // Return paths at exactly this frontier, excluding source.
                let mut paths: Vec<String> = next_frontier
                    .into_iter()
                    .filter(|&nid| nid != id)
                    .filter_map(|nid| self.trunk.path_of(nid).map(str::to_owned))
                    .collect();
                paths.sort();
                paths.dedup();
                return paths;
            }
            frontier = next_frontier;
            if frontier.is_empty() {
                return Vec::new();
            }
        }
        Vec::new()
    }

    /// Compute normalized betweenness centrality for all symbol nodes using
    /// Brandes' O(V×(V+E)) algorithm.
    ///
    /// BC(v) = Σ_{s≠t≠v} σ(s,t|v)/σ(s,t), normalized by (n-1)×(n-2).
    /// `n < 2` → empty.  `n == 2` → all scores 0.0.
    /// File nodes excluded.  Returns entries sorted descending by score.
    #[must_use]
    pub fn betweenness_centrality(&self, kind: EdgeKind) -> Vec<BetweennessEntry> {
        // Collect symbol nodes in stable order.
        let symbols: Vec<NodeId> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter_map(|p| self.trunk.lookup_path(p))
            .collect();
        let n = symbols.len();
        if n < 2 {
            return Vec::new();
        }

        // Index map NodeId → usize for O(1) lookup.
        let idx: HashMap<NodeId, usize> =
            symbols.iter().enumerate().map(|(i, &id)| (id, i)).collect();

        let mut bc = vec![0.0_f64; n];

        // Brandes: one BFS per source.
        for &src in &symbols {
            let src_i = idx[&src];

            // BFS state.
            let mut sigma = vec![0.0_f64; n]; // #shortest paths from src
            let mut dist = vec![-1_i64; n]; // BFS distance (-1 = unvisited)
            let mut pred: Vec<Vec<usize>> = vec![Vec::new(); n]; // predecessors

            sigma[src_i] = 1.0;
            dist[src_i] = 0;

            let mut queue: VecDeque<usize> = VecDeque::new();
            queue.push_back(src_i);
            let mut bfs_order: Vec<usize> = Vec::new();

            while let Some(vi) = queue.pop_front() {
                bfs_order.push(vi);
                let v = symbols[vi];
                for &w in self.synapse.outgoing(v, kind) {
                    // Skip file nodes.
                    if self.trunk.path_of(w).is_none_or(|p| !p.contains('>')) {
                        continue;
                    }
                    let Some(&wi) = idx.get(&w) else { continue };
                    if dist[wi] < 0 {
                        dist[wi] = dist[vi] + 1;
                        queue.push_back(wi);
                    }
                    if dist[wi] == dist[vi] + 1 {
                        sigma[wi] += sigma[vi];
                        pred[wi].push(vi);
                    }
                }
            }

            // Backward accumulation.
            let mut delta = vec![0.0_f64; n];
            for &wi in bfs_order.iter().rev() {
                for &pi in &pred[wi] {
                    if sigma[wi] > 0.0 {
                        delta[pi] += (sigma[pi] / sigma[wi]) * (1.0 + delta[wi]);
                    }
                }
                if wi != src_i {
                    bc[wi] += delta[wi];
                }
            }
        }

        // Normalize by (n-1)*(n-2) for directed graph.
        #[allow(clippy::cast_precision_loss)]
        let norm = ((n - 1) * (n - 2)) as f64;
        let mut entries: Vec<BetweennessEntry> = symbols
            .into_iter()
            .zip(bc)
            .filter_map(|(nid, raw)| {
                self.trunk.path_of(nid).map(|p| BetweennessEntry {
                    path: p.to_owned(),
                    score: if norm > 0.0 { raw / norm } else { 0.0 },
                })
            })
            .collect();
        entries.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        entries
    }

    /// Compute strongly connected components (SCCs) over symbol nodes via Tarjan's algorithm.
    ///
    /// Returns one [`SccEntry`] per component.  Members within each component are sorted
    /// alphabetically.  Results are sorted descending by size, then alphabetically by first
    /// member.  File nodes are excluded.  O(V + E).
    ///
    /// # Panics
    ///
    /// Does not panic under normal operation; the internal `unwrap` on `scc_stack.pop()` is
    /// protected by Tarjan's invariant that the SCC stack is non-empty when popping a root.
    #[must_use]
    pub fn strongly_connected_components(&self, kind: EdgeKind) -> Vec<SccEntry> {
        // RFC-0118 Part A.2: real-symbol induced subgraph — phantoms excluded as
        // nodes; the existing idx-restricted adjacency excludes phantom edges.
        let symbols: Vec<NodeId> = self.symbol_universe();
        let n = symbols.len();
        if n == 0 {
            return Vec::new();
        }
        let idx: HashMap<NodeId, usize> =
            symbols.iter().enumerate().map(|(i, &id)| (id, i)).collect();

        // Precompute adjacency lists (symbol-to-symbol edges only) for O(V+E) total.
        let adj: Vec<Vec<usize>> = symbols
            .iter()
            .map(|&nid| {
                self.synapse
                    .outgoing(nid, kind)
                    .iter()
                    .filter(|&&w| self.trunk.path_of(w).is_some_and(|p| p.contains('>')))
                    .filter_map(|&w| idx.get(&w).copied())
                    .collect()
            })
            .collect();

        // Iterative Tarjan's SCC.
        // call_stack frame = (node_index, next_neighbor_index).
        // Nodes are initialized (index/lowlink set) when first pushed.
        let mut index_counter = 0usize;
        let mut scc_stack: Vec<usize> = Vec::new();
        let mut on_stack = vec![false; n];
        let mut index = vec![usize::MAX; n]; // MAX = unvisited
        let mut lowlink = vec![0usize; n];
        let mut sccs: Vec<Vec<usize>> = Vec::new();

        for start in 0..n {
            if index[start] != usize::MAX {
                continue;
            }
            index[start] = index_counter;
            lowlink[start] = index_counter;
            index_counter += 1;
            scc_stack.push(start);
            on_stack[start] = true;
            let mut call_stack: Vec<(usize, usize)> = vec![(start, 0)];

            while !call_stack.is_empty() {
                // Copy the top frame (both fields are usize — Copy).
                let (vi, ci) = *call_stack.last().unwrap();
                if ci < adj[vi].len() {
                    call_stack.last_mut().unwrap().1 += 1;
                    let wi = adj[vi][ci];
                    if index[wi] == usize::MAX {
                        // Tree edge — push child.
                        index[wi] = index_counter;
                        lowlink[wi] = index_counter;
                        index_counter += 1;
                        scc_stack.push(wi);
                        on_stack[wi] = true;
                        call_stack.push((wi, 0));
                    } else if on_stack[wi] {
                        // Back edge — update lowlink.
                        if index[wi] < lowlink[vi] {
                            lowlink[vi] = index[wi];
                        }
                    }
                } else {
                    // All neighbors of vi processed — pop.
                    call_stack.pop();
                    if let Some(&(parent, _)) = call_stack.last() {
                        if lowlink[vi] < lowlink[parent] {
                            lowlink[parent] = lowlink[vi];
                        }
                    }
                    if lowlink[vi] == index[vi] {
                        let mut component: Vec<usize> = Vec::new();
                        loop {
                            let w = scc_stack.pop().unwrap();
                            on_stack[w] = false;
                            component.push(w);
                            if w == vi {
                                break;
                            }
                        }
                        sccs.push(component);
                    }
                }
            }
        }

        let mut entries: Vec<SccEntry> = sccs
            .into_iter()
            .map(|component| {
                let mut members: Vec<String> = component
                    .into_iter()
                    .filter_map(|i| self.trunk.path_of(symbols[i]).map(ToOwned::to_owned))
                    .collect();
                members.sort();
                let size = members.len();
                SccEntry { members, size }
            })
            .collect();

        entries.sort_by(|a, b| {
            b.size
                .cmp(&a.size)
                .then_with(|| a.members.first().cmp(&b.members.first()))
        });
        entries
    }

    /// Compute normalized in-degree and out-degree centrality for all symbol nodes.
    ///
    /// Both centrality scores are normalized by `(n-1)` where `n` is the symbol count.
    /// Results are sorted descending by `in_centrality`, then `out_centrality`, then
    /// alphabetically by path.  File nodes excluded.  O(V + E).
    #[must_use]
    pub fn degree_centrality(&self, kind: EdgeKind) -> Vec<DegreeCentralityEntry> {
        let symbols: Vec<NodeId> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter_map(|p| self.trunk.lookup_path(p))
            .collect();
        let n = symbols.len();
        if n == 0 {
            return Vec::new();
        }
        let idx: HashMap<NodeId, usize> =
            symbols.iter().enumerate().map(|(i, &id)| (id, i)).collect();

        let mut in_deg = vec![0usize; n];
        let mut out_deg = vec![0usize; n];

        for (i, &nid) in symbols.iter().enumerate() {
            for &w in self.synapse.outgoing(nid, kind) {
                if self.trunk.path_of(w).is_some_and(|p| p.contains('>')) {
                    if let Some(&j) = idx.get(&w) {
                        out_deg[i] += 1;
                        in_deg[j] += 1;
                    }
                }
            }
        }

        #[allow(clippy::cast_precision_loss)]
        let norm = if n > 1 { (n - 1) as f64 } else { 1.0 };

        #[allow(clippy::cast_precision_loss)]
        let mut entries: Vec<DegreeCentralityEntry> = symbols
            .iter()
            .enumerate()
            .filter_map(|(i, &nid)| {
                self.trunk.path_of(nid).map(|p| DegreeCentralityEntry {
                    path: p.to_owned(),
                    in_degree: in_deg[i],
                    out_degree: out_deg[i],
                    in_centrality: in_deg[i] as f64 / norm,
                    out_centrality: out_deg[i] as f64 / norm,
                })
            })
            .collect();

        entries.sort_by(|a, b| {
            b.in_centrality
                .partial_cmp(&a.in_centrality)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    b.out_centrality
                        .partial_cmp(&a.out_centrality)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.path.cmp(&b.path))
        });
        entries
    }

    /// Compute the **dependency depth** of symbol node `id` for `kind` edges.
    ///
    /// Dependency depth is the length of the *longest* path from any root
    /// (symbol with no incoming `kind` edges within the symbol-only subgraph)
    /// to `id`, following *incoming* edges.
    ///
    /// A leaf node (no incoming symbol edges) returns 0.
    /// File nodes are excluded and return `None`.
    /// Cycles are handled safely: each node is updated at most once per
    /// improvement step, so the algorithm terminates even with cycles.
    ///
    /// # Example
    ///
    /// ```
    /// use mycelium_core::store::Store;
    /// use mycelium_core::trunk::TrunkPath;
    /// use mycelium_core::types::EdgeKind;
    ///
    /// let mut store = Store::new();
    /// let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
    /// let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
    /// let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
    /// store.upsert_edge(EdgeKind::Calls, a, b);
    /// store.upsert_edge(EdgeKind::Calls, b, c);
    ///
    /// assert_eq!(store.dependency_depth(a, EdgeKind::Calls), Some(0));
    /// assert_eq!(store.dependency_depth(b, EdgeKind::Calls), Some(1));
    /// assert_eq!(store.dependency_depth(c, EdgeKind::Calls), Some(2));
    /// ```
    #[must_use]
    pub fn dependency_depth(&self, id: NodeId, kind: EdgeKind) -> Option<usize> {
        let path = self.trunk.path_of(id)?;
        if !path.contains('>') {
            return None;
        }
        // Algorithm: Kahn's topological sort + longest-path DP.
        // O(V + E) time, O(V) space.  Cycle-safe: Kahn's never processes nodes
        // whose in-degree never reaches zero (cycle members), so the queue is
        // bounded by V and the total work by E.  No OOM risk.
        //
        // Step 1 — Collect the ancestor subgraph: all symbol nodes that can
        // reach `id` by following INCOMING (caller) edges.  A simple BFS with
        // a visited-set terminates even for cyclic graphs.
        let mut ancestors: HashSet<NodeId> = HashSet::new();
        ancestors.insert(id);
        let mut bfs: VecDeque<NodeId> = VecDeque::new();
        bfs.push_back(id);
        while let Some(node) = bfs.pop_front() {
            for &caller in self.synapse.incoming(node, kind) {
                if self.trunk.path_of(caller).is_none_or(|p| !p.contains('>')) {
                    continue;
                }
                if ancestors.insert(caller) {
                    bfs.push_back(caller);
                }
            }
        }

        // Step 2 — Count in-degrees within the subgraph (caller edges only).
        let mut in_deg: HashMap<NodeId, usize> = ancestors.iter().map(|&n| (n, 0usize)).collect();
        for &node in &ancestors {
            for &callee in self.synapse.outgoing(node, kind) {
                if let Some(d) = in_deg.get_mut(&callee) {
                    *d += 1;
                }
            }
        }

        // Step 3 — Kahn's BFS: start from roots (in-deg == 0), propagate dp.
        // dp[node] = longest incoming-path depth within the subgraph.
        let mut dp: HashMap<NodeId, usize> = HashMap::new();
        let mut queue: VecDeque<NodeId> = in_deg
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(&n, _)| n)
            .collect();
        while let Some(node) = queue.pop_front() {
            let d = *dp.get(&node).unwrap_or(&0);
            for &callee in self.synapse.outgoing(node, kind) {
                let Some(deg) = in_deg.get_mut(&callee) else {
                    continue; // callee outside subgraph
                };
                let e = dp.entry(callee).or_insert(0);
                if d + 1 > *e {
                    *e = d + 1;
                }
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(callee);
                }
            }
        }

        // Cycle members are never enqueued (in-deg never reaches 0),
        // so dp[cycle_node] stays 0 — a safe, bounded answer.
        Some(*dp.get(&id).unwrap_or(&0))
    }

    /// Compute Wasserman-Faust normalized closeness centrality for all symbol nodes.
    ///
    /// `CC_WF(v) = (n_reach/(n-1))^2 * (n_reach / Σ d(v,u))`
    /// where `n_reach` is the number of nodes reachable from `v` (excluding `v` itself).
    /// Nodes that reach no others get score 0.0.  File nodes excluded.  O(V × (V + E)).
    ///
    /// # Panics
    ///
    /// Does not panic under normal operation.
    #[must_use]
    pub fn closeness_centrality(&self, kind: EdgeKind) -> Vec<ClosenessCentralityEntry> {
        let symbols: Vec<NodeId> = self
            .trunk
            .all_paths()
            .filter(|p| p.contains('>'))
            .filter_map(|p| self.trunk.lookup_path(p))
            .collect();
        let n = symbols.len();
        if n == 0 {
            return Vec::new();
        }
        let idx: HashMap<NodeId, usize> =
            symbols.iter().enumerate().map(|(i, &id)| (id, i)).collect();

        #[allow(clippy::cast_precision_loss)]
        let n_minus_1 = (n - 1) as f64;

        let mut entries: Vec<ClosenessCentralityEntry> = symbols
            .iter()
            .enumerate()
            .filter_map(|(src_i, &src)| {
                // BFS from src over symbol nodes.
                let mut dist = vec![-1_i64; n];
                dist[src_i] = 0;
                let mut queue = VecDeque::new();
                queue.push_back(src_i);
                let mut sum_dist = 0u64;
                let mut n_reach = 0usize;
                while let Some(vi) = queue.pop_front() {
                    let v = symbols[vi];
                    for &w in self.synapse.outgoing(v, kind) {
                        if self.trunk.path_of(w).is_none_or(|p| !p.contains('>')) {
                            continue;
                        }
                        let Some(&wi) = idx.get(&w) else { continue };
                        if dist[wi] < 0 {
                            dist[wi] = dist[vi] + 1;
                            #[allow(clippy::cast_sign_loss)]
                            {
                                sum_dist += dist[wi] as u64;
                            }
                            n_reach += 1;
                            queue.push_back(wi);
                        }
                    }
                }
                let score = if n_reach == 0 || sum_dist == 0 {
                    0.0
                } else {
                    #[allow(clippy::cast_precision_loss)]
                    let nr = n_reach as f64;
                    #[allow(clippy::cast_precision_loss)]
                    let sd = sum_dist as f64;
                    let frac = nr / n_minus_1;
                    // Wasserman-Faust: (n_reach/(n-1))^2 * (n_reach/sum_dist)
                    frac.powi(2) * (nr / sd)
                };
                self.trunk.path_of(src).map(|p| ClosenessCentralityEntry {
                    path: p.to_owned(),
                    score,
                })
            })
            .collect();

        entries.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        entries
    }
}
