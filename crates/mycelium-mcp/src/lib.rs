//! # mycelium-mcp
//!
//! Model Context Protocol server for Mycelium.
//!
//! Exposes the code intelligence graph over MCP stdio transport (JSON-RPC 2.0).
//! Four tools are provided:
//!
//! | Tool | Description |
//! |------|-------------|
//! | `mycelium_index_workspace` | Index a directory into the in-memory graph |
//! | `mycelium_search_symbol` | Search symbols by name prefix / substring |
//! | `mycelium_get_ancestors` | Return the containment chain for a trunk path |
//! | `mycelium_get_descendants` | Return all symbols nested under a trunk path |
//! | `mycelium_load_index` | Load a previously saved `.mycelium/index.rmp` |
//! | `mycelium_server_status` | Return node count, edge count, root, and ready status |
//! | `mycelium_watch_status` | Return file-watch loop status and batch count |
//! | `mycelium_get_callees` | Return all symbols a given path calls directly |
//! | `mycelium_get_callers` | Return all symbols that call a given path |
//! | `mycelium_get_symbol_info` | Return ancestors, descendants, callers, and callees in one call |
//! | `mycelium_find_call_path` | Find the shortest call chain between two symbols via BFS |
//! | `mycelium_get_files` | List all indexed source files with optional path-prefix filter |
//! | `mycelium_rank_symbols` | Return top-N symbols by caller count (most-called first) |
//! | `mycelium_get_callee_tree` | Return depth-limited transitive callee tree for a symbol |
//! | `mycelium_get_caller_tree` | Return depth-limited transitive caller tree for a symbol |
//! | `mycelium_get_entry_points` | Return all zero-caller symbols (entry points or dead code candidates) |
//! | `mycelium_get_imports` | Return direct import neighbors (`imports` / `imported_by`) for a path |
//! | `mycelium_get_import_tree` | Return depth-limited transitive import dependency tree |
//! | `mycelium_batch_symbol_info` | Get symbol info for multiple paths in one call (max 50) |
//! | `mycelium_get_extends` | Return direct inheritance neighbors (`extends` / `extended_by`) for a path |
//! | `mycelium_get_implements` | Return direct interface-implementation neighbors (`implements` / `implemented_by`) for a path |
//! | `mycelium_get_node_kind` | Return the `NodeKind` for a given path |
//! | `mycelium_get_symbols_by_kind` | Return all symbols of a given `NodeKind`, optionally filtered by path prefix |
//! | `mycelium_find_import_path` | Find the shortest import-dependency chain between two symbols via BFS |
//! | `mycelium_get_source_span` | Return the source location (line/col/byte) for a given path |
//! | `mycelium_find_extends_path` | Find the shortest extends (inheritance) chain between two symbols via BFS |
//! | `mycelium_get_extends_tree` | Return the depth-limited superclass tree for a symbol (outgoing Extends edges) |
//! | `mycelium_get_subclasses_tree` | Return the depth-limited subclass forest for a symbol (incoming Extends edges) |
//! | `mycelium_find_implements_path` | Find the shortest implements chain between two symbols via BFS |
//! | `mycelium_get_implements_tree` | Return the depth-limited interface tree for a symbol (outgoing Implements edges) |
//! | `mycelium_get_implementors_tree` | Return the depth-limited implementor forest for an interface (incoming Implements edges) |
//! | `mycelium_get_importers_tree` | Return the depth-limited reverse-dependency forest for a module (incoming Imports edges) |
//!
//! See RFC-0004, RFC-0005, RFC-0006, RFC-0007, RFC-0008, RFC-0010, RFC-0011, RFC-0012, RFC-0016, RFC-0017, RFC-0018, RFC-0019, RFC-0020, RFC-0021, RFC-0022, RFC-0023, RFC-0024, RFC-0025, RFC-0026, RFC-0027, RFC-0028, RFC-0029, RFC-0030, RFC-0031, RFC-0032, RFC-0033, RFC-0034, RFC-0035, and RFC-0036 for the design.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use anyhow::Context as _;
use mycelium_core::{
    CalleeNode, CallerNode, CrossRefs, ExtendsNode, GraphStats, ImplementorNode, ImplementsNode,
    ImportNode, ImporterNode, NodeDegree, OutgoingRefs, SubclassNode, SymbolNeighborhood,
    TopologicalOrder, extractor::Extractor, store::Store, types::EdgeKind,
};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use rmcp::{
    ServerHandler, ServiceExt, handler::server::wrapper::Parameters, model::Implementation,
    model::ServerCapabilities, model::ServerInfo, tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::warn;

/// Shared state for the background watch loop.
#[derive(Debug, Default)]
struct WatchState {
    watching: AtomicBool,
    batches_processed: AtomicU64,
}

// ── embedded pack queries ─────────────────────────────────────────────────────

const JAVASCRIPT_QUERIES: &str = include_str!("../../../packs/javascript/queries.scm");
const PYTHON_QUERIES: &str = include_str!("../../../packs/python/queries.scm");
const TYPESCRIPT_QUERIES: &str = include_str!("../../../packs/typescript/queries.scm");
const RUST_QUERIES: &str = include_str!("../../../packs/rust/queries.scm");

// ── request schemas ───────────────────────────────────────────────────────────

/// Input parameters for `mycelium_index_workspace`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct IndexWorkspaceRequest {
    /// Absolute or relative path to the workspace root to index.
    pub path: String,
}

/// Input parameters for `mycelium_search_symbol`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchSymbolRequest {
    /// Name prefix or substring to search for (case-insensitive).
    pub query: String,
    /// Maximum number of results to return (default: 20).
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Input parameters for `mycelium_get_ancestors`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetAncestorsRequest {
    /// Trunk path to look up, e.g. `"src/main.rs>greet"`.
    pub path: String,
}

/// Input parameters for `mycelium_get_descendants`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetDescendantsRequest {
    /// Trunk path to look up, e.g. `"src/lib.rs"`.
    pub path: String,
}

/// Input parameters for `mycelium_load_index`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LoadIndexRequest {
    /// Workspace root that contains a `.mycelium/index.rmp` snapshot.
    pub path: String,
}

/// Input parameters for `mycelium_get_callees`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCalleesRequest {
    /// Trunk path to look up callees for, e.g. `"src/lib.rs>process"`.
    pub path: String,
}

/// Input parameters for `mycelium_get_callers`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCallersRequest {
    /// Trunk path to look up callers for, e.g. `"src/lib.rs>helper"`.
    pub path: String,
}

/// Input parameters for `mycelium_get_symbol_info`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSymbolInfoRequest {
    /// Trunk path to query, e.g. `"src/lib.rs>AuthService>login"`.
    pub path: String,
}

/// Input parameters for `mycelium_get_callee_tree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCalleeTreeRequest {
    /// Root symbol path, e.g. `"src/main.rs>main"`.
    pub path: String,
    /// Maximum traversal depth. Defaults to 4, capped at 10.
    pub max_depth: Option<usize>,
}

/// Input parameters for `mycelium_get_caller_tree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCallerTreeRequest {
    /// Root symbol path, e.g. `"src/db.rs>query"`.
    pub path: String,
    /// Maximum traversal depth. Defaults to 4, capped at 10.
    pub max_depth: Option<usize>,
}

/// Input parameters for `mycelium_get_imports`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetImportsRequest {
    /// Trunk path to query, e.g. `"src/auth.rs"`.
    pub path: String,
}

/// Input parameters for `mycelium_get_import_tree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetImportTreeRequest {
    /// Root path, e.g. `"src/auth.rs"`.
    pub path: String,
    /// Maximum traversal depth. Defaults to 4, capped at 10.
    pub max_depth: Option<usize>,
}

/// Input parameters for `mycelium_batch_symbol_info`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BatchSymbolInfoRequest {
    /// List of trunk paths to query (maximum 50).
    pub paths: Vec<String>,
}

/// Input parameters for `mycelium_find_import_path`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindImportPathRequest {
    /// Start of the import chain, e.g. `"src/main.rs"`.
    pub from_path: String,
    /// End of the import chain, e.g. `"src/db.rs"`.
    pub to_path: String,
    /// Maximum traversal depth (hops). Defaults to 8, capped at 20.
    pub max_depth: Option<usize>,
}

/// Input parameters for `mycelium_get_extends_tree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetExtendsTreeRequest {
    /// Root symbol path, e.g. `"src/child.ts>Child"`.
    pub path: String,
    /// Maximum DFS depth. Defaults to 4, capped at 10.
    pub max_depth: Option<usize>,
}

/// Input parameters for `mycelium_get_subclasses_tree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSubclassesTreeRequest {
    /// Root symbol path, e.g. `"src/base.ts>Base"`.
    pub path: String,
    /// Maximum DFS depth. Defaults to 4, capped at 10.
    pub max_depth: Option<usize>,
}

/// Input parameters for `mycelium_find_extends_path`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindExtendsPathRequest {
    /// Start of the extends chain, e.g. `"src/io.ts>ReadStream"`.
    pub from_path: String,
    /// End of the extends chain, e.g. `"src/base.ts>EventEmitter"`.
    pub to_path: String,
    /// Maximum traversal depth (hops). Defaults to 8, capped at 20.
    pub max_depth: Option<usize>,
}

/// Input parameters for `mycelium_get_implements_tree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetImplementsTreeRequest {
    /// Root symbol path, e.g. `"src/cls.ts>Cls"`.
    pub path: String,
    /// Maximum DFS depth. Defaults to 4, capped at 10.
    pub max_depth: Option<usize>,
}

/// Input parameters for `mycelium_get_implementors_tree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetImplementorsTreeRequest {
    /// Root symbol path (interface), e.g. `"src/iface.ts>IFace"`.
    pub path: String,
    /// Maximum DFS depth. Defaults to 4, capped at 10.
    pub max_depth: Option<usize>,
}

/// Input parameters for `mycelium_get_importers_tree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetImportersTreeRequest {
    /// Root symbol path (module), e.g. `"src/utils.ts>utils"`.
    pub path: String,
    /// Maximum DFS depth. Defaults to 4, capped at 10.
    pub max_depth: Option<usize>,
}

/// Input parameters for `mycelium_find_implements_path`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindImplementsPathRequest {
    /// Start symbol path, e.g. `"src/foo.ts>Foo"`.
    pub from_path: String,
    /// End symbol path (interface), e.g. `"src/iface.ts>IFace"`.
    pub to_path: String,
    /// Maximum traversal depth (hops). Defaults to 8, capped at 20.
    pub max_depth: Option<usize>,
}

/// Input parameters for `mycelium_get_node_kind`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetNodeKindRequest {
    /// Trunk path to query, e.g. `"src/auth.rs>login"`.
    pub path: String,
}

/// Input parameters for `mycelium_get_symbols_by_kind`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSymbolsByKindRequest {
    /// `NodeKind` wire string, e.g. `"function"`, `"class"`, `"method"`.
    pub kind: String,
    /// Optional path prefix to restrict results, e.g. `"src/"`.
    pub path_prefix: Option<String>,
}

/// Input parameters for `mycelium_get_source_span`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSourceSpanRequest {
    /// Trunk path to query, e.g. `"src/auth.rs>login"`.
    pub path: String,
}

/// Input parameters for `mycelium_get_extends`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetExtendsRequest {
    /// Trunk path to query, e.g. `"src/shapes.py>Rectangle"`.
    pub path: String,
}

/// Input parameters for `mycelium_get_implements`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetImplementsRequest {
    /// Trunk path to query, e.g. `"src/io.ts>FileReader"`.
    pub path: String,
}

/// Input parameters for `mycelium_get_entry_points`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetEntryPointsRequest {
    /// Optional path prefix to restrict results (e.g. `"src/handlers/"`).
    pub path_prefix: Option<String>,
}

/// Input parameters for `mycelium_rank_symbols`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RankSymbolsRequest {
    /// Maximum results to return (default 10, capped at 100).
    pub limit: Option<usize>,
}

/// Input parameters for `mycelium_get_top_files`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetTopFilesRequest {
    /// Maximum results to return (default 10, capped at 100).
    pub limit: Option<usize>,
}

/// Input parameters for `mycelium_get_most_connected`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetMostConnectedRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Maximum results to return (default 10, capped at 100).
    pub limit: Option<usize>,
}

/// Input parameters for `mycelium_get_leaf_symbols`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetLeafSymbolsRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Maximum results to return (default 10, capped at 100).
    pub limit: Option<usize>,
}

/// Input parameters for `mycelium_get_shortest_path`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetShortestPathRequest {
    /// Source node path (e.g. `"src/a.rs>main"`).
    pub from: String,
    /// Target node path (e.g. `"src/b.rs>helper"`).
    pub to: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_symbol_count_by_kind` (no parameters).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSymbolCountByKindRequest {}

/// Input parameters for `mycelium_get_common_callers`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCommonCallersRequest {
    /// Target node paths to intersect (1–20 entries).
    pub paths: Vec<String>,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_common_callees`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCommonCalleesRequest {
    /// Source node paths to intersect (1–20 entries).
    pub paths: Vec<String>,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_fan_out_rank`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFanOutRankRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Maximum results to return (default 10, capped at 100).
    pub limit: Option<usize>,
}

/// Input parameters for `mycelium_get_fan_in_rank`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFanInRankRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Maximum results to return (default 10, capped at 100).
    pub limit: Option<usize>,
}

/// Input parameters for `mycelium_get_files`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFilesRequest {
    /// Optional path prefix to filter results (e.g. `"src/"`).
    pub path_prefix: Option<String>,
}

/// Input parameters for `mycelium_get_dead_symbols`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetDeadSymbolsRequest {
    /// Optional path prefix to filter results (e.g. `"src/"`).
    pub path_prefix: Option<String>,
}

/// Input parameters for `mycelium_get_isolated_symbols`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetIsolatedSymbolsRequest {
    /// Optional path prefix to filter results (e.g. `"src/"`).
    pub path_prefix: Option<String>,
}

/// Input parameters for `mycelium_get_stats` (no parameters).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetStatsRequest {}

/// Input parameters for `mycelium_get_cross_refs`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCrossRefsRequest {
    /// Symbol path to look up, e.g. `"src/lib.rs>MyClass"`.
    pub path: String,
}

/// Input parameters for `mycelium_get_outgoing_refs`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetOutgoingRefsRequest {
    /// Symbol path to look up, e.g. `"src/app.rs>App"`.
    pub path: String,
}

/// Input parameters for `mycelium_get_scc_groups`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSccGroupsRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_dependency_layers`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetDependencyLayersRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_two_hop_neighbors`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetTwoHopNeighborsRequest {
    /// Symbol path, e.g. `"src/service.rs>Service"`.
    pub path: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_symbol_neighborhood`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSymbolNeighborhoodRequest {
    /// Symbol path, e.g. `"src/service.rs>Service"`.
    pub path: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_hub_symbols`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetHubSymbolsRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Minimum in-degree. Defaults to 1 if omitted.
    pub min_in: Option<usize>,
    /// Minimum out-degree. Defaults to 1 if omitted.
    pub min_out: Option<usize>,
    /// Maximum results returned. Defaults to 10, capped at 100.
    pub limit: Option<usize>,
}

/// Input parameters for `mycelium_get_singly_referenced`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSinglyReferencedRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Maximum results returned. Defaults to 10, capped at 100.
    pub limit: Option<usize>,
}

/// Input parameters for `mycelium_batch_reachable_to`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BatchReachableToRequest {
    /// Symbol paths to find dependents of (up to 20 entries).
    pub paths: Vec<String>,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Maximum BFS depth per source. Defaults to 10, capped at 20.
    pub max_depth: Option<usize>,
}

/// Input parameters for `mycelium_batch_reachable_from`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BatchReachableFromRequest {
    /// Symbol paths to start from (up to 20 entries).
    pub paths: Vec<String>,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Maximum BFS depth per source. Defaults to 10, capped at 20.
    pub max_depth: Option<usize>,
}

/// Input parameters for `mycelium_batch_node_degree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BatchNodeDegreeRequest {
    /// Symbol paths to query (up to 50 entries).
    pub paths: Vec<String>,
}

/// Input parameters for `mycelium_get_wcc`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetWccRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Only return components with at least this many symbols. Defaults to 1.
    pub min_size: Option<usize>,
}

/// Input parameters for `mycelium_find_articulation_points`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindArticulationPointsRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_find_bridge_edges`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindBridgeEdgesRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_biconnected_components`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BiconnectedComponentsRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_degree_histogram`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DegreeHistogramRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_graph_metrics`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphMetricsRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_neighbor_similarity`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct NeighborSimilarityRequest {
    /// First symbol path.
    pub path1: String,
    /// Second symbol path.
    pub path2: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_clustering_coefficient`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClusteringCoefficientRequest {
    /// Symbol path, e.g. `"src/a.rs>MyStruct"`.
    pub path: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_eccentricity`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EccentricityRequest {
    /// Symbol path, e.g. `"src/a.rs>MyStruct"`.
    pub path: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_harmonic_centrality`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HarmonicCentralityRequest {
    /// Symbol path, e.g. `"src/a.rs>MyStruct"`.
    pub path: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_mutual_reachability`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct MutualReachabilityRequest {
    /// First symbol path, e.g. `"src/a.rs>A"`.
    pub path1: String,
    /// Second symbol path, e.g. `"src/b.rs>B"`.
    pub path2: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_betweenness_centrality`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BetweennessCentralityRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// How many top entries to return; defaults to 10 if absent.
    pub top_n: Option<usize>,
}

/// Input parameters for `mycelium_get_closeness_centrality`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClosenessCentralityRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// How many top entries to return; defaults to 10 if absent.
    pub top_n: Option<usize>,
}

/// Input parameters for `mycelium_get_degree_centrality`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DegreeCentralityRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// How many top entries to return; defaults to 10 if absent.
    pub top_n: Option<usize>,
    /// Sort order: `"in"` (default, by in-degree centrality) or `"out"` (by out-degree centrality).
    pub sort_by: Option<String>,
}

/// Input parameters for `mycelium_get_strongly_connected_components`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StronglyConnectedComponentsRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Minimum component size to include; defaults to 1 (all components).
    /// Use `2` to return only non-trivial SCCs (circular dependencies).
    pub min_size: Option<usize>,
}

/// Input parameters for `mycelium_get_k_hop_neighbors`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct KHopNeighborsRequest {
    /// Symbol path, e.g. `"src/a.rs>A"`.
    pub path: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Number of hops (k ≥ 1; k = 0 returns empty).
    pub k: usize,
}

/// Input parameters for `mycelium_get_common_reachable`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CommonReachableRequest {
    /// First symbol path, e.g. `"src/a.rs>A"`.
    pub path1: String,
    /// Second symbol path, e.g. `"src/b.rs>B"`.
    pub path2: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_page_rank`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PageRankRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Damping factor ∈ [0.0, 1.0]; defaults to 0.85 if absent.
    pub damping: Option<f64>,
    /// Number of power iterations; defaults to 20 if absent.
    pub iterations: Option<usize>,
    /// How many top entries to return; defaults to 10 if absent.
    pub top_n: Option<usize>,
}

/// Input parameters for `mycelium_get_reaches_into`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReachesIntoRequest {
    /// Symbol path, e.g. `"src/a.rs>A"`.
    pub path: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_reachable_set`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReachableSetRequest {
    /// Symbol path, e.g. `"src/a.rs>A"`.
    pub path: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_topological_sort`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TopologicalSortRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_find_cycle_members`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindCycleMembersRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
}

/// Input parameters for `mycelium_get_k_core`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetKCoreRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Minimum total degree (in + out) within the induced subgraph. Defaults to 2.
    pub k: Option<usize>,
}

/// Input parameters for `mycelium_get_all_symbols`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetAllSymbolsRequest {
    /// Optional path prefix to restrict results, e.g. `"src/"`.
    pub path_prefix: Option<String>,
    /// Optional kind filter: `"function"`, `"class"`, `"method"`, etc.
    pub kind: Option<String>,
}

/// Input parameters for `mycelium_get_reachable`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetReachableRequest {
    /// Starting symbol path, e.g. `"src/app.rs>App"`.
    pub path: String,
    /// Edge kind to follow: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Maximum BFS depth. Defaults to 10, capped at 20.
    pub max_depth: Option<usize>,
}

/// Input parameters for `mycelium_get_reachable_to`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetReachableToRequest {
    /// Target symbol path, e.g. `"src/utils.rs>helper"`.
    pub path: String,
    /// Edge kind to follow backwards: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Maximum BFS depth. Defaults to 10, capped at 20.
    pub max_depth: Option<usize>,
}

/// Input parameters for `mycelium_get_siblings`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSiblingsRequest {
    /// Symbol path whose siblings to look up, e.g. `"src/app.rs>App>render"`.
    pub path: String,
}

/// Input parameters for `mycelium_get_node_degree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetNodeDegreeRequest {
    /// Symbol or file path to analyse, e.g. `"src/app.rs>App"`.
    pub path: String,
}

/// Input parameters for `mycelium_detect_cycles`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DetectCyclesRequest {
    /// Edge kind to analyze: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Optional path prefix to filter returned cycle nodes (e.g. `"src/"`).
    pub path_prefix: Option<String>,
}

/// Input parameters for `mycelium_find_call_path`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindCallPathRequest {
    /// Start of the call chain, e.g. `"src/main.rs>main"`.
    pub from_path: String,
    /// End of the call chain, e.g. `"src/db.rs>query"`.
    pub to_path: String,
    /// Maximum traversal depth (hops). Defaults to 10, capped at 20.
    pub max_depth: Option<usize>,
}

// ── server ────────────────────────────────────────────────────────────────────

/// Stateful MCP server holding the in-memory symbol graph.
///
/// Construct with [`MyceliumServer::new`] or [`MyceliumServer::with_root`]
/// and start with [`serve_stdio`].
#[derive(Debug, Clone)]
pub struct MyceliumServer {
    store: Arc<RwLock<Store>>,
    indexed_root: Arc<RwLock<Option<PathBuf>>>,
    watch_state: Arc<WatchState>,
    watch_abort: Arc<tokio::sync::Mutex<Option<tokio::task::AbortHandle>>>,
}

impl Default for MyceliumServer {
    fn default() -> Self {
        Self::new()
    }
}

impl MyceliumServer {
    /// Create a fresh server with an empty in-memory store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(Store::new())),
            indexed_root: Arc::new(RwLock::new(None)),
            watch_state: Arc::new(WatchState::default()),
            watch_abort: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Create a server pre-loaded from `root`.
    ///
    /// If `<root>/.mycelium/index.rmp` exists, loads the snapshot.
    /// Otherwise runs a full live index and saves the snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error only if the live index cannot be initiated (e.g.
    /// `root` is inaccessible). Snapshot load failures fall back to live
    /// indexing silently.
    pub async fn with_root(root: PathBuf) -> anyhow::Result<Self> {
        let snap = root.join(".mycelium").join("index.rmp");
        let server = Self::new();

        if snap.exists() {
            match Store::load(&snap) {
                Ok(loaded) => {
                    tracing::info!(
                        nodes = loaded.node_count(),
                        path = %snap.display(),
                        "loaded index from snapshot"
                    );
                    *server.store.write().await = loaded;
                    *server.indexed_root.write().await = Some(root.clone());
                    server.start_watch(root).await?;
                    return Ok(server);
                }
                Err(e) => {
                    tracing::warn!("snapshot load failed ({e}), falling back to live index");
                }
            }
        }

        // Fall back: run live index.
        let root_clone = root.clone();
        let (new_store, files, errors, _languages, _stubs) =
            tokio::task::spawn_blocking(move || run_index(&root_clone))
                .await
                .map_err(|e| anyhow::anyhow!("indexing task panicked: {e}"))??;
        tracing::info!(files, errors, "live index completed");
        if let Err(e) = new_store.save(&snap) {
            tracing::warn!("could not save snapshot after live index: {e}");
        }
        *server.store.write().await = new_store;
        *server.indexed_root.write().await = Some(root.clone());
        server.start_watch(root).await?;
        Ok(server)
    }

    /// Start the background file-system watch loop for `root`.
    ///
    /// Events are debounced over a 300 ms window.  Modified/created files
    /// are re-extracted; deleted files are removed from the store.  A new
    /// snapshot is saved after each batch.
    ///
    /// Calling `start_watch` on an already-watching server replaces the
    /// previous watcher.
    ///
    /// # Errors
    ///
    /// Returns an error if the OS watcher cannot be created or `root` cannot
    /// be watched.
    pub async fn start_watch(&self, root: PathBuf) -> anyhow::Result<()> {
        use tokio::time::{Duration, Instant, timeout_at};

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<notify::Event>();

        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                if let Ok(ev) = res {
                    tx.send(ev).ok();
                }
            },
            Config::default(),
        )
        .context("creating file system watcher")?;

        watcher
            .watch(&root, RecursiveMode::Recursive)
            .context("starting recursive watch")?;

        let store = Arc::clone(&self.store);
        let watch_state = Arc::clone(&self.watch_state);
        let snap = root.join(".mycelium").join("index.rmp");

        // Build ignore matcher from root .gitignore / .myceliumignore.
        let gitignore = {
            let mut gb = ignore::gitignore::GitignoreBuilder::new(&root);
            for name in &[".gitignore", ".myceliumignore"] {
                let p = root.join(name);
                if p.exists() {
                    gb.add(p);
                }
            }
            gb.build()
                .unwrap_or_else(|_| ignore::gitignore::Gitignore::empty())
        };

        watch_state.watching.store(true, Ordering::Relaxed);

        let handle = tokio::spawn(async move {
            let _watcher = watcher; // keep the watcher alive for the task lifetime

            loop {
                // Wait for the first event of a batch.
                let Some(first) = rx.recv().await else {
                    break; // channel closed
                };

                let mut batch: Vec<PathBuf> = first.paths;

                // Debounce: collect additional events arriving within 300 ms.
                let deadline = Instant::now() + Duration::from_millis(300);
                while let Ok(Some(ev)) = timeout_at(deadline, rx.recv()).await {
                    batch.extend(ev.paths);
                }

                // Deduplicate and process.
                batch.sort_unstable();
                batch.dedup();

                let mut store_w = store.write().await;
                let mut changed = false;

                for abs_path in &batch {
                    // Skip paths that match the ignore rules or are always excluded.
                    let is_ignored = abs_path
                        .strip_prefix(&root)
                        .ok()
                        .and_then(|rel| rel.components().next())
                        .is_some_and(|first_comp| {
                            matches!(
                                first_comp.as_os_str().to_string_lossy().as_ref(),
                                "target" | ".mycelium"
                            )
                        });
                    if is_ignored {
                        continue;
                    }
                    if gitignore.matched(abs_path, abs_path.is_dir()).is_ignore() {
                        continue;
                    }

                    let rel = abs_path
                        .strip_prefix(&root)
                        .unwrap_or(abs_path)
                        .to_string_lossy()
                        .replace('\\', "/");

                    // Remove old data for this file regardless of event kind.
                    store_w.remove_file(&rel);

                    // Re-index if the file still exists and is a known type.
                    if abs_path.is_file() {
                        if let Some(ext) = abs_path.extension().and_then(|e| e.to_str()) {
                            if matches!(ext, "js" | "jsx" | "py" | "pyi" | "ts" | "tsx" | "rs") {
                                if let Ok(src) = std::fs::read(abs_path) {
                                    let rel_owned = rel.clone();
                                    let src_owned = src;
                                    // Re-use run_index logic via a single-file helper.
                                    reindex_file(&rel_owned, &src_owned, ext, &mut store_w);
                                }
                            }
                        }
                    }
                    changed = true;
                }
                store_w.resolve_bare_call_stubs();
                drop(store_w);

                if changed {
                    watch_state
                        .batches_processed
                        .fetch_add(1, Ordering::Relaxed);
                    // Save snapshot (best-effort; failures are non-fatal).
                    store.read().await.save(&snap).ok();
                }
            }

            watch_state.watching.store(false, Ordering::Relaxed);
        });

        {
            let mut guard = self.watch_abort.lock().await;
            if let Some(old) = guard.replace(handle.abort_handle()) {
                old.abort();
            }
        }

        Ok(())
    }
}

