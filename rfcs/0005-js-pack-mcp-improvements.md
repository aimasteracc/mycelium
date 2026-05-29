# RFC-0005: JavaScript Pack & MCP Improvements

| Field       | Value                                |
|-------------|--------------------------------------|
| RFC         | 0005                                 |
| Status      | Implemented                             |
| Author      | Hive (rust-implementer, orchestrator)|
| Date        | 2026-05-29                           |
| Supersedes  | (none)                               |
| Related     | RFC-0002, RFC-0004                   |

---

## 1. Motivation

RFC-0004 shipped the MCP server with three issues discovered post-merge:

1. **Server identity**: `ServerInfo` reports `{"name":"rmcp","version":"1.7.0"}` instead
   of `{"name":"mycelium-mcp","version":"0.0.1"}`. Clients use this for display and
   capability negotiation; the wrong name is confusing.
2. **Missing languages field**: `mycelium_index_workspace` returns `{"files":N,"errors":N}`
   but the RFC-0004 spec promised `{"files":N,"errors":N,"languages":[...]}`. Missing field
   breaks any client that relies on it.
3. **No JavaScript support**: JavaScript / JSX / TSX are the most common languages in
   frontend codebases. Mycelium can index Rust, Python, TypeScript, but not `.js`, `.jsx`,
   or `.tsx`.
4. **No descendants tool**: `mycelium_get_ancestors` lets agents traverse up the
   containment tree; a symmetric `mycelium_get_descendants` is needed to traverse down.

## 2. Goals

- Correct server identity in MCP handshake.
- Add `"languages"` field to `mycelium_index_workspace` response.
- Ship `packs/javascript/` (3 files: pack.toml + queries.scm; zero core changes).
- Wire `.jsx` and `.tsx` dispatch in CLI and MCP delivery layers.
- Add `mycelium_get_descendants` MCP tool.

## 3. Non-Goals

- JSX-specific semantic analysis (JSX elements are transparent to the symbol graph).
- Persistent index storage.
- Hyphae integration (RFC-0003).

## 4. Design

### 4.1 Server Identity

Override `get_info()` inside the `#[tool_handler]` impl block:

```rust
#[tool_handler]
impl ServerHandler for MyceliumServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
    }
}
```

### 4.2 `languages` Field

`run_index` already dispatches per-extension. Collect unique language names into a
`BTreeSet` during iteration and include them in the JSON return:

```json
{ "files": 12, "errors": 0, "languages": ["javascript", "rust", "typescript"] }
```

### 4.3 JavaScript Language Pack

New crate dependency: `tree-sitter-javascript = "0.23"` in workspace.

Pack files: `packs/javascript/pack.toml` + `packs/javascript/queries.scm`.

Query captures mirror the TypeScript pack: top-level functions, arrow functions, class
declarations, methods, import statements. JavaScript uses `program` as the root node
(same as tree-sitter-typescript).

Dispatch extensions: `.js`, `.jsx`. TSX uses the TypeScript extractor already loaded;
wired to `.tsx` in both CLI and MCP.

### 4.4 `mycelium_get_descendants`

New MCP tool symmetric to `mycelium_get_ancestors`:

```
Input:  { "path": "src/lib.rs" }
Output: { "descendants": ["src/lib.rs>greet", "src/lib.rs>helper"] }
```

Delegates to `Store::descendants_of_path(&self, path: &str) -> Option<Vec<String>>`.
Returns `None` (→ empty list on the wire) if the path is not in the index.

## 5. Acceptance Criteria

- `cargo test --all` passes (no regressions).
- `cargo clippy --all-targets --all-features -- -D warnings` clean.
- Smoke test: `echo '...' | mycelium serve --mcp` shows
  `"serverInfo":{"name":"mycelium-mcp","version":"0.0.1"}`.
- `mycelium_index_workspace` on a JS fixture returns `"languages":["javascript"]`.
- `mycelium_get_descendants` on `src/greet.rs` returns both nested symbols.
- All new code covered by tests.
