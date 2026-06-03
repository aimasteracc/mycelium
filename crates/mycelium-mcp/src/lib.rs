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
//! | `mycelium_set_compact_mode` | Enable or disable compact (`MessagePack` hex) output for `mycelium_search_symbol` |
//! | `mycelium_get_token_stats` | Return JSON vs `MessagePack` byte counts and compression ratio for a sample payload |
//!
//! See RFC-0004, RFC-0005, RFC-0006, RFC-0007, RFC-0008, RFC-0010, RFC-0011, RFC-0012, RFC-0016, RFC-0017, RFC-0018, RFC-0019, RFC-0020, RFC-0021, RFC-0022, RFC-0023, RFC-0024, RFC-0025, RFC-0026, RFC-0027, RFC-0028, RFC-0029, RFC-0030, RFC-0031, RFC-0032, RFC-0033, RFC-0034, RFC-0035, RFC-0036, and RFC-0090 for the design.

pub mod error;
pub mod formatter;
mod push;
pub mod query_delta;
mod query_eval;
pub mod subscription;

/// Internal fan-out work-item produced inside `start_watch`'s `on_batch`
/// closure (RFC-0107 + RFC-0108). Lifted to module scope so clippy's
/// `items_after_statements` is satisfied.
enum Fanout {
    Delta {
        sub_id: String,
        new_set: Option<std::collections::BTreeSet<String>>,
        payload: subscription::SubscriptionDeltaEvent,
    },
    QueryDelta {
        sub_id: String,
        is_set_shaped: bool,
        payload: query_delta::QueryResultChangedEvent,
    },
    PauseQuery {
        sub_id: String,
    },
}

/// Parse a `"b3:<32-hex>"` hash back to its `[u8; 16]` bytes. Returns
/// `None` on any prefix / length / hex-digit mismatch. RFC-0108.
fn parse_hash_hex(s: &str) -> Option<[u8; 16]> {
    let hex_part = s.strip_prefix("b3:")?;
    if hex_part.len() != 32 {
        return None;
    }
    let mut out = [0_u8; 16];
    for (i, slot) in out.iter_mut().enumerate() {
        let byte_hex = hex_part.get(i * 2..i * 2 + 2)?;
        *slot = u8::from_str_radix(byte_hex, 16).ok()?;
    }
    Some(out)
}

use std::collections::BTreeSet;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use anyhow::Context as _;
use mycelium_core::{
    CalleeNode, CallerNode, CrossRefs, ExtendsNode, GraphStats, ImplementorNode, ImplementsNode,
    ImportNode, ImporterNode, NodeDegree, OutgoingRefs, SubclassNode, SymbolNeighborhood,
    TopologicalOrder, cortex::Cortex, extractor::Extractor, store::Store, types::EdgeKind,
};
// notify is now used inside `mycelium_core::watch::WatchEngine` (RFC-0105).
use rmcp::{
    ServerHandler, ServiceExt, handler::server::wrapper::Parameters, model::CallToolResult,
    model::Implementation, model::ServerCapabilities, model::ServerInfo, tool, tool_handler,
    tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::error::{application_error, not_found, success_str};
use crate::formatter::{OutputFormat, formatter_for};

fn legacy_index_path(root: &Path) -> PathBuf {
    root.join(".mycelium").join("index.rmp")
}

#[cfg(feature = "redb-backend")]
fn redb_index_path(root: &Path) -> PathBuf {
    root.join(".mycelium").join("index.redb")
}

fn existing_index_path(root: &Path) -> Option<PathBuf> {
    #[cfg(feature = "redb-backend")]
    {
        let redb = redb_index_path(root);
        if redb.exists() {
            return Some(redb);
        }
    }

    let legacy = legacy_index_path(root);
    legacy.exists().then_some(legacy)
}

fn source_extension(path: &Path) -> Option<&str> {
    let ext = path.extension().and_then(|e| e.to_str())?;
    matches!(
        ext,
        "js" | "jsx"
            | "py"
            | "pyi"
            | "ts"
            | "tsx"
            | "rs"
            | "go"
            | "java"
            | "c"
            | "h"
            | "rb"
            | "cpp"
            | "cc"
            | "cxx"
            | "hpp"
            | "cs"
    )
    .then_some(ext)
}

#[cfg(feature = "redb-backend")]
fn is_supported_source_rel(path: &str) -> bool {
    source_extension(Path::new(path)).is_some()
}

#[cfg(feature = "redb-backend")]
fn persist_full_redb_index(root: &Path, store: &Store) -> anyhow::Result<()> {
    use mycelium_core::store::redb_backend::RedbBackend;

    let redb = redb_index_path(root);
    if let Some(parent) = redb.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating redb index dir {}", parent.display()))?;
    }
    let mut backend = RedbBackend::open(&redb)
        .map_err(|e| anyhow::anyhow!("opening redb index {}: {e}", redb.display()))?;

    for file_path in store
        .all_file_paths()
        .into_iter()
        .filter(|path| is_supported_source_rel(path))
    {
        backend
            .replace_file_from_store(&file_path, store)
            .map_err(|e| anyhow::anyhow!("persisting {file_path} to redb: {e}"))?;
    }
    Ok(())
}

#[cfg(feature = "redb-backend")]
fn persist_redb_watch_batch(
    root: &Path,
    store: &Store,
    changed_files: &[String],
) -> anyhow::Result<()> {
    use mycelium_core::store::redb_backend::RedbBackend;

    let redb = redb_index_path(root);
    if let Some(parent) = redb.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating redb index dir {}", parent.display()))?;
    }
    let mut backend = RedbBackend::open(&redb)
        .map_err(|e| anyhow::anyhow!("opening redb index {}: {e}", redb.display()))?;

    let mut files = changed_files.to_vec();
    files.sort_unstable();
    files.dedup();
    for file_path in files
        .into_iter()
        .filter(|path| is_supported_source_rel(path))
    {
        backend
            .replace_file_from_store(&file_path, store)
            .map_err(|e| anyhow::anyhow!("persisting {file_path} to redb: {e}"))?;
    }
    Ok(())
}

fn persist_watch_batch(root: &Path, store: &Store, changed_files: &[String]) -> anyhow::Result<()> {
    #[cfg(feature = "redb-backend")]
    {
        persist_redb_watch_batch(root, store, changed_files)
    }

    #[cfg(not(feature = "redb-backend"))]
    {
        use mycelium_core::store::journal::Journal;
        let snap = legacy_index_path(root);
        let mut journal = Journal::open(&snap)?;
        for file_path in changed_files {
            let sub = store.extract_file_substore(file_path);
            journal.append(file_path, &sub)?;
        }
        if journal.should_compact() {
            journal.compact(store)?;
        }
        Ok(())
    }
}

/// Shared state for the background watch loop.
#[derive(Debug, Default)]
struct WatchState {
    watching: AtomicBool,
    batches_processed: AtomicU64,
}

/// Parse a user-supplied `edge_kind` string into an [`EdgeKind`].
///
/// Accepts any ASCII case form (`"calls"`, `"Calls"`, `"CALLS"`, ...). The
/// tool descriptions used to advertise `PascalCase` examples (`EdgeKind::Calls`)
/// while the runtime only accepted lowercase, which surprised users.
/// Issue #152.
fn parse_edge_kind(s: &str) -> Result<EdgeKind, String> {
    match s.to_ascii_lowercase().as_str() {
        "calls" => Ok(EdgeKind::Calls),
        "imports" => Ok(EdgeKind::Imports),
        "extends" => Ok(EdgeKind::Extends),
        "implements" => Ok(EdgeKind::Implements),
        other => Err(format!(
            "unknown edge_kind '{other}'; expected one of: calls, imports, extends, implements"
        )),
    }
}

// `extract_symbol_candidates`, the stop-word list, and the path helpers used to
// live here as a near-duplicate of the CLI twin. They now have a single home in
// `mycelium_core::context` so both surfaces are byte-identical by construction
// (RFC-0101 / Three-Surface Rule).

#[cfg(test)]
mod edge_kind_tests {
    use super::*;

    #[test]
    fn lowercase_canonical_forms_parse() {
        assert_eq!(parse_edge_kind("calls"), Ok(EdgeKind::Calls));
        assert_eq!(parse_edge_kind("imports"), Ok(EdgeKind::Imports));
        assert_eq!(parse_edge_kind("extends"), Ok(EdgeKind::Extends));
        assert_eq!(parse_edge_kind("implements"), Ok(EdgeKind::Implements));
    }

    #[test]
    fn pascalcase_matches_lowercase() {
        // Matches the form the original tool descriptions advertised.
        assert_eq!(parse_edge_kind("Calls"), Ok(EdgeKind::Calls));
        assert_eq!(parse_edge_kind("Imports"), Ok(EdgeKind::Imports));
        assert_eq!(parse_edge_kind("Extends"), Ok(EdgeKind::Extends));
        assert_eq!(parse_edge_kind("Implements"), Ok(EdgeKind::Implements));
    }

    #[test]
    fn screaming_case_matches_lowercase() {
        assert_eq!(parse_edge_kind("CALLS"), Ok(EdgeKind::Calls));
        assert_eq!(parse_edge_kind("IMPORTS"), Ok(EdgeKind::Imports));
    }

    #[test]
    fn unknown_value_returns_helpful_error() {
        let err = parse_edge_kind("contains").unwrap_err();
        assert!(err.contains("unknown edge_kind"));
        assert!(err.contains("calls"));
        assert!(err.contains("imports"));
        assert!(err.contains("extends"));
        assert!(err.contains("implements"));
    }
}

// ── embedded pack queries ─────────────────────────────────────────────────────
// Paths are relative to this crate's root so the crate is self-contained
// for `cargo publish` (workspace-level `packs/` is mirrored under `packs/`
// inside this crate directory).

