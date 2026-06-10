//! Request schema types for all `mycelium_*` MCP tools.
//!
//! All types derive [`serde::Deserialize`] and [`schemars::JsonSchema`].
//! The handler implementations live in `lib.rs`.

use schemars::JsonSchema;
use serde::Deserialize;

pub use crate::formatter::OutputFormat;

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
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_ancestors`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetAncestorsRequest {
    /// Trunk path to look up, e.g. `"src/main.rs>greet"`.
    pub path: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_descendants`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetDescendantsRequest {
    /// Trunk path to look up, e.g. `"src/lib.rs"`.
    pub path: String,
    /// When `true`, also return methods inherited from base classes via
    /// Extends edges. Inherited methods appear in an `inherited_descendants`
    /// array, each entry as `{"path": "...", "from": "..."}`. Methods
    /// overridden by the class are excluded from the inherited list.
    /// Defaults to `false` for backward compatibility.
    #[serde(default)]
    pub include_inherited: Option<bool>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
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
    /// Edge kind to traverse: `"calls"` (default), `"imports"`, `"extends"`, `"implements"`.
    #[serde(default)]
    pub edge_kind: Option<String>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
    /// Per-call output budget (RFC-0102): `"auto"` (default), `"small"` /
    /// `"medium"` / `"large"`, or `"disabled"`. Unknown values are rejected.
    /// The CLI `--budget` flag is the byte-identical twin.
    #[serde(default)]
    pub budget: Option<String>,
}

/// Input parameters for `mycelium_get_callers`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCallersRequest {
    /// Trunk path to look up callers for, e.g. `"src/lib.rs>helper"`.
    pub path: String,
    /// Edge kind to traverse: `"calls"` (default), `"imports"`, `"extends"`, `"implements"`.
    #[serde(default)]
    pub edge_kind: Option<String>,
    /// When true, also include callers that reach this symbol via virtual dispatch —
    /// i.e., callers that call an ancestor (base class) method of the same name.
    /// Only applies when `edge_kind` is `"calls"` (the default). Default: false.
    #[serde(default)]
    pub include_virtual: Option<bool>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
    /// Per-call output budget (RFC-0102): `"auto"` (default), `"small"` /
    /// `"medium"` / `"large"`, or `"disabled"`. Unknown values are rejected.
    /// The CLI `--budget` flag is the byte-identical twin.
    #[serde(default)]
    pub budget: Option<String>,
}

