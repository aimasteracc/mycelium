---
name: index-management
description: Build, reload, and maintain the Mycelium symbol index — the prerequisite for every other Skill.
allowed-tools:
  - mcp__mycelium__index_workspace
  - mcp__mycelium__load_index
  - mcp__mycelium__server_status
  - mcp__mycelium__watch_status
  - mcp__mycelium__sync_file
  - mcp__mycelium__set_compact_mode
  - mcp__mycelium__get_token_stats
  - mcp__mycelium__subscribe
  - mcp__mycelium__unsubscribe
  - mcp__mycelium__subscription_status
  # CLI surface variant of watch (RFC-0105 Three-Surface EXCEPTION:
  # a foreground CLI watch vs the server's background start/stop/status is a
  # documented lifecycle mismatch; both drive the same `WatchEngine`).
  # `mycelium watch --subscribe <SPEC>` is the CLI surface for RFC-0107
  # (extends the RFC-0105 EXCEPTION — same `subscription::match_batch` code
  # path; wire shape byte-identical to MCP, asserted by
  # `tests/contract_subscription`).
  - cli:mycelium-watch
category: operations
icon: 🗃️
marketplace_examples:
  - query: "Index this workspace"
    tool: index_workspace
  - query: "Check the file watcher status"
    tool: watch_status
  - query: "Sync a single changed file"
    tool: sync_file
  - query: "Get token usage stats"
    tool: get_token_stats
  - query: "Switch to compact output mode"
    tool: set_compact_mode
---

# `index-management` — keeping the symbol graph fresh

This Skill bundles the seven lifecycle and configuration tools. Reach for it any time the agent needs to:

- Bootstrap the index for a project it has not yet seen.
- Reload a persisted snapshot instead of re-indexing from scratch.
- Check server readiness before issuing queries.
- Reactively refresh a single file after an edit.
- Tune output verbosity (compact MessagePack vs verbose JSON).

Every other Skill depends on a loaded index. When `server_status` returns `"is_loaded": false`, call `index_workspace` or `load_index` first.

## When to invoke this Skill

Use **when**:

- The agent is starting a new session and needs to bootstrap the index.
- `server_status` shows `"is_loaded": false` or `"node_count": 0`.
- The agent just wrote a file and needs fresh query results within Charter §2's 10 ms reactive SLA.
- The agent wants to verify that token-efficiency is within the ≤ 30 % Charter §2 SLA.
- Diagnosing stale results by checking `watch_status`.

Do **NOT** use when:

- The index is already loaded and you just want to query it — see `basic-queries`, `call-graph`, etc.
- You need to query the file system without going through the index — this is not a file tool.

## Quick examples

| Developer question | Tool |
|---|---|
| "Index this workspace" | `mcp__mycelium__index_workspace` |
| "Check the file watcher status" | `mcp__mycelium__watch_status` |
| "Sync a single changed file" | `mcp__mycelium__sync_file` |
| "Get token usage stats" | `mcp__mycelium__get_token_stats` |
| "Switch to compact output mode" | `mcp__mycelium__set_compact_mode` |

## Capabilities under this umbrella

### `index_workspace` — build the index from source

```
mcp__mycelium__index_workspace({ "path": "/path/to/project" })
→ { "files": 143, "errors": 0, "languages": ["rust", "python", "typescript"] }
```

Walks the workspace, parses every supported source file with tree-sitter, builds the Trunk + Synapse storage layers, and auto-saves a `.mycelium/index.rmp` snapshot alongside the project.

### `load_index` — reload a saved snapshot

```
mcp__mycelium__load_index({ "path": "/path/to/project" })
→ { "nodes": 926, "loaded_from": ".mycelium/index.rmp" }
```

Reads the `.mycelium/index.rmp` written by `index_workspace`. Much faster than re-indexing; use when the codebase hasn't changed since the last session.

### `server_status` — readiness check

```
mcp__mycelium__server_status({})
→ { "node_count": 926, "edge_count": 3748, "indexed_root": "/path/to/project", "is_loaded": true }
```

Returns node/edge counts and the indexed root. Always check `"is_loaded": true` before querying. If false, call `index_workspace` or `load_index`.

### `watch_status` — reactive watcher diagnostics

