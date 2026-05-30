//! `mycelium` — the command-line entry point.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tokio::runtime::Runtime;

mod index;
#[allow(
    clippy::redundant_pub_crate,
    reason = "items used by main.rs require pub(crate); bin-crate root cannot consume private child-mod items"
)]
mod queries;
#[allow(
    clippy::redundant_pub_crate,
    reason = "items used by main.rs require pub(crate); bin-crate root cannot consume private child-mod items"
)]
mod query;

/// The `mycelium` CLI. See `mycelium --help` for details.
#[derive(Debug, Parser)]
#[command(
    name = "mycelium",
    version,
    about = "Reactive code intelligence graph — the wood-wide-web of your codebase.",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

/// Output format for `mycelium query`. Stable values; the MCP twin tool
/// `mycelium_query` accepts the same set.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum QueryFormat {
    Text,
    Json,
}

impl From<QueryFormat> for query::Format {
    fn from(f: QueryFormat) -> Self {
        match f {
            QueryFormat::Text => Self::Text,
            QueryFormat::Json => Self::Json,
        }
    }
}

impl From<QueryFormat> for queries::Format {
    fn from(f: QueryFormat) -> Self {
        match f {
            QueryFormat::Text => Self::Text,
            QueryFormat::Json => Self::Json,
        }
    }
}