#[tool_router]
impl MyceliumServer {
    #[tool(
        description = "Index a workspace directory and populate the in-memory symbol graph. \
                       Call this before searching. Returns file count, error count, and the \
                       list of indexed language names."
    )]
    async fn mycelium_index_workspace(
        &self,
        Parameters(req): Parameters<IndexWorkspaceRequest>,
    ) -> String {
        let root = PathBuf::from(&req.path);
        let result = tokio::task::spawn_blocking(move || run_index(&root)).await;
        match result {
            Err(e) => serde_json::json!({ "error": format!("task panicked: {e}") }).to_string(),
            Ok(Err(e)) => serde_json::json!({ "error": e.to_string() }).to_string(),
            Ok(Ok((new_store, files, errors, languages, stubs_resolved))) => {
                // RFC-0006: auto-save snapshot alongside the workspace
                let snap = PathBuf::from(&req.path).join(".mycelium").join("index.rmp");
                if let Err(e) = new_store.save(&snap) {
                    warn!("could not save index snapshot: {e}");
                }
                *self.store.write().await = new_store;
                *self.indexed_root.write().await = Some(PathBuf::from(&req.path));
                serde_json::json!({
                    "files": files,
                    "errors": errors,
                    "languages": languages,
                    "stubs_resolved": stubs_resolved,
                })
                .to_string()
            }
        }
    }

    #[tool(
        description = "Search for symbols by name prefix or substring (case-insensitive). \
                       Returns matching trunk paths. Call mycelium_index_workspace first."
    )]
    async fn mycelium_search_symbol(
        &self,
        Parameters(req): Parameters<SearchSymbolRequest>,
    ) -> String {
        let limit = req.limit.unwrap_or(20);
        let matches = self.store.read().await.search_symbol(&req.query, limit);
        serde_json::json!({ "matches": matches }).to_string()
    }

    #[tool(
        description = "Return the ancestor chain (containment hierarchy) for a given trunk path, \
                       in child-to-root order. Returns an empty list if the path has no ancestors."
    )]
    async fn mycelium_get_ancestors(
        &self,
        Parameters(req): Parameters<GetAncestorsRequest>,
    ) -> String {
        let ancestors = self
            .store
            .read()
            .await
            .ancestors_of_path(&req.path)
            .unwrap_or_default();
        serde_json::json!({ "ancestors": ancestors }).to_string()
    }

    #[tool(
        description = "Return all symbols nested under a given trunk path (strict descendants). \
                       Returns an empty list if the path is a leaf node or is not in the index."
    )]
    async fn mycelium_get_descendants(
        &self,
        Parameters(req): Parameters<GetDescendantsRequest>,
    ) -> String {
        let descendants = self
            .store
            .read()
            .await
            .descendants_of_path(&req.path)
            .unwrap_or_default();
        serde_json::json!({ "descendants": descendants }).to_string()
    }

    #[tool(
        description = "Load a previously saved index from disk without re-indexing. \
                       Reads the .mycelium/index.rmp snapshot created by mycelium_index_workspace. \
                       Returns the number of nodes loaded."
    )]
    async fn mycelium_load_index(&self, Parameters(req): Parameters<LoadIndexRequest>) -> String {
        let snap = PathBuf::from(&req.path).join(".mycelium").join("index.rmp");
        match Store::load(&snap) {
            Err(e) => serde_json::json!({ "error": e.to_string() }).to_string(),
            Ok(loaded) => {
                let nodes = loaded.node_count();
                *self.store.write().await = loaded;
                *self.indexed_root.write().await = Some(PathBuf::from(&req.path));
                serde_json::json!({
                    "nodes": nodes,
                    "loaded_from": ".mycelium/index.rmp"
                })
                .to_string()
            }
        }
    }

    #[tool(
        description = "Return the current server status: indexed root directory, node count, \
                       and whether an index has been loaded. Useful for diagnostics and \
                       confirming the server is ready before issuing queries."
    )]
    async fn mycelium_server_status(&self) -> String {
        let store_guard = self.store.read().await;
        let node_count = store_guard.node_count();
        let edge_count = store_guard.edge_count();
        drop(store_guard);
        let root_guard = self.indexed_root.read().await;
        let indexed_root = root_guard
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let is_loaded = root_guard.is_some();
        drop(root_guard);
        serde_json::json!({
            "node_count": node_count,
            "edge_count": edge_count,
            "indexed_root": indexed_root,
            "is_loaded": is_loaded,
        })
        .to_string()
    }

    #[tool(
        description = "Return the current file-watch loop status: whether the watcher is active, \
                       the root being watched, and how many change batches have been processed."
    )]
    async fn mycelium_watch_status(&self) -> String {
        let watching = self.watch_state.watching.load(Ordering::Relaxed);
        let batches_processed = self.watch_state.batches_processed.load(Ordering::Relaxed);
        let root_guard = self.indexed_root.read().await;
        let root = root_guard
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        drop(root_guard);
        serde_json::json!({
            "watching": watching,
            "root": root,
            "batches_processed": batches_processed,
        })
        .to_string()
    }

    #[tool(
        description = "Return all symbols (callee paths) that a given symbol calls directly. \
                       Uses the Calls edges populated during indexing. Returns a sorted list \
                       of trunk paths."
    )]
    async fn mycelium_get_callees(&self, Parameters(req): Parameters<GetCalleesRequest>) -> String {
        let store_guard = self.store.read().await;
        let lookup_result = store_guard.lookup(&req.path);
        let Some(id) = lookup_result else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        let mut paths: Vec<String> = store_guard
            .outgoing(id, mycelium_core::types::EdgeKind::Calls)
            .iter()
            .filter_map(|&dst| store_guard.path_of(dst).map(str::to_owned))
            .collect();
        drop(store_guard);
        paths.sort();
        paths.dedup();
        serde_json::json!({ "callee_paths": paths }).to_string()
    }

    #[tool(
        description = "Return all symbols (caller paths) that call a given symbol directly. \
                       Uses the reverse Calls edges populated during indexing. Returns a sorted \
                       list of trunk paths."
    )]
    async fn mycelium_get_callers(&self, Parameters(req): Parameters<GetCallersRequest>) -> String {
        let store_guard = self.store.read().await;
        let lookup_result = store_guard.lookup(&req.path);
        let Some(id) = lookup_result else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        let mut paths: Vec<String> = store_guard
            .incoming(id, mycelium_core::types::EdgeKind::Calls)
            .iter()
            .filter_map(|&src| store_guard.path_of(src).map(str::to_owned))
            .collect();
        drop(store_guard);
        paths.sort();
        paths.dedup();
        serde_json::json!({ "caller_paths": paths }).to_string()
    }

    #[tool(
        description = "Return all structural information about a symbol in one call: \
                       its ancestors (containing scopes), descendants (nested symbols), \
                       callers (functions that call it), and callees (functions it calls). \
                       Returns an error if the path is not in the index."
    )]
    async fn mycelium_get_symbol_info(
        &self,
        Parameters(req): Parameters<GetSymbolInfoRequest>,
    ) -> String {
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };

        let ancestors: Vec<String> = store_guard
            .ancestors(id)
            .filter_map(|aid| store_guard.path_of(aid).map(str::to_owned))
            .collect();

        let mut descendants: Vec<String> = store_guard
            .descendants(id)
            .filter_map(|did| store_guard.path_of(did).map(str::to_owned))
            .collect();
        descendants.sort_unstable();

        let mut callers: Vec<String> = store_guard
            .incoming(id, mycelium_core::types::EdgeKind::Calls)
            .iter()
            .filter_map(|&src| store_guard.path_of(src).map(str::to_owned))
            .collect();
        callers.sort_unstable();
        callers.dedup();

        let mut callees: Vec<String> = store_guard
            .outgoing(id, mycelium_core::types::EdgeKind::Calls)
            .iter()
            .filter_map(|&dst| store_guard.path_of(dst).map(str::to_owned))
            .collect();
        callees.sort_unstable();
        callees.dedup();

        drop(store_guard);

        serde_json::json!({
            "path": req.path,
            "ancestors": ancestors,
            "descendants": descendants,
            "callers": callers,
            "callees": callees,
        })
        .to_string()
    }

    #[tool(
        description = "Return symbol info for multiple paths in one call. Equivalent to calling \
                       mycelium_get_symbol_info for each path individually, but as a single \
                       round-trip. Each entry contains ancestors, descendants, callers, and \
                       callees for found paths, or an error field for unknown paths. \
                       Maximum 50 paths per request."
    )]
    async fn mycelium_batch_symbol_info(
        &self,
        Parameters(req): Parameters<BatchSymbolInfoRequest>,
    ) -> String {
        let store_guard = self.store.read().await;
        let symbols: Vec<serde_json::Value> = req
            .paths
            .iter()
            .take(50)
            .map(|path| {
                let Some(id) = store_guard.lookup(path) else {
                    return serde_json::json!({ "path": path, "error": "path not found" });
                };

                let ancestors: Vec<String> = store_guard
                    .ancestors(id)
                    .filter_map(|aid| store_guard.path_of(aid).map(str::to_owned))
                    .collect();

                let mut descendants: Vec<String> = store_guard
                    .descendants(id)
                    .filter_map(|did| store_guard.path_of(did).map(str::to_owned))
                    .collect();
                descendants.sort_unstable();

                let mut callers: Vec<String> = store_guard
                    .incoming(id, mycelium_core::types::EdgeKind::Calls)
                    .iter()
                    .filter_map(|&src| store_guard.path_of(src).map(str::to_owned))
                    .collect();
                callers.sort_unstable();
                callers.dedup();

                let mut callees: Vec<String> = store_guard
                    .outgoing(id, mycelium_core::types::EdgeKind::Calls)
                    .iter()
                    .filter_map(|&dst| store_guard.path_of(dst).map(str::to_owned))
                    .collect();
                callees.sort_unstable();
                callees.dedup();

                serde_json::json!({
                    "path": path,
                    "ancestors": ancestors,
                    "descendants": descendants,
                    "callers": callers,
                    "callees": callees,
                })
            })
            .collect();
        drop(store_guard);
        serde_json::json!({ "symbols": symbols }).to_string()
    }

    #[tool(
        description = "Return the transitive callee tree rooted at a given symbol, up to \
                       max_depth hops. Each node contains its path and a list of callee subtrees. \
                       Cycles are represented as leaf nodes. max_depth defaults to 4, capped at 10."
    )]
    async fn mycelium_get_callee_tree(
        &self,
        Parameters(req): Parameters<GetCalleeTreeRequest>,
    ) -> String {
        let max_depth = req.max_depth.unwrap_or(4).min(10);
        let store_guard = self.store.read().await;
        let Some(root_id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        let tree = store_guard.callee_tree(root_id, max_depth);
        let json_tree = callee_node_to_json(&tree, &store_guard);
        drop(store_guard);
        serde_json::json!({ "root": json_tree }).to_string()
    }

    #[tool(
        description = "Return a depth-limited tree of all transitive callers that can reach a \
                       given symbol, walking incoming Calls edges up to max_depth hops. Each \
                       node contains its path and a list of caller subtrees. Cycles are \
                       represented as leaf nodes. max_depth defaults to 4, capped at 10."
    )]
    async fn mycelium_get_caller_tree(
        &self,
        Parameters(req): Parameters<GetCallerTreeRequest>,
    ) -> String {
        let max_depth = req.max_depth.unwrap_or(4).min(10);
        let store_guard = self.store.read().await;
        let Some(root_id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        let tree = store_guard.caller_tree(root_id, max_depth);
        let json_tree = caller_node_to_json(&tree, &store_guard);
        drop(store_guard);
        serde_json::json!({ "root": json_tree }).to_string()
    }

    #[tool(description = "Return the direct import neighbors for a trunk path: \
                       'imports' (outgoing Imports edges — what this node imports) and \
                       'imported_by' (incoming Imports edges — what imports this node). \
                       Both lists sorted lexicographically. Unknown path returns { error }.")]
    async fn mycelium_get_imports(&self, Parameters(req): Parameters<GetImportsRequest>) -> String {
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        let imports = store_guard.imports_of(id);
        let imported_by = store_guard.imported_by(id);
        drop(store_guard);
        serde_json::json!({ "imports": imports, "imported_by": imported_by }).to_string()
    }

    #[tool(
        description = "Return a depth-limited tree of all transitive import dependencies for a \
                       path, walking outgoing Imports edges up to max_depth hops. Each node \
                       contains its path and a list of import subtrees. Cycles are represented \
                       as leaf nodes. max_depth defaults to 4, capped at 10."
    )]
    async fn mycelium_get_import_tree(
        &self,
        Parameters(req): Parameters<GetImportTreeRequest>,
    ) -> String {
        let max_depth = req.max_depth.unwrap_or(4).min(10);
        let store_guard = self.store.read().await;
        let Some(root_id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        let tree = store_guard.import_tree(root_id, max_depth);
        let json_tree = import_node_to_json(&tree, &store_guard);
        drop(store_guard);
        serde_json::json!({ "root": json_tree }).to_string()
    }

    #[tool(
        description = "Return the NodeKind for a given path. kind is the wire-string representation \
                       (e.g. \"function\", \"class\", \"method\", \"file\"). Returns { path, kind } \
                       where kind may be null if the kind was not recorded during indexing. \
                       Unknown path returns { error }."
    )]
    async fn mycelium_get_node_kind(
        &self,
        Parameters(req): Parameters<GetNodeKindRequest>,
    ) -> String {
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        let kind_str: serde_json::Value = store_guard
            .kind_of(id)
            .map_or(serde_json::Value::Null, |k| {
                serde_json::Value::String(k.as_str().to_owned())
            });
        drop(store_guard);
        serde_json::json!({ "path": req.path, "kind": kind_str }).to_string()
    }

    #[tool(
        description = "Return all indexed symbol paths whose recorded NodeKind matches kind. \
                       kind must be a valid wire string, e.g. \"function\", \"class\", \
                       \"method\", \"interface\", \"struct\", \"enum\", \"type_alias\", \
                       \"constant\", \"module\", \"file\". Unknown kind returns { error }. \
                       Optional path_prefix restricts results. Results sorted lexicographically."
    )]
    async fn mycelium_get_symbols_by_kind(
        &self,
        Parameters(req): Parameters<GetSymbolsByKindRequest>,
    ) -> String {
        let Some(kind) = mycelium_core::types::NodeKind::try_from_wire(&req.kind) else {
            return serde_json::json!({ "error": format!("unknown kind: {}", req.kind) })
                .to_string();
        };
        let symbols = self
            .store
            .read()
            .await
            .symbols_of_kind(kind, req.path_prefix.as_deref());
        serde_json::json!({ "symbols": symbols }).to_string()
    }

    #[tool(
        description = "Return the source location of a symbol: start_line (1-indexed), start_col \
                       (0-indexed), end_line (1-indexed), end_col (0-indexed), start_byte, \
                       end_byte. Returns { path, start_line, start_col, end_line, end_col, \
                       start_byte, end_byte } when the span is recorded, { path, span: null } \
                       when the node exists but has no recorded span, or { error } when the \
                       path is not found."
    )]
    async fn mycelium_get_source_span(
        &self,
        Parameters(req): Parameters<GetSourceSpanRequest>,
    ) -> String {
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        if let Some(span) = store_guard.span_of(id) {
            drop(store_guard);
            serde_json::json!({
                "path": req.path,
                "start_line": span.start_line,
                "start_col": span.start_col,
                "end_line": span.end_line,
                "end_col": span.end_col,
                "start_byte": span.start_byte,
                "end_byte": span.end_byte,
            })
            .to_string()
        } else {
            drop(store_guard);
            serde_json::json!({ "path": req.path, "span": serde_json::Value::Null }).to_string()
        }
    }

    #[tool(
        description = "Return the direct inheritance relationships for a path. extends lists \
                       symbols this path directly extends (outgoing Extends edges). extended_by \
                       lists symbols that extend this path (incoming Extends edges). Both lists \
                       are sorted lexicographically. Unknown path returns { error }."
    )]
    async fn mycelium_get_extends(&self, Parameters(req): Parameters<GetExtendsRequest>) -> String {
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        let mut extends: Vec<String> = store_guard
            .outgoing(id, mycelium_core::types::EdgeKind::Extends)
            .iter()
            .filter_map(|&dst| store_guard.path_of(dst).map(str::to_owned))
            .collect();
        extends.sort_unstable();
        let mut extended_by: Vec<String> = store_guard
            .incoming(id, mycelium_core::types::EdgeKind::Extends)
            .iter()
            .filter_map(|&src| store_guard.path_of(src).map(str::to_owned))
            .collect();
        extended_by.sort_unstable();
        drop(store_guard);
        serde_json::json!({ "extends": extends, "extended_by": extended_by }).to_string()
    }

    #[tool(
        description = "Return the direct interface-implementation relationships for a path. \
                       implements lists symbols this path directly implements (outgoing Implements \
                       edges). implemented_by lists symbols that implement this path (incoming \
                       Implements edges). Both lists are sorted lexicographically. Unknown path \
                       returns { error }."
    )]
    async fn mycelium_get_implements(
        &self,
        Parameters(req): Parameters<GetImplementsRequest>,
    ) -> String {
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        let mut implements: Vec<String> = store_guard
            .outgoing(id, mycelium_core::types::EdgeKind::Implements)
            .iter()
            .filter_map(|&dst| store_guard.path_of(dst).map(str::to_owned))
            .collect();
        implements.sort_unstable();
        let mut implemented_by: Vec<String> = store_guard
            .incoming(id, mycelium_core::types::EdgeKind::Implements)
            .iter()
            .filter_map(|&src| store_guard.path_of(src).map(str::to_owned))
            .collect();
        implemented_by.sort_unstable();
        drop(store_guard);
        serde_json::json!({ "implements": implements, "implemented_by": implemented_by })
            .to_string()
    }

    #[tool(
        description = "Return all indexed symbols (non-file nodes) that have zero incoming Calls \
                       edges. These are either genuine entry points (main, test functions, public \
                       API handlers) or potentially dead code. Optional path_prefix restricts \
                       results to a subdirectory. Results are sorted lexicographically."
    )]
    async fn mycelium_get_entry_points(
        &self,
        Parameters(req): Parameters<GetEntryPointsRequest>,
    ) -> String {
        let eps = self
            .store
            .read()
            .await
            .entry_points(req.path_prefix.as_deref());
        serde_json::json!({ "entry_points": eps }).to_string()
    }

    #[tool(
        description = "Return all indexed symbols (non-file nodes) with zero incoming Calls edges \
                       AND zero incoming Imports edges. These are dead-code candidates — no other \
                       symbol calls or imports them. Optional path_prefix filters to a subtree. \
                       Returns { dead_symbols: [...], count: N } sorted lexicographically."
    )]
    async fn mycelium_get_dead_symbols(
        &self,
        Parameters(req): Parameters<GetDeadSymbolsRequest>,
    ) -> String {
        let dead = self
            .store
            .read()
            .await
            .dead_symbols(req.path_prefix.as_deref());
        let count = dead.len();
        serde_json::json!({ "dead_symbols": dead, "count": count }).to_string()
    }

    #[tool(
        description = "Return symbol nodes that have zero connectivity across ALL four edge kinds \
                       (calls, imports, extends, implements) — completely isolated from the graph. \
                       Stronger than dead_symbols (which only checks incoming Calls+Imports). \
                       Isolated symbols have no incoming or outgoing edges of any kind and are \
                       strong deletion candidates. Optional path_prefix filter. \
                       Returns { isolated_symbols, count }."
    )]
    async fn mycelium_get_isolated_symbols(
        &self,
        Parameters(req): Parameters<GetIsolatedSymbolsRequest>,
    ) -> String {
        let isolated = self
            .store
            .read()
            .await
            .isolated_symbols(req.path_prefix.as_deref());
        let count = isolated.len();
        serde_json::json!({ "isolated_symbols": isolated, "count": count }).to_string()
    }

    #[tool(
        description = "Return comprehensive per-kind statistics about the indexed symbol graph: \
                       total node and edge counts plus breakdowns by NodeKind (file, function, \
                       class, …) and EdgeKind (calls, imports, extends, …). \
                       Returns { total_nodes, total_edges, nodes_by_kind, edges_by_kind }. \
                       Kinds with zero count are omitted."
    )]
    async fn mycelium_get_stats(&self, Parameters(_req): Parameters<GetStatsRequest>) -> String {
        let stats: GraphStats = self.store.read().await.graph_stats();
        serde_json::json!({
            "total_nodes": stats.total_nodes,
            "total_edges": stats.total_edges,
            "nodes_by_kind": stats.nodes_by_kind,
            "edges_by_kind": stats.edges_by_kind,
        })
        .to_string()
    }

    #[tool(
        description = "Return ALL incoming edge references to a symbol, grouped by edge kind: \
                       callers (Calls), importers (Imports), extended_by (Extends), \
                       implemented_by (Implements). This is the unified 'who references this?' \
                       primitive for impact analysis. All lists are sorted lexicographically. \
                       Empty lists are included. Unknown path returns { error }."
    )]
    async fn mycelium_get_cross_refs(
        &self,
        Parameters(req): Parameters<GetCrossRefsRequest>,
    ) -> String {
        let refs_opt: Option<CrossRefs> = {
            let store = self.store.read().await;
            store.lookup(&req.path).map(|id| store.cross_refs(id))
        };
        let Some(refs) = refs_opt else {
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        serde_json::json!({
            "callers": refs.callers,
            "importers": refs.importers,
            "extended_by": refs.extended_by,
            "implemented_by": refs.implemented_by,
        })
        .to_string()
    }

    #[tool(
        description = "Return ALL outgoing edge references from a symbol, grouped by edge kind: \
                       callees (Calls), imports (Imports), extends (Extends), \
                       implements (Implements). Symmetric complement to mycelium_get_cross_refs. \
                       All lists are sorted lexicographically. Empty lists are included. \
                       Unknown path returns { error }."
    )]
    async fn mycelium_get_outgoing_refs(
        &self,
        Parameters(req): Parameters<GetOutgoingRefsRequest>,
    ) -> String {
        let refs_opt: Option<OutgoingRefs> = {
            let store = self.store.read().await;
            store.lookup(&req.path).map(|id| store.outgoing_refs(id))
        };
        let Some(refs) = refs_opt else {
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        serde_json::json!({
            "callees": refs.callees,
            "imports": refs.imports,
            "extends": refs.extends,
            "implements": refs.implements,
        })
        .to_string()
    }

    #[tool(
        description = "List all non-file symbol paths in the graph, sorted lexicographically. \
                       Optionally filter by path_prefix (e.g. 'src/') and/or kind \
                       ('function', 'class', 'method', etc.). \
                       Returns { symbols: [...], count: N } or { error } for an unknown kind string."
    )]
    async fn mycelium_get_all_symbols(
        &self,
        Parameters(req): Parameters<GetAllSymbolsRequest>,
    ) -> String {
        if let Some(ref k) = req.kind {
            if mycelium_core::types::NodeKind::try_from_wire(k).is_none() {
                return serde_json::json!({ "error": format!("unknown kind: {k}") }).to_string();
            }
        }
        let kind = req
            .kind
            .as_deref()
            .and_then(mycelium_core::types::NodeKind::try_from_wire);
        let symbols = self
            .store
            .read()
            .await
            .all_symbols(req.path_prefix.as_deref(), kind);
        let count = symbols.len();
        serde_json::json!({ "symbols": symbols, "count": count }).to_string()
    }

    #[tool(
        description = "Return all symbols reachable from a starting path via outgoing edges of a \
                       given kind, up to max_depth BFS hops (default 10, cap 20). \
                       edge_kind must be 'calls', 'imports', 'extends', or 'implements'. \
                       Starting node is excluded from the result. Cycle-safe. \
                       Returns { reachable: [...], count: N } or { error } for unknown path or edge_kind."
    )]
    async fn mycelium_get_reachable(
        &self,
        Parameters(req): Parameters<GetReachableRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        let max_depth = req.max_depth.unwrap_or(10);
        let reachable_opt: Option<Vec<String>> = {
            let store = self.store.read().await;
            store
                .lookup(&req.path)
                .map(|id| store.reachable_from(id, kind, max_depth))
        };
        let Some(reachable) = reachable_opt else {
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        let count = reachable.len();
        serde_json::json!({ "reachable": reachable, "count": count }).to_string()
    }

    #[tool(
        description = "Return all symbols that can reach a target path via incoming edges of a \
                       given kind, up to max_depth BFS hops (default 10, cap 20). \
                       edge_kind must be 'calls', 'imports', 'extends', or 'implements'. \
                       Starting node excluded. Cycle-safe. Answers: 'who depends on this symbol?' \
                       Returns { reachable: [...], count: N } or { error } for unknown path or edge_kind."
    )]
    async fn mycelium_get_reachable_to(
        &self,
        Parameters(req): Parameters<GetReachableToRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        let max_depth = req.max_depth.unwrap_or(10);
        let reachable_opt: Option<Vec<String>> = {
            let store = self.store.read().await;
            store
                .lookup(&req.path)
                .map(|id| store.reachable_to(id, kind, max_depth))
        };
        let Some(reachable) = reachable_opt else {
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        let count = reachable.len();
        serde_json::json!({ "reachable": reachable, "count": count }).to_string()
    }

    #[tool(
        description = "Return all sibling symbols — direct children of the same parent container, \
                       excluding the given path itself. Useful for 'what else is in this class/file?'. \
                       Returns { siblings: [...], count: N } or { error } for unknown path. \
                       Root nodes (no parent) return { siblings: [], count: 0 }."
    )]
    async fn mycelium_get_siblings(
        &self,
        Parameters(req): Parameters<GetSiblingsRequest>,
    ) -> String {
        let siblings_opt: Option<Vec<String>> = {
            let store = self.store.read().await;
            store.lookup(&req.path).map(|id| store.siblings(id))
        };
        let Some(siblings) = siblings_opt else {
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        let count = siblings.len();
        serde_json::json!({ "siblings": siblings, "count": count }).to_string()
    }

    #[tool(
        description = "Return in/out edge counts for all four EdgeKinds for a given path — \
                       a fast connectivity summary without pulling full edge lists. \
                       Returns { in_calls, out_calls, in_imports, out_imports, \
                       in_extends, out_extends, in_implements, out_implements } \
                       or { error } for unknown paths."
    )]
    async fn mycelium_get_node_degree(
        &self,
        Parameters(req): Parameters<GetNodeDegreeRequest>,
    ) -> String {
        let degree_opt: Option<NodeDegree> = {
            let store = self.store.read().await;
            store.lookup(&req.path).map(|id| store.node_degree(id))
        };
        let Some(deg) = degree_opt else {
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        serde_json::json!({
            "in_calls": deg.in_calls,
            "out_calls": deg.out_calls,
            "in_imports": deg.in_imports,
            "out_imports": deg.out_imports,
            "in_extends": deg.in_extends,
            "out_extends": deg.out_extends,
            "in_implements": deg.in_implements,
            "out_implements": deg.out_implements,
        })
        .to_string()
    }

    #[tool(
        description = "Detect nodes that participate in at least one cycle for a given edge kind. \
                       edge_kind must be 'calls', 'imports', 'extends', or 'implements'. \
                       Optional path_prefix filters the returned node list. \
                       Returns { cycle_nodes: [...], count: N } or { error } for unknown edge_kind."
    )]
    async fn mycelium_detect_cycles(
        &self,
        Parameters(req): Parameters<DetectCyclesRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        let nodes = self
            .store
            .read()
            .await
            .nodes_in_cycles(kind, req.path_prefix.as_deref());
        let count = nodes.len();
        serde_json::json!({ "cycle_nodes": nodes, "count": count }).to_string()
    }

    #[tool(
        description = "Tarjan's Strongly Connected Components — groups of symbol nodes that are \
                       mutually reachable via the given edge kind (size ≥ 2). Each group is a \
                       dependency cycle cluster. Groups sorted by size descending (largest first). \
                       Paths within each group sorted ascending. \
                       Complements detect_cycles (which returns individual cycle nodes). \
                       edge_kind: 'calls', 'imports', 'extends', or 'implements'. \
                       Returns { groups, group_count, total_symbols } or { error }."
    )]
    async fn mycelium_get_scc_groups(
        &self,
        Parameters(req): Parameters<GetSccGroupsRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        let groups = self.store.read().await.scc_groups(kind);
        let group_count = groups.len();
        let total_symbols: usize = groups.iter().map(Vec::len).sum();
        serde_json::json!({
            "groups": groups,
            "group_count": group_count,
            "total_symbols": total_symbols,
        })
        .to_string()
    }

    #[tool(
        description = "Return symbol nodes grouped into Kahn BFS dependency layers for a given edge \
                       kind. Layer 0 = utility/leaf symbols with no outgoing edges (no dependencies). \
                       Layer k+1 = symbols all of whose direct dependencies are in layers 0..=k. \
                       Symbols in cycles are excluded and reported in cycle_excluded_count. \
                       Useful for understanding architectural layering and build-order dependencies. \
                       edge_kind: 'calls', 'imports', 'extends', or 'implements'. \
                       Returns { layers, layer_count, total_symbols, cycle_excluded_count } or { error }."
    )]
    async fn mycelium_get_dependency_layers(
        &self,
        Parameters(req): Parameters<GetDependencyLayersRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        let (layers, all_symbol_count) = {
            let store = self.store.read().await;
            let layers = store.dependency_layers(kind);
            let all_symbol_count = store.all_symbols(None, None).len();
            drop(store);
            (layers, all_symbol_count)
        };
        let layer_count = layers.len();
        let total_symbols: usize = layers.iter().map(Vec::len).sum();
        let cycle_excluded_count = all_symbol_count.saturating_sub(total_symbols);
        serde_json::json!({
            "layers": layers,
            "layer_count": layer_count,
            "total_symbols": total_symbols,
            "cycle_excluded_count": cycle_excluded_count,
        })
        .to_string()
    }

    #[tool(
        description = "Return symbol paths reachable from `path` in exactly 2 outgoing steps for \
                       `edge_kind`. Excludes the source symbol itself and its direct (1-hop) \
                       neighbours. Useful for discovering indirect dependencies or bridges without \
                       traversing the full reachability set. \
                       Unknown path returns { neighbors: [], count: 0 }. \
                       edge_kind: 'calls', 'imports', 'extends', or 'implements'. \
                       Returns { neighbors, count } or { error } for unknown edge_kind."
    )]
    async fn mycelium_get_two_hop_neighbors(
        &self,
        Parameters(req): Parameters<GetTwoHopNeighborsRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        let store = self.store.read().await;
        let Some(id) = store.lookup(&req.path) else {
            drop(store);
            return serde_json::json!({ "neighbors": [], "count": 0 }).to_string();
        };
        let neighbors = store.two_hop_neighbors(id, kind);
        drop(store);
        let count = neighbors.len();
        serde_json::json!({ "neighbors": neighbors, "count": count }).to_string()
    }

    #[tool(
        description = "Return the ego-graph of a symbol for a given edge kind: the symbol's own \
                       path plus all direct incoming and outgoing neighbours. Combines cross_refs \
                       (incoming) and outgoing_refs (outgoing) into a single focused call for one \
                       edge kind. Both lists are sorted ascending. \
                       Unknown path returns { path: '', incoming: [], outgoing: [], incoming_count: 0, outgoing_count: 0 }. \
                       edge_kind: 'calls', 'imports', 'extends', or 'implements'. \
                       Returns { path, incoming, outgoing, incoming_count, outgoing_count } or { error }."
    )]
    async fn mycelium_get_symbol_neighborhood(
        &self,
        Parameters(req): Parameters<GetSymbolNeighborhoodRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        let nb = {
            let store = self.store.read().await;
            let id = store.lookup(&req.path);
            let nb = id.map_or_else(SymbolNeighborhood::default, |id| {
                store.symbol_neighborhood(id, kind)
            });
            drop(store);
            nb
        };
        let incoming_count = nb.incoming.len();
        let outgoing_count = nb.outgoing.len();
        serde_json::json!({
            "path": nb.path,
            "incoming": nb.incoming,
            "outgoing": nb.outgoing,
            "incoming_count": incoming_count,
            "outgoing_count": outgoing_count,
        })
        .to_string()
    }

    #[tool(
        description = "Return symbols that are architectural hubs: high in-degree AND high out-degree \
                       for the given edge kind. A hub is called by many (hotspot) and also calls \
                       many (orchestrator) — the intersection of fan_in_rank and fan_out_rank. \
                       min_in: minimum in-degree (default 1). min_out: minimum out-degree (default 1). \
                       limit: max results (default 10, capped 100). \
                       Results sorted by in_degree + out_degree descending, ties by path ascending. \
                       edge_kind: 'calls', 'imports', 'extends', or 'implements'. \
                       Returns { hubs: [{ path, in_degree, out_degree }], count } or { error }."
    )]
    async fn mycelium_get_hub_symbols(
        &self,
        Parameters(req): Parameters<GetHubSymbolsRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        let min_in = req.min_in.unwrap_or(1);
        let min_out = req.min_out.unwrap_or(1);
        let limit = req.limit.unwrap_or(10);
        let hubs = self
            .store
            .read()
            .await
            .hub_symbols(kind, min_in, min_out, limit);
        let count = hubs.len();
        let hubs_json: Vec<serde_json::Value> = hubs
            .into_iter()
            .map(|(path, in_degree, out_degree)| {
                serde_json::json!({ "path": path, "in_degree": in_degree, "out_degree": out_degree })
            })
            .collect();
        serde_json::json!({ "hubs": hubs_json, "count": count }).to_string()
    }

    #[tool(
        description = "Return symbols that have exactly one incoming edge for a given EdgeKind. \
                       These 'singly-referenced' symbols are only depended on by a single caller, \
                       importer, or subclass — making them candidates for inlining, privatisation, \
                       or co-location with their sole consumer. Returns { symbols: [{ path, \
                       referenced_by }], count } or { error } for unknown edge_kind. limit defaults \
                       to 10, capped at 100. File nodes are excluded from results."
    )]
    async fn mycelium_get_singly_referenced(
        &self,
        Parameters(req): Parameters<GetSinglyReferencedRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        let limit = req.limit.unwrap_or(10);
        let pairs = {
            let store = self.store.read().await;
            let result = store.singly_referenced(kind, limit);
            drop(store);
            result
        };
        let count = pairs.len();
        let symbols: Vec<serde_json::Value> = pairs
            .into_iter()
            .map(|(path, referenced_by)| {
                serde_json::json!({ "path": path, "referenced_by": referenced_by })
            })
            .collect();
        serde_json::json!({ "symbols": symbols, "count": count }).to_string()
    }

    #[tool(
        description = "Return the union of all symbols that can transitively reach ANY of the \
                       given target paths via incoming EdgeKind edges. Answers: 'if I change these \
                       symbols, what is the total blast radius?' Accepts up to 20 paths; union is \
                       deduplicated; input paths excluded from result. Returns { reachable, count } \
                       or { error } for unknown edge_kind. max_depth defaults to 10, capped at 20."
    )]
    async fn mycelium_batch_reachable_to(
        &self,
        Parameters(req): Parameters<BatchReachableToRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        let max_depth = req.max_depth.unwrap_or(10);
        let reachable = {
            let store = self.store.read().await;
            let ids: Vec<_> = req
                .paths
                .iter()
                .take(20)
                .filter_map(|p| store.lookup(p))
                .collect();
            let result = store.batch_reachable_to(&ids, kind, max_depth);
            drop(store);
            result
        };
        let count = reachable.len();
        serde_json::json!({ "reachable": reachable, "count": count }).to_string()
    }

    #[tool(
        description = "Return the union of all symbols transitively reachable FROM any of the \
                       given source paths via outgoing EdgeKind edges. Answers: 'if these symbols \
                       run, what else do they collectively touch transitively?' Symmetric complement \
                       of mycelium_batch_reachable_to (impact = who depends on me; this = what do \
                       I depend on). Accepts up to 20 paths; union is deduplicated; source paths \
                       excluded from result. Returns { reachable, count } or { error } for unknown \
                       edge_kind. max_depth defaults to 10, capped at 20."
    )]
    async fn mycelium_batch_reachable_from(
        &self,
        Parameters(req): Parameters<BatchReachableFromRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        let max_depth = req.max_depth.unwrap_or(10);
        let reachable = {
            let store = self.store.read().await;
            let ids: Vec<_> = req
                .paths
                .iter()
                .take(20)
                .filter_map(|p| store.lookup(p))
                .collect();
            let result = store.batch_reachable_from(&ids, kind, max_depth);
            drop(store);
            result
        };
        let count = reachable.len();
        serde_json::json!({ "reachable": reachable, "count": count }).to_string()
    }

    #[tool(
        description = "Return the full in/out degree breakdown across all four EdgeKinds for up to \
                       50 symbol paths in a single call. Eliminates N round trips when analysing \
                       a set of related symbols. Known paths return { path, in_calls, out_calls, \
                       in_imports, out_imports, in_extends, out_extends, in_implements, \
                       out_implements }. Unknown paths return { path, error: 'path not found' }. \
                       Results in input order."
    )]
    async fn mycelium_batch_node_degree(
        &self,
        Parameters(req): Parameters<BatchNodeDegreeRequest>,
    ) -> String {
        let store = self.store.read().await;
        let degrees: Vec<serde_json::Value> = req
            .paths
            .iter()
            .take(50)
            .map(|p| {
                store.lookup(p).map_or_else(
                    || serde_json::json!({ "path": p, "error": "path not found" }),
                    |id| {
                        let d = store.node_degree(id);
                        serde_json::json!({
                            "path": p,
                            "in_calls": d.in_calls,
                            "out_calls": d.out_calls,
                            "in_imports": d.in_imports,
                            "out_imports": d.out_imports,
                            "in_extends": d.in_extends,
                            "out_extends": d.out_extends,
                            "in_implements": d.in_implements,
                            "out_implements": d.out_implements,
                        })
                    },
                )
            })
            .collect();
        let count = degrees.len();
        drop(store);
        serde_json::json!({ "degrees": degrees, "count": count }).to_string()
    }

    #[tool(
        description = "Return a topological ordering of the symbol graph for a given EdgeKind \
                       using Kahn's BFS algorithm. Each symbol appears after all its dependencies. \
                       Symbols in directed cycles cannot be ordered and are returned separately in \
                       cycle_members. Useful for build order analysis, initialization sequences, \
                       and layered architecture validation. Returns { order, cycle_members, \
                       ordered_count, cycle_count } or { error } for unknown edge_kind."
    )]
    async fn mycelium_topological_sort(
        &self,
        Parameters(req): Parameters<TopologicalSortRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let TopologicalOrder {
            order,
            cycle_members,
        } = {
            let store = self.store.read().await;
            let r = store.topological_sort(kind);
            drop(store);
            r
        };
        let ordered_count = order.len();
        let cycle_count = cycle_members.len();
        serde_json::json!({
            "order": order,
            "cycle_members": cycle_members,
            "ordered_count": ordered_count,
            "cycle_count": cycle_count,
        })
        .to_string()
    }

    #[tool(
        description = "Find articulation points (cut vertices) in the undirected symbol graph \
                       for a given EdgeKind. An articulation point is a symbol whose removal \
                       would disconnect a connected component. These are single points of \
                       structural failure: if an articulation-point module breaks, parts of the \
                       codebase become unreachable. Uses Tarjan's DFS with low-link values \
                       O(V+E). Returns { points, count } or { error } for unknown edge_kind."
    )]
    async fn mycelium_find_articulation_points(
        &self,
        Parameters(req): Parameters<FindArticulationPointsRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let points = {
            let store = self.store.read().await;
            let p = store.articulation_points(kind);
            drop(store);
            p
        };
        let count = points.len();
        serde_json::json!({ "points": points, "count": count }).to_string()
    }

    #[tool(
        description = "Find bridge edges (cut edges) in the undirected symbol graph for a given \
                       EdgeKind. A bridge is an edge whose removal disconnects its weakly-connected \
                       component — the fragile single-link connection between two subsystems. \
                       Complements mycelium_find_articulation_points (vertex cut-points). \
                       Uses Tarjan's iterative bridge-finding DFS O(V+E). \
                       Returns { bridges: [{ from, to }], count } or { error } for unknown edge_kind."
    )]
    async fn mycelium_find_bridge_edges(
        &self,
        Parameters(req): Parameters<FindBridgeEdgesRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let bridges = {
            let store = self.store.read().await;
            let b = store.bridge_edges(kind);
            drop(store);
            b
        };
        let count = bridges.len();
        let bridge_list: Vec<serde_json::Value> = bridges
            .into_iter()
            .map(|(from, to)| serde_json::json!({ "from": from, "to": to }))
            .collect();
        serde_json::json!({ "bridges": bridge_list, "count": count }).to_string()
    }

    #[tool(
        description = "Partition the undirected symbol graph into biconnected components (BCCs) \
                       for a given EdgeKind. A BCC is a maximal subgraph with no articulation \
                       point — removing any single vertex keeps the BCC connected. BCCs reveal \
                       tightly-coupled code clusters. Bridge edges (RFC-0071) produce 2-node BCCs; \
                       larger BCCs represent cycle-rich cohesive subsystems. \
                       Uses Tarjan's iterative BCC detection O(V+E). \
                       Returns { components, component_count, total_symbols } or { error }."
    )]
    async fn mycelium_get_biconnected_components(
        &self,
        Parameters(req): Parameters<BiconnectedComponentsRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let comps = {
            let store = self.store.read().await;
            let c = store.biconnected_components(kind);
            drop(store);
            c
        };
        let component_count = comps.len();
        let total_symbols: usize = comps.iter().map(Vec::len).sum();
        serde_json::json!({
            "components": comps,
            "component_count": component_count,
            "total_symbols": total_symbols
        })
        .to_string()
    }

    #[tool(
        description = "In- and out-degree frequency distribution for symbol nodes for a given \
                       EdgeKind. Returns { in_degrees: [{degree, count}], out_degrees: [{degree, count}], \
                       total_symbols }. Reveals graph shape: power-law = hub-spoke; uniform = modular. \
                       Degree 0 is always included when symbols have no edges of the given kind. \
                       O(V). Returns { error } for unknown edge_kind."
    )]
    async fn mycelium_get_degree_histogram(
        &self,
        Parameters(req): Parameters<DegreeHistogramRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let hist = {
            let store = self.store.read().await;
            let h = store.degree_histogram(kind);
            drop(store);
            h
        };
        let total_symbols: u64 = hist.in_degrees.iter().map(|&(_, c)| c).sum();
        let in_list: Vec<serde_json::Value> = hist
            .in_degrees
            .iter()
            .map(|&(d, c)| serde_json::json!({ "degree": d, "count": c }))
            .collect();
        let out_list: Vec<serde_json::Value> = hist
            .out_degrees
            .iter()
            .map(|&(d, c)| serde_json::json!({ "degree": d, "count": c }))
            .collect();
        serde_json::json!({
            "in_degrees": in_list,
            "out_degrees": out_list,
            "total_symbols": total_symbols
        })
        .to_string()
    }

    #[tool(
        description = "Structural summary metrics for the symbol graph for a given EdgeKind: \
                       symbol_count, directed_edge_count, density (E/V(V-1)), avg_degree, \
                       max_in_degree, max_out_degree. Instant architectural health check: \
                       density near 0 = sparse/modular; near 1 = tightly coupled. \
                       O(V+E). Returns { error } for unknown edge_kind."
    )]
    async fn mycelium_get_graph_metrics(
        &self,
        Parameters(req): Parameters<GraphMetricsRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let m = {
            let store = self.store.read().await;
            let metrics = store.graph_metrics(kind);
            drop(store);
            metrics
        };
        serde_json::json!({
            "symbol_count": m.symbol_count,
            "directed_edge_count": m.directed_edge_count,
            "density": m.density,
            "avg_degree": m.avg_degree,
            "max_in_degree": m.max_in_degree,
            "max_out_degree": m.max_out_degree,
        })
        .to_string()
    }

    #[tool(
        description = "Jaccard similarity between the combined neighbor sets of two symbol nodes \
                       for a given EdgeKind. N(x) = all outgoing + incoming neighbors of x for \
                       kind; similarity = |N(u)∩N(v)| / |N(u)∪N(v)|. 1.0 = identical structural \
                       roles (same callers+callees); 0.0 = no overlap. Both isolated nodes → 0.0. \
                       Useful for refactoring candidates and duplicate detection. O(max_degree). \
                       Returns { similarity, shared, total } or { error }."
    )]
    async fn mycelium_get_neighbor_similarity(
        &self,
        Parameters(req): Parameters<NeighborSimilarityRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let store = self.store.read().await;
        let Some(id1) = store.lookup(&req.path1) else {
            return serde_json::json!({ "error": format!("unknown path: {}", req.path1) })
                .to_string();
        };
        let Some(id2) = store.lookup(&req.path2) else {
            return serde_json::json!({ "error": format!("unknown path: {}", req.path2) })
                .to_string();
        };
        let (similarity, shared, total) = store.neighbor_similarity_stats(id1, id2, kind);
        drop(store);
        serde_json::json!({
            "similarity": similarity,
            "shared": shared,
            "total": total,
        })
        .to_string()
    }

    #[tool(
        description = "Local clustering coefficient for a symbol node and a given EdgeKind. \
                       CC(u) = #{directed edges among N(u)} / (|N(u)| * (|N(u)|-1)), where \
                       N(u) = outgoing ∪ incoming neighbors (self and file nodes excluded). \
                       Returns 0.0 when |N(u)| < 2. Score 1.0 = every neighbor calls every other \
                       neighbor (maximum local density); 0.0 = no two neighbors are connected. \
                       High CC identifies nodes embedded in tightly-coupled clusters. O(degree²). \
                       Returns { coefficient, neighbor_count, neighbor_edge_count } or { error }."
    )]
    async fn mycelium_get_clustering_coefficient(
        &self,
        Parameters(req): Parameters<ClusteringCoefficientRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let store = self.store.read().await;
        let Some(id) = store.lookup(&req.path) else {
            return serde_json::json!({ "error": format!("unknown path: {}", req.path) })
                .to_string();
        };
        let (coefficient, neighbor_count, neighbor_edge_count) =
            store.clustering_coefficient_stats(id, kind);
        drop(store);
        serde_json::json!({
            "coefficient": coefficient,
            "neighbor_count": neighbor_count,
            "neighbor_edge_count": neighbor_edge_count,
        })
        .to_string()
    }

    #[tool(
        description = "Eccentricity of a symbol node: maximum BFS distance from the node to any \
                       reachable symbol node for a given EdgeKind. Measures 'how deep is this \
                       node's directed reach?'. Isolated node or no outgoing reachability → 0. \
                       File nodes excluded from traversal and count. O(V+E). \
                       Returns { eccentricity, reachable_count } or { error }."
    )]
    async fn mycelium_get_eccentricity(
        &self,
        Parameters(req): Parameters<EccentricityRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let store = self.store.read().await;
        let Some(id) = store.lookup(&req.path) else {
            return serde_json::json!({ "error": format!("unknown path: {}", req.path) })
                .to_string();
        };
        let (eccentricity, reachable_count) = store.eccentricity_stats(id, kind);
        drop(store);
        serde_json::json!({
            "eccentricity": eccentricity,
            "reachable_count": reachable_count,
        })
        .to_string()
    }

    #[tool(
        description = "Harmonic centrality of a symbol node: (1/(n-1)) × Σ_{v reachable} (1/d(v)) \
                       for a given EdgeKind. Unreachable nodes contribute 0, making this \
                       well-defined for directed graphs. Near 1.0 = reaches all others in ~1 hop; \
                       0.0 = isolated. Complements eccentricity (max distance) with average \
                       closeness. n = total symbol count (file nodes excluded). O(V+E). \
                       Returns { harmonic_centrality, reachable_count, symbol_count } or { error }."
    )]
    async fn mycelium_get_harmonic_centrality(
        &self,
        Parameters(req): Parameters<HarmonicCentralityRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let store = self.store.read().await;
        let Some(id) = store.lookup(&req.path) else {
            return serde_json::json!({ "error": format!("unknown path: {}", req.path) })
                .to_string();
        };
        let (harmonic_centrality, reachable_count, symbol_count) =
            store.harmonic_centrality_stats(id, kind);
        drop(store);
        serde_json::json!({
            "harmonic_centrality": harmonic_centrality,
            "reachable_count": reachable_count,
            "symbol_count": symbol_count,
        })
        .to_string()
    }

    #[tool(
        description = "Check bidirectional reachability between two symbol nodes for a given \
                       EdgeKind. Returns forward BFS distance (id1→id2), backward BFS distance \
                       (id2→id1), and derived flags: forward, backward, mutual. \
                       Same node → both distances 0, both directions true. \
                       Returns { forward, backward, mutual, forward_distance, backward_distance } \
                       or { error }."
    )]
    async fn mycelium_get_mutual_reachability(
        &self,
        Parameters(req): Parameters<MutualReachabilityRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let store = self.store.read().await;
        let Some(id1) = store.lookup(&req.path1) else {
            return serde_json::json!({ "error": format!("unknown path: {}", req.path1) })
                .to_string();
        };
        let Some(id2) = store.lookup(&req.path2) else {
            return serde_json::json!({ "error": format!("unknown path: {}", req.path2) })
                .to_string();
        };
        let result = store.mutual_reachability(id1, id2, kind);
        drop(store);
        serde_json::json!({
            "forward": result.forward,
            "backward": result.backward,
            "mutual": result.mutual,
            "forward_distance": result.forward_distance,
            "backward_distance": result.backward_distance,
        })
        .to_string()
    }

    #[tool(
        description = "Return all symbol paths transitively reachable from a given node via a \
                       given EdgeKind (full BFS transitive closure). Answers 'what does this \
                       symbol transitively call/import/extend?'. The source node itself is \
                       excluded. Results sorted alphabetically. File nodes excluded. O(V+E). \
                       Returns { reachable, count } or { error }."
    )]
    async fn mycelium_get_reachable_set(
        &self,
        Parameters(req): Parameters<ReachableSetRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let store = self.store.read().await;
        let Some(id) = store.lookup(&req.path) else {
            return serde_json::json!({ "error": format!("unknown path: {}", req.path) })
                .to_string();
        };
        let reachable = store.reachable_set(id, kind);
        drop(store);
        let count = reachable.len();
        serde_json::json!({
            "reachable": reachable,
            "count": count,
        })
        .to_string()
    }

    #[tool(
        description = "Return all symbol paths that can transitively reach a given node via a \
                       given EdgeKind (reverse BFS transitive closure). Answers 'what symbols \
                       transitively call/import/extend this one?'. The target node itself is \
                       excluded. Results sorted alphabetically. File nodes excluded. O(V+E). \
                       Returns { callers, count } or { error }."
    )]
    async fn mycelium_get_reaches_into(
        &self,
        Parameters(req): Parameters<ReachesIntoRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let store = self.store.read().await;
        let Some(id) = store.lookup(&req.path) else {
            return serde_json::json!({ "error": format!("unknown path: {}", req.path) })
                .to_string();
        };
        let callers = store.reaches_into(id, kind);
        drop(store);
        let count = callers.len();
        serde_json::json!({
            "callers": callers,
            "count": count,
        })
        .to_string()
    }

    #[tool(
        description = "Return the intersection of the transitive reachable sets of two symbol nodes \
                       for a given EdgeKind. Answers 'what symbols do both of these nodes \
                       transitively call/import/extend?'. Useful for finding shared dependencies, \
                       impact analysis, and refactoring candidates. Results sorted alphabetically. \
                       File nodes excluded. O(V+E). \
                       Returns { common, count } or { error }."
    )]
    async fn mycelium_get_common_reachable(
        &self,
        Parameters(req): Parameters<CommonReachableRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let store = self.store.read().await;
        let Some(id1) = store.lookup(&req.path1) else {
            return serde_json::json!({ "error": format!("unknown path: {}", req.path1) })
                .to_string();
        };
        let Some(id2) = store.lookup(&req.path2) else {
            return serde_json::json!({ "error": format!("unknown path: {}", req.path2) })
                .to_string();
        };
        let common = store.common_reachable(id1, id2, kind);
        drop(store);
        let count = common.len();
        serde_json::json!({
            "common": common,
            "count": count,
        })
        .to_string()
    }

    #[tool(
        description = "Return all symbol paths reachable from a node in exactly k BFS hops for a \
                       given EdgeKind. Only returns nodes at exactly depth k — nodes closer than k \
                       are excluded. k=0 returns empty; k=1 returns direct neighbors. \
                       File nodes excluded. Results sorted alphabetically. O(V+E). \
                       Returns { neighbors, count, k } or { error }."
    )]
    async fn mycelium_get_k_hop_neighbors(
        &self,
        Parameters(req): Parameters<KHopNeighborsRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let store = self.store.read().await;
        let Some(id) = store.lookup(&req.path) else {
            return serde_json::json!({ "error": format!("unknown path: {}", req.path) })
                .to_string();
        };
        let neighbors = store.k_hop_neighbors(id, kind, req.k);
        drop(store);
        let count = neighbors.len();
        serde_json::json!({
            "neighbors": neighbors,
            "count": count,
            "k": req.k,
        })
        .to_string()
    }

    #[tool(
        description = "Compute normalized betweenness centrality for all symbol nodes using \
                       Brandes' algorithm. Identifies bridge nodes that lie on many shortest \
                       dependency paths — bottlenecks whose change ripples through many call \
                       chains. Score ∈ [0.0, 1.0]; normalized by (n-1)*(n-2). \
                       File nodes excluded. O(V*(V+E)). \
                       Returns { nodes: [{path, score}], symbol_count, top_n } or { error }."
    )]
    async fn mycelium_get_betweenness_centrality(
        &self,
        Parameters(req): Parameters<BetweennessCentralityRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let top_n = req.top_n.unwrap_or(10);
        let store = self.store.read().await;
        let entries = store.betweenness_centrality(kind);
        let symbol_count = entries.len();
        drop(store);
        let nodes: Vec<serde_json::Value> = entries
            .into_iter()
            .take(top_n)
            .map(|e| serde_json::json!({ "path": e.path, "score": e.score }))
            .collect();
        serde_json::json!({
            "nodes": nodes,
            "symbol_count": symbol_count,
            "top_n": top_n,
        })
        .to_string()
    }

    #[tool(
        description = "Compute PageRank scores for all symbol nodes. Identifies globally important \
                       symbols: entry points, heavily-called utilities, and hub nodes. Uses the \
                       iterative power method with configurable damping (default 0.85) and \
                       iterations (default 20). Returns top_n results (default 10) sorted \
                       descending by score. File nodes excluded. \
                       Returns { nodes: [{path, score}], symbol_count, top_n } or { error }."
    )]
    async fn mycelium_page_rank(&self, Parameters(req): Parameters<PageRankRequest>) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let damping = req.damping.unwrap_or(0.85);
        let iterations = req.iterations.unwrap_or(20);
        let top_n = req.top_n.unwrap_or(10);
        let store = self.store.read().await;
        let entries = store.page_rank(kind, damping, iterations);
        let symbol_count = entries.len();
        drop(store);
        let nodes: Vec<serde_json::Value> = entries
            .into_iter()
            .take(top_n)
            .map(|e| serde_json::json!({ "path": e.path, "score": e.score }))
            .collect();
        serde_json::json!({
            "nodes": nodes,
            "symbol_count": symbol_count,
            "top_n": top_n,
        })
        .to_string()
    }

    #[tool(
        description = "Group symbol nodes into weakly-connected components (WCCs) for a given \
                       EdgeKind, treating edges as undirected. WCCs surface isolated clusters: \
                       self-contained subsystems, orphaned utilities, or modules with no \
                       dependency links to the rest of the codebase. Complements SCC (directed \
                       mutual reachability). Use min_size=2 to hide singleton isolated nodes. \
                       Returns { components, component_count, total_symbols } or { error }."
    )]
    async fn mycelium_get_wcc(&self, Parameters(req): Parameters<GetWccRequest>) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let min_size = req.min_size.unwrap_or(1).max(1);
        let components: Vec<Vec<String>> = {
            let store = self.store.read().await;
            let all = store.weakly_connected_components(kind);
            drop(store);
            all.into_iter().filter(|c| c.len() >= min_size).collect()
        };
        let component_count = components.len();
        let total_symbols: usize = components.iter().map(Vec::len).sum();
        serde_json::json!({
            "components": components,
            "component_count": component_count,
            "total_symbols": total_symbols,
        })
        .to_string()
    }

    #[tool(
        description = "Return all symbol nodes that participate in at least one directed cycle for \
                       a given EdgeKind. Uses Kosaraju's SCC algorithm: any node in a strongly- \
                       connected component of size ≥ 2 is a cycle member. Useful for detecting \
                       circular imports, mutually-recursive functions, or inheritance cycles. \
                       Returns { members, count } or { error } for unknown edge_kind."
    )]
    async fn mycelium_find_cycle_members(
        &self,
        Parameters(req): Parameters<FindCycleMembersRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let members = {
            let store = self.store.read().await;
            let m = store.cycle_members(kind);
            drop(store);
            m
        };
        let count = members.len();
        serde_json::json!({ "members": members, "count": count }).to_string()
    }

    #[tool(
        description = "Return the k-core of the symbol graph for a given EdgeKind — the maximal \
                       induced subgraph where every node has total degree (in + out within the \
                       subgraph) ≥ k. Identifies the tightly-interconnected core that is hardest \
                       to refactor. k defaults to 2. k=0 returns all symbols. Returns { core, \
                       count, k } or { error } for unknown edge_kind."
    )]
    async fn mycelium_get_k_core(&self, Parameters(req): Parameters<GetKCoreRequest>) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        let k = req.k.unwrap_or(2);
        let core = {
            let store = self.store.read().await;
            let result = store.k_core(kind, k);
            drop(store);
            result
        };
        let count = core.len();
        serde_json::json!({ "core": core, "count": count, "k": k }).to_string()
    }

    #[tool(
        description = "Return the top-N symbols ranked by incoming Calls edge count (most-called \
                       first). Useful for identifying architectural hot spots, widely-used \
                       utilities, and high-coupling functions. limit defaults to 10, capped at 100. \
                       Symbols with no callers are excluded."
    )]
    async fn mycelium_rank_symbols(
        &self,
        Parameters(req): Parameters<RankSymbolsRequest>,
    ) -> String {
        let limit = req.limit.unwrap_or(10).min(100);
        let ranked = self.store.read().await.top_callee_symbols(limit);
        let symbols: Vec<serde_json::Value> = ranked
            .into_iter()
            .map(|(path, caller_count)| {
                serde_json::json!({ "path": path, "caller_count": caller_count })
            })
            .collect();
        serde_json::json!({ "symbols": symbols }).to_string()
    }

    #[tool(
        description = "Return top-N source files ranked by direct symbol count (god-file detector). \
                       limit defaults to 10, capped at 100. Files with no direct symbols are excluded. \
                       Returns { files: [{ path, symbol_count }, ...], count: N }."
    )]
    async fn mycelium_get_top_files(
        &self,
        Parameters(req): Parameters<GetTopFilesRequest>,
    ) -> String {
        let limit = req.limit.unwrap_or(10);
        let entries = self.store.read().await.top_files(limit);
        let count = entries.len();
        let files: Vec<serde_json::Value> = entries
            .into_iter()
            .map(|(path, symbol_count)| {
                serde_json::json!({ "path": path, "symbol_count": symbol_count })
            })
            .collect();
        serde_json::json!({ "files": files, "count": count }).to_string()
    }

    #[tool(
        description = "Return top-N symbol nodes ranked by total degree (in + out) for a given \
                       edge kind — hub-node detector for any EdgeKind. \
                       edge_kind must be 'calls', 'imports', 'extends', or 'implements'. \
                       limit defaults to 10, capped at 100. Nodes with degree 0 are excluded. \
                       Returns { symbols: [{ path, degree }, ...], count } or { error }."
    )]
    async fn mycelium_get_most_connected(
        &self,
        Parameters(req): Parameters<GetMostConnectedRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        let limit = req.limit.unwrap_or(10);
        let entries = self.store.read().await.most_connected(limit, kind);
        let count = entries.len();
        let symbols: Vec<serde_json::Value> = entries
            .into_iter()
            .map(|(path, degree)| serde_json::json!({ "path": path, "degree": degree }))
            .collect();
        serde_json::json!({ "symbols": symbols, "count": count }).to_string()
    }

    #[tool(
        description = "Return symbol nodes with out-degree 0 for a given edge kind — leaf \
                       implementations that call/import nothing for that kind. \
                       Symmetric complement to mycelium_get_entry_points (in-degree 0). \
                       edge_kind must be 'calls', 'imports', 'extends', or 'implements'. \
                       limit defaults to 10, capped at 100. \
                       Returns { symbols: [...], count } or { error } for unknown edge_kind."
    )]
    async fn mycelium_get_leaf_symbols(
        &self,
        Parameters(req): Parameters<GetLeafSymbolsRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        let limit = req.limit.unwrap_or(10);
        let symbols = self.store.read().await.leaf_symbols(kind, limit);
        let count = symbols.len();
        serde_json::json!({ "symbols": symbols, "count": count }).to_string()
    }

    #[tool(
        description = "BFS shortest path between two symbol nodes along edges of a given edge kind. \
                       Returns { path: [...], length: N } if a path exists, \
                       { path: null, length: null } if no path, \
                       or { error } for unknown edge_kind or unrecognised from/to paths."
    )]
    async fn mycelium_get_shortest_path(
        &self,
        Parameters(req): Parameters<GetShortestPathRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        // Two lookups then shortest_path — hold read guard for the whole block.
        #[allow(clippy::significant_drop_tightening)]
        let path_opt: Result<Option<Vec<String>>, String> = {
            let store = self.store.read().await;
            let Some(from_id) = store.lookup(&req.from) else {
                return serde_json::json!({ "error": format!("path not found: {}", req.from) })
                    .to_string();
            };
            let Some(to_id) = store.lookup(&req.to) else {
                return serde_json::json!({ "error": format!("path not found: {}", req.to) })
                    .to_string();
            };
            Ok(store.shortest_path(from_id, to_id, kind))
        };
        path_opt.unwrap().map_or_else(
            || serde_json::json!({ "path": null, "length": null }).to_string(),
            |p| {
                let length = p.len() - 1;
                serde_json::json!({ "path": p, "length": length }).to_string()
            },
        )
    }

    #[tool(description = "Return a per-kind breakdown of indexed symbol counts. \
                       Answers 'what is this codebase made of?' — how many functions, \
                       classes, methods, interfaces, etc. Only nodes with a recorded \
                       NodeKind are counted. \
                       Returns { kinds: [{ kind, count }], total }.")]
    async fn mycelium_get_symbol_count_by_kind(
        &self,
        Parameters(_req): Parameters<GetSymbolCountByKindRequest>,
    ) -> String {
        let counts = self.store.read().await.symbol_count_by_kind();
        let total: usize = counts.iter().map(|(_, n)| n).sum();
        let kinds: Vec<serde_json::Value> = counts
            .into_iter()
            .map(|(kind, count)| serde_json::json!({ "kind": kind, "count": count }))
            .collect();
        serde_json::json!({ "kinds": kinds, "total": total }).to_string()
    }

    #[tool(
        description = "Return symbol nodes that are incoming neighbours for ALL of the given \
                       target paths via the specified edge kind — set intersection of each \
                       target's in-neighbour set. \
                       Useful for finding shared callers, shared importers, etc. \
                       edge_kind: 'calls', 'imports', 'extends', or 'implements'. \
                       Returns { callers, count } or { error }."
    )]
    async fn mycelium_get_common_callers(
        &self,
        Parameters(req): Parameters<GetCommonCallersRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        if req.paths.is_empty() {
            return serde_json::json!({ "callers": [], "count": 0 }).to_string();
        }
        let callers_opt: Result<Vec<String>, String> = {
            let store = self.store.read().await;
            let mut ids = Vec::with_capacity(req.paths.len());
            for p in &req.paths {
                let Some(id) = store.lookup(p) else {
                    return serde_json::json!({ "error": format!("path not found: {p}") })
                        .to_string();
                };
                ids.push(id);
            }
            Ok(store.common_callers(&ids, kind))
        };
        let callers = callers_opt.unwrap();
        let count = callers.len();
        serde_json::json!({ "callers": callers, "count": count }).to_string()
    }

    #[tool(
        description = "Return symbol nodes that are outgoing neighbours for ALL of the given \
                       source paths via the specified edge kind — set intersection of each \
                       source's out-neighbour set. Symmetric complement to common_callers. \
                       Useful for finding shared callees, shared imports, common base classes, etc. \
                       edge_kind: 'calls', 'imports', 'extends', or 'implements'. \
                       Returns { callees, count } or { error }."
    )]
    async fn mycelium_get_common_callees(
        &self,
        Parameters(req): Parameters<GetCommonCalleesRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        if req.paths.is_empty() {
            return serde_json::json!({ "callees": [], "count": 0 }).to_string();
        }
        let callees_opt: Result<Vec<String>, String> = {
            let store = self.store.read().await;
            let mut ids = Vec::with_capacity(req.paths.len());
            for p in &req.paths {
                let Some(id) = store.lookup(p) else {
                    return serde_json::json!({ "error": format!("path not found: {p}") })
                        .to_string();
                };
                ids.push(id);
            }
            Ok(store.common_callees(&ids, kind))
        };
        let callees = callees_opt.unwrap();
        let count = callees.len();
        serde_json::json!({ "callees": callees, "count": count }).to_string()
    }

    #[tool(
        description = "Return top-N symbol nodes ranked by out-degree (outgoing edge count) for \
                       a given edge kind — orchestrator/high-fan-out detector. \
                       Complements rank_symbols (in-degree) and most_connected (total degree). \
                       edge_kind: 'calls', 'imports', 'extends', or 'implements'. \
                       limit defaults to 10, capped at 100. Nodes with out-degree 0 excluded. \
                       Returns { symbols: [{ path, out_degree }], count } or { error }."
    )]
    async fn mycelium_get_fan_out_rank(
        &self,
        Parameters(req): Parameters<GetFanOutRankRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        let limit = req.limit.unwrap_or(10);
        let entries = self.store.read().await.fan_out_rank(kind, limit);
        let count = entries.len();
        let symbols: Vec<serde_json::Value> = entries
            .into_iter()
            .map(|(path, out_degree)| serde_json::json!({ "path": path, "out_degree": out_degree }))
            .collect();
        serde_json::json!({ "symbols": symbols, "count": count }).to_string()
    }

    #[tool(
        description = "Return top-N symbol nodes ranked by in-degree (incoming edge count) for \
                       a given edge kind — hotspot/high-fan-in detector. \
                       Complements fan_out_rank (out-degree) and most_connected (total degree). \
                       edge_kind: 'calls', 'imports', 'extends', or 'implements'. \
                       limit defaults to 10, capped at 100. Nodes with in-degree 0 excluded. \
                       Returns { symbols: [{ path, in_degree }], count } or { error }."
    )]
    async fn mycelium_get_fan_in_rank(
        &self,
        Parameters(req): Parameters<GetFanInRankRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge_kind: {other}") })
                    .to_string();
            }
        };
        let limit = req.limit.unwrap_or(10);
        let entries = self.store.read().await.fan_in_rank(kind, limit);
        let count = entries.len();
        let symbols: Vec<serde_json::Value> = entries
            .into_iter()
            .map(|(path, in_degree)| serde_json::json!({ "path": path, "in_degree": in_degree }))
            .collect();
        serde_json::json!({ "symbols": symbols, "count": count }).to_string()
    }

    #[tool(
        description = "Return all source files currently in the index as a sorted list of trunk \
                       paths. An optional path_prefix filters results to files whose path starts \
                       with the given string (e.g. \"src/\")."
    )]
    async fn mycelium_get_files(&self, Parameters(req): Parameters<GetFilesRequest>) -> String {
        let files = self.store.read().await.all_file_paths();
        let files: Vec<String> = match req.path_prefix {
            None => files,
            Some(ref prefix) => files
                .into_iter()
                .filter(|p| p.starts_with(prefix.as_str()))
                .collect(),
        };
        serde_json::json!({ "files": files }).to_string()
    }

    #[tool(
        description = "Find the shortest call path from one symbol to another using BFS over \
                       Calls edges. Returns the path as an ordered list of trunk paths (including \
                       both endpoints) and the number of hops. Returns an empty path with \
                       hops=null if unreachable within max_depth. max_depth defaults to 10, \
                       capped at 20."
    )]
    async fn mycelium_find_call_path(
        &self,
        Parameters(req): Parameters<FindCallPathRequest>,
    ) -> String {
        let max_depth = req.max_depth.unwrap_or(10).min(20);
        let store_guard = self.store.read().await;
        let Some(from_id) = store_guard.lookup(&req.from_path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.from_path) })
                .to_string();
        };
        let Some(to_id) = store_guard.lookup(&req.to_path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.to_path) })
                .to_string();
        };
        let maybe_path = store_guard.find_call_path(from_id, to_id, max_depth);
        let path_strings: Option<Vec<String>> = maybe_path.as_ref().map(|ids| {
            ids.iter()
                .filter_map(|&id| store_guard.path_of(id).map(str::to_owned))
                .collect()
        });
        drop(store_guard);
        path_strings.map_or_else(
            || {
                serde_json::json!({
                    "path": [],
                    "hops": serde_json::Value::Null,
                    "message": format!("no call path found within depth {max_depth}"),
                })
                .to_string()
            },
            |path| {
                let hops = path.len().saturating_sub(1);
                serde_json::json!({ "path": path, "hops": hops }).to_string()
            },
        )
    }

    #[tool(
        description = "Find the shortest import-dependency path between two paths via BFS over \
                       Imports edges. Returns { path, hops } on success or \
                       { path: [], hops: null, message } when unreachable. \
                       max_depth defaults to 8, capped at 20. Unknown paths return { error }."
    )]
    async fn mycelium_find_import_path(
        &self,
        Parameters(req): Parameters<FindImportPathRequest>,
    ) -> String {
        let max_depth = req.max_depth.unwrap_or(8).min(20);
        let store_guard = self.store.read().await;
        let Some(from_id) = store_guard.lookup(&req.from_path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.from_path) })
                .to_string();
        };
        let Some(to_id) = store_guard.lookup(&req.to_path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.to_path) })
                .to_string();
        };
        let maybe_path = store_guard.find_import_path(from_id, to_id, max_depth);
        let path_strings: Option<Vec<String>> = maybe_path.as_ref().map(|ids| {
            ids.iter()
                .filter_map(|&id| store_guard.path_of(id).map(str::to_owned))
                .collect()
        });
        drop(store_guard);
        path_strings.map_or_else(
            || {
                serde_json::json!({
                    "path": [],
                    "hops": serde_json::Value::Null,
                    "message": format!("no import path found within max_depth={max_depth}"),
                })
                .to_string()
            },
            |path| {
                let hops = path.len().saturating_sub(1);
                serde_json::json!({ "path": path, "hops": hops }).to_string()
            },
        )
    }

    #[tool(
        description = "Find the shortest extends (inheritance) chain between two symbols via BFS \
                       over Extends edges. Returns { path, hops } where path is the ordered list \
                       of symbol paths from from_path to to_path inclusive, and hops is the number \
                       of edges. Returns { path: [], hops: null, message } when unreachable. \
                       Unknown paths return { error }. max_depth defaults to 8, capped at 20."
    )]
    async fn mycelium_find_extends_path(
        &self,
        Parameters(req): Parameters<FindExtendsPathRequest>,
    ) -> String {
        let max_depth = req.max_depth.unwrap_or(8).min(20);
        let store_guard = self.store.read().await;
        let Some(from_id) = store_guard.lookup(&req.from_path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.from_path) })
                .to_string();
        };
        let Some(to_id) = store_guard.lookup(&req.to_path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.to_path) })
                .to_string();
        };
        let maybe_path = store_guard.find_extends_path(from_id, to_id, max_depth);
        let path_strings: Option<Vec<String>> = maybe_path.as_ref().map(|ids| {
            ids.iter()
                .filter_map(|&id| store_guard.path_of(id).map(str::to_owned))
                .collect()
        });
        drop(store_guard);
        path_strings.map_or_else(
            || {
                serde_json::json!({
                    "path": [],
                    "hops": serde_json::Value::Null,
                    "message": format!("no extends path found within max_depth={max_depth}"),
                })
                .to_string()
            },
            |path| {
                let hops = path.len().saturating_sub(1);
                serde_json::json!({ "path": path, "hops": hops }).to_string()
            },
        )
    }

    #[tool(
        description = "Return the depth-limited superclass tree for a symbol, following outgoing \
                       Extends edges. The result is { root: { path, parents: [...] } } where \
                       each node has a path and its own parents list. Cycles are cut as leaf \
                       nodes. max_depth defaults to 4, capped at 10. Unknown path returns { error }."
    )]
    async fn mycelium_get_extends_tree(
        &self,
        Parameters(req): Parameters<GetExtendsTreeRequest>,
    ) -> String {
        let max_depth = req.max_depth.unwrap_or(4).min(10);
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        let tree = store_guard.extends_tree(id, max_depth);
        let json = extends_node_to_json(&tree, &store_guard);
        drop(store_guard);
        serde_json::json!({ "root": json }).to_string()
    }

    #[tool(
        description = "Return the depth-limited subclass forest for a symbol, following incoming \
                       Extends edges. The result is { root: { path, subclasses: [...] } } where \
                       each node has a path and its own subclasses list. Cycles are cut as leaf \
                       nodes. max_depth defaults to 4, capped at 10. Unknown path returns { error }."
    )]
    async fn mycelium_get_subclasses_tree(
        &self,
        Parameters(req): Parameters<GetSubclassesTreeRequest>,
    ) -> String {
        let max_depth = req.max_depth.unwrap_or(4).min(10);
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        let tree = store_guard.subclasses_tree(id, max_depth);
        let json = subclass_node_to_json(&tree, &store_guard);
        drop(store_guard);
        serde_json::json!({ "root": json }).to_string()
    }

    #[tool(
        description = "Find the shortest implements chain between two symbols via BFS over \
                       outgoing Implements edges. Returns { path: [path, ...], hops: N } if \
                       reachable, or { path: [], hops: null, message } if not. Unknown \
                       from_path or to_path returns { error }. max_depth defaults to 8, capped at 20."
    )]
    async fn mycelium_find_implements_path(
        &self,
        Parameters(req): Parameters<FindImplementsPathRequest>,
    ) -> String {
        let max_depth = req.max_depth.unwrap_or(8).min(20);
        let store_guard = self.store.read().await;
        let Some(from_id) = store_guard.lookup(&req.from_path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.from_path) })
                .to_string();
        };
        let Some(to_id) = store_guard.lookup(&req.to_path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.to_path) })
                .to_string();
        };
        if let Some(ids) = store_guard.find_implements_path(from_id, to_id, max_depth) {
            let path: Vec<String> = ids
                .iter()
                .map(|&id| store_guard.path_of(id).unwrap_or("<unknown>").to_owned())
                .collect();
            let hops = path.len() - 1;
            drop(store_guard);
            serde_json::json!({ "path": path, "hops": hops }).to_string()
        } else {
            drop(store_guard);
            serde_json::json!({
                "path": [],
                "hops": null,
                "message": format!("no implements path found within max_depth={max_depth}")
            })
            .to_string()
        }
    }

    #[tool(
        description = "Return the depth-limited interface tree for a symbol, following outgoing \
                       Implements edges. The result is { root: { path, interfaces: [...] } } \
                       where each node has a path and its own interfaces list. Cycles are cut \
                       as leaf nodes. max_depth defaults to 4, capped at 10. Unknown path \
                       returns { error }."
    )]
    async fn mycelium_get_implements_tree(
        &self,
        Parameters(req): Parameters<GetImplementsTreeRequest>,
    ) -> String {
        let max_depth = req.max_depth.unwrap_or(4).min(10);
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        let tree = store_guard.implements_tree(id, max_depth);
        let json = implements_node_to_json(&tree, &store_guard);
        drop(store_guard);
        serde_json::json!({ "root": json }).to_string()
    }

    #[tool(
        description = "Return the depth-limited implementor forest for an interface, following \
                       incoming Implements edges. The result is { root: { path, implementors: \
                       [...] } } where each node has a path and its own implementors list. \
                       Cycles are cut as leaf nodes. max_depth defaults to 4, capped at 10. \
                       Unknown path returns { error }."
    )]
    async fn mycelium_get_implementors_tree(
        &self,
        Parameters(req): Parameters<GetImplementorsTreeRequest>,
    ) -> String {
        let max_depth = req.max_depth.unwrap_or(4).min(10);
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        let tree = store_guard.implementors_tree(id, max_depth);
        let json = implementor_node_to_json(&tree, &store_guard);
        drop(store_guard);
        serde_json::json!({ "root": json }).to_string()
    }

    #[tool(
        description = "Return the depth-limited reverse-dependency forest for a module, following \
                       incoming Imports edges. The result is { root: { path, importers: [...] } } \
                       where each node has a path and its own importers list. Cycles are cut as \
                       leaf nodes. max_depth defaults to 4, capped at 10. Unknown path returns \
                       { error }."
    )]
    async fn mycelium_get_importers_tree(
        &self,
        Parameters(req): Parameters<GetImportersTreeRequest>,
    ) -> String {
        let max_depth = req.max_depth.unwrap_or(4).min(10);
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return serde_json::json!({ "error": format!("path not found: {}", req.path) })
                .to_string();
        };
        let tree = store_guard.importers_tree(id, max_depth);
        let json = importer_node_to_json(&tree, &store_guard);
        drop(store_guard);
        serde_json::json!({ "root": json }).to_string()
    }

    #[tool(
        description = "Compute Wasserman-Faust normalized closeness centrality for all symbol nodes. \
                       Identifies well-connected hubs that can propagate influence quickly through \
                       the dependency graph. Score = (n_reach/(n-1))^2 * (n_reach/sum_dist). \
                       Returns { nodes: [{path, score}], symbol_count, top_n }. \
                       Unknown edge_kind returns { error }."
    )]
    async fn mycelium_get_closeness_centrality(
        &self,
        Parameters(req): Parameters<ClosenessCentralityRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let top_n = req.top_n.unwrap_or(10);
        let store = self.store.read().await;
        let entries = store.closeness_centrality(kind);
        let symbol_count = entries.len();
        drop(store);
        let nodes: Vec<serde_json::Value> = entries
            .into_iter()
            .take(top_n)
            .map(|e| serde_json::json!({ "path": e.path, "score": e.score }))
            .collect();
        serde_json::json!({
            "nodes": nodes,
            "symbol_count": symbol_count,
            "top_n": top_n,
        })
        .to_string()
    }

    #[tool(
        description = "Return normalized in-degree and out-degree centrality scores for all symbol \
                       nodes. In-degree centrality identifies widely-used dependencies (fan-in hubs); \
                       out-degree centrality identifies symbols with a wide surface area (fan-out hubs). \
                       Returns { nodes: [{path, in_degree, out_degree, in_centrality, out_centrality}], \
                       symbol_count, top_n, sort_by }. sort_by: 'in' (default) or 'out'. \
                       Unknown edge_kind or sort_by returns { error }."
    )]
    async fn mycelium_get_degree_centrality(
        &self,
        Parameters(req): Parameters<DegreeCentralityRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let sort_by = req.sort_by.as_deref().unwrap_or("in");
        if sort_by != "in" && sort_by != "out" {
            return serde_json::json!({ "error": format!("unknown sort_by: {sort_by}") })
                .to_string();
        }
        let top_n = req.top_n.unwrap_or(10);
        let store = self.store.read().await;
        let mut entries = store.degree_centrality(kind);
        let symbol_count = entries.len();
        drop(store);
        if sort_by == "out" {
            entries.sort_by(|a, b| {
                b.out_centrality
                    .partial_cmp(&a.out_centrality)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        b.in_centrality
                            .partial_cmp(&a.in_centrality)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .then_with(|| a.path.cmp(&b.path))
            });
        }
        let nodes: Vec<serde_json::Value> = entries
            .into_iter()
            .take(top_n)
            .map(|e| {
                serde_json::json!({
                    "path": e.path,
                    "in_degree": e.in_degree,
                    "out_degree": e.out_degree,
                    "in_centrality": e.in_centrality,
                    "out_centrality": e.out_centrality,
                })
            })
            .collect();
        serde_json::json!({
            "nodes": nodes,
            "symbol_count": symbol_count,
            "top_n": top_n,
            "sort_by": sort_by,
        })
        .to_string()
    }

    #[tool(
        description = "Find strongly connected components (SCCs) in the symbol graph — groups of \
                       symbols that mutually depend on each other (circular dependencies). Returns \
                       { components: [{ members, size }], total_components, symbol_count, min_size }. \
                       Set min_size=2 to show only non-trivial cycles. Uses Tarjan's O(V+E) algorithm. \
                       Unknown edge_kind returns { error }."
    )]
    async fn mycelium_get_strongly_connected_components(
        &self,
        Parameters(req): Parameters<StronglyConnectedComponentsRequest>,
    ) -> String {
        let kind = match req.edge_kind.as_str() {
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            other => {
                return serde_json::json!({ "error": format!("unknown edge kind: {other}") })
                    .to_string();
            }
        };
        let min_size = req.min_size.unwrap_or(1);
        let store = self.store.read().await;
        let all_sccs = store.strongly_connected_components(kind);
        let symbol_count: usize = all_sccs.iter().map(|e| e.size).sum();
        let total_components = all_sccs.len();
        drop(store);
        let components: Vec<serde_json::Value> = all_sccs
            .into_iter()
            .filter(|e| e.size >= min_size)
            .map(|e| serde_json::json!({ "members": e.members, "size": e.size }))
            .collect();
        serde_json::json!({
            "components": components,
            "total_components": total_components,
            "symbol_count": symbol_count,
            "min_size": min_size,
        })
        .to_string()
    }
}

