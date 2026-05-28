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
//! | `mycelium_server_status` | Return node count, root, and ready status |
//!
//! See RFC-0004, RFC-0005, RFC-0006, and RFC-0007 for the design.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context as _;
use mycelium_core::{extractor::Extractor, store::Store};
use rmcp::{
    ServerHandler, ServiceExt, handler::server::wrapper::Parameters, model::Implementation,
    model::ServerCapabilities, model::ServerInfo, tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::warn;

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

// ── server ────────────────────────────────────────────────────────────────────

/// Stateful MCP server holding the in-memory symbol graph.
///
/// Construct with [`MyceliumServer::new`] or [`MyceliumServer::with_root`]
/// and start with [`serve_stdio`].
#[derive(Debug, Clone)]
pub struct MyceliumServer {
    store: Arc<RwLock<Store>>,
    indexed_root: Arc<RwLock<Option<PathBuf>>>,
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
                    *server.indexed_root.write().await = Some(root);
                    return Ok(server);
                }
                Err(e) => {
                    tracing::warn!("snapshot load failed ({e}), falling back to live index");
                }
            }
        }

        // Fall back: run live index.
        let root_clone = root.clone();
        let (new_store, files, errors, _languages) =
            tokio::task::spawn_blocking(move || run_index(&root_clone))
                .await
                .map_err(|e| anyhow::anyhow!("indexing task panicked: {e}"))??;
        tracing::info!(files, errors, "live index completed");
        if let Err(e) = new_store.save(&snap) {
            tracing::warn!("could not save snapshot after live index: {e}");
        }
        *server.store.write().await = new_store;
        *server.indexed_root.write().await = Some(root);
        Ok(server)
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
            Ok(Ok((new_store, files, errors, languages))) => {
                // RFC-0006: auto-save snapshot alongside the workspace
                let snap = PathBuf::from(&req.path).join(".mycelium").join("index.rmp");
                if let Err(e) = new_store.save(&snap) {
                    warn!("could not save index snapshot: {e}");
                }
                *self.store.write().await = new_store;
                *self.indexed_root.write().await = Some(PathBuf::from(&req.path));
                serde_json::json!({ "files": files, "errors": errors, "languages": languages })
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
        let node_count = self.store.read().await.node_count();
        let root_guard = self.indexed_root.read().await;
        let indexed_root = root_guard
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let is_loaded = root_guard.is_some();
        drop(root_guard);
        serde_json::json!({
            "node_count": node_count,
            "indexed_root": indexed_root,
            "is_loaded": is_loaded,
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

// ── indexing helper (CPU-bound, run via spawn_blocking) ───────────────────────

// ts_lang / tsx_lang differ only by one letter — similarity is intentional.
#[allow(clippy::similar_names)]
fn run_index(root: &std::path::Path) -> anyhow::Result<(Store, usize, usize, Vec<String>)> {
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

    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
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
    Ok((
        store,
        files,
        errors,
        languages.into_iter().map(str::to_owned).collect(),
    ))
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
    async fn server_status_returns_node_count() {
        let server = server_with_fixture().await;
        let raw = server.mycelium_server_status().await;
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            val["node_count"].as_u64().unwrap() > 0,
            "node_count must be non-zero"
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
}
