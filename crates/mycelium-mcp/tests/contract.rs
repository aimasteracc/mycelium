//! Cross-tool MCP response contract tests (Issue #211 / Sugg 3 of #206).
//!
//! Spins up an in-process `MyceliumServer` + rmcp client via a `tokio::io::duplex`
//! pipe and verifies the invariants that every registered MCP tool must satisfy:
//!
//!   1. Non-empty `content` array in the response (every call must return data).
//!   2. Non-empty `description` on every tool manifest (aids AI agent tool selection).
//!
//! The tests intentionally use an **empty, unindexed** server and pass a catch-all
//! argument map so that tools which need a `path` / `query` / etc. get a value they
//! can at least deserialize — they will then return an "unknown path" or "not indexed"
//! response, which still satisfies the non-empty-content contract.

use mycelium_mcp::MyceliumServer;
use rmcp::{
    ClientHandler, ServiceExt,
    model::{CallToolRequestParams, ClientInfo},
};

/// Expected number of registered MCP tools.
/// Update this constant when you add or remove a tool.
const EXPECTED_TOOL_COUNT: usize = 93;

/// Minimal no-op client handler — only needs to exist for the MCP handshake.
#[derive(Debug, Clone, Default)]
struct TestClient;

impl ClientHandler for TestClient {
    fn get_info(&self) -> ClientInfo {
        ClientInfo::default()
    }
}