```
mcp__mycelium__watch_status({})
→ { "watching": true, "root": "/path/to/project", "batches_processed": 7 }
```

Reports whether the file-watch loop is active and how many change batches have been processed. Use when query results look stale despite recent file edits.

### `mycelium/graphChanged` — server-initiated push notification (RFC-0106)

Whenever the watch loop commits a batch (i.e. one or more watched files have been re-indexed), the MCP server fires **one** notification per batch with this method name. **Register a handler** in your MCP client to react without polling.

```jsonc
// Notification shape (v1, frozen):
{
  "method": "mycelium/graphChanged",
  "params": {
    "event": "graphChanged",
    "v": 1,
    "root": "/abs/path/to/workspace",
    "batch_seq": 17,             // monotonic; detect dropped batches
    "changed_files": ["src/auth.rs", "src/db/query.rs"],
    "changed_count": 2,
    "truncated": false,          // true when changed_count > 50
    "hint": "Re-query mycelium_context for the area you care about."
  }
}
```

`changed_files` is capped at 50. When the underlying batch had more, `truncated` flips true and `changed_count` reports the real magnitude — react by broadly re-querying instead of trying to enumerate every affected file. Delivery is best-effort; if your client dropped or never registered the handler, the server logs and continues (zero impact on indexing).

### `mycelium/subscriptionDelta` — per-subscription scoped notification (RFC-0107)

Where `mycelium/graphChanged` (RFC-0106) broadcasts one notification per batch with the changed file list, **SUBSCRIBE** lets an agent register an **Interest** (Files / Symbols / Hyphae selector) and receive only the matching slice of each batch — as added / modified / removed trunk paths per file. Three new tools manage the in-memory subscription map; one new notification method delivers the matched payloads.

```jsonc
// Notification shape (v1, frozen):
{
  "method": "mycelium/subscriptionDelta",
  "params": {
    "event": "subscriptionDelta",
    "v": 1,
    "subscription_id": "f3c1...",
    "root": "/abs/path/to/workspace",
    "batch_seq": 42,
    "per_file": [
      {
        "file": "src/auth.rs",
        "added": ["src/auth.rs>fn:login"],
        "added_count": 1,
        "added_truncated": false,
        "modified": [],
        "modified_count": 0,
        "modified_truncated": false,
        "removed": ["src/auth.rs>fn:legacy_signin"],
        "removed_count": 1,
        "removed_truncated": false
      }
    ],
    "files_truncated": false,
    "interest_kind": "files",
    "hint": "Apply the delta locally or re-query the affected paths."
  }
}
```

```
mcp__mycelium__subscribe({
  "interest": { "kind": "files", "paths": ["src/auth/**/*.rs"] },
  "ttl_seconds": 3600
})
→ { "subscription_id": "f3c1...", "root": "/abs/path", "ttl_seconds": 3600,
    "interest_kind": "files", "active_count": 1 }
```

`interest` is a tagged union (mutually exclusive): `{"kind":"files","paths":[...]}` | `{"kind":"symbols","paths":[...]}` | `{"kind":"selector","hyphae":"..."}`. The server enforces caps (256 server-wide, 32 per-client, 64 Selector-specific) and a rolling TTL (default 3600s, max 86400s, bumped on every successful delivery). `mycelium_unsubscribe` is idempotent — unknown ids return `{removed: false}`. `mycelium_subscription_status` returns the active subscription list plus the configured caps for visibility.

Per-array cap = 50; `per_file` cap = 16 entries (above which `files_truncated: true` flips). All arrays are sorted + deduped before send. Selector removals follow the **(ii-strict)** policy — a removal is reported only when the path was in the OLD match-set AND its file was touched this batch — so unrelated state flips never produce phantom removals.

CLI surface (RFC-0105 Three-Surface EXCEPTION, extended): `mycelium watch --subscribe '<SPEC>'` registers the same Interest and streams identical NDJSON payloads to stdout. SPEC = `files:<glob1>,<glob2>,...` | `symbols:<glob1>,<glob2>,...` | `selector:<hyphae source>`. The MCP + CLI wire shapes are byte-identical by construction (both surfaces share `subscription::match_batch`); asserted by `tests/contract_subscription`.