#[tool_handler]
impl ServerHandler for MyceliumServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
        )
    }
}

// ── callee tree serialization ─────────────────────────────────────────────────

fn callee_node_to_json(node: &CalleeNode, store: &Store) -> serde_json::Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let children: Vec<serde_json::Value> = node
        .children
        .iter()
        .map(|child| callee_node_to_json(child, store))
        .collect();
    serde_json::json!({ "path": path, "children": children })
}

fn caller_node_to_json(node: &CallerNode, store: &Store) -> serde_json::Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let callers: Vec<serde_json::Value> = node
        .callers
        .iter()
        .map(|caller| caller_node_to_json(caller, store))
        .collect();
    serde_json::json!({ "path": path, "callers": callers })
}

fn import_node_to_json(node: &ImportNode, store: &Store) -> serde_json::Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let imports: Vec<serde_json::Value> = node
        .imports
        .iter()
        .map(|dep| import_node_to_json(dep, store))
        .collect();
    serde_json::json!({ "path": path, "imports": imports })
}

fn extends_node_to_json(node: &ExtendsNode, store: &Store) -> serde_json::Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let parents: Vec<serde_json::Value> = node
        .parents
        .iter()
        .map(|p| extends_node_to_json(p, store))
        .collect();
    serde_json::json!({ "path": path, "parents": parents })
}

