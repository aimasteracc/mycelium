# RFC-0008 — Watch Mode (Incremental Re-indexing)

| Field      | Value                          |
|------------|--------------------------------|
| RFC        | 0008                           |
| Title      | Watch mode — incremental index |
| Status     | Draft                          |
| Author     | Hive / rust-implementer        |
| Created    | 2026-05-29                     |

---

## Motivation

After RFC-0007 the MCP server can pre-load an index at startup.  However the
index then **drifts** as the user edits their codebase.  An AI agent relying on
stale symbol data will give wrong answers.

Watch mode solves this by listening for file-system events and incrementally
re-indexing changed files in the background.  The server never has to be
restarted; the in-memory `Store` is kept coherent automatically.

---

## Design

### Crate additions

Add `notify = "6"` (stable cross-platform FSE library) to the workspace and to
`mycelium-mcp`.

### `MyceliumServer::start_watch(root)`

An async method that:

1. Spawns a `notify::RecommendedWatcher` pointed at `root`.
2. Relays events through a `tokio::sync::mpsc` channel.
3. Debounces events: coalesces events within a 300 ms window so that a
   `git checkout` of 500 files doesn't fire 500 re-index operations.
4. For each coalesced batch of changed paths:
   - Calls `store.remove_file(rel_path)` for each changed or deleted path.
   - For modified/created paths that match a known extension: re-extracts.
   - Saves a new `.mycelium/index.rmp` snapshot after each batch.

### `MyceliumServer::with_root` integration

After pre-loading the store, `with_root` calls `start_watch` so the watch
loop is active from the moment the server is ready.

### Shutdown

The watcher is owned by a `JoinHandle` stored in the server struct.  When the
`MyceliumServer` is dropped (or `serve_stdio` returns), the channel is closed
and the watcher task exits.

---

## New tool: `mycelium_watch_status`

Returns whether the watch loop is running, the root, and the number of batches
processed since startup.  Complements `mycelium_server_status`.

```json
{
  "watching": true,
  "root": "/home/user/project",
  "batches_processed": 3
}
```

---

## Acceptance criteria

| # | Criterion |
|---|-----------|
| 1 | `start_watch(root)` returns a `JoinHandle` and begins watching `root` |
| 2 | Modifying a file triggers re-index of that file within ~500 ms |
| 3 | Deleting a file removes its nodes from the store |
| 4 | Adding a new file inserts its nodes into the store |
| 5 | `mycelium_watch_status` returns `watching: true` when watch is active |
| 6 | `with_root` automatically starts the watch loop |
| 7 | All existing 131 tests continue to pass |

---

## Testing strategy

- Unit: mock watcher events → verify `Store` mutations.
- Integration: write/delete real temp files, poll store state.
- Debounce: inject rapid events, confirm single re-index.

---

## Non-goals

- `.gitignore` / `.myceliumignore` filtering (RFC-0009).
- Watching multiple roots (single root per server).
- Network file-system reliability (best-effort on NFS/SMB).
