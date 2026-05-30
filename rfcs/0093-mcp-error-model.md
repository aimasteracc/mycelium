# RFC-0093: MCP Error Model — distinguish tool vs application errors

- **Status**: draft
- **Author(s)**: @aimasteracc (orchestrator dispatch)
- **Created**: 2026-05-30
- **Last updated**: 2026-05-30
- **Tracking issue**: #209 (umbrella #206)
- **Affected source paths**:
  - `crates/mycelium-mcp/src/lib.rs` — every `async fn mycelium_*` tool
  - `crates/mycelium-cli/src/queries.rs` — text-output helpers that mirror the MCP JSON shape

## Summary

Change every MCP tool's return type from `String` (current) to
`Result<CallToolResult, rmcp::Error>` (or rmcp's equivalent typed-error
mechanism). For application-level errors — "symbol not found", "index
empty", "path is unknown" — return a `CallToolResult` with
`is_error: Some(true)` plus a structured payload, so MCP callers can
branch on the typed flag instead of string-matching the response body.

## Motivation

### Current behaviour

Every tool returns `String` — the rmcp framework wraps it in
`CallToolResult::success(text)` automatically. Errors come back as
in-band JSON:

```rust
async fn mycelium_get_ancestors(&self, Parameters(req): Parameters<...>) -> String {
    let ancestors = self.store.read().await.ancestors_of_path(&req.path).unwrap_or_default();
    serde_json::json!({ "ancestors": ancestors }).to_string()
}

async fn mycelium_index_workspace(&self, ...) -> String {
    match ... {
        Err(e) => serde_json::json!({ "error": format!("task panicked: {e}") }).to_string(),
        Ok(Err(e)) => serde_json::json!({ "error": e.to_string() }).to_string(),
        Ok(Ok((store, files, errors, ...))) => serde_json::json!({ "files": files, ... }).to_string(),
    }
}
```

Result: from the agent's perspective, all 89 tool responses look
identical at the protocol layer:

```json
{"content": [{"text": "{...}"}]}
```

The only way to know whether a call failed is to parse the inner
text and look for an `"error"` key. Tool errors (wrong params,
unknown tool) and application errors (symbol not found, index empty)
are indistinguishable.

### Why this matters

Per the [MCP spec](https://spec.modelcontextprotocol.io/specification/server/tools/#error-handling):

- **Protocol-level errors** (wrong tool name, malformed params) → JSON-RPC
  `error` response. Agent should re-plan or surface to user.
- **Application-level errors** (operation succeeded but result is "not
  found" / "empty" / etc.) → `CallToolResult` with `is_error: true`.
  Agent should accept gracefully and continue.

Today every Mycelium error looks the same. An agent that hits `symbol
not found` cannot easily distinguish that from `index not loaded`
without parsing the text. This pushes brittle string-matching into
every consumer.

### Real-world signal (#206)

> "Both currently look like `{"content": [{"text": "{\"error\": \"...\"}"}]}` to
> the caller, requiring the agent to string-parse the error to know what went
> wrong."

## Detailed design

### Phased rollout

Phase 1 (this RFC's scope, target **v0.2.0**): change every tool's return
type + structured error payload. Backwards-incompatible at the protocol
layer for any caller that depended on the in-band `"error"` string.

Phase 2 (deferred, optional): emit additional structured metadata
(error categories: `not_found`, `not_indexed`, `not_supported`) — a
richer taxonomy.

### Return-type contract

```rust
// Before
async fn mycelium_get_ancestors(&self, Parameters(req): Parameters<GetAncestorsRequest>) -> String {
    let ancestors = self.store.read().await.ancestors_of_path(&req.path).unwrap_or_default();
    serde_json::json!({ "ancestors": ancestors }).to_string()
}

// After
async fn mycelium_get_ancestors(
    &self,
    Parameters(req): Parameters<GetAncestorsRequest>,
) -> Result<CallToolResult, rmcp::Error> {
    let store = self.store.read().await;
    match store.ancestors_of_path(&req.path) {
        Some(ancestors) => Ok(success_json(json!({ "ancestors": ancestors }))),
        None => Ok(application_error(json!({
            "found": false,
            "reason": "symbol not found",
            "path": req.path,
        }))),
    }
}
```

With helpers:

```rust
fn success_json(value: serde_json::Value) -> CallToolResult {
    CallToolResult {
        content: vec![Content::text(value.to_string())],
        is_error: Some(false),  // explicit for clarity
    }
}