fn subclass_node_to_json(node: &SubclassNode, store: &Store) -> serde_json::Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let subclasses: Vec<serde_json::Value> = node
        .subclasses
        .iter()
        .map(|s| subclass_node_to_json(s, store))
        .collect();
    serde_json::json!({ "path": path, "subclasses": subclasses })
}

fn implements_node_to_json(node: &ImplementsNode, store: &Store) -> serde_json::Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let interfaces: Vec<serde_json::Value> = node
        .interfaces
        .iter()
        .map(|i| implements_node_to_json(i, store))
        .collect();
    serde_json::json!({ "path": path, "interfaces": interfaces })
}

fn implementor_node_to_json(node: &ImplementorNode, store: &Store) -> serde_json::Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let implementors: Vec<serde_json::Value> = node
        .implementors
        .iter()
        .map(|i| implementor_node_to_json(i, store))
        .collect();
    serde_json::json!({ "path": path, "implementors": implementors })
}

fn importer_node_to_json(node: &ImporterNode, store: &Store) -> serde_json::Value {
    let path = store.path_of(node.id).unwrap_or("<unknown>").to_owned();
    let importers: Vec<serde_json::Value> = node
        .importers
        .iter()
        .map(|i| importer_node_to_json(i, store))
        .collect();
    serde_json::json!({ "path": path, "importers": importers })
}

// ── indexing helper (CPU-bound, run via spawn_blocking) ───────────────────────

// ts_lang / tsx_lang differ only by one letter — similarity is intentional.
#[allow(clippy::similar_names)]
fn run_index(root: &std::path::Path) -> anyhow::Result<(Store, usize, usize, Vec<String>, usize)> {
    let js_lang: tree_sitter::Language = tree_sitter_javascript::LANGUAGE.into();
    let js_ext = Extractor::new(js_lang, JAVASCRIPT_QUERIES)
        .context("failed to compile JavaScript extractor")?;

    let py_lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
    let py_ext =
        Extractor::new(py_lang, PYTHON_QUERIES).context("failed to compile Python extractor")?;

    let ts_lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    let ts_ext = Extractor::new(ts_lang, TYPESCRIPT_QUERIES)
        .context("failed to compile TypeScript extractor")?;

    // TSX uses a distinct grammar that extends TypeScript with JSX node types.
    let tsx_lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TSX.into();
    let tsx_ext =
        Extractor::new(tsx_lang, TYPESCRIPT_QUERIES).context("failed to compile TSX extractor")?;

    let rs_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let rs_ext =
        Extractor::new(rs_lang, RUST_QUERIES).context("failed to compile Rust extractor")?;

    let mut store = Store::new();
    let mut files = 0usize;
    let mut errors = 0usize;
    let mut languages: BTreeSet<&'static str> = BTreeSet::new();

    // Build a walker that respects .gitignore / .myceliumignore; always skips
    // target/ and .mycelium/ regardless of whether a .gitignore exists.
    let mut walk_builder = ignore::WalkBuilder::new(root);
    walk_builder
        .follow_links(false)
        .add_custom_ignore_filename(".myceliumignore");
    for name in &[".gitignore", ".myceliumignore"] {
        let p = root.join(name);
        if p.exists() {
            walk_builder.add_ignore(&p);
        }
    }
    let walker = walk_builder
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !matches!(name.as_ref(), "target" | ".mycelium")
        })
        .build();

    for entry in walker
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_some_and(|ft| ft.is_file()))
    {
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let (extractor, lang_name) = match ext {
            "js" | "jsx" => (&js_ext, "javascript"),
            "py" | "pyi" => (&py_ext, "python"),
            "ts" => (&ts_ext, "typescript"),
            "tsx" => (&tsx_ext, "typescript"),
            "rs" => (&rs_ext, "rust"),
            _ => continue,
        };
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        match std::fs::read(path) {
            Err(e) => {
                warn!("could not read {}: {e}", path.display());
                errors += 1;
            }
            Ok(src) => {
                if let Err(e) = extractor.extract(&rel, &src, &mut store) {
                    warn!("extraction failed for {}: {e}", path.display());
                    errors += 1;
                } else {
                    files += 1;
                    languages.insert(lang_name);
                }
            }
        }
    }
    let stubs_resolved = store.resolve_bare_call_stubs();
    Ok((
        store,
        files,
        errors,
        languages.into_iter().map(str::to_owned).collect(),
        stubs_resolved,
    ))
}

/// Extract a single file into an existing store.  Called from the watch loop.
///
/// `ext` must be one of `js`, `jsx`, `py`, `pyi`, `ts`, `tsx`, `rs`.
/// Errors are silently ignored (the watcher should not crash on bad files).
fn reindex_file(rel: &str, src: &[u8], ext: &str, store: &mut Store) {
    let make_ext = |lang: tree_sitter::Language, queries: &str| Extractor::new(lang, queries).ok();

    let extractor = match ext {
        "js" | "jsx" => make_ext(tree_sitter_javascript::LANGUAGE.into(), JAVASCRIPT_QUERIES),
        "py" | "pyi" => make_ext(tree_sitter_python::LANGUAGE.into(), PYTHON_QUERIES),
        "ts" => make_ext(
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            TYPESCRIPT_QUERIES,
        ),
        "tsx" => make_ext(
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            TYPESCRIPT_QUERIES,
        ),
        "rs" => make_ext(tree_sitter_rust::LANGUAGE.into(), RUST_QUERIES),
        _ => return,
    };

    if let Some(ext_obj) = extractor {
        if let Err(e) = ext_obj.extract(rel, src, store) {
            warn!("watch re-extract failed for {rel}: {e}");
        }
    }
}

// ── stdio entry point ─────────────────────────────────────────────────────────