const JAVASCRIPT_QUERIES: &str = include_str!("../packs/javascript/queries.scm");
const PYTHON_QUERIES: &str = include_str!("../packs/python/queries.scm");
const TYPESCRIPT_QUERIES: &str = include_str!("../packs/typescript/queries.scm");
const RUST_QUERIES: &str = include_str!("../packs/rust/queries.scm");
const GO_QUERIES: &str = include_str!("../packs/go/queries.scm");
const JAVA_QUERIES: &str = include_str!("../packs/java/queries.scm");
const C_QUERIES: &str = include_str!("../packs/c/queries.scm");
const RUBY_QUERIES: &str = include_str!("../packs/ruby/queries.scm");
const CPP_QUERIES: &str = include_str!("../packs/cpp/queries.scm");
const CSHARP_QUERIES: &str = include_str!("../packs/csharp/queries.scm");

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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_ancestors`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetAncestorsRequest {
    /// Trunk path to look up, e.g. `"src/main.rs>greet"`.
    pub path: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_imports`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetImportsRequest {
    /// Trunk path to query, e.g. `"src/auth.rs"`.
    pub path: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_batch_symbol_info`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BatchSymbolInfoRequest {
    /// List of trunk paths to query (maximum 50).
    pub paths: Vec<String>,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_node_kind`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetNodeKindRequest {
    /// Trunk path to query, e.g. `"src/auth.rs>login"`.
    pub path: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_source_span`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSourceSpanRequest {
    /// Trunk path to query, e.g. `"src/auth.rs>login"`.
    pub path: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_extends`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetExtendsRequest {
    /// Trunk path to query, e.g. `"src/shapes.py>Rectangle"`.
    pub path: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_implements`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetImplementsRequest {
    /// Trunk path to query, e.g. `"src/io.ts>FileReader"`.
    pub path: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_entry_points`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetEntryPointsRequest {
    /// Optional path prefix to restrict results (e.g. `"src/handlers/"`).
    pub path_prefix: Option<String>,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_rank_symbols`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RankSymbolsRequest {
    /// Maximum results to return (default 10, capped at 100).
    pub limit: Option<usize>,
    /// Edge kind to rank by incoming-edge count: `"calls"` (default), `"imports"`, `"extends"`, `"implements"`.
    #[serde(default)]
    pub edge_kind: Option<String>,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_top_files`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetTopFilesRequest {
    /// Maximum results to return (default 10, capped at 100).
    pub limit: Option<usize>,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_symbol_count_by_kind` (no parameters).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSymbolCountByKindRequest {
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_files`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFilesRequest {
    /// Optional path prefix to filter results (e.g. `"src/"`).
    pub path_prefix: Option<String>,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_cross_refs`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCrossRefsRequest {
    /// Symbol path to look up, e.g. `"src/lib.rs>MyClass"`.
    pub path: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_outgoing_refs`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetOutgoingRefsRequest {
    /// Symbol path to look up, e.g. `"src/app.rs>App"`.
    pub path: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_scc_groups`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSccGroupsRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_dependency_layers`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetDependencyLayersRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_batch_node_degree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BatchNodeDegreeRequest {
    /// Symbol paths to query (up to 50 entries).
    pub paths: Vec<String>,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_find_articulation_points`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindArticulationPointsRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_find_bridge_edges`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindBridgeEdgesRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_biconnected_components`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BiconnectedComponentsRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_degree_histogram`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DegreeHistogramRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_graph_metrics`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphMetricsRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_topological_sort`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TopologicalSortRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_find_cycle_members`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindCycleMembersRequest {
    /// Edge kind: `"calls"`, `"imports"`, `"extends"`, or `"implements"`.
    pub edge_kind: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_siblings`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSiblingsRequest {
    /// Symbol path whose siblings to look up, e.g. `"src/app.rs>App>render"`.
    pub path: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
}

/// Input parameters for `mycelium_get_node_degree`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetNodeDegreeRequest {
    /// Symbol or file path to analyse, e.g. `"src/app.rs>App"`.
    pub path: String,
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
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
    /// Response format: `"json"` (default), `"text"` (TOON, fewer tokens),
    /// `"msgpack"` (hex-encoded binary). Omit for JSON.
    #[serde(default)]
    pub output_format: Option<OutputFormat>,
    /// Per-call output budget (RFC-0102): `"auto"` (default, follows project
    /// size), `"small"` / `"medium"` / `"large"` (pin a tier), or `"disabled"`
    /// (no truncation). Unknown values are rejected. The CLI `--budget` flag is
    /// the byte-identical twin.
    #[serde(default)]
    pub budget: Option<String>,
}

// ── server ────────────────────────────────────────────────────────────────────

// `OutputBudget` and `apply_budget` now live in `mycelium_core::budget` so the
// CLI applies the *same* truncation and CLI↔MCP output stays byte-identical
// (Three-Surface Rule). The two dead fields (`max_code_lines`/`max_total_chars`)
// were removed there — they were never enforced.
use mycelium_core::budget::{BudgetOverride, OutputBudget, apply_budget};

#[allow(dead_code)]
fn is_core_tool(name: &str) -> bool {
    matches!(
        name,
        "mycelium_context"
            | "mycelium_search_symbol"
            | "mycelium_get_symbol_info"
            | "mycelium_query"
            | "mycelium_server_status"
            | "mycelium_index_workspace"
    )
}

/// MCP server for Mycelium code graph analysis.
///
/// Construct with [`MyceliumServer::new`] or [`MyceliumServer::with_root`]
/// and start with [`serve_stdio`].
#[derive(Debug, Clone)]
pub struct MyceliumServer {
    store: Arc<RwLock<Store>>,
    indexed_root: Arc<RwLock<Option<PathBuf>>>,
    watch_state: Arc<WatchState>,
    watch_abort: Arc<tokio::sync::Mutex<Option<tokio::task::AbortHandle>>>,
    /// PUSH notifier (RFC-0106 / Option B).
    ///
    /// Holds the rmcp `Peer<RoleServer>` captured after [`Self::serve`] returns
    /// `RunningService`. The watch `on_batch` closure clones this Arc, gates
    /// on `Option::is_some`, and best-effort fires `mycelium/graphChanged`
    /// notifications. Batches that fire before a client has connected (or
    /// after a disconnect) simply skip the send.
    notifier: Arc<tokio::sync::Mutex<Option<rmcp::Peer<rmcp::RoleServer>>>>,
    /// When `true`, symbol-search results are returned as `MessagePack` hex
    /// instead of JSON, achieving the Charter §2 AI token-efficiency SLA.
    compact_mode: Arc<AtomicBool>,
    /// Salsa reactive database for incremental file indexing (Cortex / RFC-0003).
    ///
    /// Wraps file content as [`Cortex`] inputs; the watch loop updates these
    /// on every file-system change so Salsa handles memoisation automatically.
    cortex: Arc<tokio::sync::Mutex<Cortex>>,
    /// RFC-0097: filesystem access boundary.
    ///
    /// When non-empty, every path-based MCP call canonicalizes the input and
    /// verifies it is prefixed by at least one of these roots. Empty = unrestricted
    /// (used only in unit tests; CLI always sets this to `[CWD]` by default).
    allowed_roots: Arc<Vec<PathBuf>>,
    /// Adaptive output budget (issue #380).
    ///
    /// Recomputed after each index operation based on node count.
    /// Prevents a single tool call from flooding the Agent context.
    output_budget: Arc<tokio::sync::Mutex<OutputBudget>>,
    /// SUBSCRIBE (RFC-0107) in-memory subscription store.
    ///
    /// Populated by `mycelium_subscribe`, consumed by the watch `on_batch`
    /// fan-out, evicted by a periodic background task + dead-peer GC. Survives
    /// `start_watch` restarts; cleared only on server drop.
    subscriptions: subscription::Store_,
    /// Aborts the periodic subscription-eviction task spawned in `serve`.
    eviction_abort: Arc<tokio::sync::Mutex<Option<tokio::task::AbortHandle>>>,
}

impl Default for MyceliumServer {
    fn default() -> Self {
        Self::new()
    }
}

impl MyceliumServer {
    /// Create a fresh server with an empty in-memory store and no path restrictions.
    ///
    /// **For unit tests only.** Production code should use [`Self::new_with_allowed_roots`]
    /// or [`Self::with_root_and_allowed_roots`] so the server enforces RFC-0097 boundaries.
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(Store::new())),
            indexed_root: Arc::new(RwLock::new(None)),
            watch_state: Arc::new(WatchState::default()),
            watch_abort: Arc::new(tokio::sync::Mutex::new(None)),
            compact_mode: Arc::new(AtomicBool::new(false)),
            cortex: Arc::new(tokio::sync::Mutex::new(Cortex::default())),
            allowed_roots: Arc::new(vec![]),
            output_budget: Arc::new(tokio::sync::Mutex::new(OutputBudget::for_project(0))),
            notifier: Arc::new(tokio::sync::Mutex::new(None)),
            subscriptions: subscription::new_store(),
            eviction_abort: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Create a fresh server restricted to the given filesystem roots (RFC-0097).
    ///
    /// Any `mycelium_index_workspace` or `mycelium_load_index` call whose
    /// canonicalized path does not fall under one of `roots` is rejected with
    /// `is_error: true` before touching the filesystem.
    #[must_use]
    pub fn new_with_allowed_roots(roots: Vec<PathBuf>) -> Self {
        let canonical_roots: Vec<PathBuf> = roots
            .into_iter()
            .filter_map(|r| std::fs::canonicalize(&r).ok().or(Some(r)))
            .collect();
        Self {
            store: Arc::new(RwLock::new(Store::new())),
            indexed_root: Arc::new(RwLock::new(None)),
            watch_state: Arc::new(WatchState::default()),
            watch_abort: Arc::new(tokio::sync::Mutex::new(None)),
            compact_mode: Arc::new(AtomicBool::new(false)),
            cortex: Arc::new(tokio::sync::Mutex::new(Cortex::default())),
            allowed_roots: Arc::new(canonical_roots),
            output_budget: Arc::new(tokio::sync::Mutex::new(OutputBudget::for_project(0))),
            notifier: Arc::new(tokio::sync::Mutex::new(None)),
            subscriptions: subscription::new_store(),
            eviction_abort: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Create a server pre-loaded from `root`.
    ///
    /// If `<root>/.mycelium/index.rmp` exists, loads the snapshot.
    /// Otherwise runs a full live index and saves the snapshot.
    /// Sets `root` as the sole allowed root (RFC-0097).
    ///
    /// # Errors
    ///
    /// Returns an error only if the live index cannot be initiated (e.g.
    /// `root` is inaccessible). Snapshot load failures fall back to live
    /// indexing silently.
    pub async fn with_root(root: PathBuf) -> anyhow::Result<Self> {
        let allowed = vec![root.clone()];
        Self::with_root_and_allowed_roots(root, allowed).await
    }

    /// Capture the rmcp `Peer<RoleServer>` for PUSH notifications (RFC-0106).
    ///
    /// Call this in the `Cmd::Serve` dispatch **after** `server.serve()`
    /// returns `RunningService` and before `running.waiting()`. Until this is
    /// called, every committed watch batch silently skips the `mycelium/
    /// graphChanged` notification — the watch loop and `persist_watch_batch`
    /// still run unchanged.
    pub async fn set_notifier(&self, peer: rmcp::Peer<rmcp::RoleServer>) {
        *self.notifier.lock().await = Some(peer);
    }

    /// Spawn a periodic background task that evicts expired SUBSCRIBE
    /// subscriptions (RFC-0107 D3 defence-in-depth). One task per server
    /// lifetime; subsequent calls replace the previous task.
    ///
    /// Two evictions per tick:
    /// 1. **TTL** — drop any subscription whose `expires_at <= now`.
    /// 2. **Dead-peer GC** — if the captured `Peer<RoleServer>` reports
    ///    `is_closed()`, evict every subscription owned by that peer's
    ///    `client_tag`. Stdio is single-peer so this clears the whole map;
    ///    multi-peer transports get per-peer eviction once `client_tag` is
    ///    plumbed per-call.
    ///
    /// Tick interval is 60 s. Cooperatively cancelled when the abort handle
    /// is dropped on server shutdown.
    pub async fn start_subscription_eviction(&self) {
        let subs = Arc::clone(&self.subscriptions);
        let notifier = Arc::clone(&self.notifier);
        let handle = tokio::spawn(async move {
            let interval = std::time::Duration::from_secs(60);
            loop {
                tokio::time::sleep(interval).await;
                let n_ttl = subscription::evict_expired(&subs).await;
                if n_ttl > 0 {
                    tracing::debug!(evicted = n_ttl, "subscription TTL eviction tick");
                }
                // Dead-peer GC: if the captured rmcp Peer reports closed,
                // evict every subscription it owns. Stdio currently tags
                // every subscription `stdio-default` (single peer); future
                // multi-peer transports keep this loop unchanged — only
                // the `client_tag` source moves.
                let peer_closed = notifier
                    .lock()
                    .await
                    .as_ref()
                    .is_some_and(rmcp::Peer::is_transport_closed);
                if peer_closed {
                    let n_peer = subscription::evict_for_dead_peer(&subs, "stdio-default").await;
                    if n_peer > 0 {
                        tracing::debug!(
                            evicted = n_peer,
                            client_tag = "stdio-default",
                            "subscription dead-peer GC tick"
                        );
                    }
                }
            }
        });
        let mut guard = self.eviction_abort.lock().await;
        if let Some(old) = guard.replace(handle.abort_handle()) {
            old.abort();
        }
    }

    /// Create a server pre-loaded from `root`, restricted to `allowed_roots` (RFC-0097).
    ///
    /// # Errors
    ///
    /// Returns an error only if the live index cannot be initiated.
    pub async fn with_root_and_allowed_roots(
        root: PathBuf,
        allowed_roots: Vec<PathBuf>,
    ) -> anyhow::Result<Self> {
        let server = Self::new_with_allowed_roots(allowed_roots);

        if let Some(snap) = existing_index_path(&root) {
            match Store::load_with_journal(&snap) {
                Ok(loaded) => {
                    tracing::info!(
                        nodes = loaded.node_count(),
                        path = %snap.display(),
                        "loaded index from snapshot"
                    );
                    #[cfg(feature = "redb-backend")]
                    if let Err(e) = persist_full_redb_index(&root, &loaded) {
                        tracing::warn!("could not persist redb index after load: {e}");
                    }
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
        if let Err(e) = new_store.save(&legacy_index_path(&root)) {
            tracing::warn!("could not save snapshot after live index: {e}");
        }
        #[cfg(feature = "redb-backend")]
        if let Err(e) = persist_full_redb_index(&root, &new_store) {
            tracing::warn!("could not persist redb index after live index: {e}");
        }
        *server.store.write().await = new_store;
        *server.indexed_root.write().await = Some(root.clone());
        server.start_watch(root).await?;
        Ok(server)
    }

    /// Start the background file-system watch loop for `root`.
    ///
    /// Drives the surface-agnostic [`mycelium_core::watch::WatchEngine`]
    /// (RFC-0105) — modified/created files are re-extracted, deleted files
    /// removed, and a new snapshot is persisted after each batch. Calling
    /// `start_watch` on an already-watching server replaces the previous
    /// watcher.
    ///
    /// # Errors
    ///
    /// Returns an error if the OS watcher cannot be created or `root` cannot
    /// be watched.
    #[allow(clippy::too_many_lines)] // single coherent watch-prologue + on_batch closure
    pub async fn start_watch(&self, root: PathBuf) -> anyhow::Result<()> {
        use mycelium_core::watch::{CancelToken, WatchConfig, WatchEngine};

        let store = Arc::clone(&self.store);
        let watch_state = Arc::clone(&self.watch_state);
        let cortex = Arc::clone(&self.cortex);

        let cfg = WatchConfig::new(root.clone());
        let cancel = CancelToken::new();

        // Attach the OS-level recursive watch SYNCHRONOUSLY in this prologue
        // (before the spawn) so the watcher is live by the time `start_watch`
        // returns — i.e. before the caller has a chance to mutate the
        // filesystem. The async loop runs in the spawned task.
        let session = WatchEngine::attach(&cfg)?;

        // on_batch: the deliberate emit seam (RFC-0105). For this server it
        // (1) bumps the batches counter, (2) persists the changed files, and
        // (3) PUSH (RFC-0106): best-effort fires one `mycelium/graphChanged`
        // notification per committed batch via the captured `Peer`. SUBSCRIBE
        // (RFC-0107) will hook here too with scoped per-subscription deltas.
        let watch_state_cb = Arc::clone(&watch_state);
        let root_cb = root.clone();
        let notifier_cb = Arc::clone(&self.notifier);
        let subscriptions_cb = Arc::clone(&self.subscriptions);
        let on_batch = move |ev: &mycelium_core::watch::WatchEvent,
                             delta: &mycelium_core::watch::BatchDelta,
                             store_r: &mycelium_core::store::Store| {
            watch_state_cb
                .batches_processed
                .fetch_add(1, Ordering::Relaxed);
            // Best-effort persist; failures are non-fatal.
            if let Err(e) = persist_watch_batch(&root_cb, store_r, &ev.changed_files) {
                warn!("could not persist watch batch: {e}");
            }
            // PUSH (RFC-0106): fire a CustomNotification per committed batch.
            // Build payload synchronously, send asynchronously, fire-and-forget.
            // Dead client → log + continue (never abort the loop).
            let event = push::GraphChangedEvent::from_watch_event(ev);
            let notifier_for_send = Arc::clone(&notifier_cb);
            tokio::spawn(async move {
                let peer = notifier_for_send.lock().await.clone();
                if let Some(peer) = peer {
                    if let Some(custom) = event.into_custom_notification() {
                        if let Err(e) = peer
                            .send_notification(rmcp::model::ServerNotification::CustomNotification(
                                custom,
                            ))
                            .await
                        {
                            warn!("could not push graphChanged notification: {e}");
                        }
                    }
                }
                // notifier None → silently skip (pre-serve / client disconnected).
            });

            // SUBSCRIBE (RFC-0107): per-subscription scoped fan-out. We
            // build every match payload synchronously here, then spawn one
            // task per matching subscription for the actual send.
            //
            // `on_batch` is called from within a Tokio async task (the watch
            // loop), so `blocking_read()` would block the executor thread.
            // Use `try_read()` instead — if the lock is briefly contended
            // (only possible during concurrent subscribe/unsubscribe), we
            // skip this batch's notifications rather than panicking.
            //
            // For Selector subscriptions, also recompute the fresh match
            // set so the next batch can diff against it. Persisted via a
            // spawned task to avoid blocking the loop.
            let subs_for_match = Arc::clone(&subscriptions_cb);
            let ev_clone = ev.clone();
            let delta_clone = delta.clone();
            // `try_read()` instead of `blocking_read()` — the latter blocks
            // the Tokio executor and panics if any concurrent subscribe /
            // unsubscribe holds the write lock. Briefly contended → skip
            // this batch's notifications rather than crash the watch loop.
            let subs_snapshot: Vec<subscription::Subscription> = subs_for_match
                .try_read()
                .map(|g| g.by_id.values().cloned().collect())
                .unwrap_or_default();
            // RFC-0107 + RFC-0108: collect every pending fan-out work-item
            // (file/symbol/selector delta or query delta or pause).
            let mut payloads: Vec<Fanout> = Vec::new();
            for sub in &subs_snapshot {
                match subscription::match_batch(sub, &ev_clone, &delta_clone, store_r) {
                    Some(subscription::BatchMatch::Delta(payload)) => {
                        // Recompute the canonical NEW set for Selector subs so
                        // we can persist it as `last_match_set` after delivery.
                        let new_set =
                            if let subscription::Interest::Selector { hyphae } = &sub.interest {
                                Some(subscription::evaluate_selector_set(hyphae, store_r))
                            } else {
                                None
                            };
                        payloads.push(Fanout::Delta {
                            sub_id: sub.id.clone(),
                            new_set,
                            payload,
                        });
                    }
                    Some(subscription::BatchMatch::QueryDelta(payload)) => {
                        let is_set_shaped = matches!(
                            &sub.interest,
                            subscription::Interest::Query { query, .. }
                                if subscription::query_is_set_shaped(query)
                        );
                        payloads.push(Fanout::QueryDelta {
                            sub_id: sub.id.clone(),
                            is_set_shaped,
                            payload,
                        });
                    }
                    Some(subscription::BatchMatch::PauseQuery { subscription_id }) => {
                        payloads.push(Fanout::PauseQuery {
                            sub_id: subscription_id,
                        });
                    }
                    None => {}
                }
            }
            drop(subs_snapshot);
            for f in payloads {
                let notifier_for_send = Arc::clone(&notifier_cb);
                let subs_for_bump = Arc::clone(&subscriptions_cb);
                match f {
                    Fanout::Delta {
                        sub_id,
                        new_set,
                        payload,
                    } => {
                        tokio::spawn(async move {
                            let peer = notifier_for_send.lock().await.clone();
                            if let Some(peer) = peer {
                                if let Some(custom) = payload.into_custom_notification() {
                                    if let Err(e) = peer
                                        .send_notification(
                                            rmcp::model::ServerNotification::CustomNotification(
                                                custom,
                                            ),
                                        )
                                        .await
                                    {
                                        warn!("could not push subscriptionDelta notification: {e}");
                                        return;
                                    }
                                }
                            }
                            subscription::bump_ttl(&subs_for_bump, &sub_id).await;
                            if let Some(ns) = new_set {
                                subscription::update_last_match_set(&subs_for_bump, &sub_id, ns)
                                    .await;
                            }
                        });
                    }
                    Fanout::PauseQuery { sub_id } => {
                        tokio::spawn(async move {
                            subscription::pause_query_subscription(&subs_for_bump, &sub_id).await;
                        });
                    }
                    Fanout::QueryDelta {
                        sub_id,
                        is_set_shaped,
                        payload,
                    } => {
                        // Extract pieces needed for persistence BEFORE moving
                        // the payload into the notification envelope. The
                        // hash is hex `"b3:<32-hex>"` — parse back to 16 bytes
                        // for storage.
                        let hash_hex_str = payload.result_hash_new.clone();
                        let new_set: Option<std::collections::BTreeSet<String>> = if is_set_shaped {
                            payload.new_result.as_array().map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(str::to_owned))
                                    .collect()
                            })
                        } else {
                            None
                        };
                        tokio::spawn(async move {
                            let peer = notifier_for_send.lock().await.clone();
                            if let Some(peer) = peer {
                                if let Some(custom) = payload.into_custom_notification() {
                                    if let Err(e) = peer
                                        .send_notification(
                                            rmcp::model::ServerNotification::CustomNotification(
                                                custom,
                                            ),
                                        )
                                        .await
                                    {
                                        warn!(
                                            "could not push queryResultChanged notification: {e}"
                                        );
                                        return;
                                    }
                                }
                            }
                            subscription::bump_ttl(&subs_for_bump, &sub_id).await;
                            // Parse `"b3:xx..xx"` back to 16 bytes for storage.
                            if let Some(bytes) = parse_hash_hex(&hash_hex_str) {
                                subscription::update_query_state(
                                    &subs_for_bump,
                                    &sub_id,
                                    bytes,
                                    new_set,
                                )
                                .await;
                            }
                        });
                    }
                }
            }
        };

        watch_state.watching.store(true, Ordering::Relaxed);

        let watch_state_done = Arc::clone(&watch_state);
        let handle = tokio::spawn(async move {
            let reindexer = McpReindexer;
            if let Err(e) = WatchEngine::drive(
                session,
                cfg,
                store,
                &reindexer,
                Some(cortex),
                on_batch,
                cancel,
            )
            .await
            {
                warn!("watch engine exited with error: {e}");
            }
            watch_state_done.watching.store(false, Ordering::Relaxed);
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

/// MCP-side [`FileReindexer`] — wraps the existing per-extension
/// `reindex_file` (which owns all 11 grammar/QUERIES pairs in this crate).
struct McpReindexer;

impl mycelium_core::watch::FileReindexer for McpReindexer {
    fn reindex(&self, rel: &str, src: &[u8], ext: &str, store: &mut mycelium_core::store::Store) {
        reindex_file(rel, src, ext, store);
    }
}

/// RFC-0097: verify `raw_path` is under one of the `allowed_roots` after canonicalization.
///
/// Returns the canonicalized path on success, or an error string on rejection.
/// When `allowed_roots` is empty, all paths are permitted (unit-test mode).
fn check_path_in_allowed_roots(
    raw_path: &str,
    allowed_roots: &[PathBuf],
) -> Result<PathBuf, String> {
    if allowed_roots.is_empty() {
        return Ok(PathBuf::from(raw_path));
    }
    let canonical =
        std::fs::canonicalize(raw_path).map_err(|e| format!("path not accessible: {e}"))?;
    if allowed_roots.iter().any(|root| canonical.starts_with(root)) {
        Ok(canonical)
    } else {
        Err(format!(
            "path '{}' is outside allowed roots: {:?}",
            canonical.display(),
            allowed_roots
        ))
    }
}

impl MyceliumServer {
    async fn refresh_budget(&self, node_count: usize) {
        let budget = OutputBudget::for_project(node_count);
        *self.output_budget.lock().await = budget;
    }

    async fn current_budget(&self) -> OutputBudget {
        *self.output_budget.lock().await
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
    ) -> CallToolResult {
        // RFC-0097: enforce filesystem access boundary before touching disk.
        let root = match check_path_in_allowed_roots(&req.path, &self.allowed_roots) {
            Ok(p) => p,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let root_save = root.clone();
        let result = tokio::task::spawn_blocking(move || run_index(&root)).await;
        match result {
            Err(e) => {
                application_error(&serde_json::json!({ "error": format!("task panicked: {e}") }))
            }
            Ok(Err(e)) => application_error(&serde_json::json!({ "error": e.to_string() })),
            Ok(Ok((new_store, files, errors, languages, stubs_resolved))) => {
                // RFC-0006: auto-save snapshot. Path derives from already-validated root.
                let snap = root_save.join(".mycelium").join("index.rmp");
                if let Err(e) = new_store.save(&snap) {
                    warn!("could not save index snapshot: {e}");
                }
                *self.store.write().await = new_store;
                *self.indexed_root.write().await = Some(root_save);
                let node_count = self.store.read().await.node_count();
                self.refresh_budget(node_count).await;
                success_str(
                    serde_json::json!({
                        "files": files,
                        "errors": errors,
                        "languages": languages,
                        "stubs_resolved": stubs_resolved,
                    })
                    .to_string(),
                )
            }
        }
    }

    #[tool(
        description = "Search for symbols by name prefix or substring (case-insensitive). \
                       Returns matching trunk paths. Call mycelium_index_workspace first. \
                       When compact mode is enabled (see mycelium_set_compact_mode) the \
                       response is MessagePack-encoded and returned as \
                       { \"fmt\": \"msgpack_hex\", \"data\": \"<hex>\" }, reducing AI \
                       token consumption to ≤ 30 % of the JSON equivalent."
    )]
    async fn mycelium_search_symbol(
        &self,
        Parameters(req): Parameters<SearchSymbolRequest>,
    ) -> CallToolResult {
        let limit = req.limit.unwrap_or(20);
        let matches = self.store.read().await.search_symbol(&req.query, limit);
        let mut value = serde_json::json!({ "matches": matches });
        apply_budget(&mut value, &self.current_budget().await);
        match req.output_format {
            Some(fmt) => success_str(formatter_for(fmt).format(&value)),
            None if self.compact_mode.load(Ordering::Relaxed) => {
                success_str(encode_msgpack_hex(&value))
            }
            None => success_str(value.to_string()),
        }
    }

    #[tool(
        description = "Return the ancestor chain (containment hierarchy) for a given trunk path, \
                       in child-to-root order. Returns an empty list if the path has no ancestors."
    )]
    async fn mycelium_get_ancestors(
        &self,
        Parameters(req): Parameters<GetAncestorsRequest>,
    ) -> CallToolResult {
        let ancestors = self
            .store
            .read()
            .await
            .ancestors_of_path(&req.path)
            .unwrap_or_default();
        let value = serde_json::json!({ "ancestors": ancestors });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
    }

    #[tool(
        description = "Return all symbols nested under a given trunk path (strict descendants). \
                       Returns an empty list if the path is a leaf node or is not in the index."
    )]
    async fn mycelium_get_descendants(
        &self,
        Parameters(req): Parameters<GetDescendantsRequest>,
    ) -> CallToolResult {
        let (descendants, inherited_opt) = {
            let store = self.store.read().await;
            let d = store.descendants_of_path(&req.path).unwrap_or_default();
            let i = if req.include_inherited == Some(true) {
                store
                    .inherited_descendants_of_path(&req.path)
                    .unwrap_or_default()
            } else {
                vec![]
            };
            (d, i)
        };
        let mut value = serde_json::json!({ "descendants": descendants });
        if req.include_inherited == Some(true) {
            let inherited = inherited_opt
                .into_iter()
                .map(|(path, from)| serde_json::json!({ "path": path, "from": from }))
                .collect::<Vec<_>>();
            value["inherited_descendants"] = serde_json::Value::Array(inherited);
        }
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
    }

    #[tool(
        description = "Load a previously saved index from disk without re-indexing. \
                       Reads the .mycelium/index.rmp snapshot created by mycelium_index_workspace. \
                       Returns the number of nodes loaded."
    )]
    async fn mycelium_load_index(
        &self,
        Parameters(req): Parameters<LoadIndexRequest>,
    ) -> CallToolResult {
        // RFC-0097: validate path before reading from disk.
        let root = match check_path_in_allowed_roots(&req.path, &self.allowed_roots) {
            Ok(p) => p,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let snap = root.join(".mycelium").join("index.rmp");
        match Store::load_with_journal(&snap) {
            Err(e) => application_error(&serde_json::json!({ "error": e.to_string() })),
            Ok(loaded) => {
                let nodes = loaded.node_count();
                *self.store.write().await = loaded;
                *self.indexed_root.write().await = Some(root);
                self.refresh_budget(nodes).await;
                success_str(
                    serde_json::json!({
                        "nodes": nodes,
                        "loaded_from": ".mycelium/index.rmp"
                    })
                    .to_string(),
                )
            }
        }
    }

    #[tool(
        description = "Return the current server status: indexed root directory, node count, \
                       and whether an index has been loaded. Useful for diagnostics and \
                       confirming the server is ready before issuing queries."
    )]
    async fn mycelium_server_status(&self) -> CallToolResult {
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
        success_str(
            serde_json::json!({
                "node_count": node_count,
                "edge_count": edge_count,
                "indexed_root": indexed_root,
                "is_loaded": is_loaded,
            })
            .to_string(),
        )
    }

    #[tool(
        description = "Return the current file-watch loop status: whether the watcher is active, \
                       the root being watched, and how many change batches have been processed."
    )]
    async fn mycelium_watch_status(&self) -> CallToolResult {
        let watching = self.watch_state.watching.load(Ordering::Relaxed);
        let batches_processed = self.watch_state.batches_processed.load(Ordering::Relaxed);
        let root_guard = self.indexed_root.read().await;
        let root = root_guard
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        drop(root_guard);
        success_str(
            serde_json::json!({
                "watching": watching,
                "root": root,
                "batches_processed": batches_processed,
            })
            .to_string(),
        )
    }

    #[tool(
        description = "Register a per-batch SUBSCRIBE interest (RFC-0107). Files / Symbols / \
                       Selector tagged union; subsequent watch batches emit one \
                       `mycelium/subscriptionDelta` notification per matching subscription. \
                       Defence-in-depth: rolling TTL (default 3600s, max 86400s) + caps \
                       (256 server-wide, 32 per-client, 64 Selector-specific) + peer-close GC. \
                       Frozen-at-v1 wire shape."
    )]
    async fn mycelium_subscribe(
        &self,
        Parameters(req): Parameters<subscription::SubscribeRequest>,
    ) -> CallToolResult {
        let root: PathBuf = match req.root.as_deref() {
            Some(r) => PathBuf::from(r),
            None => self
                .indexed_root
                .read()
                .await
                .clone()
                .unwrap_or_else(|| PathBuf::from(".")),
        };
        // RFC-0097: subscriptions inherit the same root-allowlist as
        // index/load. Empty allowed_roots = unrestricted (unit-test mode).
        if !self.allowed_roots.is_empty() {
            if let Err(e) =
                check_path_in_allowed_roots(&root.to_string_lossy(), &self.allowed_roots)
            {
                let err = subscription::SubscribeError::RootNotAllowed(e);
                return application_error(&serde_json::json!({
                    "code": err.code(),
                    "error": err.to_string(),
                }));
            }
        }
        let client_tag = "stdio-default".to_owned(); // single-peer stdio transport
        match subscription::subscribe(&self.subscriptions, req, client_tag, root).await {
            Ok(resp) => success_str(
                serde_json::to_value(&resp)
                    .map_or_else(|e| format!("{{\"error\":\"{e}\"}}"), |v| v.to_string()),
            ),
            Err(e) => application_error(&serde_json::json!({
                "code": e.code(),
                "error": e.to_string(),
            })),
        }
    }

    #[tool(
        description = "Idempotently remove a SUBSCRIBE subscription by id (RFC-0107). \
                       Unknown ids return `{removed: false}` rather than an error, so \
                       agents can run cleanup blindly on reconnect."
    )]
    async fn mycelium_unsubscribe(
        &self,
        Parameters(req): Parameters<subscription::UnsubscribeRequest>,
    ) -> CallToolResult {
        let resp = subscription::unsubscribe(&self.subscriptions, &req.subscription_id).await;
        success_str(
            serde_json::to_value(&resp)
                .map_or_else(|e| format!("{{\"error\":\"{e}\"}}"), |v| v.to_string()),
        )
    }

    #[tool(
        description = "Inspect SUBSCRIBE subscriptions (RFC-0107). When `subscription_id` is \
                       supplied returns at most one row; otherwise returns every active \
                       subscription plus the configured caps and the watch loop's \
                       `watching` flag."
    )]
    async fn mycelium_subscription_status(
        &self,
        Parameters(req): Parameters<subscription::SubscriptionStatusRequest>,
    ) -> CallToolResult {
        let watching = self.watch_state.watching.load(Ordering::Relaxed);
        let resp = subscription::status(
            &self.subscriptions,
            req.subscription_id.as_deref(),
            watching,
        )
        .await;
        success_str(
            serde_json::to_value(&resp)
                .map_or_else(|e| format!("{{\"error\":\"{e}\"}}"), |v| v.to_string()),
        )
    }

    #[tool(
        description = "Return all symbols (callee paths) that a given symbol calls directly. \
                       Uses the Calls edges populated during indexing. Returns a sorted list \
                       of trunk paths."
    )]
    async fn mycelium_get_callees(
        &self,
        Parameters(req): Parameters<GetCalleesRequest>,
    ) -> CallToolResult {
        let kind = match parse_edge_kind(req.edge_kind.as_deref().unwrap_or("calls")) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let budget_override = match req.budget.as_deref() {
            None => None,
            Some(s) => match s.parse::<BudgetOverride>() {
                Ok(o) => Some(o),
                Err(e) => return application_error(&serde_json::json!({ "error": e })),
            },
        };
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return not_found(&req.path);
        };
        // Shared core builder → byte-identical with the CLI twin (RFC-0109).
        let mut value = mycelium_core::queries::callees_payload(&store_guard, id, kind);
        let budget = OutputBudget::resolve(budget_override, store_guard.node_count());
        drop(store_guard);
        apply_budget(&mut value, &budget);
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
    }

    #[tool(
        description = "Return all symbols (caller paths) that call a given symbol directly, \
                       and optionally via virtual dispatch. Direct callers use reverse Calls edges. \
                       When include_virtual is true, also includes callers that call an ancestor \
                       (base class) method of the same name — surfacing virtual dispatch call sites \
                       that reference the abstract base rather than the concrete override. \
                       Returns a sorted, deduplicated list of trunk paths."
    )]
    async fn mycelium_get_callers(
        &self,
        Parameters(req): Parameters<GetCallersRequest>,
    ) -> CallToolResult {
        let kind = match parse_edge_kind(req.edge_kind.as_deref().unwrap_or("calls")) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let budget_override = match req.budget.as_deref() {
            None => None,
            Some(s) => match s.parse::<BudgetOverride>() {
                Ok(o) => Some(o),
                Err(e) => return application_error(&serde_json::json!({ "error": e })),
            },
        };
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            return application_error(
                &serde_json::json!({ "error": format!("path not found: {}", req.path) }),
            );
        };
        // Shared core builder → byte-identical with the CLI twin (RFC-0109).
        let mut value = mycelium_core::queries::callers_payload(
            &store_guard,
            id,
            &req.path,
            kind,
            req.include_virtual == Some(true),
        );
        let budget = OutputBudget::resolve(budget_override, store_guard.node_count());
        drop(store_guard);
        apply_budget(&mut value, &budget);
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return not_found(&req.path);
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

        let value = serde_json::json!({
            "path": req.path,
            "ancestors": ancestors,
            "descendants": descendants,
            "callers": callers,
            "callees": callees,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
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
        let value = serde_json::json!({ "symbols": symbols });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
    }

    #[tool(
        description = "Return the transitive callee tree rooted at a given symbol, up to \
                       max_depth hops. Each node contains its path and a list of callee subtrees. \
                       Cycles are represented as leaf nodes. max_depth defaults to 4, capped at 10."
    )]
    async fn mycelium_get_callee_tree(
        &self,
        Parameters(req): Parameters<GetCalleeTreeRequest>,
    ) -> CallToolResult {
        let max_depth = req.max_depth.unwrap_or(4).min(10);
        let store_guard = self.store.read().await;
        let Some(root_id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return not_found(&req.path);
        };
        let tree = store_guard.callee_tree(root_id, max_depth);
        let json_tree = callee_node_to_json(&tree, &store_guard);
        drop(store_guard);
        let value = serde_json::json!({ "root": json_tree });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let max_depth = req.max_depth.unwrap_or(4).min(10);
        let store_guard = self.store.read().await;
        let Some(root_id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return not_found(&req.path);
        };
        let tree = store_guard.caller_tree(root_id, max_depth);
        let json_tree = caller_node_to_json(&tree, &store_guard);
        drop(store_guard);
        let value = serde_json::json!({ "root": json_tree });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
    }

    #[tool(description = "Return the direct import neighbors for a trunk path: \
                       'imports' (outgoing Imports edges — what this node imports) and \
                       'imported_by' (incoming Imports edges — what imports this node). \
                       Both lists sorted lexicographically. Unknown path returns { error }.")]
    async fn mycelium_get_imports(
        &self,
        Parameters(req): Parameters<GetImportsRequest>,
    ) -> CallToolResult {
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return not_found(&req.path);
        };
        let imports = store_guard.imports_of(id);
        let imported_by = store_guard.imported_by(id);
        drop(store_guard);
        let value = serde_json::json!({ "imports": imports, "imported_by": imported_by });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let max_depth = req.max_depth.unwrap_or(4).min(10);
        let store_guard = self.store.read().await;
        let Some(root_id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return not_found(&req.path);
        };
        let tree = store_guard.import_tree(root_id, max_depth);
        let json_tree = import_node_to_json(&tree, &store_guard);
        drop(store_guard);
        let value = serde_json::json!({ "root": json_tree });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return not_found(&req.path);
        };
        let kind_str: serde_json::Value = store_guard
            .kind_of(id)
            .map_or(serde_json::Value::Null, |k| {
                serde_json::Value::String(k.as_str().to_owned())
            });
        drop(store_guard);
        let value = serde_json::json!({ "path": req.path, "kind": kind_str });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let Some(kind) = mycelium_core::types::NodeKind::try_from_wire(&req.kind) else {
            return application_error(
                &serde_json::json!({ "error": format!("unknown kind: {}", req.kind) }),
            );
        };
        let symbols = self
            .store
            .read()
            .await
            .symbols_of_kind(kind, req.path_prefix.as_deref());
        let value = serde_json::json!({ "symbols": symbols });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return not_found(&req.path);
        };
        let fmt = req.output_format;
        if let Some(span) = store_guard.span_of(id) {
            drop(store_guard);
            let value = serde_json::json!({
                "path": req.path,
                "start_line": span.start_line,
                "start_col": span.start_col,
                "end_line": span.end_line,
                "end_col": span.end_col,
                "start_byte": span.start_byte,
                "end_byte": span.end_byte,
            });
            success_str(fmt.map_or_else(|| value.to_string(), |f| formatter_for(f).format(&value)))
        } else {
            drop(store_guard);
            let value = serde_json::json!({ "path": req.path, "span": serde_json::Value::Null });
            success_str(fmt.map_or_else(|| value.to_string(), |f| formatter_for(f).format(&value)))
        }
    }

    #[tool(
        description = "Return the direct inheritance relationships for a path. extends lists \
                       symbols this path directly extends (outgoing Extends edges). extended_by \
                       lists symbols that extend this path (incoming Extends edges). Both lists \
                       are sorted lexicographically. Unknown path returns { error }."
    )]
    async fn mycelium_get_extends(
        &self,
        Parameters(req): Parameters<GetExtendsRequest>,
    ) -> CallToolResult {
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return not_found(&req.path);
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
        let value = serde_json::json!({ "extends": extends, "extended_by": extended_by });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return not_found(&req.path);
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
        let value =
            serde_json::json!({ "implements": implements, "implemented_by": implemented_by });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let eps = self
            .store
            .read()
            .await
            .entry_points(req.path_prefix.as_deref());
        let value = serde_json::json!({ "entry_points": eps });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let budget_override = match req.budget.as_deref() {
            None => None,
            Some(s) => match s.parse::<BudgetOverride>() {
                Ok(o) => Some(o),
                Err(e) => return application_error(&serde_json::json!({ "error": e })),
            },
        };
        let store = self.store.read().await;
        let dead = match req.edge_kind.as_deref() {
            None => store.dead_symbols(req.path_prefix.as_deref()),
            Some(ek) => match parse_edge_kind(ek) {
                Ok(kind) => store.dead_symbols_for_kind(kind, req.path_prefix.as_deref()),
                Err(e) => return application_error(&serde_json::json!({ "error": e })),
            },
        };
        let node_count = store.node_count();
        drop(store);
        // Shared core builder → byte-identical with the CLI twin (RFC-0109).
        let mut value = mycelium_core::queries::dead_symbols_payload(&dead);
        apply_budget(
            &mut value,
            &OutputBudget::resolve(budget_override, node_count),
        );
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let budget_override = match req.budget.as_deref() {
            None => None,
            Some(s) => match s.parse::<BudgetOverride>() {
                Ok(o) => Some(o),
                Err(e) => return application_error(&serde_json::json!({ "error": e })),
            },
        };
        let store = self.store.read().await;
        let isolated = store.isolated_symbols(req.path_prefix.as_deref());
        let node_count = store.node_count();
        drop(store);
        // Shared core builder → byte-identical with the CLI twin (RFC-0109).
        let mut value = mycelium_core::queries::isolated_symbols_payload(&isolated);
        apply_budget(
            &mut value,
            &OutputBudget::resolve(budget_override, node_count),
        );
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
    }

    #[tool(
        description = "Return comprehensive per-kind statistics about the indexed symbol graph: \
                       total node and edge counts plus breakdowns by NodeKind (file, function, \
                       class, …) and EdgeKind (calls, imports, extends, …). \
                       Returns { total_nodes, total_edges, nodes_by_kind, edges_by_kind }. \
                       Kinds with zero count are omitted."
    )]
    async fn mycelium_get_stats(
        &self,
        Parameters(req): Parameters<GetStatsRequest>,
    ) -> CallToolResult {
        let stats: GraphStats = self.store.read().await.graph_stats();
        let value = serde_json::json!({
            "total_nodes": stats.total_nodes,
            "total_edges": stats.total_edges,
            "nodes_by_kind": stats.nodes_by_kind,
            "edges_by_kind": stats.edges_by_kind,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let refs_opt: Option<CrossRefs> = {
            let store = self.store.read().await;
            store.lookup(&req.path).map(|id| store.cross_refs(id))
        };
        let Some(refs) = refs_opt else {
            return not_found(&req.path);
        };
        let value = serde_json::json!({
            "callers": refs.callers,
            "importers": refs.importers,
            "extended_by": refs.extended_by,
            "implemented_by": refs.implemented_by,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let refs_opt: Option<OutgoingRefs> = {
            let store = self.store.read().await;
            store.lookup(&req.path).map(|id| store.outgoing_refs(id))
        };
        let Some(refs) = refs_opt else {
            return not_found(&req.path);
        };
        let value = serde_json::json!({
            "callees": refs.callees,
            "imports": refs.imports,
            "extends": refs.extends,
            "implements": refs.implements,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        if let Some(ref k) = req.kind {
            if mycelium_core::types::NodeKind::try_from_wire(k).is_none() {
                return application_error(
                    &serde_json::json!({ "error": format!("unknown kind: {k}") }),
                );
            }
        }
        let kind = req
            .kind
            .as_deref()
            .and_then(mycelium_core::types::NodeKind::try_from_wire);
        let all_symbols = self
            .store
            .read()
            .await
            .all_symbols(req.path_prefix.as_deref(), kind);
        let total_count = all_symbols.len();
        let offset = req.offset.unwrap_or(0);
        let limit = req.limit.unwrap_or(0);
        let page: Vec<String> = all_symbols
            .into_iter()
            .skip(offset)
            .take(if limit == 0 { usize::MAX } else { limit })
            .collect();
        let count = page.len();
        let mut value = serde_json::json!({
            "symbols": page,
            "count": count,
            "total_count": total_count,
        });
        apply_budget(&mut value, &self.current_budget().await);
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let max_depth = req.max_depth.unwrap_or(10);
        let reachable_opt: Option<Vec<String>> = {
            let store = self.store.read().await;
            store
                .lookup(&req.path)
                .map(|id| store.reachable_from(id, kind, max_depth))
        };
        let Some(reachable) = reachable_opt else {
            return not_found(&req.path);
        };
        let count = reachable.len();
        let mut value = serde_json::json!({ "reachable": reachable, "count": count });
        apply_budget(&mut value, &self.current_budget().await);
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let max_depth = req.max_depth.unwrap_or(10);
        let reachable_opt: Option<Vec<String>> = {
            let store = self.store.read().await;
            store
                .lookup(&req.path)
                .map(|id| store.reachable_to(id, kind, max_depth))
        };
        let Some(reachable) = reachable_opt else {
            return not_found(&req.path);
        };
        let count = reachable.len();
        let mut value = serde_json::json!({ "reachable": reachable, "count": count });
        apply_budget(&mut value, &self.current_budget().await);
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let siblings_opt: Option<Vec<String>> = {
            let store = self.store.read().await;
            store.lookup(&req.path).map(|id| store.siblings(id))
        };
        let Some(siblings) = siblings_opt else {
            return not_found(&req.path);
        };
        let count = siblings.len();
        let value = serde_json::json!({ "siblings": siblings, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
    }

    #[tool(
        description = "Execute a Hyphae DSL selector against the project's index. \
                       Hyphae is a CSS-selector-inspired query language (RFC-0003) that lets agents \
                       fetch a set of matching symbols in one call instead of multiple JSON tool round-trips. \
                       Returns { matches: [...], count: N } on success, { error: \"...\" } on parse failure. \
                       Twin of the CLI `mycelium query <expr>` subcommand — same selector grammar, \
                       same match-set shape (RFC-0090 Three-Surface Rule)."
    )]
    async fn mycelium_query(&self, Parameters(req): Parameters<QueryRequest>) -> CallToolResult {
        let ast = match mycelium_hyphae::parse(&req.expr) {
            Ok(ast) => ast,
            Err(e) => {
                return application_error(&serde_json::json!({
                    "error": format!("hyphae parse error: {e:?}")
                }));
            }
        };
        let store = self.store.read().await;
        let evaluator = mycelium_hyphae::evaluator::Evaluator::new(&store);
        let matches = evaluator.eval(&ast);
        drop(store);
        let count = matches.len();
        let value = serde_json::json!({ "matches": matches, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let degree_opt: Option<NodeDegree> = {
            let store = self.store.read().await;
            store.lookup(&req.path).map(|id| store.node_degree(id))
        };
        let Some(deg) = degree_opt else {
            return not_found(&req.path);
        };
        let value = serde_json::json!({
            "in_calls": deg.in_calls,
            "out_calls": deg.out_calls,
            "in_imports": deg.in_imports,
            "out_imports": deg.out_imports,
            "in_extends": deg.in_extends,
            "out_extends": deg.out_extends,
            "in_implements": deg.in_implements,
            "out_implements": deg.out_implements,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let nodes = self
            .store
            .read()
            .await
            .nodes_in_cycles(kind, req.path_prefix.as_deref());
        let count = nodes.len();
        let value = serde_json::json!({ "cycle_nodes": nodes, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let groups = self.store.read().await.scc_groups(kind);
        let group_count = groups.len();
        let total_symbols: usize = groups.iter().map(Vec::len).sum();
        let value = serde_json::json!({
            "groups": groups,
            "group_count": group_count,
            "total_symbols": total_symbols,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
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
        let value = serde_json::json!({
            "layers": layers,
            "layer_count": layer_count,
            "total_symbols": total_symbols,
            "cycle_excluded_count": cycle_excluded_count,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let store = self.store.read().await;
        let Some(id) = store.lookup(&req.path) else {
            drop(store);
            return success_str(serde_json::json!({ "neighbors": [], "count": 0 }).to_string());
        };
        let neighbors = store.two_hop_neighbors(id, kind);
        drop(store);
        let count = neighbors.len();
        let value = serde_json::json!({ "neighbors": neighbors, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
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
        let value = serde_json::json!({
            "path": nb.path,
            "incoming": nb.incoming,
            "outgoing": nb.outgoing,
            "incoming_count": incoming_count,
            "outgoing_count": outgoing_count,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
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
        let value = serde_json::json!({ "hubs": hubs_json, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
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
        let value = serde_json::json!({ "symbols": symbols, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
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
        let value = serde_json::json!({ "reachable": reachable, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
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
        let value = serde_json::json!({ "reachable": reachable, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
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
        let value = serde_json::json!({ "degrees": degrees, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
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
        let value = serde_json::json!({
            "order": order,
            "cycle_members": cycle_members,
            "ordered_count": ordered_count,
            "cycle_count": cycle_count,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let points = {
            let store = self.store.read().await;
            let p = store.articulation_points(kind);
            drop(store);
            p
        };
        let count = points.len();
        let value = serde_json::json!({ "points": points, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
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
        let value = serde_json::json!({ "bridges": bridge_list, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let comps = {
            let store = self.store.read().await;
            let c = store.biconnected_components(kind);
            drop(store);
            c
        };
        let component_count = comps.len();
        let total_symbols: usize = comps.iter().map(Vec::len).sum();
        let value = serde_json::json!({
            "components": comps,
            "component_count": component_count,
            "total_symbols": total_symbols
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
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
        let value = serde_json::json!({
            "in_degrees": in_list,
            "out_degrees": out_list,
            "total_symbols": total_symbols
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let m = {
            let store = self.store.read().await;
            let metrics = store.graph_metrics(kind);
            drop(store);
            metrics
        };
        let value = serde_json::json!({
            "symbol_count": m.symbol_count,
            "directed_edge_count": m.directed_edge_count,
            "density": m.density,
            "avg_degree": m.avg_degree,
            "max_in_degree": m.max_in_degree,
            "max_out_degree": m.max_out_degree,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let store = self.store.read().await;
        let Some(id1) = store.lookup(&req.path1) else {
            return application_error(
                &serde_json::json!({ "error": format!("unknown path: {}", req.path1) }),
            );
        };
        let Some(id2) = store.lookup(&req.path2) else {
            return application_error(
                &serde_json::json!({ "error": format!("unknown path: {}", req.path2) }),
            );
        };
        let (similarity, shared, total) = store.neighbor_similarity_stats(id1, id2, kind);
        drop(store);
        let value = serde_json::json!({
            "similarity": similarity,
            "shared": shared,
            "total": total,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let store = self.store.read().await;
        let Some(id) = store.lookup(&req.path) else {
            return application_error(
                &serde_json::json!({ "error": format!("unknown path: {}", req.path) }),
            );
        };
        let (coefficient, neighbor_count, neighbor_edge_count) =
            store.clustering_coefficient_stats(id, kind);
        drop(store);
        let value = serde_json::json!({
            "coefficient": coefficient,
            "neighbor_count": neighbor_count,
            "neighbor_edge_count": neighbor_edge_count,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let store = self.store.read().await;
        let Some(id) = store.lookup(&req.path) else {
            return application_error(
                &serde_json::json!({ "error": format!("unknown path: {}", req.path) }),
            );
        };
        let (eccentricity, reachable_count) = store.eccentricity_stats(id, kind);
        drop(store);
        let value = serde_json::json!({
            "eccentricity": eccentricity,
            "reachable_count": reachable_count,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let store = self.store.read().await;
        let Some(id) = store.lookup(&req.path) else {
            return application_error(
                &serde_json::json!({ "error": format!("unknown path: {}", req.path) }),
            );
        };
        let (harmonic_centrality, reachable_count, symbol_count) =
            store.harmonic_centrality_stats(id, kind);
        drop(store);
        let value = serde_json::json!({
            "harmonic_centrality": harmonic_centrality,
            "reachable_count": reachable_count,
            "symbol_count": symbol_count,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let store = self.store.read().await;
        let Some(id1) = store.lookup(&req.path1) else {
            return application_error(
                &serde_json::json!({ "error": format!("unknown path: {}", req.path1) }),
            );
        };
        let Some(id2) = store.lookup(&req.path2) else {
            return application_error(
                &serde_json::json!({ "error": format!("unknown path: {}", req.path2) }),
            );
        };
        let result = store.mutual_reachability(id1, id2, kind);
        drop(store);
        let value = serde_json::json!({
            "forward": result.forward,
            "backward": result.backward,
            "mutual": result.mutual,
            "forward_distance": result.forward_distance,
            "backward_distance": result.backward_distance,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let store = self.store.read().await;
        let Some(id) = store.lookup(&req.path) else {
            return application_error(
                &serde_json::json!({ "error": format!("unknown path: {}", req.path) }),
            );
        };
        let reachable = store.reachable_set(id, kind);
        drop(store);
        let count = reachable.len();
        let value = serde_json::json!({
            "reachable": reachable,
            "count": count,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let store = self.store.read().await;
        let Some(id) = store.lookup(&req.path) else {
            return application_error(
                &serde_json::json!({ "error": format!("unknown path: {}", req.path) }),
            );
        };
        let callers = store.reaches_into(id, kind);
        drop(store);
        let count = callers.len();
        let value = serde_json::json!({
            "callers": callers,
            "count": count,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let store = self.store.read().await;
        let Some(id1) = store.lookup(&req.path1) else {
            return application_error(
                &serde_json::json!({ "error": format!("unknown path: {}", req.path1) }),
            );
        };
        let Some(id2) = store.lookup(&req.path2) else {
            return application_error(
                &serde_json::json!({ "error": format!("unknown path: {}", req.path2) }),
            );
        };
        let common = store.common_reachable(id1, id2, kind);
        drop(store);
        let count = common.len();
        let value = serde_json::json!({
            "common": common,
            "count": count,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let store = self.store.read().await;
        let Some(id) = store.lookup(&req.path) else {
            return application_error(
                &serde_json::json!({ "error": format!("unknown path: {}", req.path) }),
            );
        };
        let neighbors = store.k_hop_neighbors(id, kind, req.k);
        drop(store);
        let count = neighbors.len();
        let value = serde_json::json!({
            "neighbors": neighbors,
            "count": count,
            "k": req.k,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
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
        let value = serde_json::json!({
            "nodes": nodes,
            "symbol_count": symbol_count,
            "top_n": top_n,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
    }

    #[tool(
        description = "Compute PageRank scores for all symbol nodes. Identifies globally important \
                       symbols: entry points, heavily-called utilities, and hub nodes. Uses the \
                       iterative power method with configurable damping (default 0.85) and \
                       iterations (default 20). Returns top_n results (default 10) sorted \
                       descending by score. File nodes excluded. \
                       Returns { nodes: [{path, score}], symbol_count, top_n } or { error }."
    )]
    async fn mycelium_page_rank(
        &self,
        Parameters(req): Parameters<PageRankRequest>,
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
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
        let value = serde_json::json!({
            "nodes": nodes,
            "symbol_count": symbol_count,
            "top_n": top_n,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
    }

    #[tool(
        description = "Group symbol nodes into weakly-connected components (WCCs) for a given \
                       EdgeKind, treating edges as undirected. WCCs surface isolated clusters: \
                       self-contained subsystems, orphaned utilities, or modules with no \
                       dependency links to the rest of the codebase. Complements SCC (directed \
                       mutual reachability). Use min_size=2 to hide singleton isolated nodes. \
                       Returns { components, component_count, total_symbols } or { error }."
    )]
    async fn mycelium_get_wcc(&self, Parameters(req): Parameters<GetWccRequest>) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
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
        let value = serde_json::json!({
            "components": components,
            "component_count": component_count,
            "total_symbols": total_symbols,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let members = {
            let store = self.store.read().await;
            let m = store.cycle_members(kind);
            drop(store);
            m
        };
        let count = members.len();
        let value = serde_json::json!({ "members": members, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
    }

    #[tool(
        description = "Return the k-core of the symbol graph for a given EdgeKind — the maximal \
                       induced subgraph where every node has total degree (in + out within the \
                       subgraph) ≥ k. Identifies the tightly-interconnected core that is hardest \
                       to refactor. k defaults to 2. k=0 returns all symbols. Returns { core, \
                       count, k } or { error } for unknown edge_kind."
    )]
    async fn mycelium_get_k_core(
        &self,
        Parameters(req): Parameters<GetKCoreRequest>,
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let k = req.k.unwrap_or(2);
        let core = {
            let store = self.store.read().await;
            let result = store.k_core(kind, k);
            drop(store);
            result
        };
        let count = core.len();
        let value = serde_json::json!({ "core": core, "count": count, "k": k });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(req.edge_kind.as_deref().unwrap_or("calls")) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let limit = req.limit.unwrap_or(10).min(100);
        let store = self.store.read().await;
        let ranked = if kind == mycelium_core::types::EdgeKind::Calls {
            store.top_callee_symbols(limit)
        } else {
            store.top_symbols_by_incoming(kind, limit)
        };
        drop(store);
        let symbols: Vec<serde_json::Value> = ranked
            .into_iter()
            .map(|(path, count)| serde_json::json!({ "path": path, "caller_count": count }))
            .collect();
        let value = serde_json::json!({ "symbols": symbols });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
    }

    #[tool(
        description = "Return top-N source files ranked by direct symbol count (god-file detector). \
                       limit defaults to 10, capped at 100. Files with no direct symbols are excluded. \
                       Returns { files: [{ path, symbol_count }, ...], count: N }."
    )]
    async fn mycelium_get_top_files(
        &self,
        Parameters(req): Parameters<GetTopFilesRequest>,
    ) -> CallToolResult {
        let limit = req.limit.unwrap_or(10);
        let entries = self.store.read().await.top_files(limit);
        let count = entries.len();
        let files: Vec<serde_json::Value> = entries
            .into_iter()
            .map(|(path, symbol_count)| {
                serde_json::json!({ "path": path, "symbol_count": symbol_count })
            })
            .collect();
        let value = serde_json::json!({ "files": files, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let limit = req.limit.unwrap_or(10);
        let entries = self.store.read().await.most_connected(limit, kind);
        let count = entries.len();
        let symbols: Vec<serde_json::Value> = entries
            .into_iter()
            .map(|(path, degree)| serde_json::json!({ "path": path, "degree": degree }))
            .collect();
        let value = serde_json::json!({ "symbols": symbols, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let limit = req.limit.unwrap_or(10);
        let symbols = self.store.read().await.leaf_symbols(kind, limit);
        let count = symbols.len();
        let value = serde_json::json!({ "symbols": symbols, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        // Two lookups then shortest_path — hold read guard for the whole block.
        #[allow(clippy::significant_drop_tightening)]
        let path_opt: Result<Option<Vec<String>>, String> = {
            let store = self.store.read().await;
            let Some(from_id) = store.lookup(&req.from) else {
                return application_error(
                    &serde_json::json!({ "error": format!("path not found: {}", req.from) }),
                );
            };
            let Some(to_id) = store.lookup(&req.to) else {
                return application_error(
                    &serde_json::json!({ "error": format!("path not found: {}", req.to) }),
                );
            };
            Ok(store.shortest_path(from_id, to_id, kind))
        };
        let fmt = req.output_format;
        path_opt.unwrap().map_or_else(
            || {
                let value = serde_json::json!({ "path": null, "length": null });
                success_str(
                    fmt.map_or_else(|| value.to_string(), |f| formatter_for(f).format(&value)),
                )
            },
            |p| {
                let length = p.len() - 1;
                let value = serde_json::json!({ "path": p, "length": length });
                success_str(
                    fmt.map_or_else(|| value.to_string(), |f| formatter_for(f).format(&value)),
                )
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
        Parameters(req): Parameters<GetSymbolCountByKindRequest>,
    ) -> CallToolResult {
        let counts = self.store.read().await.symbol_count_by_kind();
        let total: usize = counts.iter().map(|(_, n)| n).sum();
        let kinds: Vec<serde_json::Value> = counts
            .into_iter()
            .map(|(kind, count)| serde_json::json!({ "kind": kind, "count": count }))
            .collect();
        let value = serde_json::json!({ "kinds": kinds, "total": total });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        if req.paths.is_empty() {
            return success_str(serde_json::json!({ "callers": [], "count": 0 }).to_string());
        }
        let callers_opt: Result<Vec<String>, String> = {
            let store = self.store.read().await;
            let mut ids = Vec::with_capacity(req.paths.len());
            for p in &req.paths {
                let Some(id) = store.lookup(p) else {
                    return application_error(
                        &serde_json::json!({ "error": format!("path not found: {p}") }),
                    );
                };
                ids.push(id);
            }
            Ok(store.common_callers(&ids, kind))
        };
        let callers = callers_opt.unwrap();
        let count = callers.len();
        let value = serde_json::json!({ "callers": callers, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        if req.paths.is_empty() {
            return success_str(serde_json::json!({ "callees": [], "count": 0 }).to_string());
        }
        let callees_opt: Result<Vec<String>, String> = {
            let store = self.store.read().await;
            let mut ids = Vec::with_capacity(req.paths.len());
            for p in &req.paths {
                let Some(id) = store.lookup(p) else {
                    return application_error(
                        &serde_json::json!({ "error": format!("path not found: {p}") }),
                    );
                };
                ids.push(id);
            }
            Ok(store.common_callees(&ids, kind))
        };
        let callees = callees_opt.unwrap();
        let count = callees.len();
        let value = serde_json::json!({ "callees": callees, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let limit = req.limit.unwrap_or(10);
        let entries = self.store.read().await.fan_out_rank(kind, limit);
        let count = entries.len();
        let symbols: Vec<serde_json::Value> = entries
            .into_iter()
            .map(|(path, out_degree)| serde_json::json!({ "path": path, "out_degree": out_degree }))
            .collect();
        let value = serde_json::json!({ "symbols": symbols, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let limit = req.limit.unwrap_or(10);
        let entries = self.store.read().await.fan_in_rank(kind, limit);
        let count = entries.len();
        let symbols: Vec<serde_json::Value> = entries
            .into_iter()
            .map(|(path, in_degree)| serde_json::json!({ "path": path, "in_degree": in_degree }))
            .collect();
        let value = serde_json::json!({ "symbols": symbols, "count": count });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
    }

    #[tool(
        description = "Return all source files currently in the index as a sorted list of trunk \
                       paths. An optional path_prefix filters results to files whose path starts \
                       with the given string (e.g. \"src/\")."
    )]
    async fn mycelium_get_files(
        &self,
        Parameters(req): Parameters<GetFilesRequest>,
    ) -> CallToolResult {
        let files = self.store.read().await.all_file_paths();
        let files: Vec<String> = match req.path_prefix {
            None => files,
            Some(ref prefix) => files
                .into_iter()
                .filter(|p| p.starts_with(prefix.as_str()))
                .collect(),
        };
        let value = serde_json::json!({ "files": files });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let max_depth = req.max_depth.unwrap_or(10).min(20);
        let store_guard = self.store.read().await;
        let Some(from_id) = store_guard.lookup(&req.from_path) else {
            drop(store_guard);
            return application_error(
                &serde_json::json!({ "error": format!("path not found: {}", req.from_path) }),
            );
        };
        let Some(to_id) = store_guard.lookup(&req.to_path) else {
            drop(store_guard);
            return application_error(
                &serde_json::json!({ "error": format!("path not found: {}", req.to_path) }),
            );
        };
        let maybe_path = store_guard.find_call_path(from_id, to_id, max_depth);
        let path_strings: Option<Vec<String>> = maybe_path.as_ref().map(|ids| {
            ids.iter()
                .filter_map(|&id| store_guard.path_of(id).map(str::to_owned))
                .collect()
        });
        let fmt = req.output_format;
        drop(store_guard);
        path_strings.map_or_else(
            || {
                let value = serde_json::json!({
                    "path": [],
                    "hops": serde_json::Value::Null,
                    "message": format!("no call path found within depth {max_depth}"),
                });
                success_str(
                    fmt.map_or_else(|| value.to_string(), |f| formatter_for(f).format(&value)),
                )
            },
            |path| {
                let hops = path.len().saturating_sub(1);
                let value = serde_json::json!({ "path": path, "hops": hops });
                success_str(
                    fmt.map_or_else(|| value.to_string(), |f| formatter_for(f).format(&value)),
                )
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
    ) -> CallToolResult {
        let max_depth = req.max_depth.unwrap_or(8).min(20);
        let store_guard = self.store.read().await;
        let Some(from_id) = store_guard.lookup(&req.from_path) else {
            drop(store_guard);
            return application_error(
                &serde_json::json!({ "error": format!("path not found: {}", req.from_path) }),
            );
        };
        let Some(to_id) = store_guard.lookup(&req.to_path) else {
            drop(store_guard);
            return application_error(
                &serde_json::json!({ "error": format!("path not found: {}", req.to_path) }),
            );
        };
        let maybe_path = store_guard.find_import_path(from_id, to_id, max_depth);
        let path_strings: Option<Vec<String>> = maybe_path.as_ref().map(|ids| {
            ids.iter()
                .filter_map(|&id| store_guard.path_of(id).map(str::to_owned))
                .collect()
        });
        let fmt = req.output_format;
        drop(store_guard);
        path_strings.map_or_else(
            || {
                let value = serde_json::json!({
                    "path": [],
                    "hops": serde_json::Value::Null,
                    "message": format!("no import path found within max_depth={max_depth}"),
                });
                success_str(
                    fmt.map_or_else(|| value.to_string(), |f| formatter_for(f).format(&value)),
                )
            },
            |path| {
                let hops = path.len().saturating_sub(1);
                let value = serde_json::json!({ "path": path, "hops": hops });
                success_str(
                    fmt.map_or_else(|| value.to_string(), |f| formatter_for(f).format(&value)),
                )
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
    ) -> CallToolResult {
        let max_depth = req.max_depth.unwrap_or(8).min(20);
        let store_guard = self.store.read().await;
        let Some(from_id) = store_guard.lookup(&req.from_path) else {
            drop(store_guard);
            return application_error(
                &serde_json::json!({ "error": format!("path not found: {}", req.from_path) }),
            );
        };
        let Some(to_id) = store_guard.lookup(&req.to_path) else {
            drop(store_guard);
            return application_error(
                &serde_json::json!({ "error": format!("path not found: {}", req.to_path) }),
            );
        };
        let maybe_path = store_guard.find_extends_path(from_id, to_id, max_depth);
        let path_strings: Option<Vec<String>> = maybe_path.as_ref().map(|ids| {
            ids.iter()
                .filter_map(|&id| store_guard.path_of(id).map(str::to_owned))
                .collect()
        });
        let fmt = req.output_format;
        drop(store_guard);
        path_strings.map_or_else(
            || {
                let value = serde_json::json!({
                    "path": [],
                    "hops": serde_json::Value::Null,
                    "message": format!("no extends path found within max_depth={max_depth}"),
                });
                success_str(
                    fmt.map_or_else(|| value.to_string(), |f| formatter_for(f).format(&value)),
                )
            },
            |path| {
                let hops = path.len().saturating_sub(1);
                let value = serde_json::json!({ "path": path, "hops": hops });
                success_str(
                    fmt.map_or_else(|| value.to_string(), |f| formatter_for(f).format(&value)),
                )
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
    ) -> CallToolResult {
        let max_depth = req.max_depth.unwrap_or(4).min(10);
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return not_found(&req.path);
        };
        let tree = store_guard.extends_tree(id, max_depth);
        let json = extends_node_to_json(&tree, &store_guard);
        drop(store_guard);
        let value = serde_json::json!({ "root": json });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let max_depth = req.max_depth.unwrap_or(4).min(10);
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return not_found(&req.path);
        };
        let tree = store_guard.subclasses_tree(id, max_depth);
        let json = subclass_node_to_json(&tree, &store_guard);
        drop(store_guard);
        let value = serde_json::json!({ "root": json });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let max_depth = req.max_depth.unwrap_or(8).min(20);
        let store_guard = self.store.read().await;
        let Some(from_id) = store_guard.lookup(&req.from_path) else {
            drop(store_guard);
            return application_error(
                &serde_json::json!({ "error": format!("path not found: {}", req.from_path) }),
            );
        };
        let Some(to_id) = store_guard.lookup(&req.to_path) else {
            drop(store_guard);
            return application_error(
                &serde_json::json!({ "error": format!("path not found: {}", req.to_path) }),
            );
        };
        let fmt = req.output_format;
        if let Some(ids) = store_guard.find_implements_path(from_id, to_id, max_depth) {
            let path: Vec<String> = ids
                .iter()
                .map(|&id| store_guard.path_of(id).unwrap_or("<unknown>").to_owned())
                .collect();
            let hops = path.len() - 1;
            drop(store_guard);
            let value = serde_json::json!({ "path": path, "hops": hops });
            success_str(fmt.map_or_else(|| value.to_string(), |f| formatter_for(f).format(&value)))
        } else {
            drop(store_guard);
            let value = serde_json::json!({
                "path": [],
                "hops": null,
                "message": format!("no implements path found within max_depth={max_depth}")
            });
            success_str(fmt.map_or_else(|| value.to_string(), |f| formatter_for(f).format(&value)))
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
    ) -> CallToolResult {
        let max_depth = req.max_depth.unwrap_or(4).min(10);
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return not_found(&req.path);
        };
        let tree = store_guard.implements_tree(id, max_depth);
        let json = implements_node_to_json(&tree, &store_guard);
        drop(store_guard);
        let value = serde_json::json!({ "root": json });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let max_depth = req.max_depth.unwrap_or(4).min(10);
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return not_found(&req.path);
        };
        let tree = store_guard.implementors_tree(id, max_depth);
        let json = implementor_node_to_json(&tree, &store_guard);
        drop(store_guard);
        let value = serde_json::json!({ "root": json });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let max_depth = req.max_depth.unwrap_or(4).min(10);
        let store_guard = self.store.read().await;
        let Some(id) = store_guard.lookup(&req.path) else {
            drop(store_guard);
            return not_found(&req.path);
        };
        let tree = store_guard.importers_tree(id, max_depth);
        let json = importer_node_to_json(&tree, &store_guard);
        drop(store_guard);
        let value = serde_json::json!({ "root": json });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
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
        let value = serde_json::json!({
            "nodes": nodes,
            "symbol_count": symbol_count,
            "top_n": top_n,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
    }

    #[tool(
        description = "Return the dependency depth of a symbol — the length of the longest                        path from any root (symbol with no incoming edges of the given kind)                        to this symbol, following incoming edges. Depth 0 = the symbol is a                        root; depth 1 = only predecessors are roots. Cycle-safe. File nodes                        excluded. Returns { path, depth, edge_kind } or { error }."
    )]
    async fn mycelium_get_dependency_depth(
        &self,
        Parameters(req): Parameters<DependencyDepthRequest>,
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let store = self.store.read().await;
        let Some(id) = store.lookup(&req.path) else {
            return not_found(&req.path);
        };
        let Some(depth) = store.dependency_depth(id, kind) else {
            return application_error(
                &serde_json::json!({ "error": format!("not a symbol node: {}", req.path) }),
            );
        };
        drop(store);
        let value = serde_json::json!({
            "path": req.path,
            "depth": depth,
            "edge_kind": req.edge_kind,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
        };
        let sort_by = req.sort_by.as_deref().unwrap_or("in");
        if sort_by != "in" && sort_by != "out" {
            return application_error(
                &serde_json::json!({ "error": format!("unknown sort_by: {sort_by}") }),
            );
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
        let value = serde_json::json!({
            "nodes": nodes,
            "symbol_count": symbol_count,
            "top_n": top_n,
            "sort_by": sort_by,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
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
    ) -> CallToolResult {
        let kind = match parse_edge_kind(&req.edge_kind) {
            Ok(k) => k,
            Err(e) => return application_error(&serde_json::json!({ "error": e })),
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
        let value = serde_json::json!({
            "components": components,
            "total_components": total_components,
            "symbol_count": symbol_count,
            "min_size": min_size,
        });
        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
    }

    #[tool(
        description = "Immediately re-index a single file, bypassing the watch debounce. \
                       Use this to satisfy the Charter §2 reactive <10 ms SLA: call this tool \
                       right after writing a file to get fresh query results in <10 ms. \
                       Returns { path, symbols_before, symbols_after, elapsed_us }. \
                       Unknown extension returns { error }."
    )]
    async fn mycelium_sync_file(
        &self,
        Parameters(req): Parameters<SyncFileRequest>,
    ) -> CallToolResult {
        let ext = std::path::Path::new(&req.path)
            .extension()
            .and_then(|e| e.to_str())
            .map(ToOwned::to_owned);
        let Some(ext) = ext else {
            return application_error(
                &serde_json::json!({ "error": format!("no extension: {}", req.path) }),
            );
        };
        if !matches!(
            ext.as_str(),
            "js" | "jsx" | "py" | "pyi" | "ts" | "tsx" | "rs"
        ) {
            return application_error(
                &serde_json::json!({ "error": format!("unsupported extension: {ext}") }),
            );
        }

        // Locate the file on disk relative to the workspace root.
        // We store the root in watch_state indirectly; fall back to CWD.
        let abs_path = std::env::current_dir().unwrap_or_default().join(&req.path);

        let Ok(src) = std::fs::read(&abs_path) else {
            return application_error(
                &serde_json::json!({ "error": format!("cannot read: {}", req.path) }),
            );
        };

        let start = std::time::Instant::now();
        let mut store_w = self.store.write().await;
        let symbols_before = store_w.node_count();
        store_w.remove_file(&req.path);
        reindex_file(&req.path, &src, &ext, &mut store_w);
        store_w.resolve_bare_call_stubs();
        let symbols_after = store_w.node_count();
        drop(store_w);
        self.refresh_budget(symbols_after).await;
        let elapsed_us = start.elapsed().as_micros();

        success_str(
            serde_json::json!({
                "path": req.path,
                "symbols_before": symbols_before,
                "symbols_after": symbols_after,
                "elapsed_us": elapsed_us,
            })
            .to_string(),
        )
    }

    // ── RFC-0090: compact output ──────────────────────────────────────────────

    #[tool(
        description = "Enable or disable compact (MessagePack hex) output mode. \
                       When enabled, `mycelium_search_symbol` returns \
                       { \"fmt\": \"msgpack_hex\", \"data\": \"<hex>\" } instead of plain JSON, \
                       reducing AI token consumption to ≤ 30 % of the JSON equivalent \
                       (Charter §2 SLA). Accepts { \"enabled\": true | false }. \
                       Returns { \"compact_mode\": <bool>, \"message\": \"...\" }."
    )]
    async fn mycelium_set_compact_mode(
        &self,
        Parameters(req): Parameters<SetCompactModeRequest>,
    ) -> CallToolResult {
        self.compact_mode.store(req.enabled, Ordering::Relaxed);
        let msg = if req.enabled {
            "compact MessagePack hex output enabled"
        } else {
            "compact mode disabled; reverting to JSON output"
        };
        success_str(
            serde_json::json!({
                "compact_mode": req.enabled,
                "message": msg,
            })
            .to_string(),
        )
    }

    #[tool(
        description = "PRIMARY for understanding an area or tracing a flow. One call \
                       returns entry points, related call-graph nodes, edges, and source \
                       snippets from a natural-language task. Use before chaining \
                       mycelium_search_symbol, callers, callees, and file reads."
    )]
    async fn mycelium_context(
        &self,
        Parameters(req): Parameters<GetContextRequest>,
    ) -> CallToolResult {
        use mycelium_core::context::{self, ContextOptions, Routing};

        let max_nodes = req.max_nodes.unwrap_or(30).min(100);
        let max_code_blocks = req.max_code_blocks.unwrap_or(6).min(25);
        let edge_kinds: Vec<mycelium_core::types::EdgeKind> = req
            .edge_kinds
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter_map(|s| context::parse_edge_kind(s))
            .collect();
        let opts = ContextOptions {
            max_nodes,
            max_code_blocks,
            edge_kinds,
        };

        let store = self.store.read().await;
        // Hyphae-first routing (RFC-0101 §classify): a selector task is
        // evaluated by the DSL engine; prose goes through candidate search.
        let (routing, candidates, entry_points) = if context::looks_like_hyphae(&req.task) {
            if let Ok(ast) = mycelium_hyphae::parse(&req.task) {
                let eps = mycelium_hyphae::evaluator::Evaluator::new(&store)
                    .eval(&ast)
                    .into_iter()
                    .take(max_nodes)
                    .collect::<Vec<String>>();
                (Routing::Hyphae, Vec::new(), eps)
            } else {
                let cands = context::extract_symbol_candidates(&req.task);
                let eps = context::seed_entry_points(&store, &cands, max_nodes);
                (Routing::Natural, cands, eps)
            }
        } else {
            let cands = context::extract_symbol_candidates(&req.task);
            let eps = context::seed_entry_points(&store, &cands, max_nodes);
            (Routing::Natural, cands, eps)
        };
        // Per-call budget override (RFC-0102 §"Request knobs"). Parsed here so
        // an invalid value fails fast with an application error before any
        // formatting. The CLI twin parses the identical string via the same
        // core `FromStr`, so both surfaces resolve the same budget.
        let budget_override = match req.budget.as_deref() {
            None => None,
            Some(s) => match s.parse::<BudgetOverride>() {
                Ok(o) => Some(o),
                Err(e) => {
                    return application_error(&serde_json::json!({ "error": e }));
                }
            },
        };

        let mut value = context::build_payload(
            &store,
            &req.task,
            &candidates,
            &entry_points,
            routing,
            &opts,
        );
        // Apply the resolved budget over the same payload — the CLI twin runs
        // the identical `resolve(override, node_count)`, so the truncated JSON
        // stays byte-identical (RFC-0102 / Three-Surface Rule).
        apply_budget(
            &mut value,
            &OutputBudget::resolve(budget_override, store.node_count()),
        );
        drop(store);

        success_str(req.output_format.map_or_else(
            || value.to_string(),
            |fmt| formatter_for(fmt).format(&value),
        ))
    }

    #[tool(
        description = "Return a byte-count comparison between JSON and MessagePack serialisation \
                       for a fixed sample payload (three symbol search results). \
                       Use this to verify the Charter §2 token-efficiency SLA (≤ 30 % of JSON). \
                       Returns { sample_query, json_bytes, msgpack_bytes, ratio }."
    )]
    async fn mycelium_get_token_stats(&self) -> CallToolResult {
        // Fixed sample payload — three representative symbol paths.
        let sample = serde_json::json!({
            "matches": [
                "src/engine/store.rs>Store",
                "src/engine/store.rs>Store::upsert_node",
                "src/engine/store.rs>Store::search_symbol"
            ]
        });
        let json_bytes = sample.to_string().len();
        // The sample value is entirely static strings; serialisation cannot fail.
        #[allow(clippy::unwrap_used)]
        let msgpack_bytes = rmp_serde::to_vec_named(&sample).unwrap_or_default().len();
        // Byte ratio: raw msgpack binary vs JSON text.
        #[allow(clippy::cast_precision_loss)]
        let ratio = msgpack_bytes as f64 / json_bytes as f64;
        // Token ratio: abbreviated compact-JSON text vs verbose JSON text.
        // The compact format uses single-char key "m" instead of "matches", reducing
        // AI-visible token consumption without binary encoding overhead.
        let compact = serde_json::json!({
            "m": [
                "src/engine/store.rs>Store",
                "src/engine/store.rs>Store::upsert_node",
                "src/engine/store.rs>Store::search_symbol"
            ]
        });
        let compact_chars = compact.to_string().len();
        #[allow(clippy::cast_precision_loss)]
        let token_ratio = compact_chars as f64 / json_bytes as f64;
        success_str(
            serde_json::json!({
                "sample_query": "top 3 symbols",
                "json_bytes": json_bytes,
                "msgpack_bytes": msgpack_bytes,
                "ratio": ratio,
                "compact_chars": compact_chars,
                "token_ratio": token_ratio,
            })
            .to_string(),
        )
    }
}

const MCP_INSTRUCTIONS_BASE: &str = "\
## Mycelium — AI-native symbol graph (93 tools)

**Setup (always first):**
- Index a workspace → `mycelium_index_workspace`
- Reload a saved index → `mycelium_load_index`
- Check readiness → `mycelium_server_status`

## Primary Tool Selection

1. **\"How does X work?\" / trace A to B / broad code-area understanding**
   → Use `mycelium_context` FIRST (one call returns entry points + graph + source).
   Do NOT chain `mycelium_search_symbol` → `mycelium_get_callers` → `mycelium_get_callees`.

2. **\"Where is X defined?\" / \"find symbol\"**
   → Use `mycelium_search_symbol`, then `mycelium_get_symbol_info` only for
   the best matching symbol.

3. **\"What calls X?\" / \"what does X call?\"**
   → Use `mycelium_get_callers` / `mycelium_get_callees` directly. Use
   `mycelium_get_caller_tree` / `mycelium_get_callee_tree` only when the task
   asks for transitive reachability.

4. **Class hierarchy / inheritance / interface questions**
   → Use `mycelium_get_subclasses_tree`, `mycelium_get_extends_tree`,
   `mycelium_get_implementors_tree`, or `mycelium_get_implements_tree`.

5. **Complex multi-hop graph queries**
   → Use `mycelium_query` with Hyphae DSL. Prefer one precise query over a
   loop of broad exploratory calls.

**Intent → tool quick map:**
- Find symbol by name/prefix → `mycelium_search_symbol`
- Full symbol info (ancestors, callers, callees) → `mycelium_get_symbol_info`
- Direct callers of a function → `mycelium_get_callers`
- Direct callees of a function → `mycelium_get_callees`
- Transitive callee tree → `mycelium_get_callee_tree`
- Transitive caller tree → `mycelium_get_caller_tree`
- Common callers of N symbols → `mycelium_get_common_callers`
- Shortest call path between two symbols → `mycelium_find_call_path`
- Direct import neighbors → `mycelium_get_imports`
- Transitive import tree → `mycelium_get_import_tree`
- Shortest import path → `mycelium_find_import_path`
- Reverse-dependency forest → `mycelium_get_importers_tree`
- Direct superclass/subclass → `mycelium_get_extends` / `mycelium_get_subclasses_tree`
- Inheritance chain → `mycelium_get_extends_tree` / `mycelium_find_extends_path`
- Interface implementations → `mycelium_get_implements` / `mycelium_get_implementors_tree`
- All symbols of a kind (function/class/…) → `mycelium_get_symbols_by_kind`
- Entry points / dead-code candidates → `mycelium_get_entry_points`
- Hyphae DSL query → `mycelium_query`
- Batch symbol info (up to 50) → `mycelium_batch_symbol_info`

## Anti-patterns

- Do NOT chain `mycelium_search_symbol` → `mycelium_get_callers` →
  `mycelium_get_callees` → `mycelium_get_symbol_info` for architecture
  questions. Use the smallest composite/tree/path tool that answers the task.
- Do NOT loop `mycelium_get_symbol_info` over many symbols. Use
  `mycelium_batch_symbol_info` or a graph/tree tool.
- Do NOT re-verify routine Mycelium graph results with grep or broad file reads.
  Read source only when editing code, resolving a `NOT_FOUND`, or investigating
  a Mycelium inconsistency.
- Do NOT call broad enumeration tools without limits on large projects.

## Sufficiency Check

Stop after 3–5 calls and synthesize. If a response is truncated, follow up with
one targeted symbol/file query, not another broad exploration.";

fn build_mcp_instructions(node_count: Option<usize>) -> String {
    let mut instructions = MCP_INSTRUCTIONS_BASE.to_owned();

    if node_count.is_some_and(|count| count < 500) {
        instructions.push_str(
            "\n\n## Small Project Mode\n\n\
This index has fewer than 500 nodes. Prefer the core direct tools \
(`mycelium_search_symbol`, `mycelium_get_symbol_info`, `mycelium_query`, \
`mycelium_server_status`) and avoid heavy batch/whole-graph exploration unless \
the task explicitly asks for it.",
        );
    }

    instructions
}

#[tool_handler]
impl ServerHandler for MyceliumServer {
    fn get_info(&self) -> ServerInfo {
        let node_count = self.store.try_read().ok().map(|store| store.node_count());
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(build_mcp_instructions(node_count))
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

/// Source-language extensions used by compound-extension detection (Issue #294).
const SOURCE_EXTS: &[&str] = &[
    "js", "jsx", "ts", "tsx", "py", "pyi", "rs", "go", "java", "c", "h", "cpp", "cc", "cxx", "hpp",
    "rb", "cs",
];

// ── indexing helper (CPU-bound, run via spawn_blocking) ───────────────────────

// ts_lang / tsx_lang differ only by one letter — similarity is intentional.
#[allow(clippy::similar_names, clippy::too_many_lines)]
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

    let go_lang: tree_sitter::Language = tree_sitter_go::LANGUAGE.into();
    let go_ext = Extractor::new(go_lang, GO_QUERIES).context("failed to compile Go extractor")?;

    let java_lang: tree_sitter::Language = tree_sitter_java::LANGUAGE.into();
    let java_ext =
        Extractor::new(java_lang, JAVA_QUERIES).context("failed to compile Java extractor")?;

    let c_lang: tree_sitter::Language = tree_sitter_c::LANGUAGE.into();
    let c_ext = Extractor::new(c_lang, C_QUERIES).context("failed to compile C extractor")?;

    let ruby_lang: tree_sitter::Language = tree_sitter_ruby::LANGUAGE.into();
    let ruby_ext =
        Extractor::new(ruby_lang, RUBY_QUERIES).context("failed to compile Ruby extractor")?;

    let cpp_lang: tree_sitter::Language = tree_sitter_cpp::LANGUAGE.into();
    let cpp_ext =
        Extractor::new(cpp_lang, CPP_QUERIES).context("failed to compile C++ extractor")?;

    let csharp_lang: tree_sitter::Language = tree_sitter_c_sharp::LANGUAGE.into();
    let csharp_ext =
        Extractor::new(csharp_lang, CSHARP_QUERIES).context("failed to compile C# extractor")?;

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

        // Issue #294: skip files with compound source-language extensions like
        // `module.ts.py` — artefacts/cache files that would be indexed under
        // the wrong language and produce misleading results.
        if let Some(stem_ext) = path
            .file_stem()
            .and_then(|s| std::path::Path::new(s).extension())
            .and_then(|e| e.to_str())
        {
            if SOURCE_EXTS.contains(&stem_ext) && stem_ext != ext {
                debug!("skipping compound-extension file: {}", path.display());
                continue;
            }
        }

        let (extractor, lang_name) = match ext {
            "js" | "jsx" => (&js_ext, "javascript"),
            "py" | "pyi" => (&py_ext, "python"),
            "ts" => (&ts_ext, "typescript"),
            "tsx" => (&tsx_ext, "typescript"),
            "rs" => (&rs_ext, "rust"),
            "go" => (&go_ext, "go"),
            "java" => (&java_ext, "java"),
            "c" | "h" => (&c_ext, "c"),
            "rb" => (&ruby_ext, "ruby"),
            "cpp" | "cc" | "cxx" | "hpp" => (&cpp_ext, "cpp"),
            "cs" => (&csharp_ext, "csharp"),
            _ => continue,
        };
        // Issue #294: skip rather than fall back to the raw absolute path when
        // strip_prefix fails — absolute paths produce `///`-prefixed Trunk paths
        // that cannot be used for further queries.
        let Ok(rel_path) = path.strip_prefix(root) else {
            warn!(
                "could not relativize {} against root {}; skipping",
                path.display(),
                root.display()
            );
            errors += 1;
            continue;
        };
        let rel = rel_path.to_string_lossy().replace('\\', "/");
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
        "go" => make_ext(tree_sitter_go::LANGUAGE.into(), GO_QUERIES),
        "java" => make_ext(tree_sitter_java::LANGUAGE.into(), JAVA_QUERIES),
        "c" | "h" => make_ext(tree_sitter_c::LANGUAGE.into(), C_QUERIES),
        "rb" => make_ext(tree_sitter_ruby::LANGUAGE.into(), RUBY_QUERIES),
        "cpp" | "cc" | "cxx" | "hpp" => make_ext(tree_sitter_cpp::LANGUAGE.into(), CPP_QUERIES),
        "cs" => make_ext(tree_sitter_c_sharp::LANGUAGE.into(), CSHARP_QUERIES),
        _ => return,
    };

    if let Some(ext_obj) = extractor {
        if let Err(e) = ext_obj.extract(rel, src, store) {
            warn!("watch re-extract failed for {rel}: {e}");
        }
    }
}

// ── compact MessagePack output (RFC-0090 Hyphae token efficiency) ────────────

/// Encode `value` as `MessagePack` and return a hex-encoded JSON wrapper.
///
/// Format: `{ "fmt": "msgpack_hex", "data": "<hex>", "bytes": N }`
///
/// Token savings vs raw JSON: typically 40-65% fewer bytes.
fn encode_msgpack_hex(value: &serde_json::Value) -> String {
    match rmp_serde::to_vec_named(value) {
        Ok(bytes) => {
            let hex: String =
                bytes
                    .iter()
                    .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
                        use std::fmt::Write as _;
                        let _ = write!(s, "{b:02x}");
                        s
                    });
            let byte_count = bytes.len();
            serde_json::json!({
                "fmt": "msgpack_hex",
                "data": hex,
                "bytes": byte_count,
            })
            .to_string()
        }
        Err(e) => {
            // Fallback to plain JSON on serialization failure.
            warn!("msgpack encode failed: {e}; falling back to JSON");
            value.to_string()
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
/// Start the MCP server over stdio.
///
/// `allowed_roots` restricts which filesystem paths `mycelium_index_workspace`
/// and `mycelium_load_index` may access (RFC-0097). When empty, all paths are
/// permitted — **do not pass an empty vec from production CLI code**; use
/// `[CWD]` as the minimum safe default.
pub async fn serve_stdio(root: Option<PathBuf>, allowed_roots: Vec<PathBuf>) -> anyhow::Result<()> {
    let server = match root {
        Some(r) => {
            MyceliumServer::with_root_and_allowed_roots(r.clone(), {
                let mut roots = allowed_roots;
                if roots.is_empty() {
                    roots.push(r);
                }
                roots
            })
            .await?
        }
        None => {
            if allowed_roots.is_empty() {
                MyceliumServer::new()
            } else {
                MyceliumServer::new_with_allowed_roots(allowed_roots)
            }
        }
    };
    let transport = rmcp::transport::stdio();
    let running = server.serve(transport).await?;
    // PUSH (RFC-0106): capture the client peer now that the rmcp service is
    // running, so the watch loop's on_batch can fire `mycelium/graphChanged`
    // notifications. Setting this AFTER `.serve()` returns is unavoidable —
    // the Peer only materialises here. Any watch batch that fires before this
    // point (e.g. from a constructor's initial index) silently skips the
    // notification (the notifier Option is still None).
    running.service().set_notifier(running.peer().clone()).await;
    // SUBSCRIBE (RFC-0107): start the 60s periodic TTL-eviction task. One
    // task per server lifetime; survives client reconnects.
    running.service().start_subscription_eviction().await;
    running.waiting().await?;
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;

#[cfg(test)]
mod server_info_tests {
    use super::*;
    use mycelium_core::trunk::TrunkPath;

    #[test]
    fn get_info_includes_routing_instructions() {
        let server = MyceliumServer::default();
        let info = server.get_info();
        let instructions = info
            .instructions
            .expect("get_info() must expose MCP server instructions for agent routing");
        assert!(!instructions.is_empty(), "instructions must be non-empty");
        assert!(
            instructions.contains("mycelium_search_symbol"),
            "routing table must mention mycelium_search_symbol; got: {instructions}"
        );
        assert!(
            instructions.contains("mycelium_get_callers"),
            "routing table must mention mycelium_get_callers; got: {instructions}"
        );
        assert!(
            instructions.contains("mycelium_index_workspace"),
            "routing table must mention mycelium_index_workspace as setup step; got: {instructions}"
        );
    }

    #[test]
    fn get_info_includes_primary_tool_selection_rules() {
        let server = MyceliumServer::default();
        let instructions = server
            .get_info()
            .instructions
            .expect("instructions must be present");

        assert!(
            instructions.contains("Primary Tool Selection"),
            "instructions must include an explicit decision tree; got: {instructions}"
        );
        assert!(
            instructions.contains("\"How does X work?\""),
            "decision tree must name architecture-understanding prompts; got: {instructions}"
        );
        assert!(
            instructions.contains("mycelium_query"),
            "decision tree must route complex multi-hop prompts to Hyphae; got: {instructions}"
        );
    }

    #[test]
    fn get_info_includes_agent_anti_patterns() {
        let server = MyceliumServer::default();
        let instructions = server
            .get_info()
            .instructions
            .expect("instructions must be present");

        assert!(
            instructions.contains("Anti-patterns"),
            "instructions must include an anti-pattern section; got: {instructions}"
        );
        assert!(
            instructions.contains("Do NOT chain"),
            "instructions must discourage broad multi-tool chains; got: {instructions}"
        );
        assert!(
            instructions.contains("Do NOT re-verify"),
            "instructions must discourage routine grep/file re-verification; got: {instructions}"
        );
    }

    #[test]
    fn get_info_includes_small_project_mode_for_empty_server() {
        let server = MyceliumServer::default();
        let instructions = server
            .get_info()
            .instructions
            .expect("instructions must be present");

        assert!(
            instructions.contains("Small Project Mode"),
            "empty or tiny indexes must get small-project guidance; got: {instructions}"
        );
    }

    #[test]
    fn get_info_omits_small_project_mode_for_large_index() {
        let server = MyceliumServer::default();
        {
            let mut store = server.store.try_write().expect("store lock must be free");
            for i in 0..500 {
                let path = TrunkPath::parse(&format!("src/file_{i}.rs")).unwrap();
                store.upsert_node(path);
            }
        }

        let instructions = server
            .get_info()
            .instructions
            .expect("instructions must be present");

        assert!(
            !instructions.contains("Small Project Mode"),
            "large indexes must not receive small-project guidance; got: {instructions}"
        );
    }
}

#[cfg(test)]
mod output_budget_tests {
    use super::*;

    #[test]
    fn output_budget_small_project() {
        let budget = OutputBudget::for_project(100);
        assert_eq!(budget.max_nodes, 15);
        assert_eq!(budget.max_edges, 30);
    }

    #[test]
    fn output_budget_medium_project() {
        let budget = OutputBudget::for_project(1000);
        assert_eq!(budget.max_nodes, 30);
        assert_eq!(budget.max_edges, 60);
    }

    #[test]
    fn output_budget_large_project() {
        let budget = OutputBudget::for_project(10_000);
        assert_eq!(budget.max_nodes, 50);
        assert_eq!(budget.max_edges, 100);
    }

    #[test]
    fn apply_budget_truncates_node_array() {
        let budget = OutputBudget::for_project(100);
        let mut value = serde_json::json!({
            "nodes": (0..30).map(|i| format!("node_{i}")).collect::<Vec<_>>(),
            "count": 30
        });
        apply_budget(&mut value, &budget);
        let nodes = value["nodes"].as_array().expect("nodes must be array");
        assert_eq!(nodes.len(), 15);
        assert_eq!(value["truncated"], true);
        assert_eq!(value["total_available"], 30);
    }

    #[test]
    fn apply_budget_no_truncation_when_under_limit() {
        let budget = OutputBudget::for_project(100);
        let mut value = serde_json::json!({
            "nodes": vec!["a", "b", "c"],
            "count": 3
        });
        apply_budget(&mut value, &budget);
        let nodes = value["nodes"].as_array().expect("nodes must be array");
        assert_eq!(nodes.len(), 3);
        assert!(
            value.get("truncated").is_none(),
            "should not have truncated flag"
        );
    }

    #[test]
    fn apply_budget_truncates_edges_array() {
        let budget = OutputBudget::for_project(100);
        let mut value = serde_json::json!({
            "edges": (0..50).map(|i| format!("edge_{i}")).collect::<Vec<_>>(),
            "count": 50
        });
        apply_budget(&mut value, &budget);
        let edges = value["edges"].as_array().expect("edges must be array");
        assert_eq!(edges.len(), 30);
        assert_eq!(value["truncated"], true);
        assert_eq!(value["total_available"], 50);
    }

    #[test]
    fn is_core_tool_identifies_core_tools() {
        assert!(is_core_tool("mycelium_context"));
        assert!(is_core_tool("mycelium_search_symbol"));
        assert!(is_core_tool("mycelium_get_symbol_info"));
        assert!(is_core_tool("mycelium_query"));
        assert!(is_core_tool("mycelium_server_status"));
        assert!(!is_core_tool("mycelium_get_all_symbols"));
        assert!(!is_core_tool("mycelium_get_callees"));
    }
}