fn application_error(value: serde_json::Value) -> CallToolResult {
    CallToolResult {
        content: vec![Content::text(value.to_string())],
        is_error: Some(true),
    }
}
```

### Error category vocabulary (Phase 1)

Three application-error categories cover ~95% of current sites:

| Category | When | Payload |
|---|---|---|
| `not_found` | path lookup returned no node | `{ "found": false, "reason": "symbol not found", "path": "..." }` |
| `not_indexed` | live store empty / never indexed | `{ "found": false, "reason": "index not loaded — run `mycelium index <root>` first" }` |
| `invalid_path` | TrunkPath::parse failed on user input | `{ "found": false, "reason": "invalid path syntax", "path": "...", "detail": "..." }` |

Anything else stays a `rmcp::Error::internal_error` (true tool error).

### Compact-mode interaction

`compact_mode` (msgpack-hex) wraps the SUCCESS payload only. Errors
always go through the text formatter so the agent can read them
without decoding hex.

### CLI mirror

`crates/mycelium-cli/src/queries.rs` produces the same JSON envelopes
for `--format=json` output. CLI should exit with non-zero status code
when MCP returns `is_error: true`, matching the existing convention
where `mycelium get-symbol-info unknown_path` exits 1.

### Test plan

- Per-tool: assert that the "happy path" returns `is_error: Some(false)`
  and the "not found" path returns `is_error: Some(true)`.
- New file `crates/mycelium-mcp/tests/error_model.rs` — table-driven
  test that walks every tool, calls it with an obviously-missing path,
  asserts the response carries `is_error: Some(true)`.
- This test is the contract-test foundation for #211; once #209 lands,
  #211 inherits the test harness shape and just adds the schema
  consistency checks (canonical key names).

## Drawbacks

- **Breaking change** for any consumer that pattern-matches on the
  current `"error"` string envelope. Mitigation: structured payload
  still includes a `"reason"` string that captures the same human-
  readable message, so primitive grep-style consumers continue to work
  if they switch from looking for `"error"` to `"reason"`.

- **~89 tools to refactor.** Mechanical but non-trivial. Best done as
  a single atomic PR so the contract change is reviewed once.

- **rmcp's exact error type** (`rmcp::Error` vs another) needs
  verification — the rmcp version in our Cargo.toml may have changed
  this signature.

## Alternatives

1. **Keep the string envelope, add a leading sentinel byte/marker**
   (`"\x01" + json` for errors, `"\x02" + json` for success). Rejected:
   non-standard, defeats the point of having `is_error` in the spec.

2. **Add a second tool family `mycelium_try_*` that returns
   `Option<value>`.** Rejected: doubles surface area for marginal
   value; breaks Three-Surface Rule (would need CLI twins).

3. **Status quo with better docs.** Rejected: agents systematically
   get the error class wrong. The cost of changing once outweighs
   ongoing miscategorisation cost.

## Acceptance criteria

- [ ] All 89 tools return `Result<CallToolResult, rmcp::Error>` (or
  rmcp's typed equivalent)
- [x] Every error path uses `is_error: Some(true)` instead of in-band
  JSON `"error"` strings (PR #266 — v0.1.11)
- [x] `success_json` / `application_error` / `success_str` helpers live in
  `crates/mycelium-mcp/src/error.rs` (Phase 1 v0.1.11; `success_str` v0.1.13)
- [ ] `tests/error_model.rs` contract test: every tool reachable via
  `list_tools()` returns `is_error: Some(true)` for a deliberately
  missing path
- [ ] CHANGELOG `[Unreleased]` BREAKING entry under v0.2.0
- [ ] Issue #209 closes; #211 (contract tests) follow-up rebases on
  this PR's harness

## Rollout plan

Single atomic PR per tool family (basic-queries first, then
call-graph, then reachability, etc.) — five small PRs, not one giant
89-tool change. Each carries the helper + the tool family + the
contract-test addition for that family. After all five land, the old
string-error pattern is gone everywhere.

Target release: **v0.2.0** (one of the headline breaking changes).