/// Start the MCP server on stdin/stdout and block until the client disconnects.
///
/// When `root` is `Some(path)`, calls [`MyceliumServer::with_root`] to
/// pre-load the index from a `.mycelium/index.rmp` snapshot (or build it).
/// When `root` is `None`, starts with an empty store.
///
/// # Errors
///
/// Returns an error if pre-loading the index fails, the MCP handshake fails,
/// or the transport encounters an I/O error.
pub async fn serve_stdio(root: Option<PathBuf>) -> anyhow::Result<()> {
    let server = match root {
        Some(r) => MyceliumServer::with_root(r).await?,
        None => MyceliumServer::new(),
    };
    let transport = rmcp::transport::stdio();
    let running = server.serve(transport).await?;
    running.waiting().await?;
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use mycelium_core::{trunk::TrunkPath, types::EdgeKind};
    use rmcp::handler::server::wrapper::Parameters;

    use super::*;

    async fn server_with_fixture() -> MyceliumServer {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let file = store.upsert_node(TrunkPath::parse("src/greet.rs").unwrap());
            let greet = store.upsert_node(TrunkPath::parse("src/greet.rs>greet").unwrap());
            let helper = store.upsert_node(TrunkPath::parse("src/greet.rs>helper").unwrap());
            store.upsert_edge(EdgeKind::Contains, file, greet);
            store.upsert_edge(EdgeKind::Contains, file, helper);
        }
        server
    }

    #[tokio::test]
    async fn search_symbol_returns_matching_paths() {
        let server = server_with_fixture().await;
        let raw = server
            .mycelium_search_symbol(Parameters(SearchSymbolRequest {
                query: "greet".to_string(),
                limit: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let arr = val["matches"].as_array().unwrap();
        assert!(
            arr.iter().any(|v| v.as_str() == Some("src/greet.rs>greet")),
            "greet symbol should match"
        );
        assert!(
            !arr.iter()
                .any(|v| v.as_str() == Some("src/greet.rs>helper")),
            "helper should not match query 'greet'"
        );
    }

    #[tokio::test]
    async fn search_symbol_respects_limit() {
        let server = server_with_fixture().await;
        let raw = server
            .mycelium_search_symbol(Parameters(SearchSymbolRequest {
                query: String::new(), // matches everything
                limit: Some(1),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            val["matches"].as_array().unwrap().len(),
            1,
            "limit should be respected"
        );
    }

    #[tokio::test]
    async fn get_ancestors_returns_containment_chain() {
        let server = server_with_fixture().await;
        let raw = server
            .mycelium_get_ancestors(Parameters(GetAncestorsRequest {
                path: "src/greet.rs>greet".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            val["ancestors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v.as_str() == Some("src/greet.rs")),
            "file node should be an ancestor"
        );
    }

    #[tokio::test]
    async fn get_ancestors_returns_empty_list_for_unknown_path() {
        let server = server_with_fixture().await;
        let raw = server
            .mycelium_get_ancestors(Parameters(GetAncestorsRequest {
                path: "no/such>path".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            val["ancestors"].as_array().unwrap().is_empty(),
            "unknown path should yield empty ancestors list"
        );
    }

    #[tokio::test]
    async fn index_workspace_indexes_rust_file() {
        use std::fs;
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("lib.rs"), "fn hello() {}").unwrap();

        let server = MyceliumServer::new();
        let raw = server
            .mycelium_index_workspace(Parameters(IndexWorkspaceRequest {
                path: tmp.path().to_string_lossy().into_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["files"], 1, "should report one indexed file");
        assert_eq!(val["errors"], 0, "no errors expected");
        assert!(
            val["languages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v.as_str() == Some("rust")),
            "rust must appear in languages list"
        );

        assert!(
            server.store.read().await.lookup("lib.rs>hello").is_some(),
            "function node must be in the store after indexing"
        );
    }

    #[tokio::test]
    async fn index_workspace_includes_languages_for_js_file() {
        use std::fs;
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("app.js"), "function greet() {}").unwrap();

        let server = MyceliumServer::new();
        let raw = server
            .mycelium_index_workspace(Parameters(IndexWorkspaceRequest {
                path: tmp.path().to_string_lossy().into_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["files"], 1, "should report one indexed file");
        assert!(
            val["languages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v.as_str() == Some("javascript")),
            "javascript must appear in languages list"
        );
        assert!(
            server.store.read().await.lookup("app.js>greet").is_some(),
            "function node must be in the store after indexing"
        );
    }

    #[tokio::test]
    async fn get_descendants_returns_all_nested_symbols() {
        let server = server_with_fixture().await;
        let raw = server
            .mycelium_get_descendants(Parameters(GetDescendantsRequest {
                path: "src/greet.rs".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let arr = val["descendants"].as_array().unwrap();
        assert!(
            arr.iter().any(|v| v.as_str() == Some("src/greet.rs>greet")),
            "greet must be a descendant of the file"
        );
        assert!(
            arr.iter()
                .any(|v| v.as_str() == Some("src/greet.rs>helper")),
            "helper must be a descendant of the file"
        );
    }

    #[tokio::test]
    async fn get_descendants_returns_empty_list_for_leaf_node() {
        let server = server_with_fixture().await;
        let raw = server
            .mycelium_get_descendants(Parameters(GetDescendantsRequest {
                path: "src/greet.rs>greet".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            val["descendants"].as_array().unwrap().is_empty(),
            "leaf node should yield empty descendants list"
        );
    }

    #[tokio::test]
    async fn get_descendants_returns_empty_list_for_unknown_path() {
        let server = server_with_fixture().await;
        let raw = server
            .mycelium_get_descendants(Parameters(GetDescendantsRequest {
                path: "no/such>path".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            val["descendants"].as_array().unwrap().is_empty(),
            "unknown path should yield empty descendants list"
        );
    }

    #[tokio::test]
    async fn index_workspace_followed_by_search_and_ancestors() {
        use std::fs;
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("lib.rs"),
            "struct Point { x: i32 } impl Point { fn new() -> Self { Point { x: 0 } } }",
        )
        .unwrap();

        let server = MyceliumServer::new();
        server
            .mycelium_index_workspace(Parameters(IndexWorkspaceRequest {
                path: tmp.path().to_string_lossy().into_owned(),
            }))
            .await;

        // Search for the impl method
        let search_raw = server
            .mycelium_search_symbol(Parameters(SearchSymbolRequest {
                query: "new".to_string(),
                limit: None,
            }))
            .await;
        let search_val: serde_json::Value = serde_json::from_str(&search_raw).unwrap();
        let matches: Vec<&str> = search_val["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(matches.iter().any(|p| p.ends_with(">new")));

        // Get ancestors of the method
        let method_path = matches
            .iter()
            .find(|p| p.ends_with(">new"))
            .copied()
            .unwrap();
        let anc_raw = server
            .mycelium_get_ancestors(Parameters(GetAncestorsRequest {
                path: method_path.to_string(),
            }))
            .await;
        let anc_val: serde_json::Value = serde_json::from_str(&anc_raw).unwrap();
        assert!(
            anc_val["ancestors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v.as_str().is_some_and(|s| s.ends_with("lib.rs"))),
            "file node must appear in ancestor chain"
        );
    }

    #[tokio::test]
    async fn index_workspace_saves_snapshot() {
        use std::fs;
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("lib.rs"), "fn greet() {}").unwrap();

        let server = MyceliumServer::new();
        server
            .mycelium_index_workspace(Parameters(IndexWorkspaceRequest {
                path: tmp.path().to_string_lossy().into_owned(),
            }))
            .await;

        let snap = tmp.path().join(".mycelium").join("index.rmp");
        assert!(snap.exists(), "snapshot must be created after indexing");
        assert!(
            snap.metadata().unwrap().len() > 0,
            "snapshot must not be empty"
        );
    }

    #[tokio::test]
    async fn load_index_restores_symbols() {
        use std::fs;
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("lib.rs"), "fn greet() {}").unwrap();

        // Index and save
        let server1 = MyceliumServer::new();
        server1
            .mycelium_index_workspace(Parameters(IndexWorkspaceRequest {
                path: tmp.path().to_string_lossy().into_owned(),
            }))
            .await;

        // Load on a fresh server
        let server2 = MyceliumServer::new();
        let raw = server2
            .mycelium_load_index(Parameters(LoadIndexRequest {
                path: tmp.path().to_string_lossy().into_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none(), "load must not return error");
        assert!(
            val["nodes"].as_u64().unwrap() > 0,
            "loaded store must contain nodes"
        );
        assert!(
            server2.store.read().await.lookup("lib.rs>greet").is_some(),
            "greet symbol must be present after load"
        );
    }

    #[tokio::test]
    async fn load_index_errors_on_missing_snapshot() {
        let tmp = tempfile::tempdir().unwrap();

        let server = MyceliumServer::new();
        let raw = server
            .mycelium_load_index(Parameters(LoadIndexRequest {
                path: tmp.path().to_string_lossy().into_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            val.get("error").is_some(),
            "loading from a directory without a snapshot must return an error"
        );
    }

    // ── RFC-0007 tests ────────────────────────────────────────────────

    #[tokio::test]
    async fn with_root_loads_existing_snapshot() {
        use std::fs;
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("lib.rs"), "fn hello() {}").unwrap();

        // Build and save a snapshot via a first server.
        let server1 = MyceliumServer::new();
        server1
            .mycelium_index_workspace(Parameters(IndexWorkspaceRequest {
                path: tmp.path().to_string_lossy().into_owned(),
            }))
            .await;
        let snap = tmp.path().join(".mycelium").join("index.rmp");
        assert!(snap.exists(), "pre-condition: snapshot must exist");

        // A second server boots from with_root — should load the snapshot.
        let server2 = MyceliumServer::with_root(tmp.path().to_owned())
            .await
            .expect("with_root must succeed when snapshot exists");
        assert!(
            server2.store.read().await.lookup("lib.rs>hello").is_some(),
            "symbol must be present after with_root loads snapshot"
        );
    }

    #[tokio::test]
    async fn with_root_indexes_when_no_snapshot() {
        use std::fs;
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("app.py"), "def run(): pass").unwrap();

        // No snapshot exists yet; with_root must fall back to live index.
        let server = MyceliumServer::with_root(tmp.path().to_owned())
            .await
            .expect("with_root must succeed even without snapshot");
        assert!(
            server.store.read().await.lookup("app.py>run").is_some(),
            "symbol must be present after with_root runs live index"
        );
        // Snapshot should now exist.
        assert!(
            tmp.path().join(".mycelium").join("index.rmp").exists(),
            "with_root must save a snapshot after live indexing"
        );
    }

    #[tokio::test]
    async fn server_status_returns_node_and_edge_count() {
        let server = server_with_fixture().await;
        let raw = server.mycelium_server_status().await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            val["node_count"].as_u64().unwrap() > 0,
            "node_count must be non-zero"
        );
        assert!(
            val.get("edge_count").is_some(),
            "edge_count key must be present"
        );
        assert!(
            val.get("indexed_root").is_some(),
            "indexed_root key must be present"
        );
        assert!(
            val.get("is_loaded").is_some(),
            "is_loaded key must be present"
        );
    }

    // ── RFC-0008 watch mode tests ─────────────────────────────────────

    #[tokio::test]
    async fn watch_status_not_watching_by_default() {
        let server = MyceliumServer::new();
        let raw = server.mycelium_watch_status().await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            val["watching"].as_bool(),
            Some(false),
            "brand-new server must not be watching"
        );
    }

    #[tokio::test]
    async fn start_watch_sets_watching_true() {
        use std::fs;
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("lib.rs"), "fn hello() {}").unwrap();

        let server = MyceliumServer::with_root(tmp.path().to_owned())
            .await
            .unwrap();

        let raw = server.mycelium_watch_status().await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            val["watching"].as_bool(),
            Some(true),
            "with_root must start the watch loop"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn watch_mode_detects_modified_file() {
        use std::fs;
        use tokio::time::Duration;
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("app.py"), "def hello(): pass").unwrap();

        let server = MyceliumServer::with_root(tmp.path().to_owned())
            .await
            .unwrap();

        assert!(
            server.store.read().await.lookup("app.py>hello").is_some(),
            "initial symbol must be present"
        );

        // Modify the file: replace hello with goodbye
        fs::write(tmp.path().join("app.py"), "def goodbye(): pass").unwrap();

        // Poll up to 2 seconds for the watcher to pick up the change.
        let found = poll_for(
            Duration::from_secs(2),
            Duration::from_millis(100),
            || async { server.store.read().await.lookup("app.py>goodbye").is_some() },
        )
        .await;

        assert!(found, "watcher must detect modification and update store");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn watch_mode_detects_deleted_file() {
        use std::fs;
        use tokio::time::Duration;
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("del.rs"), "fn drop_me() {}").unwrap();

        let server = MyceliumServer::with_root(tmp.path().to_owned())
            .await
            .unwrap();

        assert!(
            server.store.read().await.lookup("del.rs>drop_me").is_some(),
            "initial symbol must be present"
        );

        fs::remove_file(tmp.path().join("del.rs")).unwrap();

        let removed = poll_for(
            Duration::from_secs(2),
            Duration::from_millis(100),
            || async { server.store.read().await.lookup("del.rs>drop_me").is_none() },
        )
        .await;

        assert!(removed, "watcher must detect deletion and remove nodes");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn watch_mode_detects_new_file() {
        use std::fs;
        use tokio::time::Duration;
        let tmp = tempfile::tempdir().unwrap();
        // Start with an empty directory.
        let server = MyceliumServer::with_root(tmp.path().to_owned())
            .await
            .unwrap();

        // Create a new file after the server is running.
        fs::write(tmp.path().join("new.rs"), "fn fresh() {}").unwrap();

        let found = poll_for(
            Duration::from_secs(2),
            Duration::from_millis(100),
            || async { server.store.read().await.lookup("new.rs>fresh").is_some() },
        )
        .await;

        assert!(found, "watcher must detect new file and index it");
    }

    // ── RFC-0012: call-graph MCP tools ───────────────────────────────────

    async fn server_with_call_fixture() -> MyceliumServer {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let file = store.upsert_node(TrunkPath::parse("src/lib.rs").unwrap());
            let foo = store.upsert_node(TrunkPath::parse("src/lib.rs>foo").unwrap());
            let bar = store.upsert_node(TrunkPath::parse("src/lib.rs>bar").unwrap());
            let baz = store.upsert_node(TrunkPath::parse("src/lib.rs>baz").unwrap());
            store.upsert_edge(EdgeKind::Contains, file, foo);
            store.upsert_edge(EdgeKind::Contains, file, bar);
            store.upsert_edge(EdgeKind::Contains, file, baz);
            // foo calls bar and baz; baz calls bar.
            store.upsert_edge(EdgeKind::Calls, foo, bar);
            store.upsert_edge(EdgeKind::Calls, foo, baz);
            store.upsert_edge(EdgeKind::Calls, baz, bar);
        }
        server
    }

    #[tokio::test]
    async fn get_callees_returns_functions_called_by_path() {
        let server = server_with_call_fixture().await;
        let raw = server
            .mycelium_get_callees(Parameters(GetCalleesRequest {
                path: "src/lib.rs>foo".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let paths: Vec<&str> = val["callee_paths"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(
            paths.contains(&"src/lib.rs>bar"),
            "callees of foo must include bar"
        );
        assert!(
            paths.contains(&"src/lib.rs>baz"),
            "callees of foo must include baz"
        );
    }

    #[tokio::test]
    async fn get_callers_returns_functions_that_call_path() {
        let server = server_with_call_fixture().await;
        let raw = server
            .mycelium_get_callers(Parameters(GetCallersRequest {
                path: "src/lib.rs>bar".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let paths: Vec<&str> = val["caller_paths"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(
            paths.contains(&"src/lib.rs>foo"),
            "callers of bar must include foo"
        );
        assert!(
            paths.contains(&"src/lib.rs>baz"),
            "callers of bar must include baz"
        );
    }

    #[tokio::test]
    async fn get_callees_returns_error_for_unknown_path() {
        let server = server_with_call_fixture().await;
        let raw = server
            .mycelium_get_callees(Parameters(GetCalleesRequest {
                path: "no/such/path".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            val.get("error").is_some(),
            "unknown path should return error"
        );
    }

    // ── RFC-0016: mycelium_get_symbol_info ───────────────────────────────

    #[tokio::test]
    async fn get_symbol_info_returns_all_relationships() {
        let server = server_with_call_fixture().await;
        // Add containment so ancestors/descendants are non-empty
        {
            let mut store = server.store.write().await;
            let file = store.upsert_node(TrunkPath::parse("src/lib.rs").unwrap());
            let foo = store.lookup("src/lib.rs>foo").unwrap();
            store.upsert_edge(mycelium_core::types::EdgeKind::Contains, file, foo);
        }
        let raw = server
            .mycelium_get_symbol_info(Parameters(GetSymbolInfoRequest {
                path: "src/lib.rs>foo".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["path"].as_str(), Some("src/lib.rs>foo"));
        assert!(
            val.get("ancestors").is_some(),
            "ancestors field must be present"
        );
        assert!(
            val.get("descendants").is_some(),
            "descendants field must be present"
        );
        let callees: Vec<&str> = val["callees"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(
            callees.contains(&"src/lib.rs>bar"),
            "callees must include bar"
        );
        assert!(
            callees.contains(&"src/lib.rs>baz"),
            "callees must include baz"
        );
        // lists are sorted
        let mut sorted = callees.clone();
        sorted.sort_unstable();
        assert_eq!(callees, sorted, "callees must be sorted");
    }

    #[tokio::test]
    async fn get_symbol_info_returns_error_for_unknown_path() {
        let server = server_with_call_fixture().await;
        let raw = server
            .mycelium_get_symbol_info(Parameters(GetSymbolInfoRequest {
                path: "no/such>path".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some(), "unknown path must return error");
    }

    // ── RFC-0017: mycelium_find_call_path ────────────────────────────────

    #[tokio::test]
    async fn find_call_path_direct() {
        let server = server_with_call_fixture().await;
        // foo → bar is a direct Calls edge (hops = 1)
        let raw = server
            .mycelium_find_call_path(Parameters(FindCallPathRequest {
                from_path: "src/lib.rs>foo".to_string(),
                to_path: "src/lib.rs>bar".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            val.get("error").is_none(),
            "direct path must not return error"
        );
        let path = val["path"].as_array().unwrap();
        assert_eq!(path.len(), 2, "direct path must contain 2 nodes");
        assert_eq!(path[0].as_str(), Some("src/lib.rs>foo"));
        assert_eq!(path[1].as_str(), Some("src/lib.rs>bar"));
        assert_eq!(val["hops"].as_u64(), Some(1), "direct call is 1 hop");
    }

    #[tokio::test]
    async fn find_call_path_transitive() {
        // Build a chain a → b → c with no direct a → c edge.
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("x.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("x.rs>b").unwrap());
            let c = store.upsert_node(TrunkPath::parse("x.rs>c").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, b, c);
        }
        let raw = server
            .mycelium_find_call_path(Parameters(FindCallPathRequest {
                from_path: "x.rs>a".to_string(),
                to_path: "x.rs>c".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            val.get("error").is_none(),
            "transitive path must not return error"
        );
        let path = val["path"].as_array().unwrap();
        assert_eq!(path.len(), 3, "transitive path must contain 3 nodes");
        assert_eq!(path[0].as_str(), Some("x.rs>a"));
        assert_eq!(path[2].as_str(), Some("x.rs>c"));
        assert_eq!(val["hops"].as_u64(), Some(2), "transitive call is 2 hops");
    }

    #[tokio::test]
    async fn find_call_path_no_path() {
        let server = server_with_call_fixture().await;
        // bar has no outgoing Calls edge to foo, so bar → foo is unreachable.
        let raw = server
            .mycelium_find_call_path(Parameters(FindCallPathRequest {
                from_path: "src/lib.rs>bar".to_string(),
                to_path: "src/lib.rs>foo".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            val.get("error").is_none(),
            "no-path result must not return error"
        );
        assert!(
            val["path"].as_array().unwrap().is_empty(),
            "unreachable path must return empty path array"
        );
        assert!(
            val["hops"].is_null(),
            "unreachable path must return null hops"
        );
        assert!(
            val.get("message").is_some(),
            "unreachable path must include a human-readable message"
        );
    }

    #[tokio::test]
    async fn find_call_path_unknown_from_path() {
        let server = server_with_call_fixture().await;
        let raw = server
            .mycelium_find_call_path(Parameters(FindCallPathRequest {
                from_path: "no/such>symbol".to_string(),
                to_path: "src/lib.rs>bar".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            val.get("error").is_some(),
            "unknown from_path must return error"
        );
    }

    #[tokio::test]
    async fn find_call_path_unknown_to_path() {
        let server = server_with_call_fixture().await;
        let raw = server
            .mycelium_find_call_path(Parameters(FindCallPathRequest {
                from_path: "src/lib.rs>foo".to_string(),
                to_path: "no/such>symbol".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            val.get("error").is_some(),
            "unknown to_path must return error"
        );
    }

    // ── RFC-0020: mycelium_get_callee_tree ───────────────────────────────

    #[tokio::test]
    async fn get_callee_tree_returns_nested_structure() {
        let server = server_with_call_fixture().await;
        // foo → bar, foo → baz, baz → bar
        let raw = server
            .mycelium_get_callee_tree(Parameters(GetCalleeTreeRequest {
                path: "src/lib.rs>foo".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            val.get("error").is_none(),
            "known path must not return error"
        );
        let root = &val["root"];
        assert_eq!(root["path"].as_str(), Some("src/lib.rs>foo"));
        let children = root["children"].as_array().unwrap();
        assert_eq!(children.len(), 2, "foo has 2 direct callees");
    }

    #[tokio::test]
    async fn get_callee_tree_returns_error_for_unknown_path() {
        let server = server_with_call_fixture().await;
        let raw = server
            .mycelium_get_callee_tree(Parameters(GetCalleeTreeRequest {
                path: "no/such>path".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some(), "unknown path must return error");
    }

    #[tokio::test]
    async fn get_callee_tree_leaf_at_max_depth() {
        // Build a chain a → b → c
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("b.rs>b").unwrap());
            let c = store.upsert_node(TrunkPath::parse("c.rs>c").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, b, c);
        }
        let raw = server
            .mycelium_get_callee_tree(Parameters(GetCalleeTreeRequest {
                path: "a.rs>a".to_string(),
                max_depth: Some(1),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let root = &val["root"];
        let b_node = &root["children"][0];
        assert_eq!(b_node["path"].as_str(), Some("b.rs>b"));
        assert!(
            b_node["children"].as_array().unwrap().is_empty(),
            "b must be a leaf at max_depth=1"
        );
    }

    // ── RFC-0021: mycelium_get_caller_tree ──────────────────────────────────

    #[tokio::test]
    async fn get_caller_tree_returns_nested_structure() {
        let server = server_with_call_fixture().await;
        // Fixture: foo→bar, foo→baz, baz→bar → bar has 2 callers (foo, baz)
        let raw = server
            .mycelium_get_caller_tree(Parameters(GetCallerTreeRequest {
                path: "src/lib.rs>bar".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            val.get("error").is_none(),
            "known path must not return error"
        );
        let root = &val["root"];
        assert_eq!(root["path"].as_str(), Some("src/lib.rs>bar"));
        let callers = root["callers"].as_array().unwrap();
        assert_eq!(callers.len(), 2, "bar has 2 direct callers");
    }

    #[tokio::test]
    async fn get_caller_tree_returns_error_for_unknown_path() {
        let server = server_with_call_fixture().await;
        let raw = server
            .mycelium_get_caller_tree(Parameters(GetCallerTreeRequest {
                path: "no/such>path".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some(), "unknown path must return error");
    }

    #[tokio::test]
    async fn get_caller_tree_leaf_at_max_depth() {
        // Build a chain a → b → c; caller_tree of c with max_depth=1 yields b as leaf
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("b.rs>b").unwrap());
            let c = store.upsert_node(TrunkPath::parse("c.rs>c").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, b, c);
        }
        let raw = server
            .mycelium_get_caller_tree(Parameters(GetCallerTreeRequest {
                path: "c.rs>c".to_string(),
                max_depth: Some(1),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let root = &val["root"];
        let b_node = &root["callers"][0];
        assert_eq!(b_node["path"].as_str(), Some("b.rs>b"));
        assert!(
            b_node["callers"].as_array().unwrap().is_empty(),
            "b must be a leaf at max_depth=1"
        );
    }

    // ── RFC-0022: mycelium_get_entry_points ──────────────────────────────

    #[tokio::test]
    async fn get_entry_points_returns_zero_caller_symbols() {
        let server = server_with_call_fixture().await;
        // Fixture: foo→bar, foo→baz, baz→bar
        // foo has no callers → entry point; bar and baz have callers → not
        let raw = server
            .mycelium_get_entry_points(Parameters(GetEntryPointsRequest { path_prefix: None }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let eps = val["entry_points"].as_array().unwrap();
        let ep_paths: Vec<&str> = eps.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(ep_paths.contains(&"src/lib.rs>foo"), "foo has no callers");
        assert!(
            !ep_paths.contains(&"src/lib.rs>bar"),
            "bar is called by foo and baz"
        );
        assert!(
            !ep_paths.contains(&"src/lib.rs>baz"),
            "baz is called by foo"
        );
    }

    #[tokio::test]
    async fn get_entry_points_excludes_file_nodes() {
        let server = server_with_call_fixture().await;
        let raw = server
            .mycelium_get_entry_points(Parameters(GetEntryPointsRequest { path_prefix: None }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let eps = val["entry_points"].as_array().unwrap();
        for ep in eps {
            let p = ep.as_str().unwrap();
            assert!(p.contains('>'), "file nodes must not appear: {p}");
        }
    }

    #[tokio::test]
    async fn get_entry_points_prefix_filter() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/a.rs>fn_a").unwrap());
            store.upsert_node(TrunkPath::parse("tests/t.rs>test_foo").unwrap());
        }
        let raw = server
            .mycelium_get_entry_points(Parameters(GetEntryPointsRequest {
                path_prefix: Some("src/".to_string()),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let eps = val["entry_points"].as_array().unwrap();
        let ep_paths: Vec<&str> = eps.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(ep_paths.contains(&"src/a.rs>fn_a"));
        assert!(!ep_paths.contains(&"tests/t.rs>test_foo"));
    }

    // ── RFC-0023: mycelium_get_imports ───────────────────────────────────

    #[tokio::test]
    async fn get_imports_returns_import_neighbors() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let file = store.upsert_node(TrunkPath::parse("src/auth.rs").unwrap());
            let os_mod = store.upsert_node(TrunkPath::parse("os").unwrap());
            let hash_mod = store.upsert_node(TrunkPath::parse("hashlib").unwrap());
            let importer = store.upsert_node(TrunkPath::parse("src/main.rs").unwrap());
            store.upsert_edge(EdgeKind::Imports, file, os_mod);
            store.upsert_edge(EdgeKind::Imports, file, hash_mod);
            store.upsert_edge(EdgeKind::Imports, importer, file);
        }
        let raw = server
            .mycelium_get_imports(Parameters(GetImportsRequest {
                path: "src/auth.rs".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none(), "known path must not error");
        let imports = val["imports"].as_array().unwrap();
        let imported_by = val["imported_by"].as_array().unwrap();
        assert_eq!(imports.len(), 2, "auth.rs imports os and hashlib");
        assert_eq!(imported_by.len(), 1, "auth.rs imported_by src/main.rs");
    }

    #[tokio::test]
    async fn get_imports_returns_error_for_unknown_path() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_imports(Parameters(GetImportsRequest {
                path: "no/such.rs".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some(), "unknown path must return error");
    }

    #[tokio::test]
    async fn get_imports_empty_when_no_import_edges() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/isolated.rs").unwrap());
        }
        let raw = server
            .mycelium_get_imports(Parameters(GetImportsRequest {
                path: "src/isolated.rs".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["imports"].as_array().unwrap().is_empty());
        assert!(val["imported_by"].as_array().unwrap().is_empty());
    }

    // ── RFC-0024: mycelium_get_import_tree ───────────────────────────────

    #[tokio::test]
    async fn get_import_tree_returns_nested_structure() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("a.rs").unwrap());
            let b = store.upsert_node(TrunkPath::parse("b.rs").unwrap());
            let c = store.upsert_node(TrunkPath::parse("c.rs").unwrap());
            store.upsert_edge(EdgeKind::Imports, a, b);
            store.upsert_edge(EdgeKind::Imports, b, c);
        }
        let raw = server
            .mycelium_get_import_tree(Parameters(GetImportTreeRequest {
                path: "a.rs".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none(), "known path must not error");
        let root = &val["root"];
        assert_eq!(root["path"].as_str(), Some("a.rs"));
        assert_eq!(root["imports"].as_array().unwrap().len(), 1);
        assert_eq!(root["imports"][0]["path"].as_str(), Some("b.rs"));
        assert_eq!(
            root["imports"][0]["imports"][0]["path"].as_str(),
            Some("c.rs")
        );
    }

    #[tokio::test]
    async fn get_import_tree_returns_error_for_unknown_path() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_import_tree(Parameters(GetImportTreeRequest {
                path: "no/such.rs".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some(), "unknown path must return error");
    }

    #[tokio::test]
    async fn get_import_tree_leaf_at_max_depth() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("a.rs").unwrap());
            let b = store.upsert_node(TrunkPath::parse("b.rs").unwrap());
            let c = store.upsert_node(TrunkPath::parse("c.rs").unwrap());
            store.upsert_edge(EdgeKind::Imports, a, b);
            store.upsert_edge(EdgeKind::Imports, b, c);
        }
        let raw = server
            .mycelium_get_import_tree(Parameters(GetImportTreeRequest {
                path: "a.rs".to_string(),
                max_depth: Some(1),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let b_node = &val["root"]["imports"][0];
        assert_eq!(b_node["path"].as_str(), Some("b.rs"));
        assert!(
            b_node["imports"].as_array().unwrap().is_empty(),
            "b must be a leaf at max_depth=1"
        );
    }

    // ── RFC-0025: mycelium_batch_symbol_info ─────────────────────────────

    #[tokio::test]
    async fn batch_symbol_info_returns_info_for_each_path() {
        let server = server_with_call_fixture().await;
        // foo→bar, foo→baz
        let raw = server
            .mycelium_batch_symbol_info(Parameters(BatchSymbolInfoRequest {
                paths: vec!["src/lib.rs>foo".to_string(), "src/lib.rs>bar".to_string()],
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let symbols = val["symbols"].as_array().unwrap();
        assert_eq!(symbols.len(), 2);
        let foo = &symbols[0];
        let bar = &symbols[1];
        assert_eq!(foo["path"].as_str(), Some("src/lib.rs>foo"));
        assert!(foo.get("error").is_none(), "foo should be found");
        assert!(
            !foo["callees"].as_array().unwrap().is_empty(),
            "foo has callees"
        );
        assert_eq!(bar["path"].as_str(), Some("src/lib.rs>bar"));
        assert!(bar.get("error").is_none(), "bar should be found");
    }

    #[tokio::test]
    async fn batch_symbol_info_unknown_path_returns_error_entry() {
        let server = server_with_call_fixture().await;
        let raw = server
            .mycelium_batch_symbol_info(Parameters(BatchSymbolInfoRequest {
                paths: vec!["src/lib.rs>foo".to_string(), "no/such>path".to_string()],
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let symbols = val["symbols"].as_array().unwrap();
        assert_eq!(symbols.len(), 2);
        assert!(symbols[0].get("error").is_none(), "foo is found");
        assert!(
            symbols[1].get("error").is_some(),
            "unknown path has error field"
        );
    }

    #[tokio::test]
    async fn batch_symbol_info_preserves_input_order() {
        let server = server_with_call_fixture().await;
        let raw = server
            .mycelium_batch_symbol_info(Parameters(BatchSymbolInfoRequest {
                paths: vec![
                    "src/lib.rs>bar".to_string(),
                    "src/lib.rs>foo".to_string(),
                    "src/lib.rs>baz".to_string(),
                ],
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let symbols = val["symbols"].as_array().unwrap();
        assert_eq!(symbols[0]["path"].as_str(), Some("src/lib.rs>bar"));
        assert_eq!(symbols[1]["path"].as_str(), Some("src/lib.rs>foo"));
        assert_eq!(symbols[2]["path"].as_str(), Some("src/lib.rs>baz"));
    }

    // ── RFC-0019: mycelium_rank_symbols ──────────────────────────────────

    #[tokio::test]
    async fn rank_symbols_returns_top_callee_descending() {
        let server = server_with_call_fixture().await;
        // Fixture: foo→bar, foo→baz, baz→bar → bar has 2 callers, baz has 1, foo has 0
        let raw = server
            .mycelium_rank_symbols(Parameters(RankSymbolsRequest { limit: None }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let symbols = val["symbols"].as_array().unwrap();
        assert!(!symbols.is_empty(), "ranked list must be non-empty");
        // First symbol must have the highest caller_count
        let first_count = symbols[0]["caller_count"].as_u64().unwrap();
        assert!(first_count >= 2, "bar must be ranked first with 2 callers");
        // Verify foo (no callers) is excluded
        assert!(
            !symbols
                .iter()
                .any(|s| s["path"].as_str() == Some("src/lib.rs>foo")),
            "foo has no callers and must be excluded"
        );
    }

    #[tokio::test]
    async fn rank_symbols_respects_limit() {
        let server = server_with_call_fixture().await;
        let raw = server
            .mycelium_rank_symbols(Parameters(RankSymbolsRequest { limit: Some(1) }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            val["symbols"].as_array().unwrap().len(),
            1,
            "limit=1 must return exactly one symbol"
        );
    }

    #[tokio::test]
    async fn rank_symbols_empty_when_no_call_edges() {
        let server = server_with_fixture().await; // only Contains edges, no Calls
        let raw = server
            .mycelium_rank_symbols(Parameters(RankSymbolsRequest { limit: None }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            val["symbols"].as_array().unwrap().is_empty(),
            "no call edges means empty ranking"
        );
    }

    // ── RFC-0018: mycelium_get_files ─────────────────────────────────────

    #[tokio::test]
    async fn get_files_returns_only_file_paths() {
        let server = server_with_fixture().await;
        // server_with_fixture has src/greet.rs, src/greet.rs>greet, src/greet.rs>helper
        let raw = server
            .mycelium_get_files(Parameters(GetFilesRequest { path_prefix: None }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let files: Vec<&str> = val["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(files.contains(&"src/greet.rs"), "greet.rs must be listed");
        assert!(
            !files.iter().any(|p| p.contains('>')),
            "symbol paths must not appear in file list"
        );
    }

    #[tokio::test]
    async fn get_files_filters_by_prefix() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/auth.rs").unwrap());
            store.upsert_node(TrunkPath::parse("tests/auth_test.rs").unwrap());
            store.upsert_node(TrunkPath::parse("src/main.rs").unwrap());
        }
        let raw = server
            .mycelium_get_files(Parameters(GetFilesRequest {
                path_prefix: Some("src/".to_string()),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let files: Vec<&str> = val["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(
            files.contains(&"src/auth.rs"),
            "src/auth.rs must match prefix"
        );
        assert!(
            files.contains(&"src/main.rs"),
            "src/main.rs must match prefix"
        );
        assert!(
            !files.contains(&"tests/auth_test.rs"),
            "tests/ file must not match src/ prefix"
        );
    }

    #[tokio::test]
    async fn get_files_returns_sorted_order() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("z.rs").unwrap());
            store.upsert_node(TrunkPath::parse("a.rs").unwrap());
            store.upsert_node(TrunkPath::parse("m.rs").unwrap());
        }
        let raw = server
            .mycelium_get_files(Parameters(GetFilesRequest { path_prefix: None }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let files: Vec<&str> = val["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let mut sorted = files.clone();
        sorted.sort_unstable();
        assert_eq!(files, sorted, "files must be returned in sorted order");
    }

    // ── RFC-0015: watch-mode stub resolution ─────────────────────────────

    #[tokio::test(flavor = "multi_thread")]
    async fn watch_mode_resolves_stub_after_callee_file_added() {
        use std::fs;
        use tokio::time::Duration;

        let tmp = tempfile::tempdir().unwrap();
        // a.py calls bar() which is not yet defined anywhere.
        fs::write(tmp.path().join("a.py"), "def foo():\n    bar()").unwrap();

        let server = MyceliumServer::with_root(tmp.path().to_owned())
            .await
            .unwrap();

        // After initial index, bare stub "bar" exists.
        assert!(
            server.store.read().await.lookup("a.py>foo").is_some(),
            "a.py>foo must be indexed"
        );

        // Now create b.py which defines bar().
        fs::write(tmp.path().join("b.py"), "def bar(): pass").unwrap();

        // Poll until the watcher picks up b.py and resolve_bare_call_stubs runs.
        let resolved = poll_for(
            Duration::from_secs(3),
            Duration::from_millis(100),
            || async {
                let store = server.store.read().await;
                let foo = store.lookup("a.py>foo");
                let bar_def = store.lookup("b.py>bar");
                let bar_stub = store.lookup("bar");
                match (foo, bar_def) {
                    (Some(f), Some(b)) => {
                        store.outgoing(f, EdgeKind::Calls).contains(&b) && bar_stub.is_none()
                    }
                    _ => false,
                }
            },
        )
        .await;

        assert!(
            resolved,
            "watch mode must resolve bare 'bar' stub to 'b.py>bar' after b.py is created"
        );
    }

    // ── RFC-0026: mycelium_get_extends / mycelium_get_implements ─────────

    async fn server_with_inheritance_fixture() -> MyceliumServer {
        let server = MyceliumServer::new();
        let mut store = server.store.write().await;
        // Shape ← Rectangle ← Square (extends chain)
        let shape = store
            .upsert_node(mycelium_core::trunk::TrunkPath::parse("src/shapes.py>Shape").unwrap());
        let rect = store.upsert_node(
            mycelium_core::trunk::TrunkPath::parse("src/shapes.py>Rectangle").unwrap(),
        );
        let square = store
            .upsert_node(mycelium_core::trunk::TrunkPath::parse("src/shapes.py>Square").unwrap());
        // IShape interface implemented by Shape
        let ishape = store
            .upsert_node(mycelium_core::trunk::TrunkPath::parse("src/shapes.py>IShape").unwrap());
        store.upsert_edge(mycelium_core::types::EdgeKind::Extends, rect, shape);
        store.upsert_edge(mycelium_core::types::EdgeKind::Extends, square, rect);
        store.upsert_edge(mycelium_core::types::EdgeKind::Implements, shape, ishape);
        drop(store);
        server
    }

    #[tokio::test]
    async fn get_extends_returns_extends_and_extended_by() {
        let server = server_with_inheritance_fixture().await;
        let raw = server
            .mycelium_get_extends(Parameters(GetExtendsRequest {
                path: "src/shapes.py>Rectangle".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none(), "rect is found");
        let extends = val["extends"].as_array().unwrap();
        let extended_by = val["extended_by"].as_array().unwrap();
        assert_eq!(extends.len(), 1);
        assert_eq!(extends[0].as_str(), Some("src/shapes.py>Shape"));
        assert_eq!(extended_by.len(), 1);
        assert_eq!(extended_by[0].as_str(), Some("src/shapes.py>Square"));
    }

    #[tokio::test]
    async fn get_extends_empty_when_no_edges() {
        let server = server_with_inheritance_fixture().await;
        let raw = server
            .mycelium_get_extends(Parameters(GetExtendsRequest {
                path: "src/shapes.py>Shape".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["extends"].as_array().unwrap().is_empty());
        assert_eq!(val["extended_by"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn get_extends_unknown_path_returns_error() {
        let server = server_with_inheritance_fixture().await;
        let raw = server
            .mycelium_get_extends(Parameters(GetExtendsRequest {
                path: "no/such>Symbol".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    #[tokio::test]
    async fn get_implements_returns_implements_and_implemented_by() {
        let server = server_with_inheritance_fixture().await;
        let raw = server
            .mycelium_get_implements(Parameters(GetImplementsRequest {
                path: "src/shapes.py>Shape".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none());
        let implements = val["implements"].as_array().unwrap();
        let implemented_by = val["implemented_by"].as_array().unwrap();
        assert_eq!(implements.len(), 1);
        assert_eq!(implements[0].as_str(), Some("src/shapes.py>IShape"));
        assert!(implemented_by.is_empty());
    }

    #[tokio::test]
    async fn get_implements_interface_side_shows_implemented_by() {
        let server = server_with_inheritance_fixture().await;
        let raw = server
            .mycelium_get_implements(Parameters(GetImplementsRequest {
                path: "src/shapes.py>IShape".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none());
        assert!(val["implements"].as_array().unwrap().is_empty());
        let implemented_by = val["implemented_by"].as_array().unwrap();
        assert_eq!(implemented_by.len(), 1);
        assert_eq!(implemented_by[0].as_str(), Some("src/shapes.py>Shape"));
    }

    #[tokio::test]
    async fn get_implements_unknown_path_returns_error() {
        let server = server_with_inheritance_fixture().await;
        let raw = server
            .mycelium_get_implements(Parameters(GetImplementsRequest {
                path: "no/such>Symbol".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    // ── RFC-0027: mycelium_find_import_path ──────────────────────────────

    async fn server_with_import_chain_fixture() -> MyceliumServer {
        let server = MyceliumServer::new();
        let mut store = server.store.write().await;
        // a → b → c (import chain)
        let a = store.upsert_node(mycelium_core::trunk::TrunkPath::parse("a.rs").unwrap());
        let b = store.upsert_node(mycelium_core::trunk::TrunkPath::parse("b.rs").unwrap());
        let c = store.upsert_node(mycelium_core::trunk::TrunkPath::parse("c.rs").unwrap());
        store.upsert_edge(mycelium_core::types::EdgeKind::Imports, a, b);
        store.upsert_edge(mycelium_core::types::EdgeKind::Imports, b, c);
        drop(store);
        let _ = (a, b, c);
        server
    }

    #[tokio::test]
    async fn find_import_path_direct() {
        let server = server_with_import_chain_fixture().await;
        let raw = server
            .mycelium_find_import_path(Parameters(FindImportPathRequest {
                from_path: "a.rs".to_string(),
                to_path: "b.rs".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none());
        assert_eq!(val["hops"].as_u64(), Some(1));
        let path = val["path"].as_array().unwrap();
        assert_eq!(path.len(), 2);
        assert_eq!(path[0].as_str(), Some("a.rs"));
        assert_eq!(path[1].as_str(), Some("b.rs"));
    }

    #[tokio::test]
    async fn find_import_path_transitive() {
        let server = server_with_import_chain_fixture().await;
        let raw = server
            .mycelium_find_import_path(Parameters(FindImportPathRequest {
                from_path: "a.rs".to_string(),
                to_path: "c.rs".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none());
        assert_eq!(val["hops"].as_u64(), Some(2));
        let path = val["path"].as_array().unwrap();
        assert_eq!(path, &["a.rs", "b.rs", "c.rs"]);
    }

    #[tokio::test]
    async fn find_import_path_unreachable_returns_empty() {
        let server = server_with_import_chain_fixture().await;
        let raw = server
            .mycelium_find_import_path(Parameters(FindImportPathRequest {
                from_path: "c.rs".to_string(),
                to_path: "a.rs".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none());
        assert!(val["path"].as_array().unwrap().is_empty());
        assert!(val["hops"].is_null());
        assert!(val["message"].is_string());
    }

    #[tokio::test]
    async fn find_import_path_unknown_from_returns_error() {
        let server = server_with_import_chain_fixture().await;
        let raw = server
            .mycelium_find_import_path(Parameters(FindImportPathRequest {
                from_path: "no/such.rs".to_string(),
                to_path: "b.rs".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    #[tokio::test]
    async fn find_import_path_max_depth_respected() {
        let server = server_with_import_chain_fixture().await;
        // a → b → c; max_depth=1 cannot reach c
        let raw_shallow = server
            .mycelium_find_import_path(Parameters(FindImportPathRequest {
                from_path: "a.rs".to_string(),
                to_path: "c.rs".to_string(),
                max_depth: Some(1),
            }))
            .await;
        let val_shallow: serde_json::Value = serde_json::from_str(&raw_shallow).unwrap();
        assert!(val_shallow["path"].as_array().unwrap().is_empty());

        let raw_deep = server
            .mycelium_find_import_path(Parameters(FindImportPathRequest {
                from_path: "a.rs".to_string(),
                to_path: "c.rs".to_string(),
                max_depth: Some(2),
            }))
            .await;
        let val_deep: serde_json::Value = serde_json::from_str(&raw_deep).unwrap();
        assert_eq!(val_deep["hops"].as_u64(), Some(2));
    }

    // ── RFC-0028: mycelium_get_node_kind / mycelium_get_symbols_by_kind ──

    async fn server_with_kind_fixture() -> MyceliumServer {
        let server = MyceliumServer::new();
        let mut store = server.store.write().await;
        let f1 = store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/a.rs>foo").unwrap());
        let f2 = store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/b.rs>bar").unwrap());
        let c1 = store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/c.rs>Baz").unwrap());
        store.set_kind(f1, mycelium_core::types::NodeKind::Function);
        store.set_kind(f2, mycelium_core::types::NodeKind::Function);
        store.set_kind(c1, mycelium_core::types::NodeKind::Class);
        drop(store);
        server
    }

    #[tokio::test]
    async fn get_node_kind_returns_kind_for_known_path() {
        let server = server_with_kind_fixture().await;
        let raw = server
            .mycelium_get_node_kind(Parameters(GetNodeKindRequest {
                path: "src/a.rs>foo".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none());
        assert_eq!(val["kind"].as_str(), Some("function"));
    }

    #[tokio::test]
    async fn get_node_kind_returns_null_when_kind_not_recorded() {
        let server = MyceliumServer::new();
        let mut store = server.store.write().await;
        store.upsert_node(mycelium_core::trunk::TrunkPath::parse("x.rs>foo").unwrap());
        drop(store);
        let raw = server
            .mycelium_get_node_kind(Parameters(GetNodeKindRequest {
                path: "x.rs>foo".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none());
        assert!(val["kind"].is_null());
    }

    #[tokio::test]
    async fn get_node_kind_unknown_path_returns_error() {
        let server = server_with_kind_fixture().await;
        let raw = server
            .mycelium_get_node_kind(Parameters(GetNodeKindRequest {
                path: "no/such>path".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    #[tokio::test]
    async fn get_symbols_by_kind_returns_all_matching() {
        let server = server_with_kind_fixture().await;
        let raw = server
            .mycelium_get_symbols_by_kind(Parameters(GetSymbolsByKindRequest {
                kind: "function".to_string(),
                path_prefix: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none());
        let syms = val["symbols"].as_array().unwrap();
        assert_eq!(syms.len(), 2);
        assert!(syms.iter().any(|s| s.as_str() == Some("src/a.rs>foo")));
        assert!(syms.iter().any(|s| s.as_str() == Some("src/b.rs>bar")));
    }

    #[tokio::test]
    async fn get_symbols_by_kind_with_prefix_filter() {
        let server = server_with_kind_fixture().await;
        // add one function outside src/
        {
            let mut store = server.store.write().await;
            let id = store
                .upsert_node(mycelium_core::trunk::TrunkPath::parse("tests/t.rs>test_fn").unwrap());
            store.set_kind(id, mycelium_core::types::NodeKind::Function);
        }
        let raw = server
            .mycelium_get_symbols_by_kind(Parameters(GetSymbolsByKindRequest {
                kind: "function".to_string(),
                path_prefix: Some("src/".to_string()),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let syms = val["symbols"].as_array().unwrap();
        assert_eq!(syms.len(), 2, "only src/ functions");
        assert!(syms.iter().all(|s| s.as_str().unwrap().starts_with("src/")));
    }

    #[tokio::test]
    async fn get_symbols_by_kind_unknown_kind_returns_error() {
        let server = server_with_kind_fixture().await;
        let raw = server
            .mycelium_get_symbols_by_kind(Parameters(GetSymbolsByKindRequest {
                kind: "not_a_real_kind".to_string(),
                path_prefix: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    // ── RFC-0029: mycelium_get_source_span ───────────────────────────────

    async fn server_with_span_fixture() -> MyceliumServer {
        let server = MyceliumServer::new();
        let mut store = server.store.write().await;
        let id =
            store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/auth.rs>login").unwrap());
        store.set_span(
            id,
            mycelium_core::types::SourceSpan {
                start_line: 10,
                start_col: 0,
                end_line: 20,
                end_col: 1,
                start_byte: 100,
                end_byte: 300,
            },
        );
        drop(store);
        server
    }

    #[tokio::test]
    async fn get_source_span_returns_all_fields() {
        let server = server_with_span_fixture().await;
        let raw = server
            .mycelium_get_source_span(Parameters(GetSourceSpanRequest {
                path: "src/auth.rs>login".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none(), "should not error");
        assert_eq!(val["start_line"].as_u64(), Some(10));
        assert_eq!(val["start_col"].as_u64(), Some(0));
        assert_eq!(val["end_line"].as_u64(), Some(20));
        assert_eq!(val["end_col"].as_u64(), Some(1));
        assert_eq!(val["start_byte"].as_u64(), Some(100));
        assert_eq!(val["end_byte"].as_u64(), Some(300));
    }

    #[tokio::test]
    async fn get_source_span_returns_null_when_span_not_recorded() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(mycelium_core::trunk::TrunkPath::parse("x.rs>foo").unwrap());
        }
        let raw = server
            .mycelium_get_source_span(Parameters(GetSourceSpanRequest {
                path: "x.rs>foo".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none(), "should not error");
        assert!(val["span"].is_null(), "span must be null when unrecorded");
    }

    #[tokio::test]
    async fn get_source_span_unknown_path_returns_error() {
        let server = server_with_span_fixture().await;
        let raw = server
            .mycelium_get_source_span(Parameters(GetSourceSpanRequest {
                path: "no/such>path".to_string(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            val.get("error").is_some(),
            "must return error for unknown path"
        );
    }

    // ── RFC-0030: mycelium_find_extends_path ─────────────────────────────

    async fn server_with_extends_fixture() -> MyceliumServer {
        let server = MyceliumServer::new();
        let mut store = server.store.write().await;
        let base =
            store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/base.ts>Base").unwrap());
        let mid =
            store.upsert_node(mycelium_core::trunk::TrunkPath::parse("src/mid.ts>Mid").unwrap());
        let child = store
            .upsert_node(mycelium_core::trunk::TrunkPath::parse("src/child.ts>Child").unwrap());
        store.upsert_edge(mycelium_core::types::EdgeKind::Extends, child, mid);
        store.upsert_edge(mycelium_core::types::EdgeKind::Extends, mid, base);
        drop(store);
        server
    }

    #[tokio::test]
    async fn find_extends_path_direct() {
        let server = server_with_extends_fixture().await;
        let raw = server
            .mycelium_find_extends_path(Parameters(FindExtendsPathRequest {
                from_path: "src/child.ts>Child".to_string(),
                to_path: "src/mid.ts>Mid".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none());
        assert_eq!(val["hops"].as_u64(), Some(1));
    }

    #[tokio::test]
    async fn find_extends_path_transitive() {
        let server = server_with_extends_fixture().await;
        let raw = server
            .mycelium_find_extends_path(Parameters(FindExtendsPathRequest {
                from_path: "src/child.ts>Child".to_string(),
                to_path: "src/base.ts>Base".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none());
        assert_eq!(val["hops"].as_u64(), Some(2));
    }

    #[tokio::test]
    async fn find_extends_path_unreachable_returns_empty() {
        let server = server_with_extends_fixture().await;
        let raw = server
            .mycelium_find_extends_path(Parameters(FindExtendsPathRequest {
                from_path: "src/base.ts>Base".to_string(),
                to_path: "src/child.ts>Child".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none());
        assert!(val["hops"].is_null());
        assert!(val["path"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn find_extends_path_unknown_from_returns_error() {
        let server = server_with_extends_fixture().await;
        let raw = server
            .mycelium_find_extends_path(Parameters(FindExtendsPathRequest {
                from_path: "no/such>path".to_string(),
                to_path: "src/base.ts>Base".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    // ── RFC-0031: mycelium_get_extends_tree ──────────────────────────────

    #[tokio::test]
    async fn get_extends_tree_returns_superclass_chain() {
        let server = server_with_extends_fixture().await;
        let raw = server
            .mycelium_get_extends_tree(Parameters(GetExtendsTreeRequest {
                path: "src/child.ts>Child".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_none());
        let root = &val["root"];
        assert_eq!(root["path"].as_str(), Some("src/child.ts>Child"));
        assert_eq!(root["parents"].as_array().unwrap().len(), 1);
        assert_eq!(root["parents"][0]["path"].as_str(), Some("src/mid.ts>Mid"));
    }

    #[tokio::test]
    async fn get_extends_tree_leaf_at_max_depth_zero() {
        let server = server_with_extends_fixture().await;
        let raw = server
            .mycelium_get_extends_tree(Parameters(GetExtendsTreeRequest {
                path: "src/child.ts>Child".to_string(),
                max_depth: Some(0),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["root"]["parents"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn get_extends_tree_unknown_path_returns_error() {
        let server = server_with_extends_fixture().await;
        let raw = server
            .mycelium_get_extends_tree(Parameters(GetExtendsTreeRequest {
                path: "no/such>path".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    // ── RFC-0032: mycelium_get_subclasses_tree ───────────────────────────

    #[tokio::test]
    async fn get_subclasses_tree_returns_subclass_forest() {
        let server = server_with_extends_fixture().await;
        // Fixture: Child→Mid→Base (Child extends Mid, Mid extends Base)
        // From Base perspective: Base has one subclass Mid, Mid has one subclass Child
        let raw = server
            .mycelium_get_subclasses_tree(Parameters(GetSubclassesTreeRequest {
                path: "src/base.ts>Base".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let root = val.get("root").expect("root key present");
        assert_eq!(root["path"], "src/base.ts>Base");
        let subclasses = root["subclasses"].as_array().unwrap();
        assert_eq!(subclasses.len(), 1);
        assert_eq!(subclasses[0]["path"], "src/mid.ts>Mid");
        let mid_subclasses = subclasses[0]["subclasses"].as_array().unwrap();
        assert_eq!(mid_subclasses.len(), 1);
        assert_eq!(mid_subclasses[0]["path"], "src/child.ts>Child");
    }

    #[tokio::test]
    async fn get_subclasses_tree_leaf_at_max_depth_zero() {
        let server = server_with_extends_fixture().await;
        let raw = server
            .mycelium_get_subclasses_tree(Parameters(GetSubclassesTreeRequest {
                path: "src/base.ts>Base".to_string(),
                max_depth: Some(0),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let root = val.get("root").expect("root key present");
        let subclasses = root["subclasses"].as_array().unwrap();
        assert!(subclasses.is_empty());
    }

    #[tokio::test]
    async fn get_subclasses_tree_unknown_path_returns_error() {
        let server = server_with_extends_fixture().await;
        let raw = server
            .mycelium_get_subclasses_tree(Parameters(GetSubclassesTreeRequest {
                path: "no/such>path".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    // ── RFC-0033: mycelium_find_implements_path ───────────────────────────

    async fn server_with_implements_fixture() -> MyceliumServer {
        let server = MyceliumServer::new();
        {
            let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
            let cls = store
                .upsert_node(mycelium_core::trunk::TrunkPath::parse("src/cls.ts>Cls").unwrap());
            let iface = store
                .upsert_node(mycelium_core::trunk::TrunkPath::parse("src/iface.ts>IFace").unwrap());
            let base_iface = store.upsert_node(
                mycelium_core::trunk::TrunkPath::parse("src/base.ts>BaseIFace").unwrap(),
            );
            store.upsert_edge(mycelium_core::EdgeKind::Implements, cls, iface);
            store.upsert_edge(mycelium_core::EdgeKind::Implements, iface, base_iface);
        }
        server
    }

    #[tokio::test]
    async fn find_implements_path_direct_hop() {
        let server = server_with_implements_fixture().await;
        let raw = server
            .mycelium_find_implements_path(Parameters(FindImplementsPathRequest {
                from_path: "src/cls.ts>Cls".to_string(),
                to_path: "src/iface.ts>IFace".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let path = val["path"].as_array().unwrap();
        assert_eq!(path.len(), 2);
        assert_eq!(path[0], "src/cls.ts>Cls");
        assert_eq!(path[1], "src/iface.ts>IFace");
        assert_eq!(val["hops"], 1);
    }

    #[tokio::test]
    async fn find_implements_path_unreachable() {
        let server = server_with_implements_fixture().await;
        let raw = server
            .mycelium_find_implements_path(Parameters(FindImplementsPathRequest {
                from_path: "src/iface.ts>IFace".to_string(),
                to_path: "src/cls.ts>Cls".to_string(), // backwards — no path
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["path"].as_array().unwrap().len(), 0);
        assert!(val["hops"].is_null());
    }

    #[tokio::test]
    async fn find_implements_path_unknown_path_returns_error() {
        let server = server_with_implements_fixture().await;
        let raw = server
            .mycelium_find_implements_path(Parameters(FindImplementsPathRequest {
                from_path: "no/such>path".to_string(),
                to_path: "src/iface.ts>IFace".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    #[tokio::test]
    async fn find_implements_path_transitive() {
        let server = server_with_implements_fixture().await;
        let raw = server
            .mycelium_find_implements_path(Parameters(FindImplementsPathRequest {
                from_path: "src/cls.ts>Cls".to_string(),
                to_path: "src/base.ts>BaseIFace".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let path = val["path"].as_array().unwrap();
        assert_eq!(path.len(), 3);
        assert_eq!(val["hops"], 2);
    }

    // ── RFC-0034: mycelium_get_implements_tree ────────────────────────────

    #[tokio::test]
    async fn get_implements_tree_returns_interface_chain() {
        // Re-use server_with_implements_fixture: Cls→IFace→BaseIFace
        let server = server_with_implements_fixture().await;
        let raw = server
            .mycelium_get_implements_tree(Parameters(GetImplementsTreeRequest {
                path: "src/cls.ts>Cls".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let root = val.get("root").expect("root key present");
        assert_eq!(root["path"], "src/cls.ts>Cls");
        let ifaces = root["interfaces"].as_array().unwrap();
        assert_eq!(ifaces.len(), 1);
        assert_eq!(ifaces[0]["path"], "src/iface.ts>IFace");
        let base_ifaces = ifaces[0]["interfaces"].as_array().unwrap();
        assert_eq!(base_ifaces.len(), 1);
        assert_eq!(base_ifaces[0]["path"], "src/base.ts>BaseIFace");
    }

    #[tokio::test]
    async fn get_implements_tree_leaf_at_max_depth_zero() {
        let server = server_with_implements_fixture().await;
        let raw = server
            .mycelium_get_implements_tree(Parameters(GetImplementsTreeRequest {
                path: "src/cls.ts>Cls".to_string(),
                max_depth: Some(0),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let root = val.get("root").unwrap();
        assert!(root["interfaces"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn get_implements_tree_unknown_path_returns_error() {
        let server = server_with_implements_fixture().await;
        let raw = server
            .mycelium_get_implements_tree(Parameters(GetImplementsTreeRequest {
                path: "no/such>path".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    // ── RFC-0035: mycelium_get_implementors_tree ──────────────────────────

    #[tokio::test]
    async fn get_implementors_tree_returns_implementor_chain() {
        // Fixture: Cls→IFace→BaseIFace (Cls implements IFace, IFace implements BaseIFace)
        // From BaseIFace perspective: BaseIFace←IFace←Cls
        let server = server_with_implements_fixture().await;
        let raw = server
            .mycelium_get_implementors_tree(Parameters(GetImplementorsTreeRequest {
                path: "src/base.ts>BaseIFace".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let root = val.get("root").expect("root key present");
        assert_eq!(root["path"], "src/base.ts>BaseIFace");
        let impls = root["implementors"].as_array().unwrap();
        assert_eq!(impls.len(), 1);
        assert_eq!(impls[0]["path"], "src/iface.ts>IFace");
        let cls_impls = impls[0]["implementors"].as_array().unwrap();
        assert_eq!(cls_impls.len(), 1);
        assert_eq!(cls_impls[0]["path"], "src/cls.ts>Cls");
    }

    #[tokio::test]
    async fn get_implementors_tree_leaf_at_max_depth_zero() {
        let server = server_with_implements_fixture().await;
        let raw = server
            .mycelium_get_implementors_tree(Parameters(GetImplementorsTreeRequest {
                path: "src/base.ts>BaseIFace".to_string(),
                max_depth: Some(0),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let root = val.get("root").unwrap();
        assert!(root["implementors"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn get_implementors_tree_unknown_path_returns_error() {
        let server = server_with_implements_fixture().await;
        let raw = server
            .mycelium_get_implementors_tree(Parameters(GetImplementorsTreeRequest {
                path: "no/such>path".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    // ── RFC-0036: mycelium_get_importers_tree ─────────────────────────────

    async fn server_with_imports_fixture() -> MyceliumServer {
        let server = MyceliumServer::new();
        {
            let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
            let core_mod = store
                .upsert_node(mycelium_core::trunk::TrunkPath::parse("src/core.ts>core").unwrap());
            let mid_mod = store
                .upsert_node(mycelium_core::trunk::TrunkPath::parse("src/mid.ts>mid").unwrap());
            let top_mod = store
                .upsert_node(mycelium_core::trunk::TrunkPath::parse("src/top.ts>top").unwrap());
            store.upsert_edge(mycelium_core::EdgeKind::Imports, mid_mod, core_mod);
            store.upsert_edge(mycelium_core::EdgeKind::Imports, top_mod, mid_mod);
        }
        server
    }

    #[tokio::test]
    async fn get_importers_tree_returns_reverse_dependency_chain() {
        let server = server_with_imports_fixture().await;
        let raw = server
            .mycelium_get_importers_tree(Parameters(GetImportersTreeRequest {
                path: "src/core.ts>core".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let root = val.get("root").expect("root key present");
        assert_eq!(root["path"], "src/core.ts>core");
        let importers = root["importers"].as_array().unwrap();
        assert_eq!(importers.len(), 1);
        assert_eq!(importers[0]["path"], "src/mid.ts>mid");
        let top_importers = importers[0]["importers"].as_array().unwrap();
        assert_eq!(top_importers.len(), 1);
        assert_eq!(top_importers[0]["path"], "src/top.ts>top");
    }

    #[tokio::test]
    async fn get_importers_tree_leaf_at_max_depth_zero() {
        let server = server_with_imports_fixture().await;
        let raw = server
            .mycelium_get_importers_tree(Parameters(GetImportersTreeRequest {
                path: "src/core.ts>core".to_string(),
                max_depth: Some(0),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let root = val.get("root").unwrap();
        assert!(root["importers"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn get_importers_tree_unknown_path_returns_error() {
        let server = server_with_imports_fixture().await;
        let raw = server
            .mycelium_get_importers_tree(Parameters(GetImportersTreeRequest {
                path: "no/such>path".to_string(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    // ── RFC-0037: mycelium_get_dead_symbols ──────────────────────────────

    async fn server_with_mixed_fixture() -> MyceliumServer {
        let server = MyceliumServer::new();
        let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
        // live: caller calls target
        let caller = store.upsert_node(TrunkPath::parse("src/main.rs>main").unwrap());
        let target = store.upsert_node(TrunkPath::parse("src/lib.rs>helper").unwrap());
        store.upsert_edge(EdgeKind::Calls, caller, target);
        // dead: no callers, no importers
        store.upsert_node(TrunkPath::parse("src/lib.rs>dead_fn").unwrap());
        // file node (should be excluded from dead_symbols)
        store.upsert_node(TrunkPath::parse("src/lib.rs").unwrap());
        drop(store);
        server
    }

    #[tokio::test]
    async fn get_dead_symbols_returns_unreferenced_symbols() {
        let server = server_with_mixed_fixture().await;
        let raw = server
            .mycelium_get_dead_symbols(Parameters(GetDeadSymbolsRequest { path_prefix: None }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let dead: Vec<String> = val["dead_symbols"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect();
        // main has no callers → dead; helper is called → live; dead_fn → dead
        assert!(dead.contains(&"src/lib.rs>dead_fn".to_owned()));
        assert!(dead.contains(&"src/main.rs>main".to_owned()));
        assert!(!dead.contains(&"src/lib.rs>helper".to_owned()));
        // file node must not appear
        assert!(!dead.iter().any(|s| s == "src/lib.rs"));
        assert_eq!(val["count"].as_u64().unwrap(), dead.len() as u64);
    }

    #[tokio::test]
    async fn get_dead_symbols_empty_store() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_dead_symbols(Parameters(GetDeadSymbolsRequest { path_prefix: None }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["dead_symbols"].as_array().unwrap().len(), 0);
        assert_eq!(val["count"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn get_dead_symbols_prefix_filter() {
        let server = server_with_mixed_fixture().await;
        let raw = server
            .mycelium_get_dead_symbols(Parameters(GetDeadSymbolsRequest {
                path_prefix: Some("src/lib.rs".to_owned()),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let dead: Vec<String> = val["dead_symbols"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect();
        // Only dead symbols under src/lib.rs prefix
        assert!(dead.iter().all(|s| s.starts_with("src/lib.rs")));
        assert!(dead.contains(&"src/lib.rs>dead_fn".to_owned()));
        assert!(!dead.contains(&"src/main.rs>main".to_owned()));
    }

    // ── RFC-0056: mycelium_get_isolated_symbols ───────────────────────────

    #[tokio::test]
    async fn get_isolated_symbols_returns_completely_disconnected() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let _orphan = store.upsert_node(TrunkPath::parse("src/orphan.rs>orphan").unwrap());
            let conn_a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let conn_b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            store.upsert_edge(EdgeKind::Calls, conn_a, conn_b);
        }
        let raw = server
            .mycelium_get_isolated_symbols(Parameters(GetIsolatedSymbolsRequest {
                path_prefix: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 1);
        assert_eq!(
            val["isolated_symbols"].as_array().unwrap()[0]
                .as_str()
                .unwrap(),
            "src/orphan.rs>orphan"
        );
    }

    #[tokio::test]
    async fn get_isolated_symbols_empty_store() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_isolated_symbols(Parameters(GetIsolatedSymbolsRequest {
                path_prefix: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn get_isolated_symbols_prefix_filter() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let _src_orphan = store.upsert_node(TrunkPath::parse("src/orphan.rs>orphan").unwrap());
            let _lib_orphan = store.upsert_node(TrunkPath::parse("lib/orphan.rs>orphan").unwrap());
        }
        let raw = server
            .mycelium_get_isolated_symbols(Parameters(GetIsolatedSymbolsRequest {
                path_prefix: Some("src/".to_owned()),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 1);
        assert!(
            val["isolated_symbols"].as_array().unwrap()[0]
                .as_str()
                .unwrap()
                .starts_with("src/")
        );
    }

    // ── RFC-0038: mycelium_get_stats ─────────────────────────────────────

    #[tokio::test]
    async fn get_stats_empty_store() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_stats(Parameters(GetStatsRequest {}))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["total_nodes"].as_u64().unwrap(), 0);
        assert_eq!(val["total_edges"].as_u64().unwrap(), 0);
        assert_eq!(val["nodes_by_kind"].as_object().unwrap().len(), 0);
        assert_eq!(val["edges_by_kind"].as_object().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn get_stats_counts_nodes_and_edges() {
        let server = MyceliumServer::new();
        {
            let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
            let fn1 = store.upsert_node(TrunkPath::parse("src/a.rs>fn1").unwrap());
            let fn2 = store.upsert_node(TrunkPath::parse("src/b.rs>fn2").unwrap());
            store.set_kind(fn1, mycelium_core::NodeKind::Function);
            store.set_kind(fn2, mycelium_core::NodeKind::Function);
            store.upsert_edge(EdgeKind::Calls, fn1, fn2);
            store.upsert_edge(EdgeKind::Imports, fn1, fn2);
        }
        let raw = server
            .mycelium_get_stats(Parameters(GetStatsRequest {}))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["total_nodes"].as_u64().unwrap(), 2);
        assert_eq!(val["total_edges"].as_u64().unwrap(), 2);
        assert_eq!(val["nodes_by_kind"]["function"].as_u64().unwrap(), 2);
        assert_eq!(val["edges_by_kind"]["calls"].as_u64().unwrap(), 1);
        assert_eq!(val["edges_by_kind"]["imports"].as_u64().unwrap(), 1);
        assert!(val["edges_by_kind"].get("contains").is_none());
    }

    // ── RFC-0039: mycelium_get_cross_refs ────────────────────────────────

    #[tokio::test]
    async fn get_cross_refs_all_kinds() {
        let server = MyceliumServer::new();
        {
            let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
            let target = store.upsert_node(TrunkPath::parse("src/lib.rs>Base").unwrap());
            let caller = store.upsert_node(TrunkPath::parse("src/a.rs>caller").unwrap());
            let importer = store.upsert_node(TrunkPath::parse("src/b.rs>importer").unwrap());
            let child = store.upsert_node(TrunkPath::parse("src/c.rs>Child").unwrap());
            let impl_sym = store.upsert_node(TrunkPath::parse("src/d.rs>Impl").unwrap());
            store.upsert_edge(EdgeKind::Calls, caller, target);
            store.upsert_edge(EdgeKind::Imports, importer, target);
            store.upsert_edge(EdgeKind::Extends, child, target);
            store.upsert_edge(EdgeKind::Implements, impl_sym, target);
        }
        let raw = server
            .mycelium_get_cross_refs(Parameters(GetCrossRefsRequest {
                path: "src/lib.rs>Base".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["callers"][0].as_str().unwrap(), "src/a.rs>caller");
        assert_eq!(val["importers"][0].as_str().unwrap(), "src/b.rs>importer");
        assert_eq!(val["extended_by"][0].as_str().unwrap(), "src/c.rs>Child");
        assert_eq!(val["implemented_by"][0].as_str().unwrap(), "src/d.rs>Impl");
    }

    #[tokio::test]
    async fn get_cross_refs_empty_lists_present() {
        let server = MyceliumServer::new();
        {
            let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/lone.rs>lone").unwrap());
        }
        let raw = server
            .mycelium_get_cross_refs(Parameters(GetCrossRefsRequest {
                path: "src/lone.rs>lone".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["callers"].as_array().unwrap().len(), 0);
        assert_eq!(val["importers"].as_array().unwrap().len(), 0);
        assert_eq!(val["extended_by"].as_array().unwrap().len(), 0);
        assert_eq!(val["implemented_by"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn get_cross_refs_unknown_path_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_cross_refs(Parameters(GetCrossRefsRequest {
                path: "no/such>path".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    // ── RFC-0040: mycelium_detect_cycles ─────────────────────────────────

    #[tokio::test]
    async fn detect_cycles_finds_circular_imports() {
        let server = MyceliumServer::new();
        {
            let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            store.upsert_edge(EdgeKind::Imports, a, b);
            store.upsert_edge(EdgeKind::Imports, b, a); // cycle
        }
        let raw = server
            .mycelium_detect_cycles(Parameters(DetectCyclesRequest {
                edge_kind: "imports".to_owned(),
                path_prefix: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let nodes: Vec<String> = val["cycle_nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect();
        assert_eq!(nodes.len(), 2);
        assert!(nodes.contains(&"src/a.rs>a".to_owned()));
        assert!(nodes.contains(&"src/b.rs>b".to_owned()));
        assert_eq!(val["count"].as_u64().unwrap(), 2);
    }

    #[tokio::test]
    async fn detect_cycles_no_cycles_returns_empty() {
        let server = MyceliumServer::new();
        {
            let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            store.upsert_edge(EdgeKind::Imports, a, b); // no cycle
        }
        let raw = server
            .mycelium_detect_cycles(Parameters(DetectCyclesRequest {
                edge_kind: "imports".to_owned(),
                path_prefix: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["cycle_nodes"].as_array().unwrap().len(), 0);
        assert_eq!(val["count"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn detect_cycles_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_detect_cycles(Parameters(DetectCyclesRequest {
                edge_kind: "unknown_kind".to_owned(),
                path_prefix: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    // ── RFC-0057: mycelium_get_scc_groups ────────────────────────────────

    #[tokio::test]
    async fn get_scc_groups_finds_cycle_group() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let sym_a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let sym_b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            store.upsert_edge(EdgeKind::Calls, sym_a, sym_b);
            store.upsert_edge(EdgeKind::Calls, sym_b, sym_a);
        }
        let raw = server
            .mycelium_get_scc_groups(Parameters(GetSccGroupsRequest {
                edge_kind: "calls".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["group_count"].as_u64().unwrap(), 1);
        assert_eq!(val["total_symbols"].as_u64().unwrap(), 2);
        let group = val["groups"].as_array().unwrap()[0].as_array().unwrap();
        assert_eq!(group.len(), 2);
    }

    #[tokio::test]
    async fn get_scc_groups_no_cycles_returns_empty() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let sym_a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let sym_b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            store.upsert_edge(EdgeKind::Calls, sym_a, sym_b);
        }
        let raw = server
            .mycelium_get_scc_groups(Parameters(GetSccGroupsRequest {
                edge_kind: "calls".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["group_count"].as_u64().unwrap(), 0);
        assert_eq!(val["total_symbols"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn get_scc_groups_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_scc_groups(Parameters(GetSccGroupsRequest {
                edge_kind: "bad".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
    }

    // ── RFC-0058: mycelium_get_dependency_layers ─────────────────────────

    #[tokio::test]
    async fn get_dependency_layers_simple_chain() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let sym_a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let sym_b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            let sym_c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
            // c → b → a
            store.upsert_edge(EdgeKind::Calls, sym_b, sym_a);
            store.upsert_edge(EdgeKind::Calls, sym_c, sym_b);
        }
        let raw = server
            .mycelium_get_dependency_layers(Parameters(GetDependencyLayersRequest {
                edge_kind: "calls".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["layer_count"].as_u64().unwrap(), 3);
        assert_eq!(val["total_symbols"].as_u64().unwrap(), 3);
        assert_eq!(val["cycle_excluded_count"].as_u64().unwrap(), 0);
        let layers = val["layers"].as_array().unwrap();
        assert_eq!(
            layers[0].as_array().unwrap()[0].as_str().unwrap(),
            "src/a.rs>a"
        );
    }

    #[tokio::test]
    async fn get_dependency_layers_cycle_excluded() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let sym_a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let sym_b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            // cycle: a ↔ b
            store.upsert_edge(EdgeKind::Calls, sym_a, sym_b);
            store.upsert_edge(EdgeKind::Calls, sym_b, sym_a);
        }
        let raw = server
            .mycelium_get_dependency_layers(Parameters(GetDependencyLayersRequest {
                edge_kind: "calls".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["layer_count"].as_u64().unwrap(), 0);
        assert_eq!(val["cycle_excluded_count"].as_u64().unwrap(), 2);
    }

    #[tokio::test]
    async fn get_dependency_layers_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_dependency_layers(Parameters(GetDependencyLayersRequest {
                edge_kind: "bad".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
    }

    // ── RFC-0059: mycelium_get_two_hop_neighbors ─────────────────────────

    #[tokio::test]
    async fn get_two_hop_neighbors_basic() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let sym_a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let sym_b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            let sym_c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
            // a → b → c
            store.upsert_edge(EdgeKind::Calls, sym_a, sym_b);
            store.upsert_edge(EdgeKind::Calls, sym_b, sym_c);
        }
        let raw = server
            .mycelium_get_two_hop_neighbors(Parameters(GetTwoHopNeighborsRequest {
                path: "src/a.rs>a".to_owned(),
                edge_kind: "calls".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 1);
        assert_eq!(
            val["neighbors"].as_array().unwrap()[0].as_str().unwrap(),
            "src/c.rs>c"
        );
    }

    #[tokio::test]
    async fn get_two_hop_neighbors_unknown_path_returns_empty() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_two_hop_neighbors(Parameters(GetTwoHopNeighborsRequest {
                path: "nonexistent.rs>x".to_owned(),
                edge_kind: "calls".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
        assert!(val["neighbors"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn get_two_hop_neighbors_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_two_hop_neighbors(Parameters(GetTwoHopNeighborsRequest {
                path: "src/a.rs>a".to_owned(),
                edge_kind: "bad".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
    }

    // ── RFC-0060: mycelium_get_symbol_neighborhood ───────────────────────

    #[tokio::test]
    async fn get_symbol_neighborhood_basic() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let main = store.upsert_node(TrunkPath::parse("src/main.rs>main").unwrap());
            let svc = store.upsert_node(TrunkPath::parse("src/svc.rs>svc").unwrap());
            let util = store.upsert_node(TrunkPath::parse("src/util.rs>util").unwrap());
            store.upsert_edge(EdgeKind::Calls, main, svc);
            store.upsert_edge(EdgeKind::Calls, svc, util);
        }
        let raw = server
            .mycelium_get_symbol_neighborhood(Parameters(GetSymbolNeighborhoodRequest {
                path: "src/svc.rs>svc".to_owned(),
                edge_kind: "calls".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["path"].as_str().unwrap(), "src/svc.rs>svc");
        assert_eq!(val["incoming_count"].as_u64().unwrap(), 1);
        assert_eq!(val["outgoing_count"].as_u64().unwrap(), 1);
        assert_eq!(val["incoming"][0].as_str().unwrap(), "src/main.rs>main");
        assert_eq!(val["outgoing"][0].as_str().unwrap(), "src/util.rs>util");
    }

    #[tokio::test]
    async fn get_symbol_neighborhood_unknown_path_returns_empty() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_symbol_neighborhood(Parameters(GetSymbolNeighborhoodRequest {
                path: "nonexistent.rs>x".to_owned(),
                edge_kind: "calls".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["path"].as_str().unwrap(), "");
        assert_eq!(val["incoming_count"].as_u64().unwrap(), 0);
        assert_eq!(val["outgoing_count"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn get_symbol_neighborhood_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_symbol_neighborhood(Parameters(GetSymbolNeighborhoodRequest {
                path: "src/a.rs>a".to_owned(),
                edge_kind: "bad".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
    }

    // ── RFC-0061: mycelium_get_hub_symbols ───────────────────────────────

    #[tokio::test]
    async fn get_hub_symbols_basic() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let hub = store.upsert_node(TrunkPath::parse("src/hub.rs>hub").unwrap());
            // 2 callers, 2 callees
            for i in 0..2_u32 {
                let c = store.upsert_node(TrunkPath::parse(&format!("src/c{i}.rs>c{i}")).unwrap());
                store.upsert_edge(EdgeKind::Calls, c, hub);
            }
            for i in 0..2_u32 {
                let d = store.upsert_node(TrunkPath::parse(&format!("src/d{i}.rs>d{i}")).unwrap());
                store.upsert_edge(EdgeKind::Calls, hub, d);
            }
        }
        let raw = server
            .mycelium_get_hub_symbols(Parameters(GetHubSymbolsRequest {
                edge_kind: "calls".to_owned(),
                min_in: Some(2),
                min_out: Some(2),
                limit: Some(10),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 1);
        assert_eq!(val["hubs"][0]["path"].as_str().unwrap(), "src/hub.rs>hub");
        assert_eq!(val["hubs"][0]["in_degree"].as_u64().unwrap(), 2);
        assert_eq!(val["hubs"][0]["out_degree"].as_u64().unwrap(), 2);
    }

    #[tokio::test]
    async fn get_hub_symbols_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_hub_symbols(Parameters(GetHubSymbolsRequest {
                edge_kind: "bad".to_owned(),
                min_in: None,
                min_out: None,
                limit: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
    }

    #[tokio::test]
    async fn get_hub_symbols_empty_store_returns_empty() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_hub_symbols(Parameters(GetHubSymbolsRequest {
                edge_kind: "calls".to_owned(),
                min_in: None,
                min_out: None,
                limit: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
        assert!(val["hubs"].as_array().unwrap().is_empty());
    }

    // ── RFC-0062: mycelium_get_singly_referenced ──────────────────────────

    #[tokio::test]
    async fn get_singly_referenced_basic() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let caller = store.upsert_node(TrunkPath::parse("src/main.rs>main").unwrap());
            let tgt = store.upsert_node(TrunkPath::parse("src/util.rs>helper").unwrap());
            store.upsert_edge(EdgeKind::Calls, caller, tgt);
        }
        let raw = server
            .mycelium_get_singly_referenced(Parameters(GetSinglyReferencedRequest {
                edge_kind: "calls".to_owned(),
                limit: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 1);
        let syms = val["symbols"].as_array().unwrap();
        assert_eq!(syms[0]["path"].as_str().unwrap(), "src/util.rs>helper");
        assert_eq!(
            syms[0]["referenced_by"].as_str().unwrap(),
            "src/main.rs>main"
        );
    }

    #[tokio::test]
    async fn get_singly_referenced_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_singly_referenced(Parameters(GetSinglyReferencedRequest {
                edge_kind: "unknown_kind".to_owned(),
                limit: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
    }

    #[tokio::test]
    async fn get_singly_referenced_empty_store_returns_empty() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_singly_referenced(Parameters(GetSinglyReferencedRequest {
                edge_kind: "calls".to_owned(),
                limit: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
        assert!(val["symbols"].as_array().unwrap().is_empty());
    }

    // ── RFC-0063: mycelium_batch_reachable_to ─────────────────────────────

    #[tokio::test]
    async fn batch_reachable_to_basic() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let tgt = store.upsert_node(TrunkPath::parse("src/util.rs>helper").unwrap());
            let mid = store.upsert_node(TrunkPath::parse("src/mid.rs>mid").unwrap());
            store.upsert_edge(EdgeKind::Calls, mid, tgt);
        }
        let raw = server
            .mycelium_batch_reachable_to(Parameters(BatchReachableToRequest {
                paths: vec!["src/util.rs>helper".to_owned()],
                edge_kind: "calls".to_owned(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 1);
        let reachable = val["reachable"].as_array().unwrap();
        assert_eq!(reachable[0].as_str().unwrap(), "src/mid.rs>mid");
    }

    #[tokio::test]
    async fn batch_reachable_to_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_batch_reachable_to(Parameters(BatchReachableToRequest {
                paths: vec!["src/a.rs>a".to_owned()],
                edge_kind: "bogus".to_owned(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
    }

    #[tokio::test]
    async fn batch_reachable_to_empty_paths_returns_empty() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_batch_reachable_to(Parameters(BatchReachableToRequest {
                paths: vec![],
                edge_kind: "calls".to_owned(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
        assert!(val["reachable"].as_array().unwrap().is_empty());
    }

    // ── RFC-0064: mycelium_get_k_core ─────────────────────────────────────

    #[tokio::test]
    async fn get_k_core_basic_triangle() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, b, c);
            store.upsert_edge(EdgeKind::Calls, c, a);
        }
        let raw = server
            .mycelium_get_k_core(Parameters(GetKCoreRequest {
                edge_kind: "calls".to_owned(),
                k: Some(2),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 3);
        assert_eq!(val["k"].as_u64().unwrap(), 2);
    }

    #[tokio::test]
    async fn get_k_core_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_k_core(Parameters(GetKCoreRequest {
                edge_kind: "nope".to_owned(),
                k: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
    }

    #[tokio::test]
    async fn get_k_core_empty_store_returns_empty() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_k_core(Parameters(GetKCoreRequest {
                edge_kind: "calls".to_owned(),
                k: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
        assert!(val["core"].as_array().unwrap().is_empty());
    }

    // ── RFC-0065: mycelium_batch_reachable_from ───────────────────────────

    #[tokio::test]
    async fn batch_reachable_from_basic() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let src = store.upsert_node(TrunkPath::parse("src/top.rs>top").unwrap());
            let leaf = store.upsert_node(TrunkPath::parse("src/leaf.rs>leaf").unwrap());
            store.upsert_edge(EdgeKind::Calls, src, leaf);
        }
        let raw = server
            .mycelium_batch_reachable_from(Parameters(BatchReachableFromRequest {
                paths: vec!["src/top.rs>top".to_owned()],
                edge_kind: "calls".to_owned(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 1);
        let reachable = val["reachable"].as_array().unwrap();
        assert_eq!(reachable[0].as_str().unwrap(), "src/leaf.rs>leaf");
    }

    #[tokio::test]
    async fn batch_reachable_from_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_batch_reachable_from(Parameters(BatchReachableFromRequest {
                paths: vec!["src/a.rs>a".to_owned()],
                edge_kind: "invalid".to_owned(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
    }

    #[tokio::test]
    async fn batch_reachable_from_empty_paths_returns_empty() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_batch_reachable_from(Parameters(BatchReachableFromRequest {
                paths: vec![],
                edge_kind: "calls".to_owned(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
        assert!(val["reachable"].as_array().unwrap().is_empty());
    }

    // ── RFC-0066: mycelium_batch_node_degree ──────────────────────────────

    #[tokio::test]
    async fn batch_node_degree_known_path() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
        }
        let raw = server
            .mycelium_batch_node_degree(Parameters(BatchNodeDegreeRequest {
                paths: vec!["src/a.rs>a".to_owned(), "src/b.rs>b".to_owned()],
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 2);
        let degs = val["degrees"].as_array().unwrap();
        assert_eq!(degs[0]["out_calls"].as_u64().unwrap(), 1);
        assert_eq!(degs[1]["in_calls"].as_u64().unwrap(), 1);
    }

    #[tokio::test]
    async fn batch_node_degree_unknown_path_returns_error_entry() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_batch_node_degree(Parameters(BatchNodeDegreeRequest {
                paths: vec!["src/missing.rs>nope".to_owned()],
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let degs = val["degrees"].as_array().unwrap();
        assert!(degs[0]["error"].as_str().is_some());
    }

    #[tokio::test]
    async fn batch_node_degree_empty_paths_returns_empty() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_batch_node_degree(Parameters(BatchNodeDegreeRequest { paths: vec![] }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
        assert!(val["degrees"].as_array().unwrap().is_empty());
    }

    // ── RFC-0067: mycelium_find_cycle_members ─────────────────────────────

    #[tokio::test]
    async fn find_cycle_members_returns_cycle_symbols() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, b, a);
        }
        let raw = server
            .mycelium_find_cycle_members(Parameters(FindCycleMembersRequest {
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 2);
        let members: Vec<&str> = val["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(members.contains(&"src/a.rs>a"));
        assert!(members.contains(&"src/b.rs>b"));
    }

    #[tokio::test]
    async fn find_cycle_members_no_cycle_returns_empty() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
        }
        let raw = server
            .mycelium_find_cycle_members(Parameters(FindCycleMembersRequest {
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
        assert!(val["members"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn find_cycle_members_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_find_cycle_members(Parameters(FindCycleMembersRequest {
                edge_kind: "unknown_kind".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0068: mycelium_get_wcc ─────────────────────────────────────────

    #[tokio::test]
    async fn wcc_two_disjoint_components() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
            let d = store.upsert_node(TrunkPath::parse("src/d.rs>d").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, c, d);
        }
        let raw = server
            .mycelium_get_wcc(Parameters(GetWccRequest {
                edge_kind: "calls".into(),
                min_size: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["component_count"].as_u64().unwrap(), 2);
        assert_eq!(val["total_symbols"].as_u64().unwrap(), 4);
    }

    #[tokio::test]
    async fn wcc_min_size_filter() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            store.upsert_node(TrunkPath::parse("src/lone.rs>lone").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
        }
        let raw = server
            .mycelium_get_wcc(Parameters(GetWccRequest {
                edge_kind: "calls".into(),
                min_size: Some(2),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        // singleton lone filtered out
        assert_eq!(val["component_count"].as_u64().unwrap(), 1);
        assert_eq!(val["total_symbols"].as_u64().unwrap(), 2);
    }

    #[tokio::test]
    async fn wcc_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_wcc(Parameters(GetWccRequest {
                edge_kind: "bad_kind".into(),
                min_size: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0069: mycelium_topological_sort ───────────────────────────────

    #[tokio::test]
    async fn topo_sort_linear_chain_ordered() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, b, c);
        }
        let raw = server
            .mycelium_topological_sort(Parameters(TopologicalSortRequest {
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["ordered_count"].as_u64().unwrap(), 3);
        assert_eq!(val["cycle_count"].as_u64().unwrap(), 0);
        let order: Vec<&str> = val["order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let pos_a = order.iter().position(|&s| s == "src/a.rs>a").unwrap();
        let pos_b = order.iter().position(|&s| s == "src/b.rs>b").unwrap();
        let pos_c = order.iter().position(|&s| s == "src/c.rs>c").unwrap();
        assert!(pos_a < pos_b && pos_b < pos_c);
    }

    #[tokio::test]
    async fn topo_sort_cycle_surfaced() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, b, a);
        }
        let raw = server
            .mycelium_topological_sort(Parameters(TopologicalSortRequest {
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["ordered_count"].as_u64().unwrap(), 0);
        assert_eq!(val["cycle_count"].as_u64().unwrap(), 2);
    }

    #[tokio::test]
    async fn topo_sort_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_topological_sort(Parameters(TopologicalSortRequest {
                edge_kind: "bad".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0070: mycelium_find_articulation_points ───────────────────────

    #[tokio::test]
    async fn articulation_points_bridge_node_found() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, b, c);
        }
        let raw = server
            .mycelium_find_articulation_points(Parameters(FindArticulationPointsRequest {
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 1);
        assert!(
            val["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .any(|x| x == "src/b.rs>b")
        );
    }

    #[tokio::test]
    async fn articulation_points_cycle_returns_none() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, b, c);
            store.upsert_edge(EdgeKind::Calls, c, a);
        }
        let raw = server
            .mycelium_find_articulation_points(Parameters(FindArticulationPointsRequest {
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn articulation_points_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_find_articulation_points(Parameters(FindArticulationPointsRequest {
                edge_kind: "unknown".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0071: mycelium_find_bridge_edges ──────────────────────────────

    #[tokio::test]
    async fn bridge_edges_chain_returns_bridges() {
        // a — b — c: both edges are bridges
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, b, c);
        }
        let raw = server
            .mycelium_find_bridge_edges(Parameters(FindBridgeEdgesRequest {
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 2);
        let arr = val["bridges"].as_array().unwrap();
        assert!(
            arr.iter()
                .any(|b| b["from"] == "src/a.rs>a" && b["to"] == "src/b.rs>b")
        );
        assert!(
            arr.iter()
                .any(|b| b["from"] == "src/b.rs>b" && b["to"] == "src/c.rs>c")
        );
    }

    #[tokio::test]
    async fn bridge_edges_cycle_returns_none() {
        // a → b → c → a: no bridges in a cycle
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, b, c);
            store.upsert_edge(EdgeKind::Calls, c, a);
        }
        let raw = server
            .mycelium_find_bridge_edges(Parameters(FindBridgeEdgesRequest {
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn bridge_edges_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_find_bridge_edges(Parameters(FindBridgeEdgesRequest {
                edge_kind: "unknown".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0072: mycelium_get_biconnected_components ─────────────────────

    #[tokio::test]
    async fn bcc_triangle_is_one_component() {
        // a → b → c → a: one BCC of 3 nodes
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, b, c);
            store.upsert_edge(EdgeKind::Calls, c, a);
        }
        let raw = server
            .mycelium_get_biconnected_components(Parameters(BiconnectedComponentsRequest {
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["component_count"].as_u64().unwrap(), 1);
        assert_eq!(val["total_symbols"].as_u64().unwrap(), 3);
    }

    #[tokio::test]
    async fn bcc_singleton_returns_empty() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        }
        let raw = server
            .mycelium_get_biconnected_components(Parameters(BiconnectedComponentsRequest {
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["component_count"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn bcc_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_biconnected_components(Parameters(BiconnectedComponentsRequest {
                edge_kind: "unknown".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0073: mycelium_get_degree_histogram ───────────────────────────

    #[tokio::test]
    async fn degree_histogram_counts_correct() {
        // a → b, a → c: a has out=2; b and c have in=1; a has in=0
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, a, c);
        }
        let raw = server
            .mycelium_get_degree_histogram(Parameters(DegreeHistogramRequest {
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["total_symbols"].as_u64().unwrap(), 3);
        let in_arr = val["in_degrees"].as_array().unwrap();
        let in_sum: u64 = in_arr.iter().map(|e| e["count"].as_u64().unwrap()).sum();
        assert_eq!(in_sum, 3);
    }

    #[tokio::test]
    async fn degree_histogram_empty_returns_zero() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_degree_histogram(Parameters(DegreeHistogramRequest {
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["total_symbols"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn degree_histogram_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_degree_histogram(Parameters(DegreeHistogramRequest {
                edge_kind: "unknown".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0074: mycelium_get_graph_metrics ──────────────────────────────

    #[tokio::test]
    async fn graph_metrics_basic_counts() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
        }
        let raw = server
            .mycelium_get_graph_metrics(Parameters(GraphMetricsRequest {
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["symbol_count"].as_u64().unwrap(), 2);
        assert_eq!(val["directed_edge_count"].as_u64().unwrap(), 1);
        assert_eq!(val["max_out_degree"].as_u64().unwrap(), 1);
        assert_eq!(val["max_in_degree"].as_u64().unwrap(), 1);
    }

    #[tokio::test]
    async fn graph_metrics_empty_returns_zeros() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_graph_metrics(Parameters(GraphMetricsRequest {
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["symbol_count"].as_u64().unwrap(), 0);
        assert!(val["density"].as_f64().unwrap().abs() < 1e-15);
    }

    #[tokio::test]
    async fn graph_metrics_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_graph_metrics(Parameters(GraphMetricsRequest {
                edge_kind: "unknown".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0075: mycelium_get_neighbor_similarity ────────────────────────

    #[tokio::test]
    async fn neighbor_similarity_identical_neighbors_returns_one() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            // a and b both call hub; hub is shared neighbor
            let a = store.upsert_node(TrunkPath::parse("src/x.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/x.rs>b").unwrap());
            let hub = store.upsert_node(TrunkPath::parse("src/x.rs>hub").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, hub);
            store.upsert_edge(EdgeKind::Calls, b, hub);
        }
        let raw = server
            .mycelium_get_neighbor_similarity(Parameters(NeighborSimilarityRequest {
                path1: "src/x.rs>a".into(),
                path2: "src/x.rs>b".into(),
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let sim = val["similarity"].as_f64().unwrap();
        assert!((sim - 1.0).abs() < 1e-9, "expected 1.0, got {sim}");
        assert_eq!(val["shared"].as_u64().unwrap(), 1);
        assert_eq!(val["total"].as_u64().unwrap(), 1);
    }

    #[tokio::test]
    async fn neighbor_similarity_no_overlap_returns_zero() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/y.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/y.rs>b").unwrap());
            let na = store.upsert_node(TrunkPath::parse("src/y.rs>na").unwrap());
            let nb = store.upsert_node(TrunkPath::parse("src/y.rs>nb").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, na);
            store.upsert_edge(EdgeKind::Calls, b, nb);
        }
        let raw = server
            .mycelium_get_neighbor_similarity(Parameters(NeighborSimilarityRequest {
                path1: "src/y.rs>a".into(),
                path2: "src/y.rs>b".into(),
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let sim = val["similarity"].as_f64().unwrap();
        assert!(sim.abs() < 1e-9, "expected 0.0, got {sim}");
        assert_eq!(val["shared"].as_u64().unwrap(), 0);
        assert_eq!(val["total"].as_u64().unwrap(), 2);
    }

    #[tokio::test]
    async fn neighbor_similarity_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_neighbor_similarity(Parameters(NeighborSimilarityRequest {
                path1: "src/z.rs>a".into(),
                path2: "src/z.rs>b".into(),
                edge_kind: "unknown".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0076: mycelium_get_clustering_coefficient ─────────────────────

    #[tokio::test]
    async fn clustering_coefficient_complete_triangle() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let hub = store.upsert_node(TrunkPath::parse("src/cc.rs>hub").unwrap());
            let alpha = store.upsert_node(TrunkPath::parse("src/cc.rs>alpha").unwrap());
            let beta = store.upsert_node(TrunkPath::parse("src/cc.rs>beta").unwrap());
            store.upsert_edge(EdgeKind::Calls, hub, alpha);
            store.upsert_edge(EdgeKind::Calls, hub, beta);
            store.upsert_edge(EdgeKind::Calls, alpha, beta);
            store.upsert_edge(EdgeKind::Calls, beta, alpha);
        }
        let raw = server
            .mycelium_get_clustering_coefficient(Parameters(ClusteringCoefficientRequest {
                path: "src/cc.rs>hub".into(),
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let coeff = val["coefficient"].as_f64().unwrap();
        assert!((coeff - 1.0).abs() < 1e-9, "expected 1.0, got {coeff}");
        assert_eq!(val["neighbor_count"].as_u64().unwrap(), 2);
        assert_eq!(val["neighbor_edge_count"].as_u64().unwrap(), 2);
    }

    #[tokio::test]
    async fn clustering_coefficient_unknown_path_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_clustering_coefficient(Parameters(ClusteringCoefficientRequest {
                path: "src/no_such.rs>ghost".into(),
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown path"));
    }

    #[tokio::test]
    async fn clustering_coefficient_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_clustering_coefficient(Parameters(ClusteringCoefficientRequest {
                path: "src/any.rs>any".into(),
                edge_kind: "unknown".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0077: mycelium_get_eccentricity ──────────────────────────────────

    #[tokio::test]
    async fn eccentricity_chain_returns_correct_depth() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let root = store.upsert_node(TrunkPath::parse("src/ecc.rs>root").unwrap());
            let mid = store.upsert_node(TrunkPath::parse("src/ecc.rs>mid").unwrap());
            let far = store.upsert_node(TrunkPath::parse("src/ecc.rs>far").unwrap());
            store.upsert_edge(EdgeKind::Calls, root, mid);
            store.upsert_edge(EdgeKind::Calls, mid, far);
        }
        let raw = server
            .mycelium_get_eccentricity(Parameters(EccentricityRequest {
                path: "src/ecc.rs>root".into(),
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["eccentricity"].as_u64().unwrap(), 2);
        assert_eq!(val["reachable_count"].as_u64().unwrap(), 2);
    }

    #[tokio::test]
    async fn eccentricity_unknown_path_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_eccentricity(Parameters(EccentricityRequest {
                path: "src/ghost.rs>none".into(),
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown path"));
    }

    #[tokio::test]
    async fn eccentricity_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_eccentricity(Parameters(EccentricityRequest {
                path: "src/any.rs>any".into(),
                edge_kind: "unknown".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0078: mycelium_get_harmonic_centrality ────────────────────────

    #[tokio::test]
    async fn harmonic_centrality_chain_returns_correct_value() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let root = store.upsert_node(TrunkPath::parse("src/hc.rs>root").unwrap());
            let mid = store.upsert_node(TrunkPath::parse("src/hc.rs>mid").unwrap());
            let far = store.upsert_node(TrunkPath::parse("src/hc.rs>far").unwrap());
            store.upsert_edge(EdgeKind::Calls, root, mid);
            store.upsert_edge(EdgeKind::Calls, mid, far);
        }
        let raw = server
            .mycelium_get_harmonic_centrality(Parameters(HarmonicCentralityRequest {
                path: "src/hc.rs>root".into(),
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        // n=3, root reaches mid(d=1) and far(d=2)
        // HC = (1/2) * (1/1 + 1/2) = 0.5 * 1.5 = 0.75
        let hc = val["harmonic_centrality"].as_f64().unwrap();
        assert!((hc - 0.75).abs() < 1e-9, "expected 0.75, got {hc}");
        assert_eq!(val["reachable_count"].as_u64().unwrap(), 2);
        assert_eq!(val["symbol_count"].as_u64().unwrap(), 3);
    }

    #[tokio::test]
    async fn harmonic_centrality_unknown_path_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_harmonic_centrality(Parameters(HarmonicCentralityRequest {
                path: "src/ghost.rs>none_hc".into(),
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown path"));
    }

    #[tokio::test]
    async fn harmonic_centrality_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_harmonic_centrality(Parameters(HarmonicCentralityRequest {
                path: "src/any.rs>any_hc".into(),
                edge_kind: "unknown".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0079: mycelium_get_mutual_reachability ────────────────────────

    #[tokio::test]
    async fn mutual_reachability_forward_only_returns_correct_flags() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let src = store.upsert_node(TrunkPath::parse("src/mr.rs>src").unwrap());
            let dst = store.upsert_node(TrunkPath::parse("src/mr.rs>dst").unwrap());
            store.upsert_edge(EdgeKind::Calls, src, dst);
        }
        let raw = server
            .mycelium_get_mutual_reachability(Parameters(MutualReachabilityRequest {
                path1: "src/mr.rs>src".into(),
                path2: "src/mr.rs>dst".into(),
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["forward"].as_bool().unwrap());
        assert!(!val["backward"].as_bool().unwrap());
        assert!(!val["mutual"].as_bool().unwrap());
        assert_eq!(val["forward_distance"].as_u64().unwrap(), 1);
        assert!(val["backward_distance"].is_null());
    }

    #[tokio::test]
    async fn mutual_reachability_unknown_path_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_mutual_reachability(Parameters(MutualReachabilityRequest {
                path1: "src/ghost.rs>none_mr".into(),
                path2: "src/ghost.rs>other_mr".into(),
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown path"));
    }

    #[tokio::test]
    async fn mutual_reachability_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_mutual_reachability(Parameters(MutualReachabilityRequest {
                path1: "src/any.rs>any_mr1".into(),
                path2: "src/any.rs>any_mr2".into(),
                edge_kind: "unknown".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0080: mycelium_get_reachable_set ──────────────────────────────

    #[tokio::test]
    async fn reachable_set_chain_returns_all_reachable() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let head = store.upsert_node(TrunkPath::parse("src/rset.rs>head").unwrap());
            let mid = store.upsert_node(TrunkPath::parse("src/rset.rs>mid").unwrap());
            let tail = store.upsert_node(TrunkPath::parse("src/rset.rs>tail").unwrap());
            store.upsert_edge(EdgeKind::Calls, head, mid);
            store.upsert_edge(EdgeKind::Calls, mid, tail);
        }
        let raw = server
            .mycelium_get_reachable_set(Parameters(ReachableSetRequest {
                path: "src/rset.rs>head".into(),
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 2);
        let reachable: Vec<&str> = val["reachable"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(reachable.contains(&"src/rset.rs>mid"));
        assert!(reachable.contains(&"src/rset.rs>tail"));
    }

    #[tokio::test]
    async fn reachable_set_unknown_path_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_reachable_set(Parameters(ReachableSetRequest {
                path: "src/ghost.rs>none_rset".into(),
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown path"));
    }

    #[tokio::test]
    async fn reachable_set_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_reachable_set(Parameters(ReachableSetRequest {
                path: "src/any.rs>any_rset".into(),
                edge_kind: "unknown".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0081: mycelium_get_reaches_into ───────────────────────────────

    #[tokio::test]
    async fn reaches_into_chain_returns_reverse_closure() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let head = store.upsert_node(TrunkPath::parse("src/ri.rs>head_ri").unwrap());
            let mid = store.upsert_node(TrunkPath::parse("src/ri.rs>mid_ri").unwrap());
            let tail = store.upsert_node(TrunkPath::parse("src/ri.rs>tail_ri").unwrap());
            store.upsert_edge(EdgeKind::Calls, head, mid);
            store.upsert_edge(EdgeKind::Calls, mid, tail);
        }
        let raw = server
            .mycelium_get_reaches_into(Parameters(ReachesIntoRequest {
                path: "src/ri.rs>tail_ri".into(),
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 2);
        let callers: Vec<&str> = val["callers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(callers.contains(&"src/ri.rs>head_ri"));
        assert!(callers.contains(&"src/ri.rs>mid_ri"));
    }

    #[tokio::test]
    async fn reaches_into_unknown_path_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_reaches_into(Parameters(ReachesIntoRequest {
                path: "src/ghost.rs>none_ri".into(),
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown path"));
    }

    #[tokio::test]
    async fn reaches_into_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_reaches_into(Parameters(ReachesIntoRequest {
                path: "src/any.rs>any_ri".into(),
                edge_kind: "unknown".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0082: mycelium_page_rank ──────────────────────────────────────

    #[tokio::test]
    async fn page_rank_star_hub_ranks_first() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let hub = store.upsert_node(TrunkPath::parse("src/pr.rs>pr_hub").unwrap());
            let spoke_a = store.upsert_node(TrunkPath::parse("src/pr.rs>pr_spoke_a").unwrap());
            let spoke_b = store.upsert_node(TrunkPath::parse("src/pr.rs>pr_spoke_b").unwrap());
            store.upsert_edge(EdgeKind::Calls, spoke_a, hub);
            store.upsert_edge(EdgeKind::Calls, spoke_b, hub);
        }
        let raw = server
            .mycelium_page_rank(Parameters(PageRankRequest {
                edge_kind: "calls".into(),
                damping: Some(0.85),
                iterations: Some(30),
                top_n: Some(3),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["symbol_count"].as_u64().unwrap(), 3);
        let first_path = val["nodes"][0]["path"].as_str().unwrap();
        assert_eq!(first_path, "src/pr.rs>pr_hub");
    }

    #[tokio::test]
    async fn page_rank_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_page_rank(Parameters(PageRankRequest {
                edge_kind: "unknown".into(),
                damping: None,
                iterations: None,
                top_n: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0083: mycelium_get_common_reachable ───────────────────────────

    #[tokio::test]
    async fn common_reachable_shared_dep_returned() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let left = store.upsert_node(TrunkPath::parse("src/cr.rs>mcp_cr_left").unwrap());
            let right = store.upsert_node(TrunkPath::parse("src/cr.rs>mcp_cr_right").unwrap());
            let shared = store.upsert_node(TrunkPath::parse("src/cr.rs>mcp_cr_shared").unwrap());
            store.upsert_edge(EdgeKind::Calls, left, shared);
            store.upsert_edge(EdgeKind::Calls, right, shared);
        }
        let raw = server
            .mycelium_get_common_reachable(Parameters(CommonReachableRequest {
                path1: "src/cr.rs>mcp_cr_left".into(),
                path2: "src/cr.rs>mcp_cr_right".into(),
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 1);
        assert_eq!(
            val["common"][0].as_str().unwrap(),
            "src/cr.rs>mcp_cr_shared"
        );
    }

    #[tokio::test]
    async fn common_reachable_unknown_path_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_common_reachable(Parameters(CommonReachableRequest {
                path1: "src/ghost.rs>none_cr".into(),
                path2: "src/ghost.rs>also_none".into(),
                edge_kind: "calls".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown path"));
    }

    #[tokio::test]
    async fn common_reachable_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_common_reachable(Parameters(CommonReachableRequest {
                path1: "src/any.rs>any_cr1".into(),
                path2: "src/any.rs>any_cr2".into(),
                edge_kind: "unknown".into(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0084: mycelium_get_k_hop_neighbors ────────────────────────────

    #[tokio::test]
    async fn k_hop_neighbors_k2_returns_grandchildren() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let root = store.upsert_node(TrunkPath::parse("src/kh.rs>mcp_kh_root").unwrap());
            let mid = store.upsert_node(TrunkPath::parse("src/kh.rs>mcp_kh_mid").unwrap());
            let far = store.upsert_node(TrunkPath::parse("src/kh.rs>mcp_kh_far").unwrap());
            store.upsert_edge(EdgeKind::Calls, root, mid);
            store.upsert_edge(EdgeKind::Calls, mid, far);
        }
        let raw = server
            .mycelium_get_k_hop_neighbors(Parameters(KHopNeighborsRequest {
                path: "src/kh.rs>mcp_kh_root".into(),
                edge_kind: "calls".into(),
                k: 2,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["k"].as_u64().unwrap(), 2);
        assert_eq!(val["count"].as_u64().unwrap(), 1);
        assert_eq!(
            val["neighbors"][0].as_str().unwrap(),
            "src/kh.rs>mcp_kh_far"
        );
    }

    #[tokio::test]
    async fn k_hop_neighbors_unknown_path_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_k_hop_neighbors(Parameters(KHopNeighborsRequest {
                path: "src/ghost.rs>none_kh".into(),
                edge_kind: "calls".into(),
                k: 1,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown path"));
    }

    #[tokio::test]
    async fn k_hop_neighbors_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_k_hop_neighbors(Parameters(KHopNeighborsRequest {
                path: "src/any.rs>any_kh".into(),
                edge_kind: "unknown".into(),
                k: 1,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0085: mycelium_get_betweenness_centrality ─────────────────────

    #[tokio::test]
    async fn betweenness_chain_middle_ranks_first() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let head = store.upsert_node(TrunkPath::parse("src/bw.rs>bw_head").unwrap());
            let mid = store.upsert_node(TrunkPath::parse("src/bw.rs>bw_mid").unwrap());
            let tail = store.upsert_node(TrunkPath::parse("src/bw.rs>bw_tail").unwrap());
            store.upsert_edge(EdgeKind::Calls, head, mid);
            store.upsert_edge(EdgeKind::Calls, mid, tail);
        }
        let raw = server
            .mycelium_get_betweenness_centrality(Parameters(BetweennessCentralityRequest {
                edge_kind: "calls".into(),
                top_n: Some(3),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["symbol_count"].as_u64().unwrap(), 3);
        let first_path = val["nodes"][0]["path"].as_str().unwrap();
        assert_eq!(first_path, "src/bw.rs>bw_mid");
    }

    #[tokio::test]
    async fn betweenness_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_betweenness_centrality(Parameters(BetweennessCentralityRequest {
                edge_kind: "unknown".into(),
                top_n: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0086: mycelium_get_strongly_connected_components ─────────────

    #[tokio::test]
    async fn scc_finds_cycle_with_min_size_2() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/scc_mcp.rs>mcp_scc_a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/scc_mcp.rs>mcp_scc_b").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, b, a);
            // Singleton node outside the cycle.
            store.upsert_node(TrunkPath::parse("src/scc_mcp.rs>mcp_scc_c").unwrap());
        }
        let raw = server
            .mycelium_get_strongly_connected_components(Parameters(
                StronglyConnectedComponentsRequest {
                    edge_kind: "calls".into(),
                    min_size: Some(2),
                },
            ))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["min_size"].as_u64().unwrap(), 2);
        assert_eq!(val["symbol_count"].as_u64().unwrap(), 3);
        let comps = val["components"].as_array().unwrap();
        assert_eq!(comps.len(), 1);
        assert_eq!(comps[0]["size"].as_u64().unwrap(), 2);
    }

    #[tokio::test]
    async fn scc_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_strongly_connected_components(Parameters(
                StronglyConnectedComponentsRequest {
                    edge_kind: "unknown".into(),
                    min_size: None,
                },
            ))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0087: mycelium_get_degree_centrality ──────────────────────────

    #[tokio::test]
    async fn degree_centrality_identifies_fan_in_hub() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let hub = store.upsert_node(TrunkPath::parse("src/dc_mcp.rs>mcp_dc_hub").unwrap());
            let c1 = store.upsert_node(TrunkPath::parse("src/dc_mcp.rs>mcp_dc_c1").unwrap());
            let c2 = store.upsert_node(TrunkPath::parse("src/dc_mcp.rs>mcp_dc_c2").unwrap());
            store.upsert_edge(EdgeKind::Calls, c1, hub);
            store.upsert_edge(EdgeKind::Calls, c2, hub);
        }
        let raw = server
            .mycelium_get_degree_centrality(Parameters(DegreeCentralityRequest {
                edge_kind: "calls".into(),
                top_n: Some(3),
                sort_by: Some("in".into()),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["symbol_count"].as_u64().unwrap(), 3);
        assert_eq!(val["sort_by"].as_str().unwrap(), "in");
        let nodes = val["nodes"].as_array().unwrap();
        assert_eq!(
            nodes[0]["path"].as_str().unwrap(),
            "src/dc_mcp.rs>mcp_dc_hub"
        );
        assert_eq!(nodes[0]["in_degree"].as_u64().unwrap(), 2);
    }

    #[tokio::test]
    async fn degree_centrality_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_degree_centrality(Parameters(DegreeCentralityRequest {
                edge_kind: "unknown".into(),
                top_n: None,
                sort_by: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    #[tokio::test]
    async fn degree_centrality_unknown_sort_by_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_degree_centrality(Parameters(DegreeCentralityRequest {
                edge_kind: "calls".into(),
                top_n: None,
                sort_by: Some("bogus".into()),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown sort_by"));
    }

    // ── RFC-0088: mycelium_get_closeness_centrality ───────────────────────

    #[tokio::test]
    async fn closeness_chain_head_ranks_first() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/clc.rs>mcp_clc_a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/clc.rs>mcp_clc_b").unwrap());
            let c = store.upsert_node(TrunkPath::parse("src/clc.rs>mcp_clc_c").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, b, c);
        }
        let raw = server
            .mycelium_get_closeness_centrality(Parameters(ClosenessCentralityRequest {
                edge_kind: "calls".into(),
                top_n: Some(3),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["symbol_count"].as_u64().unwrap(), 3);
        // A reaches B and C with shortest total distance → highest closeness.
        assert_eq!(
            val["nodes"][0]["path"].as_str().unwrap(),
            "src/clc.rs>mcp_clc_a"
        );
    }

    #[tokio::test]
    async fn closeness_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_closeness_centrality(Parameters(ClosenessCentralityRequest {
                edge_kind: "unknown".into(),
                top_n: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge kind"));
    }

    // ── RFC-0041: mycelium_get_outgoing_refs ──────────────────────────────

    #[tokio::test]
    async fn get_outgoing_refs_all_kinds() {
        let server = MyceliumServer::new();
        {
            let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
            let src = store.upsert_node(TrunkPath::parse("src/app.rs>App").unwrap());
            let callee = store.upsert_node(TrunkPath::parse("src/a.rs>callee").unwrap());
            let imported = store.upsert_node(TrunkPath::parse("src/b.rs>imported").unwrap());
            let parent = store.upsert_node(TrunkPath::parse("src/c.rs>Parent").unwrap());
            let iface = store.upsert_node(TrunkPath::parse("src/d.rs>IFace").unwrap());
            store.upsert_edge(EdgeKind::Calls, src, callee);
            store.upsert_edge(EdgeKind::Imports, src, imported);
            store.upsert_edge(EdgeKind::Extends, src, parent);
            store.upsert_edge(EdgeKind::Implements, src, iface);
        }
        let raw = server
            .mycelium_get_outgoing_refs(Parameters(GetOutgoingRefsRequest {
                path: "src/app.rs>App".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["callees"][0].as_str().unwrap(), "src/a.rs>callee");
        assert_eq!(val["imports"][0].as_str().unwrap(), "src/b.rs>imported");
        assert_eq!(val["extends"][0].as_str().unwrap(), "src/c.rs>Parent");
        assert_eq!(val["implements"][0].as_str().unwrap(), "src/d.rs>IFace");
    }

    #[tokio::test]
    async fn get_outgoing_refs_empty_lists_present() {
        let server = MyceliumServer::new();
        {
            let mut store: tokio::sync::RwLockWriteGuard<'_, Store> = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/lone.rs>lone").unwrap());
        }
        let raw = server
            .mycelium_get_outgoing_refs(Parameters(GetOutgoingRefsRequest {
                path: "src/lone.rs>lone".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["callees"].as_array().unwrap().len(), 0);
        assert_eq!(val["imports"].as_array().unwrap().len(), 0);
        assert_eq!(val["extends"].as_array().unwrap().len(), 0);
        assert_eq!(val["implements"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn get_outgoing_refs_unknown_path_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_outgoing_refs(Parameters(GetOutgoingRefsRequest {
                path: "no/such>path".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    // ── RFC-0042: mycelium_get_all_symbols ───────────────────────────────────

    #[tokio::test]
    async fn get_all_symbols_excludes_file_nodes() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/a.rs").unwrap());
            store.upsert_node(TrunkPath::parse("src/a.rs>fn1").unwrap());
        }
        let raw = server
            .mycelium_get_all_symbols(Parameters(GetAllSymbolsRequest {
                path_prefix: None,
                kind: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let symbols = val["symbols"].as_array().unwrap();
        assert!(!symbols.iter().any(|s| s.as_str().unwrap() == "src/a.rs"));
        assert!(
            symbols
                .iter()
                .any(|s| s.as_str().unwrap() == "src/a.rs>fn1")
        );
    }

    #[tokio::test]
    async fn get_all_symbols_prefix_filter() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/a.rs>fn1").unwrap());
            store.upsert_node(TrunkPath::parse("lib/b.rs>fn2").unwrap());
        }
        let raw = server
            .mycelium_get_all_symbols(Parameters(GetAllSymbolsRequest {
                path_prefix: Some("src/".to_owned()),
                kind: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let symbols = val["symbols"].as_array().unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].as_str().unwrap(), "src/a.rs>fn1");
    }

    #[tokio::test]
    async fn get_all_symbols_kind_filter() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let fn_id = store.upsert_node(TrunkPath::parse("src/a.rs>fn1").unwrap());
            store.set_kind(fn_id, mycelium_core::types::NodeKind::Function);
            let cls_id = store.upsert_node(TrunkPath::parse("src/a.rs>MyClass").unwrap());
            store.set_kind(cls_id, mycelium_core::types::NodeKind::Class);
        }
        let raw = server
            .mycelium_get_all_symbols(Parameters(GetAllSymbolsRequest {
                path_prefix: None,
                kind: Some("function".to_owned()),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let symbols = val["symbols"].as_array().unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].as_str().unwrap(), "src/a.rs>fn1");
    }

    #[tokio::test]
    async fn get_all_symbols_unknown_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_all_symbols(Parameters(GetAllSymbolsRequest {
                path_prefix: None,
                kind: Some("bogus_kind".to_owned()),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
        assert!(val["error"].as_str().unwrap().contains("bogus_kind"));
    }

    #[tokio::test]
    async fn get_all_symbols_no_params_returns_all() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("a.rs>x").unwrap());
            store.upsert_node(TrunkPath::parse("b.rs>y").unwrap());
        }
        let raw = server
            .mycelium_get_all_symbols(Parameters(GetAllSymbolsRequest {
                path_prefix: None,
                kind: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 2);
    }

    // ── RFC-0043: mycelium_get_reachable ─────────────────────────────────────

    #[tokio::test]
    async fn get_reachable_direct_callees() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, a, c);
        }
        let raw = server
            .mycelium_get_reachable(Parameters(GetReachableRequest {
                path: "src/a.rs>a".to_owned(),
                edge_kind: "calls".to_owned(),
                max_depth: Some(1),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 2);
        let reachable: Vec<&str> = val["reachable"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect();
        assert!(reachable.contains(&"src/b.rs>b"));
        assert!(reachable.contains(&"src/c.rs>c"));
    }

    #[tokio::test]
    async fn get_reachable_cycle_safe() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
            store.upsert_edge(EdgeKind::Calls, b, a);
        }
        let raw = server
            .mycelium_get_reachable(Parameters(GetReachableRequest {
                path: "src/a.rs>a".to_owned(),
                edge_kind: "calls".to_owned(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 1);
        assert_eq!(val["reachable"][0].as_str().unwrap(), "src/b.rs>b");
    }

    #[tokio::test]
    async fn get_reachable_unknown_path_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_reachable(Parameters(GetReachableRequest {
                path: "no/such>path".to_owned(),
                edge_kind: "calls".to_owned(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    #[tokio::test]
    async fn get_reachable_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        }
        let raw = server
            .mycelium_get_reachable(Parameters(GetReachableRequest {
                path: "src/a.rs>a".to_owned(),
                edge_kind: "bogus".to_owned(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    #[tokio::test]
    async fn get_reachable_max_depth_zero_empty() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, b);
        }
        let raw = server
            .mycelium_get_reachable(Parameters(GetReachableRequest {
                path: "src/a.rs>a".to_owned(),
                edge_kind: "calls".to_owned(),
                max_depth: Some(0),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
    }

    // ── RFC-0044: mycelium_get_reachable_to ──────────────────────────────────

    #[tokio::test]
    async fn get_reachable_to_direct_callers() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
            store.upsert_edge(EdgeKind::Calls, b, a);
            store.upsert_edge(EdgeKind::Calls, c, a);
        }
        let raw = server
            .mycelium_get_reachable_to(Parameters(GetReachableToRequest {
                path: "src/a.rs>a".to_owned(),
                edge_kind: "calls".to_owned(),
                max_depth: Some(1),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 2);
        let reachable: Vec<&str> = val["reachable"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect();
        assert!(reachable.contains(&"src/b.rs>b"));
        assert!(reachable.contains(&"src/c.rs>c"));
    }

    #[tokio::test]
    async fn get_reachable_to_cycle_safe() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            store.upsert_edge(EdgeKind::Calls, b, a);
            store.upsert_edge(EdgeKind::Calls, a, b);
        }
        let raw = server
            .mycelium_get_reachable_to(Parameters(GetReachableToRequest {
                path: "src/a.rs>a".to_owned(),
                edge_kind: "calls".to_owned(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 1);
        assert_eq!(val["reachable"][0].as_str().unwrap(), "src/b.rs>b");
    }

    #[tokio::test]
    async fn get_reachable_to_unknown_path_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_reachable_to(Parameters(GetReachableToRequest {
                path: "no/such>path".to_owned(),
                edge_kind: "calls".to_owned(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    #[tokio::test]
    async fn get_reachable_to_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
        }
        let raw = server
            .mycelium_get_reachable_to(Parameters(GetReachableToRequest {
                path: "src/a.rs>a".to_owned(),
                edge_kind: "bogus".to_owned(),
                max_depth: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    #[tokio::test]
    async fn get_reachable_to_max_depth_zero_empty() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            store.upsert_edge(EdgeKind::Calls, b, a);
        }
        let raw = server
            .mycelium_get_reachable_to(Parameters(GetReachableToRequest {
                path: "src/a.rs>a".to_owned(),
                edge_kind: "calls".to_owned(),
                max_depth: Some(0),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
    }

    // ── RFC-0045: mycelium_get_siblings ──────────────────────────────────────

    #[tokio::test]
    async fn get_siblings_class_methods() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/a.rs>App>init").unwrap());
            store.upsert_node(TrunkPath::parse("src/a.rs>App>render").unwrap());
            store.upsert_node(TrunkPath::parse("src/a.rs>App>destroy").unwrap());
        }
        let raw = server
            .mycelium_get_siblings(Parameters(GetSiblingsRequest {
                path: "src/a.rs>App>render".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 2);
        let siblings: Vec<&str> = val["siblings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect();
        assert!(siblings.contains(&"src/a.rs>App>init"));
        assert!(siblings.contains(&"src/a.rs>App>destroy"));
        assert!(!siblings.contains(&"src/a.rs>App>render"));
    }

    #[tokio::test]
    async fn get_siblings_root_node_returns_empty() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/a.rs").unwrap());
        }
        let raw = server
            .mycelium_get_siblings(Parameters(GetSiblingsRequest {
                path: "src/a.rs".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
        assert_eq!(val["siblings"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn get_siblings_unknown_path_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_siblings(Parameters(GetSiblingsRequest {
                path: "no/such>path".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    #[tokio::test]
    async fn get_siblings_excludes_grandchildren() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/a.rs>App>method").unwrap());
            store.upsert_node(TrunkPath::parse("src/a.rs>App>method>inner").unwrap());
            store.upsert_node(TrunkPath::parse("src/a.rs>App>other").unwrap());
        }
        let raw = server
            .mycelium_get_siblings(Parameters(GetSiblingsRequest {
                path: "src/a.rs>App>method".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 1);
        assert_eq!(val["siblings"][0].as_str().unwrap(), "src/a.rs>App>other");
    }

    // ── RFC-0046: mycelium_get_node_degree ───────────────────────────────────

    #[tokio::test]
    async fn get_node_degree_isolated_all_zero() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/a.rs>fn1").unwrap());
        }
        let raw = server
            .mycelium_get_node_degree(Parameters(GetNodeDegreeRequest {
                path: "src/a.rs>fn1".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["in_calls"].as_u64().unwrap(), 0);
        assert_eq!(val["out_calls"].as_u64().unwrap(), 0);
        assert_eq!(val["in_imports"].as_u64().unwrap(), 0);
        assert_eq!(val["out_imports"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn get_node_degree_counts_edges() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            let c = store.upsert_node(TrunkPath::parse("src/c.rs>c").unwrap());
            store.upsert_edge(EdgeKind::Calls, b, a);
            store.upsert_edge(EdgeKind::Calls, c, a);
            store.upsert_edge(EdgeKind::Calls, a, b);
        }
        let raw = server
            .mycelium_get_node_degree(Parameters(GetNodeDegreeRequest {
                path: "src/a.rs>a".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["in_calls"].as_u64().unwrap(), 2);
        assert_eq!(val["out_calls"].as_u64().unwrap(), 1);
    }

    #[tokio::test]
    async fn get_node_degree_unknown_path_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_node_degree(Parameters(GetNodeDegreeRequest {
                path: "no/such>path".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    // ── RFC-0047: mycelium_get_top_files ─────────────────────────────────────

    #[tokio::test]
    async fn get_top_files_ranks_by_symbol_count() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/big.rs").unwrap());
            store.upsert_node(TrunkPath::parse("src/big.rs>fn1").unwrap());
            store.upsert_node(TrunkPath::parse("src/big.rs>fn2").unwrap());
            store.upsert_node(TrunkPath::parse("src/big.rs>fn3").unwrap());
            store.upsert_node(TrunkPath::parse("src/small.rs").unwrap());
            store.upsert_node(TrunkPath::parse("src/small.rs>fn1").unwrap());
        }
        let raw = server
            .mycelium_get_top_files(Parameters(GetTopFilesRequest { limit: Some(10) }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let files = val["files"].as_array().unwrap();
        assert_eq!(files[0]["path"].as_str().unwrap(), "src/big.rs");
        assert_eq!(files[0]["symbol_count"].as_u64().unwrap(), 3);
        assert_eq!(files[1]["path"].as_str().unwrap(), "src/small.rs");
    }

    #[tokio::test]
    async fn get_top_files_empty_graph_returns_empty() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_top_files(Parameters(GetTopFilesRequest { limit: None }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
        assert_eq!(val["files"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    #[allow(clippy::significant_drop_tightening)]
    async fn get_top_files_default_limit() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            for i in 0..15u32 {
                let p = format!("src/{i}.rs");
                store.upsert_node(TrunkPath::parse(&p).unwrap());
                store.upsert_node(TrunkPath::parse(&format!("{p}>fn")).unwrap());
            }
        }
        let raw = server
            .mycelium_get_top_files(Parameters(GetTopFilesRequest { limit: None }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        // default limit is 10, so at most 10 files returned even though 15 exist
        assert!(val["files"].as_array().unwrap().len() <= 10);
    }

    // ── RFC-0048: mycelium_get_most_connected ────────────────────────────────

    #[tokio::test]
    async fn get_most_connected_ranks_hub_node() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let hub = store.upsert_node(TrunkPath::parse("src/hub.rs>hub").unwrap());
            let a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            store.upsert_edge(EdgeKind::Calls, a, hub);
            store.upsert_edge(EdgeKind::Calls, b, hub);
        }
        let raw = server
            .mycelium_get_most_connected(Parameters(GetMostConnectedRequest {
                edge_kind: "calls".to_owned(),
                limit: Some(10),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let syms = val["symbols"].as_array().unwrap();
        assert_eq!(syms[0]["path"].as_str().unwrap(), "src/hub.rs>hub");
        assert_eq!(syms[0]["degree"].as_u64().unwrap(), 2);
    }

    #[tokio::test]
    async fn get_most_connected_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_most_connected(Parameters(GetMostConnectedRequest {
                edge_kind: "bogus".to_owned(),
                limit: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val.get("error").is_some());
    }

    #[tokio::test]
    async fn get_most_connected_empty_excludes_zero_degree() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/a.rs>isolated").unwrap());
        }
        let raw = server
            .mycelium_get_most_connected(Parameters(GetMostConnectedRequest {
                edge_kind: "calls".to_owned(),
                limit: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
    }

    // ── RFC-0049: mycelium_get_leaf_symbols ──────────────────────────────────

    #[tokio::test]
    async fn get_leaf_symbols_returns_out_degree_zero() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let root = store.upsert_node(TrunkPath::parse("src/a.rs>root").unwrap());
            let leaf = store.upsert_node(TrunkPath::parse("src/b.rs>leaf").unwrap());
            store.upsert_edge(EdgeKind::Calls, root, leaf);
        }
        let raw = server
            .mycelium_get_leaf_symbols(Parameters(GetLeafSymbolsRequest {
                edge_kind: "calls".to_owned(),
                limit: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let syms = val["symbols"].as_array().unwrap();
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].as_str().unwrap(), "src/b.rs>leaf");
        assert_eq!(val["count"].as_u64().unwrap(), 1);
    }

    #[tokio::test]
    async fn get_leaf_symbols_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_leaf_symbols(Parameters(GetLeafSymbolsRequest {
                edge_kind: "unknown".to_owned(),
                limit: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
    }

    #[tokio::test]
    async fn get_leaf_symbols_empty_graph_returns_empty() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_leaf_symbols(Parameters(GetLeafSymbolsRequest {
                edge_kind: "calls".to_owned(),
                limit: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
    }

    // ── RFC-0050: mycelium_get_shortest_path ─────────────────────────────────

    #[tokio::test]
    async fn get_shortest_path_direct_edge() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/a.rs>root").unwrap());
            store.upsert_node(TrunkPath::parse("src/b.rs>leaf").unwrap());
            let root = store.lookup("src/a.rs>root").unwrap();
            let leaf = store.lookup("src/b.rs>leaf").unwrap();
            store.upsert_edge(EdgeKind::Calls, root, leaf);
        }
        let raw = server
            .mycelium_get_shortest_path(Parameters(GetShortestPathRequest {
                from: "src/a.rs>root".to_owned(),
                to: "src/b.rs>leaf".to_owned(),
                edge_kind: "calls".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["length"].as_u64().unwrap(), 1);
        let path = val["path"].as_array().unwrap();
        assert_eq!(path.len(), 2);
    }

    #[tokio::test]
    async fn get_shortest_path_no_path_returns_null() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
        }
        let raw = server
            .mycelium_get_shortest_path(Parameters(GetShortestPathRequest {
                from: "src/a.rs>a".to_owned(),
                to: "src/b.rs>b".to_owned(),
                edge_kind: "calls".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["path"].is_null());
        assert!(val["length"].is_null());
    }

    #[tokio::test]
    async fn get_shortest_path_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_shortest_path(Parameters(GetShortestPathRequest {
                from: "src/a.rs>a".to_owned(),
                to: "src/b.rs>b".to_owned(),
                edge_kind: "unknown".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
    }

    #[tokio::test]
    async fn get_shortest_path_unknown_from_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_shortest_path(Parameters(GetShortestPathRequest {
                from: "no/such.rs>sym".to_owned(),
                to: "src/b.rs>b".to_owned(),
                edge_kind: "calls".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("path not found"));
    }

    // ── RFC-0051: mycelium_get_symbol_count_by_kind ──────────────────────────

    #[tokio::test]
    async fn get_symbol_count_by_kind_basic() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node_with_kind(
                TrunkPath::parse("src/a.rs>fn1").unwrap(),
                mycelium_core::types::NodeKind::Function,
            );
            store.upsert_node_with_kind(
                TrunkPath::parse("src/a.rs>MyClass").unwrap(),
                mycelium_core::types::NodeKind::Class,
            );
        }
        let raw = server
            .mycelium_get_symbol_count_by_kind(Parameters(GetSymbolCountByKindRequest {}))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["total"].as_u64().unwrap(), 2);
        let kinds = val["kinds"].as_array().unwrap();
        assert_eq!(kinds.len(), 2);
    }

    #[tokio::test]
    async fn get_symbol_count_by_kind_empty_graph() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_symbol_count_by_kind(Parameters(GetSymbolCountByKindRequest {}))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["total"].as_u64().unwrap(), 0);
        assert_eq!(val["kinds"].as_array().unwrap().len(), 0);
    }

    // ── RFC-0052: mycelium_get_common_callers ────────────────────────────────

    #[tokio::test]
    async fn get_common_callers_intersection() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            store.upsert_node(TrunkPath::parse("src/shared.rs>shared").unwrap());
            store.upsert_node(TrunkPath::parse("src/ta.rs>ta").unwrap());
            store.upsert_node(TrunkPath::parse("src/tb.rs>tb").unwrap());
            let shared = store.lookup("src/shared.rs>shared").unwrap();
            let ta = store.lookup("src/ta.rs>ta").unwrap();
            let tb = store.lookup("src/tb.rs>tb").unwrap();
            store.upsert_edge(EdgeKind::Calls, shared, ta);
            store.upsert_edge(EdgeKind::Calls, shared, tb);
        }
        let raw = server
            .mycelium_get_common_callers(Parameters(GetCommonCallersRequest {
                paths: vec!["src/ta.rs>ta".to_owned(), "src/tb.rs>tb".to_owned()],
                edge_kind: "calls".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 1);
        assert_eq!(
            val["callers"].as_array().unwrap()[0].as_str().unwrap(),
            "src/shared.rs>shared"
        );
    }

    #[tokio::test]
    async fn get_common_callers_empty_paths_returns_empty() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_common_callers(Parameters(GetCommonCallersRequest {
                paths: vec![],
                edge_kind: "calls".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn get_common_callers_unknown_path_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_common_callers(Parameters(GetCommonCallersRequest {
                paths: vec!["no/such.rs>sym".to_owned()],
                edge_kind: "calls".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("path not found"));
    }

    // ── RFC-0055: mycelium_get_common_callees ────────────────────────────────

    #[tokio::test]
    async fn get_common_callees_returns_intersection() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let shared = store.upsert_node(TrunkPath::parse("src/shared.rs>shared").unwrap());
            let src_a = store.upsert_node(TrunkPath::parse("src/a.rs>a").unwrap());
            let src_b = store.upsert_node(TrunkPath::parse("src/b.rs>b").unwrap());
            store.upsert_edge(EdgeKind::Calls, src_a, shared);
            store.upsert_edge(EdgeKind::Calls, src_b, shared);
        }
        let raw = server
            .mycelium_get_common_callees(Parameters(GetCommonCalleesRequest {
                paths: vec!["src/a.rs>a".to_owned(), "src/b.rs>b".to_owned()],
                edge_kind: "calls".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 1);
        assert_eq!(
            val["callees"].as_array().unwrap()[0].as_str().unwrap(),
            "src/shared.rs>shared"
        );
    }

    #[tokio::test]
    async fn get_common_callees_empty_paths_returns_empty() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_common_callees(Parameters(GetCommonCalleesRequest {
                paths: vec![],
                edge_kind: "calls".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn get_common_callees_unknown_path_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_common_callees(Parameters(GetCommonCalleesRequest {
                paths: vec!["no/such.rs>sym".to_owned()],
                edge_kind: "calls".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("path not found"));
    }

    #[tokio::test]
    async fn get_common_callees_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_common_callees(Parameters(GetCommonCalleesRequest {
                paths: vec!["src/a.rs>a".to_owned()],
                edge_kind: "bad".to_owned(),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
    }

    // ── RFC-0053: mycelium_get_fan_out_rank ──────────────────────────────────

    #[tokio::test]
    async fn get_fan_out_rank_ranks_by_out_degree() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let hub = store.upsert_node(TrunkPath::parse("src/hub.rs>hub").unwrap());
            let sp1 = store.upsert_node(TrunkPath::parse("src/s1.rs>s1").unwrap());
            let sp2 = store.upsert_node(TrunkPath::parse("src/s2.rs>s2").unwrap());
            store.upsert_edge(EdgeKind::Calls, hub, sp1);
            store.upsert_edge(EdgeKind::Calls, hub, sp2);
        }
        let raw = server
            .mycelium_get_fan_out_rank(Parameters(GetFanOutRankRequest {
                edge_kind: "calls".to_owned(),
                limit: Some(10),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let syms = val["symbols"].as_array().unwrap();
        assert_eq!(syms[0]["path"].as_str().unwrap(), "src/hub.rs>hub");
        assert_eq!(syms[0]["out_degree"].as_u64().unwrap(), 2);
    }

    #[tokio::test]
    async fn get_fan_out_rank_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_fan_out_rank(Parameters(GetFanOutRankRequest {
                edge_kind: "bad".to_owned(),
                limit: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
    }

    #[tokio::test]
    async fn get_fan_out_rank_empty_graph_returns_empty() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_fan_out_rank(Parameters(GetFanOutRankRequest {
                edge_kind: "calls".to_owned(),
                limit: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
    }

    // ── RFC-0054: mycelium_get_fan_in_rank ───────────────────────────────────

    #[tokio::test]
    async fn get_fan_in_rank_ranks_by_in_degree() {
        let server = MyceliumServer::new();
        {
            let mut store = server.store.write().await;
            let hub = store.upsert_node(TrunkPath::parse("src/hub.rs>hub").unwrap());
            let sp1 = store.upsert_node(TrunkPath::parse("src/s1.rs>s1").unwrap());
            let sp2 = store.upsert_node(TrunkPath::parse("src/s2.rs>s2").unwrap());
            // hub is called by both spokes
            store.upsert_edge(EdgeKind::Calls, sp1, hub);
            store.upsert_edge(EdgeKind::Calls, sp2, hub);
        }
        let raw = server
            .mycelium_get_fan_in_rank(Parameters(GetFanInRankRequest {
                edge_kind: "calls".to_owned(),
                limit: Some(10),
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let syms = val["symbols"].as_array().unwrap();
        assert_eq!(syms[0]["path"].as_str().unwrap(), "src/hub.rs>hub");
        assert_eq!(syms[0]["in_degree"].as_u64().unwrap(), 2);
    }

    #[tokio::test]
    async fn get_fan_in_rank_unknown_edge_kind_returns_error() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_fan_in_rank(Parameters(GetFanInRankRequest {
                edge_kind: "bad".to_owned(),
                limit: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(val["error"].as_str().unwrap().contains("unknown edge_kind"));
    }

    #[tokio::test]
    async fn get_fan_in_rank_default_limit() {
        let server = MyceliumServer::new();
        let raw = server
            .mycelium_get_fan_in_rank(Parameters(GetFanInRankRequest {
                edge_kind: "calls".to_owned(),
                limit: None,
            }))
            .await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["count"].as_u64().unwrap(), 0);
    }

    /// Poll `predicate` every `interval` for up to `timeout`. Returns `true`
    /// when the predicate first returns `true`, `false` on timeout.
    async fn poll_for<F, Fut>(
        timeout: tokio::time::Duration,
        interval: tokio::time::Duration,
        predicate: F,
    ) -> bool
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if predicate().await {
                return true;
            }
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(interval).await;
        }
    }
}