/// A superset of argument names covering every required/optional field used by all 89 tools.
///
/// Serde ignores unknown fields during deserialization, so every tool will
/// successfully deserialize its request from this map and return a real
/// (possibly error) response rather than an rmcp deserialization fault.
///
/// Values are chosen to be type-compatible but semantically invalid so that
/// tools perform a real lookup on the empty store and return a non-empty
/// "not found" / "not indexed" response.
fn catch_all_args() -> serde_json::Map<String, serde_json::Value> {
    serde_json::json!({
        // ── path-like fields ──────────────────────────────────────────────
        // Use a TrunkPath-style string (no leading slash) for symbol lookups,
        // and a non-existent absolute path for workspace/file roots so the
        // index tool fails gracefully without creating filesystem artifacts.
        "path":        "phantom/file.rs>symbol",
        "path1":       "phantom/file.rs>symbol",
        "path2":       "phantom/file.rs>symbol",
        "from_path":   "phantom/file.rs>symbol",
        "to_path":     "phantom/file.rs>symbol",
        "from":        "phantom/file.rs>symbol",
        "to":          "phantom/file.rs>symbol",
        "file_path":   "/nonexistent-contract-test-phantom/file.rs",
        "root":        "/nonexistent-contract-test-phantom",
        "paths":       [],
        "sources":     [],
        "path_prefix": "",
        // ── string-based classifiers ──────────────────────────────────────
        "query":     "phantom_query",
        "symbol":    "phantom_symbol",
        "task":      "phantom_task",
        "kind":      "function",
        // valid values for parse_edge_kind(): calls | imports | extends | implements
        "edge_kind": "calls",
        "sort_by":   "out",
        // Hyphae expression — an empty-match selector that won't crash the parser
        "expr":      "*",
        // ── numeric options ───────────────────────────────────────────────
        "limit":      1,
        "depth":      1,
        "k":          1,
        "max_depth":  1,
        "top_n":      1,
        "min_in":     0,
        "min_out":    0,
        "min_size":   0,
        "damping":    0.85,
        "iterations": 1,
        // ── boolean flags ─────────────────────────────────────────────────
        "enabled":    false,
        // ── SUBSCRIBE (RFC-0107) ──────────────────────────────────────────
        // Tagged-union Interest with a safe Files variant + dummy id for
        // unsubscribe/status round-trip.
        "interest":        { "kind": "files", "paths": ["phantom/*.rs"] },
        "subscription_id": "phantom-sub",
    })
    .as_object()
    .cloned()
    .expect("static JSON object")
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Spin up a fresh server + client pair over an in-memory duplex transport.
///
/// Returns `(client, server_handle)`. Caller is responsible for calling
/// `client.cancel().await?` and `server_handle.await?` to clean up.
async fn make_pair() -> anyhow::Result<(
    rmcp::service::RunningService<rmcp::RoleClient, TestClient>,
    tokio::task::JoinHandle<anyhow::Result<()>>,
)> {
    let (server_transport, client_transport) = tokio::io::duplex(131_072);

    let server = MyceliumServer::new();
    let server_handle = tokio::spawn(async move {
        // waiting() returns when the client disconnects — expected path.
        // The error branch is only hit if the MCP handshake fails.
        let _reason = server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = TestClient.serve(client_transport).await?;
    Ok((client, server_handle))
}

// ── contract tests ────────────────────────────────────────────────────────────

/// Every tool returns non-empty `content` regardless of the input.
#[tokio::test]
async fn all_tools_return_non_empty_content() -> anyhow::Result<()> {
    let (client, server_handle) = make_pair().await?;

    let tools = client.list_all_tools().await?;

    assert_eq!(
        tools.len(),
        EXPECTED_TOOL_COUNT,
        "Expected exactly {EXPECTED_TOOL_COUNT} MCP tools, found {}. \
         Update EXPECTED_TOOL_COUNT in tests/contract.rs after adding/removing tools.",
        tools.len()
    );

    let args = catch_all_args();
    let mut violations: Vec<String> = Vec::new();

    for tool in &tools {
        match client
            .call_tool(CallToolRequestParams::new(tool.name.clone()).with_arguments(args.clone()))
            .await
        {
            Err(e) => {
                // An rmcp-level error (e.g. -32602 argument deserialization failure)
                // means the tool was unreachable with our catch-all args.  This is a
                // contract violation: tools should either accept partial input or have
                // all required fields covered by catch_all_args().
                violations.push(format!(
                    "  tool `{}` returned an rmcp error (add missing fields to \
                     catch_all_args): {e}",
                    tool.name
                ));
            }
            Ok(result) if result.content.is_empty() => {
                violations.push(format!(
                    "  tool `{}` returned empty content (is_error={:?})",
                    tool.name, result.is_error
                ));
            }
            Ok(_) => {} // contract satisfied
        }
    }

    client.cancel().await?;
    // The server exits naturally when the client disconnects; ignore the
    // `Err(QuitReason::Cancelled)` that comes back through the join handle.
    let _ = server_handle.await;

    assert!(
        violations.is_empty(),
        "Contract violations:\n{}",
        violations.join("\n")
    );
    Ok(())
}

/// Every tool manifest carries a non-empty description string.
#[tokio::test]
async fn all_tools_have_non_empty_description() -> anyhow::Result<()> {
    let (client, server_handle) = make_pair().await?;

    let tools = client.list_all_tools().await?;

    let missing: Vec<&str> = tools
        .iter()
        .filter(|t| t.description.as_deref().unwrap_or("").trim().is_empty())
        .map(|t| t.name.as_ref())
        .collect();

    client.cancel().await?;
    let _ = server_handle.await;

    assert!(
        missing.is_empty(),
        "Tools missing non-empty description: {missing:?}"
    );
    Ok(())
}

/// `list_tools` returns exactly `EXPECTED_TOOL_COUNT` tools — checked
/// independently of the per-tool call test to surface regressions fast.
#[tokio::test]
async fn tool_count_matches_expected() -> anyhow::Result<()> {
    let (client, server_handle) = make_pair().await?;

    let tools = client.list_all_tools().await?;
    let count = tools.len();

    client.cancel().await?;
    let _ = server_handle.await;

    assert_eq!(
        count, EXPECTED_TOOL_COUNT,
        "Tool registry has {count} tools, expected {EXPECTED_TOOL_COUNT}. \
         Update EXPECTED_TOOL_COUNT in tests/contract.rs."
    );
    Ok(())
}

// ── is_error contract (issue #206 S1) ────────────────────────────────────────
//
// Every query tool MUST set `is_error: Some(true)` when the requested symbol
// does not exist in the index, and `is_error: Some(false)` when the call
// succeeds.  MCP clients branch on this flag without parsing the JSON body.

/// Path-lookup tools return `is_error: Some(true)` for an unknown symbol path.
#[tokio::test]
async fn path_not_found_yields_is_error_true() -> anyhow::Result<()> {
    let (client, server_handle) = make_pair().await?;

    // Representative sample — two different lookup shapes.
    // mycelium_get_ancestors is intentionally omitted: it returns an empty
    // list for root nodes (no ancestors), so unknown-path and root-node
    // are indistinguishable; the tool has "graceful empty" semantics.
    let tools_under_test = ["mycelium_get_callees", "mycelium_get_callers"];
    let args = serde_json::json!({"path": "nonexistent/file.rs>ghost_symbol"});
    let args_map = args.as_object().cloned().expect("static JSON object");

    let mut failures: Vec<String> = Vec::new();

    for name in tools_under_test {
        let result = client
            .call_tool(
                CallToolRequestParams::new(name.to_string()).with_arguments(args_map.clone()),
            )
            .await?;
        if result.is_error != Some(true) {
            failures.push(format!(
                "  `{name}` on unknown path: is_error={:?} (expected Some(true))",
                result.is_error
            ));
        }
    }

    client.cancel().await?;
    let _ = server_handle.await;

    assert!(
        failures.is_empty(),
        "is_error contract violations (not-found paths):\n{}",
        failures.join("\n")
    );
    Ok(())
}

/// After indexing a small in-process workspace, successful tool calls must set
/// `is_error: Some(false)` on the result.
#[tokio::test]
async fn successful_lookup_yields_is_error_false() -> anyhow::Result<()> {
    use mycelium_mcp::MyceliumServer;
    use rmcp::model::CallToolRequestParams;

    let (server_transport, client_transport) = tokio::io::duplex(131_072);

    let server = MyceliumServer::new();
    let server_handle = tokio::spawn(async move {
        let _r = server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });
    let client = TestClient.serve(client_transport).await?;

    // Inject two synthetic nodes directly via sync_file so we have a real
    // indexed symbol to look up without needing an actual filesystem.
    // `sync_file` accepts an absolute path; use a tempdir so the watch
    // loop doesn't race with us.
    let tmp = tempfile::tempdir()?;
    let src = tmp.path().join("a.py");
    std::fs::write(&src, b"def foo(): pass\n")?;

    // Index the temp dir first so the store is initialised.
    let index_result = client
        .call_tool(
            CallToolRequestParams::new("mycelium_index_workspace".to_string()).with_arguments(
                serde_json::json!({"path": tmp.path().to_str().unwrap()})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
        )
        .await?;
    // Indexing itself succeeds (is_error false or None, content non-empty).
    assert!(
        index_result.is_error != Some(true),
        "index_workspace returned is_error=true: {:?}",
        index_result.content
    );

    // Now look up a symbol that was just indexed.
    let callee_result = client
        .call_tool(
            CallToolRequestParams::new("mycelium_get_callees".to_string()).with_arguments(
                serde_json::json!({"path": "a.py>foo"})
                    .as_object()
                    .cloned()
                    .unwrap(),
            ),
        )
        .await?;

    client.cancel().await?;
    let _ = server_handle.await;

    assert_eq!(
        callee_result.is_error,
        Some(false),
        "mycelium_get_callees on a known symbol must return is_error=Some(false), \
         got {:?}. content: {:?}",
        callee_result.is_error,
        callee_result.content,
    );
    Ok(())
}
