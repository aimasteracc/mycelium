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
    CalleeNode, CallerNode, ExtendsNode, ImplementorNode, ImplementsNode, ImportNode, ImporterNode,
    SubclassNode, extractor::Extractor, store::Store,
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

/// Input parameters for `mycelium_get_files`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFilesRequest {
    /// Optional path prefix to filter results (e.g. `"src/"`).
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