/// Subcommands.
#[derive(Debug, Subcommand)]
enum Cmd {
    /// Print the engine version.
    Version,
    /// Placeholder for `mycelium init` (creates a `.mycelium/` config dir).
    /// Hidden until implemented — see issue #154.
    #[command(hide = true)]
    Init,
    /// Index a project directory and report symbol statistics.
    Index {
        /// Root directory to index (defaults to current directory).
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Execute a Hyphae DSL selector against the project's index.
    Query {
        /// The Hyphae expression. See RFC-0003 for the full grammar.
        ///
        /// Examples:
        ///   `#login`          match symbols named `login`
        ///   `.function`       match all function symbols
        ///   `.class>.method`  methods of classes (direct child)
        expr: String,

        /// Project root (defaults to current directory). The index is read
        /// from `<root>/.mycelium/index.rmp`.
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// Output format. `text` writes one match per line. `json` writes a
        /// JSON array of strings — the stable contract used by the MCP twin
        /// tool `mycelium_query`.
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Find symbols by case-insensitive substring match on the final
    /// path segment.
    SearchSymbol {
        /// Substring to search for in symbol names.
        query: String,
        /// Project root (defaults to current directory).
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Maximum results to return.
        #[arg(long, default_value_t = 50)]
        limit: usize,
        /// Output format.
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return all structural information about a symbol (ancestors,
    /// descendants, callers, callees) in one call.
    GetSymbolInfo {
        /// Symbol path, e.g. `src/lib.rs>App>render`.
        path: String,
        /// Project root (defaults to current directory).
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Output format.
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return the containment-chain ancestors of a symbol (child-to-root).
    GetAncestors {
        /// Symbol path, e.g. `src/lib.rs>App>render`.
        path: String,
        /// Project root (defaults to current directory).
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Output format.
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return every symbol path nested under a container, recursively.
    GetDescendants {
        /// Symbol path of the container, e.g. `src/lib.rs>App`.
        path: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Include methods inherited from base classes via Extends edges.
        /// Inherited methods not overridden by the class are listed under
        /// `inherited_descendants` in JSON output, annotated with their
        /// declaring class.
        #[arg(long, default_value_t = false)]
        include_inherited: bool,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return the `NodeKind` of a symbol (e.g. `function`, `class`).
    GetNodeKind {
        path: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return every symbol whose `NodeKind` matches the wire string
    /// (e.g. `function`, `class`, `module`).
    GetSymbolsByKind {
        /// Wire string for the kind (`function`, `class`, `method`, ...).
        kind: String,
        /// Optional path prefix restricting results.
        #[arg(long)]
        path_prefix: Option<String>,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return the source location of a symbol (start/end line+col+byte).
    GetSourceSpan {
        path: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return the direct children of the same parent (excluding the path itself).
    GetSiblings {
        path: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return every indexed symbol path, optionally filtered by prefix and/or kind.
    GetAllSymbols {
        /// Optional path prefix to scope results (e.g. `src/auth/`).
        #[arg(long)]
        prefix: Option<String>,
        /// Optional kind wire string to filter.
        #[arg(long)]
        kind: Option<String>,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Report whether an index is loaded and its node/edge counts.
    ServerStatus {
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return the direct callees of a symbol (outgoing `Calls` edges).
    GetCallees {
        path: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return the direct callers of a symbol (incoming `Calls` edges).
    GetCallers {
        path: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
        /// Also include callers that reach this symbol via virtual dispatch
        /// (i.e., callers of a base-class method of the same name).
        #[arg(long, default_value_t = false)]
        include_virtual: bool,
    },
    /// Return the recursive callee tree rooted at a symbol.
    GetCalleeTree {
        path: String,
        #[arg(long, default_value_t = 3)]
        max_depth: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return the recursive caller tree rooted at a symbol.
    GetCallerTree {
        path: String,
        #[arg(long, default_value_t = 3)]
        max_depth: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return symbols with no incoming `Calls` edges (call-graph roots).
    GetEntryPoints {
        #[arg(long)]
        prefix: Option<String>,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return symbols with no incoming or outgoing `Calls` edges.
    GetDeadSymbols {
        #[arg(long)]
        prefix: Option<String>,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return symbols with no edges of any kind.
    GetIsolatedSymbols {
        #[arg(long)]
        prefix: Option<String>,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return `imports` + `imported_by` for a file/module.
    GetImports {
        path: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return the recursive import tree rooted at a file/module.
    GetImportTree {
        path: String,
        #[arg(long, default_value_t = 4)]
        max_depth: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return the recursive "who imports me" tree rooted at a file/module.
    GetImportersTree {
        path: String,
        #[arg(long, default_value_t = 4)]
        max_depth: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return direct `extends` + `extended_by` for a class symbol.
    GetExtends {
        path: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return the recursive superclass tree (parents-of-parents).
    ExtendsTree {
        path: String,
        #[arg(long, default_value_t = 4)]
        max_depth: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return the recursive subclasses tree (children of children).
    SubclassesTree {
        path: String,
        #[arg(long, default_value_t = 4)]
        max_depth: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Find the inheritance chain between two class symbols.
    FindExtendsPath {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long, default_value_t = 8)]
        max_depth: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return direct `implements` + `implemented_by` for a class/trait symbol.
    GetImplements {
        path: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return the recursive interface tree (interfaces of interfaces).
    ImplementsTree {
        path: String,
        #[arg(long, default_value_t = 4)]
        max_depth: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return the recursive implementors tree.
    ImplementorsTree {
        path: String,
        #[arg(long, default_value_t = 4)]
        max_depth: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Find the implementation chain between two class/trait symbols.
    FindImplementsPath {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long, default_value_t = 8)]
        max_depth: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return all symbols reachable from a starting path via outgoing
    /// `edge_kind` edges.
    GetReachable {
        path: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value_t = 10)]
        max_depth: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Reverse reachability: symbols that can reach the given path.
    GetReachableTo {
        path: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value_t = 10)]
        max_depth: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return symbols at exactly k hops from the given path.
    GetKHopNeighbors {
        path: String,
        #[arg(long)]
        k: usize,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Convenience for k=2.
    GetTwoHopNeighbors {
        path: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Find the shortest path between two symbols via the given edge kind.
    GetShortestPath {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return the ego-graph of a symbol for a given edge kind.
    GetSymbolNeighborhood {
        path: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return ALL incoming references grouped by edge kind.
    GetCrossRefs {
        path: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return ALL outgoing references grouped by edge kind.
    GetOutgoingRefs {
        path: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return the longest path from the symbol to a leaf (no outgoing edges).
    GetDependencyDepth {
        path: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Transitive forward reachability (no max-depth bound).
    GetReachableSet {
        path: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Transitive reverse reachability (no max-depth bound).
    GetReachesInto {
        path: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return symbols with exactly one incoming edge of the given kind.
    GetSinglyReferenced {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Rank symbols by caller count.
    RankSymbols {
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Top-N source files by direct symbol count.
    GetTopFiles {
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Top-N symbols by total degree for an edge kind.
    GetMostConnected {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Hub symbols: high in-degree AND high out-degree.
    GetHubSymbols {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value_t = 1)]
        min_in: usize,
        #[arg(long, default_value_t = 1)]
        min_out: usize,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Top-N by out-degree.
    GetFanOutRank {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Top-N by in-degree.
    GetFanInRank {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Brandes' betweenness centrality.
    BetweennessCentrality {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value_t = 10)]
        top_n: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Wasserman-Faust closeness centrality.
    ClosenessCentrality {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value_t = 10)]
        top_n: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Normalized in- and out-degree centrality.
    DegreeCentrality {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = "in")]
        sort_by: String,
        #[arg(long, default_value_t = 10)]
        top_n: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Local clustering coefficient for a single symbol.
    ClusteringCoefficient {
        path: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Eccentricity of a single symbol.
    Eccentricity {
        path: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// `PageRank` with damping + iterations.
    PageRank {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value_t = 0.85)]
        damping: f64,
        #[arg(long, default_value_t = 20)]
        iterations: usize,
        #[arg(long, default_value_t = 10)]
        top_n: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Harmonic centrality for a single symbol.
    HarmonicCentrality {
        path: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Jaccard similarity of two symbols' neighbour sets.
    NeighborSimilarity {
        #[arg(long)]
        path1: String,
        #[arg(long)]
        path2: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Whole-graph node/edge counts and per-kind breakdown.
    GetStats {
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Density / avg-degree / max-degree summary for an edge kind.
    GetGraphMetrics {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Symbols that participate in at least one directed cycle.
    DetectCycles {
        #[arg(long)]
        edge_kind: String,
        #[arg(long)]
        path_prefix: Option<String>,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Tarjan SCC groups (size ≥ 2) as `groups: [[...]]`.
    GetSccGroups {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Kahn topological order; nodes in cycles reported separately.
    TopologicalSort {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Articulation points (cut vertices) in the undirected graph.
    FindArticulationPoints {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Bridge edges (cut edges) in the undirected graph.
    FindBridgeEdges {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Biconnected components.
    GetBiconnectedComponents {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Maximal k-core subgraph for an edge kind.
    GetKCore {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value_t = 2)]
        k: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Kahn BFS dependency layers.
    GetDependencyLayers {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Strongly connected components (raw entries with size).
    GetScc {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value_t = 1)]
        min_size: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Weakly connected components.
    GetWcc {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value_t = 1)]
        min_size: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// In/out degree frequency distribution.
    GetDegreeHistogram {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// All symbols participating in at least one directed cycle.
    FindCycleMembers {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Batch get-symbol-info for up to 50 paths.
    BatchSymbolInfo {
        /// Comma-separated list of symbol paths.
        #[arg(long)]
        paths: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Batch node-degree breakdown for up to 50 paths.
    BatchNodeDegree {
        #[arg(long)]
        paths: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Batch forward reachability from up to 20 seeds.
    BatchReachableFrom {
        #[arg(long)]
        paths: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value_t = 10)]
        max_depth: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Batch reverse reachability into up to 20 targets.
    BatchReachableTo {
        #[arg(long)]
        paths: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value_t = 10)]
        max_depth: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Return full per-edge-kind degree breakdown for a symbol.
    GetNodeDegree {
        path: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// List indexed file paths, optionally filtered by prefix.
    GetFiles {
        #[arg(long)]
        path_prefix: Option<String>,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Per-kind symbol counts across the index.
    GetSymbolCountByKind {
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Symbols with out-degree 0 for the edge kind (leaf nodes).
    GetLeafSymbols {
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Common callers across a set of target paths.
    GetCommonCallers {
        #[arg(long)]
        paths: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Common callees across a set of source paths.
    GetCommonCallees {
        #[arg(long)]
        paths: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Intersection of two symbols' transitive reachable sets.
    GetCommonReachable {
        #[arg(long)]
        path1: String,
        #[arg(long)]
        path2: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Forward/backward reachability flags + BFS distances between two symbols.
    GetMutualReachability {
        #[arg(long)]
        path1: String,
        #[arg(long)]
        path2: String,
        #[arg(long)]
        edge_kind: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// BFS call-path between two symbols.
    FindCallPath {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long, default_value_t = 10)]
        max_depth: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// BFS import-dependency path between two file/module paths.
    FindImportPath {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long, default_value_t = 8)]
        max_depth: usize,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = QueryFormat::Text)]
        format: QueryFormat,
    },
    /// Start the MCP server over stdio.
    Serve {
        /// Use MCP protocol over stdio.
        #[arg(long)]
        mcp: bool,
        /// Pre-load (or build) the symbol index from this root directory.
        ///
        /// Loads `.mycelium/index.rmp` if present; otherwise runs a full index
        /// and saves the snapshot before the server accepts connections.
        #[arg(long)]
        root: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    // Route all tracing to stderr (never stdout). For `serve --mcp` this is
    // mandatory: stdout is reserved for JSON-RPC frames. For other subcommands
    // it's harmless. ANSI is disabled so piped consumers (CI logs, MCP clients
    // that surface stderr) don't see escape sequences.
    // Regression test: tests/mcp_stdout_purity.rs (issue #150).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mycelium=info".into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let cli = Cli::parse();
    dispatch(cli.command)
}

#[allow(
    clippy::too_many_lines,
    reason = "subcommand dispatch table — each Cmd variant gets one arm, deliberately flat to keep readability"
)]
fn dispatch(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Version => {
            println!("mycelium {}", env!("CARGO_PKG_VERSION"));
        }
        Cmd::Init => {
            tracing::warn!(
                "`mycelium init` is not implemented yet — tracked under RFC-0001 follow-up"
            );
        }
        Cmd::Index { path } => {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            println!("Indexing {} …", canonical.display());
            let (store, stats) = index::index_path(&canonical)?;
            println!(
                "Done.  {} file(s) indexed, {} error(s).",
                stats.files, stats.errors
            );
            // RFC-0006: auto-save to .mycelium/index.rmp
            let snap = canonical.join(".mycelium").join("index.rmp");
            store.save(&snap)?;
            println!("Index saved to .mycelium/index.rmp");
        }
        Cmd::Query { expr, root, format } => {
            let canonical = root.canonicalize().unwrap_or(root);
            query::run(&canonical, &expr, format.into())?;
        }
        Cmd::SearchSymbol {
            query,
            root,
            limit,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_search_symbol(&canonical, &query, limit, format.into())?;
        }
        Cmd::GetSymbolInfo { path, root, format } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_symbol_info(&canonical, &path, format.into())?;
        }
        Cmd::GetAncestors { path, root, format } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_ancestors(&canonical, &path, format.into())?;
        }
        Cmd::GetDescendants {
            path,
            root,
            include_inherited,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_descendants(&canonical, &path, include_inherited, format.into())?;
        }
        Cmd::GetNodeKind { path, root, format } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_node_kind(&canonical, &path, format.into())?;
        }
        Cmd::GetSymbolsByKind {
            kind,
            path_prefix,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_symbols_by_kind(
                &canonical,
                &kind,
                path_prefix.as_deref(),
                format.into(),
            )?;
        }
        Cmd::GetSourceSpan { path, root, format } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_source_span(&canonical, &path, format.into())?;
        }
        Cmd::GetSiblings { path, root, format } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_siblings(&canonical, &path, format.into())?;
        }
        Cmd::GetAllSymbols {
            prefix,
            kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_all_symbols(
                &canonical,
                prefix.as_deref(),
                kind.as_deref(),
                format.into(),
            )?;
        }
        Cmd::ServerStatus { root, format } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_server_status(&canonical, format.into())?;
        }
        Cmd::GetCallees { path, root, format } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_callees(&canonical, &path, format.into())?;
        }
        Cmd::GetCallers {
            path,
            root,
            format,
            include_virtual,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_callers(&canonical, &path, include_virtual, format.into())?;
        }
        Cmd::GetCalleeTree {
            path,
            max_depth,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_callee_tree(&canonical, &path, max_depth, format.into())?;
        }
        Cmd::GetCallerTree {
            path,
            max_depth,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_caller_tree(&canonical, &path, max_depth, format.into())?;
        }
        Cmd::GetEntryPoints {
            prefix,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_entry_points(&canonical, prefix.as_deref(), format.into())?;
        }
        Cmd::GetDeadSymbols {
            prefix,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_dead_symbols(&canonical, prefix.as_deref(), format.into())?;
        }
        Cmd::GetIsolatedSymbols {
            prefix,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_isolated_symbols(&canonical, prefix.as_deref(), format.into())?;
        }
        Cmd::GetImports { path, root, format } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_imports(&canonical, &path, format.into())?;
        }
        Cmd::GetImportTree {
            path,
            max_depth,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_import_tree(&canonical, &path, max_depth, format.into())?;
        }
        Cmd::GetImportersTree {
            path,
            max_depth,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_importers_tree(&canonical, &path, max_depth, format.into())?;
        }
        Cmd::GetExtends { path, root, format } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_extends(&canonical, &path, format.into())?;
        }
        Cmd::ExtendsTree {
            path,
            max_depth,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_extends_tree(&canonical, &path, max_depth, format.into())?;
        }
        Cmd::SubclassesTree {
            path,
            max_depth,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_subclasses_tree(&canonical, &path, max_depth, format.into())?;
        }
        Cmd::FindExtendsPath {
            from,
            to,
            max_depth,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_find_extends_path(&canonical, &from, &to, max_depth, format.into())?;
        }
        Cmd::GetImplements { path, root, format } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_implements(&canonical, &path, format.into())?;
        }
        Cmd::ImplementsTree {
            path,
            max_depth,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_implements_tree(&canonical, &path, max_depth, format.into())?;
        }
        Cmd::ImplementorsTree {
            path,
            max_depth,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_implementors_tree(&canonical, &path, max_depth, format.into())?;
        }
        Cmd::FindImplementsPath {
            from,
            to,
            max_depth,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_find_implements_path(&canonical, &from, &to, max_depth, format.into())?;
        }
        Cmd::GetReachable {
            path,
            edge_kind,
            max_depth,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_reachable(&canonical, &path, &edge_kind, max_depth, format.into())?;
        }
        Cmd::GetReachableTo {
            path,
            edge_kind,
            max_depth,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_reachable_to(&canonical, &path, &edge_kind, max_depth, format.into())?;
        }
        Cmd::GetKHopNeighbors {
            path,
            k,
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_k_hop_neighbors(&canonical, &path, k, &edge_kind, format.into())?;
        }
        Cmd::GetTwoHopNeighbors {
            path,
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_two_hop_neighbors(&canonical, &path, &edge_kind, format.into())?;
        }
        Cmd::GetShortestPath {
            from,
            to,
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_shortest_path(&canonical, &from, &to, &edge_kind, format.into())?;
        }
        Cmd::GetSymbolNeighborhood {
            path,
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_symbol_neighborhood(&canonical, &path, &edge_kind, format.into())?;
        }
        Cmd::GetCrossRefs { path, root, format } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_cross_refs(&canonical, &path, format.into())?;
        }
        Cmd::GetOutgoingRefs { path, root, format } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_outgoing_refs(&canonical, &path, format.into())?;
        }
        Cmd::GetDependencyDepth {
            path,
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_dependency_depth(&canonical, &path, &edge_kind, format.into())?;
        }
        Cmd::GetReachableSet {
            path,
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_reachable_set(&canonical, &path, &edge_kind, format.into())?;
        }
        Cmd::GetReachesInto {
            path,
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_reaches_into(&canonical, &path, &edge_kind, format.into())?;
        }
        Cmd::GetSinglyReferenced {
            edge_kind,
            limit,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_singly_referenced(&canonical, &edge_kind, limit, format.into())?;
        }
        Cmd::RankSymbols {
            limit,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_rank_symbols(&canonical, limit, format.into())?;
        }
        Cmd::GetTopFiles {
            limit,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_top_files(&canonical, limit, format.into())?;
        }
        Cmd::GetMostConnected {
            edge_kind,
            limit,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_most_connected(&canonical, &edge_kind, limit, format.into())?;
        }
        Cmd::GetHubSymbols {
            edge_kind,
            min_in,
            min_out,
            limit,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_hub_symbols(
                &canonical,
                &edge_kind,
                min_in,
                min_out,
                limit,
                format.into(),
            )?;
        }
        Cmd::GetFanOutRank {
            edge_kind,
            limit,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_fan_out_rank(&canonical, &edge_kind, limit, format.into())?;
        }
        Cmd::GetFanInRank {
            edge_kind,
            limit,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_fan_in_rank(&canonical, &edge_kind, limit, format.into())?;
        }
        Cmd::BetweennessCentrality {
            edge_kind,
            top_n,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_betweenness_centrality(&canonical, &edge_kind, top_n, format.into())?;
        }
        Cmd::ClosenessCentrality {
            edge_kind,
            top_n,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_closeness_centrality(&canonical, &edge_kind, top_n, format.into())?;
        }
        Cmd::DegreeCentrality {
            edge_kind,
            sort_by,
            top_n,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_degree_centrality(&canonical, &edge_kind, &sort_by, top_n, format.into())?;
        }
        Cmd::ClusteringCoefficient {
            path,
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_clustering_coefficient(&canonical, &path, &edge_kind, format.into())?;
        }
        Cmd::Eccentricity {
            path,
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_eccentricity(&canonical, &path, &edge_kind, format.into())?;
        }
        Cmd::PageRank {
            edge_kind,
            damping,
            iterations,
            top_n,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_page_rank(
                &canonical,
                &edge_kind,
                damping,
                iterations,
                top_n,
                format.into(),
            )?;
        }
        Cmd::HarmonicCentrality {
            path,
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_harmonic_centrality(&canonical, &path, &edge_kind, format.into())?;
        }
        Cmd::NeighborSimilarity {
            path1,
            path2,
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_neighbor_similarity(
                &canonical,
                &path1,
                &path2,
                &edge_kind,
                format.into(),
            )?;
        }
        Cmd::GetStats { root, format } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_stats(&canonical, format.into())?;
        }
        Cmd::GetGraphMetrics {
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_graph_metrics(&canonical, &edge_kind, format.into())?;
        }
        Cmd::DetectCycles {
            edge_kind,
            path_prefix,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_detect_cycles(
                &canonical,
                &edge_kind,
                path_prefix.as_deref(),
                format.into(),
            )?;
        }
        Cmd::GetSccGroups {
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_scc_groups(&canonical, &edge_kind, format.into())?;
        }
        Cmd::TopologicalSort {
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_topological_sort(&canonical, &edge_kind, format.into())?;
        }
        Cmd::FindArticulationPoints {
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_find_articulation_points(&canonical, &edge_kind, format.into())?;
        }
        Cmd::FindBridgeEdges {
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_find_bridge_edges(&canonical, &edge_kind, format.into())?;
        }
        Cmd::GetBiconnectedComponents {
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_biconnected_components(&canonical, &edge_kind, format.into())?;
        }
        Cmd::GetKCore {
            edge_kind,
            k,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_k_core(&canonical, &edge_kind, k, format.into())?;
        }
        Cmd::GetDependencyLayers {
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_dependency_layers(&canonical, &edge_kind, format.into())?;
        }
        Cmd::GetScc {
            edge_kind,
            min_size,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_scc(&canonical, &edge_kind, min_size, format.into())?;
        }
        Cmd::GetWcc {
            edge_kind,
            min_size,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_wcc(&canonical, &edge_kind, min_size, format.into())?;
        }
        Cmd::GetDegreeHistogram {
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_degree_histogram(&canonical, &edge_kind, format.into())?;
        }
        Cmd::FindCycleMembers {
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_find_cycle_members(&canonical, &edge_kind, format.into())?;
        }
        Cmd::BatchSymbolInfo {
            paths,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            let paths: Vec<String> = paths
                .split(',')
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
                .collect();
            queries::run_batch_symbol_info(&canonical, &paths, format.into())?;
        }
        Cmd::BatchNodeDegree {
            paths,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            let paths: Vec<String> = paths
                .split(',')
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
                .collect();
            queries::run_batch_node_degree(&canonical, &paths, format.into())?;
        }
        Cmd::BatchReachableFrom {
            paths,
            edge_kind,
            max_depth,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            let paths: Vec<String> = paths
                .split(',')
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
                .collect();
            queries::run_batch_reachable_from(
                &canonical,
                &paths,
                &edge_kind,
                max_depth,
                format.into(),
            )?;
        }
        Cmd::BatchReachableTo {
            paths,
            edge_kind,
            max_depth,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            let paths: Vec<String> = paths
                .split(',')
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
                .collect();
            queries::run_batch_reachable_to(
                &canonical,
                &paths,
                &edge_kind,
                max_depth,
                format.into(),
            )?;
        }
        Cmd::GetNodeDegree { path, root, format } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_node_degree(&canonical, &path, format.into())?;
        }
        Cmd::GetFiles {
            path_prefix,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_files(&canonical, path_prefix.as_deref(), format.into())?;
        }
        Cmd::GetSymbolCountByKind { root, format } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_symbol_count_by_kind(&canonical, format.into())?;
        }
        Cmd::GetLeafSymbols {
            edge_kind,
            limit,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_leaf_symbols(&canonical, &edge_kind, limit, format.into())?;
        }
        Cmd::GetCommonCallers {
            paths,
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            let paths: Vec<String> = paths
                .split(',')
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
                .collect();
            queries::run_get_common_callers(&canonical, &paths, &edge_kind, format.into())?;
        }
        Cmd::GetCommonCallees {
            paths,
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            let paths: Vec<String> = paths
                .split(',')
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
                .collect();
            queries::run_get_common_callees(&canonical, &paths, &edge_kind, format.into())?;
        }
        Cmd::GetCommonReachable {
            path1,
            path2,
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_common_reachable(
                &canonical,
                &path1,
                &path2,
                &edge_kind,
                format.into(),
            )?;
        }
        Cmd::GetMutualReachability {
            path1,
            path2,
            edge_kind,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_get_mutual_reachability(
                &canonical,
                &path1,
                &path2,
                &edge_kind,
                format.into(),
            )?;
        }
        Cmd::FindCallPath {
            from,
            to,
            max_depth,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_find_call_path(&canonical, &from, &to, max_depth, format.into())?;
        }
        Cmd::FindImportPath {
            from,
            to,
            max_depth,
            root,
            format,
        } => {
            let canonical = root.canonicalize().unwrap_or(root);
            queries::run_find_import_path(&canonical, &from, &to, max_depth, format.into())?;
        }
        Cmd::Serve { mcp: true, root } => {
            let root = root.map(|p| p.canonicalize().unwrap_or(p));
            let rt = Runtime::new()?;
            rt.block_on(mycelium_mcp::serve_stdio(root))?;
        }
        Cmd::Serve { mcp: false, .. } => {
            tracing::warn!("`mycelium serve` requires `--mcp` flag (other transports are v0.2)");
        }
    }
    Ok(())
}