### `sync_file` — immediate single-file re-index

```
mcp__mycelium__sync_file({ "path": "/path/to/project/src/auth.rs" })
→ { "path": "src/auth.rs", "symbols_before": 12, "symbols_after": 14, "elapsed_us": 843 }
```

Bypasses the watch debounce and re-indexes one file immediately. Use right after writing a file to satisfy the Charter §2 reactive <10 ms SLA. Returns symbol delta so you can confirm the edit was picked up.

### `set_compact_mode` — switch to MessagePack output

```
mcp__mycelium__set_compact_mode({ "enabled": true })
→ { "compact_mode": true, "message": "compact MessagePack hex output enabled" }
```

When enabled, tool responses arrive as `{ "fmt": "msgpack_hex", "data": "<hex>" }` instead of plain JSON, reducing AI token consumption to ≤ 30 % of the JSON equivalent (Charter §2 SLA). Enable when indexing large codebases. Disable when you need human-readable output.

### `get_token_stats` — verify the token-efficiency SLA

```
mcp__mycelium__get_token_stats({})
→ { "sample_query": "3 symbols", "json_bytes": 128, "msgpack_bytes": 54, "ratio": 0.42 }
```

Computes byte ratio of MessagePack vs JSON on a fixed sample payload. Use to verify the Charter §2 ≤ 30 % token-efficiency SLA is satisfied for this deployment.

## RFC-0097 — filesystem boundary enforcement (v0.1.12)

`index_workspace` and `load_index` enforce a path allowlist when the MCP
server is launched via `mycelium serve --mcp`. Every path argument is
canonicalised and checked against the list of allowed roots before any
filesystem access occurs.

| Scenario | Behaviour |
|---|---|
| Path is under an allowed root | Normal operation |
| Path is outside every allowed root | Rejected immediately — `is_error: true`, no filesystem access |
| Path traversal attempt (`../../etc`) | Rejected after canonicalisation |
| No `--allowed-roots` flag given | Defaults to the current working directory only |
| Empty allowlist (unit-test mode) | Unrestricted — backward-compatible with direct API use |

When a call is rejected the tool response is:

```json
{ "is_error": true, "content": [{ "type": "text", "text": "path ... is outside allowed roots" }] }
```

The agent should present this to the user as a configuration problem, not a
code error.

## Common chains

- **"Set up a new session"** → `server_status` (check is_loaded) → if false: `load_index` or `index_workspace`.
- **"Get fresh results after editing"** → write file → `sync_file` → query.
- **"Diagnose stale results"** → `watch_status` (check batches_processed) → `sync_file` to force refresh.
- **"Reduce token cost on large codebases"** → `set_compact_mode({ "enabled": true })` → `get_token_stats` to verify.
- **"index_workspace / load_index rejected with is_error"** → verify the target path is under `--allowed-roots`; restart the server with the correct roots if needed.

## Equivalent CLI

```bash
mycelium index /path/to/project            # index_workspace
mycelium server-status                     # server_status
mycelium sync-file src/auth.rs             # sync_file

# Start the MCP server (RFC-0097: defaults allowed root to CWD)
mycelium serve --mcp

# Start the MCP server with an explicit set of allowed roots
mycelium serve --mcp --root /project --allowed-roots /project /tmp/scratch

# Multiple allowed roots (flag may be repeated)
mycelium serve --mcp --allowed-roots /workspace --allowed-roots /tmp/scratch
```

## Parity contract

Per [RFC-0090](../../rfcs/0090-cli-mcp-skill-parity.md): each CLI ↔ MCP pair is byte-identical in name, description, argument schema, and JSON output. CLI subcommands `load-index`, `watch-status`, `set-compact-mode`, and `get-token-stats` are in the parity-backfill epic (v0.1.4).

The `--allowed-roots` security boundary (RFC-0097, Issue #301, v0.1.12) is a server-launch flag; it has no MCP tool equivalent — it is intentionally configured at startup, not at call time.

## Cross-references

- Every other Skill requires a loaded index — this Skill is the entry gate.
- `basic-queries` — the first query Skill to reach for once the index is loaded.
- Implementation: `crates/mycelium-mcp/src/lib.rs` (search for `mycelium_index_workspace`, etc.).
