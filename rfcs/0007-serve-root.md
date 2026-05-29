# RFC-0007 — `mycelium serve --mcp --root <path>` (stateful startup)

| Field      | Value                        |
|------------|------------------------------|
| RFC        | 0007                         |
| Title      | Stateful MCP serve with root |
| Status     | Implemented                        |
| Author     | Hive / rust-implementer      |
| Created    | 2026-05-29                   |

---

## Motivation

After RFC-0006, `mycelium index <path>` saves a snapshot to
`.mycelium/index.rmp`. However, `mycelium serve --mcp` always starts with an
empty store — an AI agent must call `mycelium_index_workspace` before any
search or traversal tool returns results. This is an unnecessary round-trip.

**Goal:** let users start the MCP server with an already-indexed codebase:

```bash
mycelium index .          # one-time: build + persist snapshot
mycelium serve --mcp --root .   # subsequent sessions: pre-loaded, ready immediately
```

---

## Design

### CLI change

Add an optional `--root <path>` argument to `Cmd::Serve`:

```
mycelium serve --mcp [--root <path>]
```

When `--root` is supplied:

1. Look for `<root>/.mycelium/index.rmp`.
2. If found: call `Store::load()`. Log `"Loaded index: N nodes from <snap>"`.
3. If not found: run a full `index_path(<root>)`, auto-save snapshot, log stats.

When `--root` is omitted: existing behaviour (empty store, no pre-loading).

### Library change: `MyceliumServer::with_root()`

New constructor:

```rust
pub async fn with_root(root: PathBuf) -> anyhow::Result<Self>;
```

Steps:
1. Compute `snap = root.join(".mycelium/index.rmp")`.
2. Try `Store::load(&snap)`.
   - Success → use loaded store; set `indexed_root`.
   - Error (file not found or corrupt) → run `index_path(&root)`, save snapshot, use new store.

### `serve_stdio` signature extension

```rust
pub async fn serve_stdio(root: Option<PathBuf>) -> anyhow::Result<()>;
```

Passes `root` to `with_root()` or falls back to `MyceliumServer::new()`.

---

## New tool: `mycelium_server_status`

Return current server state as JSON so clients can introspect without
calling an index tool:

```json
{
  "indexed_root": "/home/user/project",
  "node_count": 4231,
  "is_loaded": true
}
```

---

## Acceptance criteria

| # | Criterion |
|---|-----------|
| 1 | `MyceliumServer::with_root(path)` loads snapshot if `.mycelium/index.rmp` exists |
| 2 | `with_root` runs full index + saves snapshot when no `.rmp` is present |
| 3 | `serve_stdio(Some(root))` passes root through to `with_root` |
| 4 | `mycelium serve --mcp --root .` starts a pre-loaded server (CLI wiring) |
| 5 | `mycelium_server_status` tool returns `node_count`, `indexed_root`, `is_loaded` |
| 6 | All existing 128 tests continue to pass |

---

## Testing strategy

- Unit: `with_root` loads from fixture snapshot (built with `Store::save`).
- Unit: `with_root` falls back to live index when snapshot absent.
- Unit: `mycelium_server_status` returns correct counts before and after index.
- Integration: existing tests unaffected (all use `MyceliumServer::new()`).

---

## Non-goals

- File watching / incremental re-index (RFC-0008).
- Hot-reload on SIGHUP (future).
- Multiple roots (single root per server instance).
