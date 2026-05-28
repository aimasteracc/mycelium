# RFC-0004: MCP Server (stdio transport, v0.1 tools)

| Field       | Value                                |
|-------------|--------------------------------------|
| RFC         | 0004                                 |
| Status      | Accepted                             |
| Author      | Hive (rust-implementer, orchestrator)|
| Date        | 2026-05-29                           |
| Supersedes  | (none)                               |
| Related     | RFC-0001, RFC-0002, RFC-0003 (future)|

---

## 1. Motivation

Mycelium's value proposition is as an AI-first code intelligence graph. To
deliver that value, it must speak the Model Context Protocol (MCP) — the
standard interface that Claude Code, Cursor, and other AI agents use to
access external tools.

Without an MCP server, Mycelium is a CLI-only tool. With one, it becomes
a first-class tool in any AI coding workflow.

## 2. Goals

- Expose Mycelium's code graph over MCP stdio transport.
- Provide three tools: `index_workspace`, `search_symbol`, `get_ancestors`.
- Keep the server stateful within a session (in-memory Store).
- Launch via `mycelium serve --mcp` (already wired as a placeholder).

## 3. Non-Goals

- Persistent index storage (deferred to P4 persistence layer).
- Cross-session state sharing.
- Hyphae query language exposure (deferred to RFC-0003).
- HTTP/SSE transport (v0.2).
- Authentication or multi-tenancy.

## 4. Design

### 4.1 Transport

JSON-RPC 2.0 over stdin/stdout. The `rmcp` crate (`rmcp = "1.7"`) provides
the server runtime; Mycelium implements the tool handlers.

### 4.2 Server State

A single `ServerState` struct holds the indexed `Store` and the root path:

```rust
struct ServerState {
    store: Arc<RwLock<Store>>,
    indexed_root: Arc<RwLock<Option<PathBuf>>>,
}
```

### 4.3 Tools

#### `mycelium_index_workspace`

Index a directory and populate the in-memory store.

```
Input:  { "path": "<absolute or relative path>" }
Output: { "files": <n>, "errors": <n>, "languages": ["python", "typescript", "rust"] }
```

#### `mycelium_search_symbol`

Search for symbols by name prefix. Returns matching trunk paths.

```
Input:  { "query": "<prefix>", "limit": <n = 20> }
Output: { "matches": ["<path>", ...] }
```

#### `mycelium_get_ancestors`

Return the ancestor chain for a given trunk path (containment hierarchy).

```
Input:  { "path": "<trunk path, e.g. src/main.rs>greet>" }
Output: { "ancestors": ["<path>", ...] }
```

### 4.4 Server declaration

Use `rmcp`'s `#[tool]` and `ServerHandler` / `tool_router!` macros
(or equivalent API per rmcp 1.7.x) to declare tools.

The server is started from `mycelium-cli` via `Cmd::Serve { mcp: true }`,
which calls `mycelium_mcp::serve_stdio()`.

### 4.5 Concurrency model

`rmcp` drives a single tokio task per stdin line. The `Store` is wrapped in
`Arc<RwLock<>>` to support future concurrent access. For v0.1, the server
handles requests sequentially (single-threaded tokio runtime is fine).

## 5. Testing

- Unit tests for each tool handler using an in-memory Store.
- Integration test: spawn the server binary with `--mcp` and exchange
  `initialize` + `tools/list` + `tools/call` messages via piped stdin/stdout.

## 6. Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `rmcp` API breaks between versions | Pin to `rmcp = "1.7"` in workspace |
| Store bloat on large repos | Add `limit` parameter to search; document in-memory constraint |
| Slow index on first call | Accept for v0.1; async indexing is a v0.2 concern |

## 7. Acceptance Criteria

- [ ] `mycelium serve --mcp` launches without error.
- [ ] `tools/list` returns the 3 tools with correct schemas.
- [ ] `mycelium_index_workspace` populates the in-memory store.
- [ ] `mycelium_search_symbol` returns matching paths after indexing.
- [ ] `mycelium_get_ancestors` returns the containment chain.
- [ ] All quality gates pass (fmt/clippy/test).