/// Input parameters for `mycelium_get_symbol_info`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSymbolInfoRequest {
    /// Trunk path to query, e.g. `"src/lib.rs>AuthService>login"`.
    pub path: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_callee_tree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCalleeTreeRequest {
    /// Root symbol path, e.g. `"src/main.rs>main"`.
    pub path: String,
    /// Maximum traversal depth. Defaults to 4, capped at 10.
    pub max_depth: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_caller_tree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCallerTreeRequest {
    /// Root symbol path, e.g. `"src/db.rs>query"`.
    pub path: String,
    /// Maximum traversal depth. Defaults to 4, capped at 10.
    pub max_depth: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_imports`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetImportsRequest {
    /// Trunk path to query, e.g. `"src/auth.rs"`.
    pub path: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_import_tree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetImportTreeRequest {
    /// Root path, e.g. `"src/auth.rs"`.
    pub path: String,
    /// Maximum traversal depth. Defaults to 4, capped at 10.
    pub max_depth: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_batch_symbol_info`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BatchSymbolInfoRequest {
    /// List of trunk paths to query (maximum 50).
    pub paths: Vec<String>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
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
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_extends_tree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetExtendsTreeRequest {
    /// Root symbol path, e.g. `"src/child.ts>Child"`.
    pub path: String,
    /// Maximum DFS depth. Defaults to 4, capped at 10.
    pub max_depth: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_subclasses_tree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSubclassesTreeRequest {
    /// Root symbol path, e.g. `"src/base.ts>Base"`.
    pub path: String,
    /// Maximum DFS depth. Defaults to 4, capped at 10.
    pub max_depth: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
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
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_implements_tree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetImplementsTreeRequest {
    /// Root symbol path, e.g. `"src/cls.ts>Cls"`.
    pub path: String,
    /// Maximum DFS depth. Defaults to 4, capped at 10.
    pub max_depth: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_implementors_tree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetImplementorsTreeRequest {
    /// Root symbol path (interface), e.g. `"src/iface.ts>IFace"`.
    pub path: String,
    /// Maximum DFS depth. Defaults to 4, capped at 10.
    pub max_depth: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_importers_tree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetImportersTreeRequest {
    /// Root symbol path (module), e.g. `"src/utils.ts>utils"`.
    pub path: String,
    /// Maximum DFS depth. Defaults to 4, capped at 10.
    pub max_depth: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
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
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_node_kind`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetNodeKindRequest {
    /// Trunk path to query, e.g. `"src/auth.rs>login"`.
    pub path: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_symbols_by_kind`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSymbolsByKindRequest {
    /// `NodeKind` wire string, e.g. `"function"`, `"class"`, `"method"`.
    pub kind: String,
    /// Optional path prefix to restrict results, e.g. `"src/"`.
    pub path_prefix: Option<String>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_source_span`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSourceSpanRequest {
    /// Trunk path to query, e.g. `"src/auth.rs>login"`.
    pub path: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_extends`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetExtendsRequest {
    /// Trunk path to query, e.g. `"src/shapes.py>Rectangle"`.
    pub path: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_implements`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetImplementsRequest {
    /// Trunk path to query, e.g. `"src/io.ts>FileReader"`.
    pub path: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_entry_points`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetEntryPointsRequest {
    /// Optional path prefix to restrict results (e.g. `"src/handlers/"`).
    pub path_prefix: Option<String>,
    /// Maximum number of symbols to return. `0` or omitted means no limit.
    #[serde(default)]
    pub limit: Option<usize>,
    /// Number of symbols to skip before returning results. Defaults to 0.
    #[serde(default)]
    pub offset: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
    /// Per-call output budget (RFC-0102): `"auto"` (default), `"small"` /
    /// `"medium"` / `"large"`, or `"disabled"`. Caps the paginated page.
    /// Unknown values are rejected. The CLI `--budget` flag is the twin.
    #[serde(default)]
    pub budget: Option<String>,
}

/// Input parameters for `mycelium_rank_symbols`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RankSymbolsRequest {
    /// Maximum results to return (default 10, capped at 100).
    pub limit: Option<usize>,
    /// Edge kind to rank by incoming-edge count: `"calls"` (default), `"imports"`, `"extends"`, `"implements"`.
    #[serde(default)]
    pub edge_kind: Option<String>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_top_files`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetTopFilesRequest {
    /// Maximum results to return (default 10, capped at 100).
    pub limit: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_most_connected`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetMostConnectedRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Maximum results to return (default 10, capped at 100).
    pub limit: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_leaf_symbols`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetLeafSymbolsRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Maximum results to return (default 10, capped at 100).
    pub limit: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
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
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_symbol_count_by_kind` (no parameters).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSymbolCountByKindRequest {
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_common_callers`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCommonCallersRequest {
    /// Target node paths to intersect (1–20 entries).
    pub paths: Vec<String>,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_common_callees`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCommonCalleesRequest {
    /// Source node paths to intersect (1–20 entries).
    pub paths: Vec<String>,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_fan_out_rank`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFanOutRankRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Maximum results to return (default 10, capped at 100).
    pub limit: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_fan_in_rank`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFanInRankRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Maximum results to return (default 10, capped at 100).
    pub limit: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_files`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFilesRequest {
    /// Optional path prefix to filter results (e.g. `"src/"`).
    pub path_prefix: Option<String>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_dead_symbols`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetDeadSymbolsRequest {
    /// Optional path prefix to filter results (e.g. `"src/"`).
    pub path_prefix: Option<String>,
    /// When set, return symbols with no incoming edges of this specific kind
    /// (`"calls"`, `"imports"`, `"extends"`, `"implements"`).
    /// When omitted (default), returns symbols with no incoming Calls AND no incoming Imports
    /// — the classic "unreachable" definition.
    #[serde(default)]
    pub edge_kind: Option<String>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
    /// Per-call output budget (RFC-0102): `"auto"` (default), `"small"` /
    /// `"medium"` / `"large"`, or `"disabled"`. Unknown values are rejected.
    /// The CLI `--budget` flag is the byte-identical twin.
    #[serde(default)]
    pub budget: Option<String>,
}

/// Input parameters for `mycelium_get_isolated_symbols`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetIsolatedSymbolsRequest {
    /// Optional path prefix to filter results (e.g. `"src/"`).
    pub path_prefix: Option<String>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
    /// Per-call output budget (RFC-0102): `"auto"` (default), `"small"` /
    /// `"medium"` / `"large"`, or `"disabled"`. Unknown values are rejected.
    /// The CLI `--budget` flag is the byte-identical twin.
    #[serde(default)]
    pub budget: Option<String>,
}

/// Input parameters for `mycelium_get_stats`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetStatsRequest {
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_cross_refs`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCrossRefsRequest {
    /// Symbol path to look up, e.g. `"src/lib.rs>MyClass"`.
    pub path: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_outgoing_refs`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetOutgoingRefsRequest {
    /// Symbol path to look up, e.g. `"src/app.rs>App"`.
    pub path: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_scc_groups`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSccGroupsRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_dependency_layers`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetDependencyLayersRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_two_hop_neighbors`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetTwoHopNeighborsRequest {
    /// Symbol path, e.g. `"src/service.rs>Service"`.
    pub path: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_symbol_neighborhood`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSymbolNeighborhoodRequest {
    /// Symbol path, e.g. `"src/service.rs>Service"`.
    pub path: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
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
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_singly_referenced`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSinglyReferencedRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Maximum results returned. Defaults to 10, capped at 100.
    pub limit: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
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
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
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
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_batch_node_degree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BatchNodeDegreeRequest {
    /// Symbol paths to query (up to 50 entries).
    pub paths: Vec<String>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_wcc`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetWccRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Only return components with at least this many symbols. Defaults to 1.
    pub min_size: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_find_articulation_points`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindArticulationPointsRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_find_bridge_edges`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindBridgeEdgesRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_biconnected_components`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BiconnectedComponentsRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_degree_histogram`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DegreeHistogramRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_graph_metrics`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphMetricsRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
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
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_clustering_coefficient`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClusteringCoefficientRequest {
    /// Symbol path, e.g. `"src/a.rs>MyStruct"`.
    pub path: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_eccentricity`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EccentricityRequest {
    /// Symbol path, e.g. `"src/a.rs>MyStruct"`.
    pub path: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_harmonic_centrality`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HarmonicCentralityRequest {
    /// Symbol path, e.g. `"src/a.rs>MyStruct"`.
    pub path: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
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
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_betweenness_centrality`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BetweennessCentralityRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// How many top entries to return; defaults to 10 if absent.
    pub top_n: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_sync_file`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SyncFileRequest {
    /// Relative path of the file to re-index (e.g. `"src/auth.rs"`).
    pub path: String,
}

/// Input parameters for `mycelium_get_dependency_depth`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DependencyDepthRequest {
    /// Symbol path, e.g. `"src/a.rs>A"`.
    pub path: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_closeness_centrality`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClosenessCentralityRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// How many top entries to return; defaults to 10 if absent.
    pub top_n: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
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
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_strongly_connected_components`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StronglyConnectedComponentsRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Minimum component size to include; defaults to 1 (all components).
    /// Use `2` to return only non-trivial SCCs (circular dependencies).
    pub min_size: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
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
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
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
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
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
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_reaches_into`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReachesIntoRequest {
    /// Symbol path, e.g. `"src/a.rs>A"`.
    pub path: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_reachable_set`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReachableSetRequest {
    /// Symbol path, e.g. `"src/a.rs>A"`.
    pub path: String,
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_topological_sort`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TopologicalSortRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_find_cycle_members`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindCycleMembersRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_k_core`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetKCoreRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Minimum total degree (in + out) within the induced subgraph. Defaults to 2.
    pub k: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_all_symbols`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetAllSymbolsRequest {
    /// Optional path prefix to restrict results, e.g. `"src/"`.
    pub path_prefix: Option<String>,
    /// Optional kind filter: `"function"`, `"class"`, `"method"`, etc.
    pub kind: Option<String>,
    /// Maximum number of symbols to return. `0` or omitted means no limit.
    #[serde(default)]
    pub limit: Option<usize>,
    /// Number of symbols to skip before returning results. Defaults to 0.
    #[serde(default)]
    pub offset: Option<usize>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
    /// Per-call output budget (RFC-0102): `"auto"` (default), `"small"` /
    /// `"medium"` / `"large"`, or `"disabled"`. Caps the paginated page.
    /// Unknown values are rejected. The CLI `--budget` flag is the twin.
    #[serde(default)]
    pub budget: Option<String>,
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
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
    /// Per-call output budget (RFC-0102): `"auto"` (default), `"small"` /
    /// `"medium"` / `"large"`, or `"disabled"`. Unknown values are rejected.
    /// The CLI `--budget` flag is the byte-identical twin.
    #[serde(default)]
    pub budget: Option<String>,
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
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
    /// Per-call output budget (RFC-0102): `"auto"` (default), `"small"` /
    /// `"medium"` / `"large"`, or `"disabled"`. Unknown values are rejected.
    /// The CLI `--budget` flag is the byte-identical twin.
    #[serde(default)]
    pub budget: Option<String>,
}

/// Input parameters for `mycelium_get_siblings`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSiblingsRequest {
    /// Symbol path whose siblings to look up, e.g. `"src/app.rs>App>render"`.
    pub path: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_query` — the MCP twin of the CLI
/// `mycelium query <expr>` subcommand (Three-Surface Rule, RFC-0090, #151).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryRequest {
    /// A Hyphae DSL selector. See RFC-0003 for the grammar.
    ///
    /// Examples: `#login` (name selector), `.function` (kind selector),
    /// `.class>.method` (direct-child combinator),
    /// `.function:calls(.function)` (pseudo-class — when executor supports it).
    pub expr: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_node_degree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetNodeDegreeRequest {
    /// Symbol or file path to analyse, e.g. `"src/app.rs>App"`.
    pub path: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_detect_cycles`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DetectCyclesRequest {
    /// Edge kind to analyze: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Optional path prefix to filter returned cycle nodes (e.g. `"src/"`).
    pub path_prefix: Option<String>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
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
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_set_compact_mode`.
///
/// When compact mode is `true`, tools that support it (currently
/// `mycelium_search_symbol`) return a MessagePack-encoded payload encoded as
/// a lowercase hexadecimal string wrapped in
/// `{ "fmt": "msgpack_hex", "data": "<hex>" }` instead of plain JSON.  This
/// typically reduces token consumption to ≤ 30 % of the equivalent JSON
/// payload (Charter §2 SLA).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetCompactModeRequest {
    /// Set to `true` to enable compact `MessagePack` output, `false` to revert
    /// to human-readable JSON.
    pub enabled: bool,
}

/// Input parameters for `mycelium_context`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetContextRequest {
    /// Natural-language task, for example "how does request routing work"
    /// or "trace `handle_request` to `get_user`".
    pub task: String,
    /// Maximum graph nodes to return (default: 30).
    #[serde(default)]
    pub max_nodes: Option<usize>,
    /// Maximum source snippets to return (default: 6).
    #[serde(default)]
    pub max_code_blocks: Option<usize>,
    /// Edge kinds to expand during one-hop graph traversal, e.g.
    /// `["calls", "imports", "extends"]`. Omit or empty ⇒ `["calls"]`
    /// (RFC-0101 `edge_kinds`). Unknown names are ignored.
    #[serde(default)]
    pub edge_kinds: Option<Vec<String>>,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
    /// Per-call output budget (RFC-0102): `"auto"` (default, follows project
    /// size), `"small"` / `"medium"` / `"large"` (pin a tier), or `"disabled"`
    /// (no truncation). Unknown values are rejected. The CLI `--budget` flag is
    /// the byte-identical twin.
    #[serde(default)]
    pub budget: Option<String>,
}

/// Input parameters for `mycelium_safe_to_edit` (RFC-0116 Phase 2).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSafeToEditRequest {
    /// Symbol path to assess, e.g. `"src/auth.rs>Session>login"`.
    pub path: String,
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_project_health`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetProjectHealthRequest {
    /// Response format override. Omit to use the transport default — `"text"`
    /// (TOON, fewer tokens) on stdio MCP for LLM callers (RFC-0094 Phase 4),
    /// `"json"` for programmatic/CLI callers. Explicit: `"json"`, `"text"`,
    /// `"msgpack"` (hex-encoded binary).
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_test_gap`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetTestGapRequest {
    /// Path to a `coverage.json` (coverage.py format) artifact, relative to
    /// the indexed workspace root. Omit to auto-discover `coverage.json` at
    /// the workspace root.
    #[serde(default)]
    pub coverage: Option<String>,
    /// Cap the returned gap list to the top-N symbols. Default: 20.
    #[serde(default)]
    pub top: Option<usize>,
    /// Response format override. Omit to use the transport default.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}
