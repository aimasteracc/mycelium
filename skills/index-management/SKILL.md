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
