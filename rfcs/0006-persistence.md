# RFC-0006: Index Persistence

| Field       | Value                                |
|-------------|--------------------------------------|
| RFC         | 0006                                 |
| Status      | Accepted                             |
| Author      | Hive (rust-implementer, orchestrator)|
| Date        | 2026-05-29                           |
| Supersedes  | (none)                               |
| Related     | RFC-0001, RFC-0004                   |

---

## 1. Motivation

Every `mycelium serve --mcp` session re-indexes the workspace from scratch.
For a 100k-file monorepo this takes tens of seconds; for the Linux kernel it
takes minutes. The fix is obvious: persist the index after building it and
reload it on subsequent sessions.

RFC-0001 deferred persistence to "P4". This is P4.

## 2. Goals

- `Store::save(path)` — write the in-memory graph to a binary snapshot file.
- `Store::load(path)` — reconstruct a `Store` from a snapshot file.
- CLI: `mycelium index` auto-saves to `<workspace>/.mycelium/index.rmp`.
- MCP: `mycelium_index_workspace` auto-saves after indexing.
- MCP: new `mycelium_load_index` tool loads a pre-built index without re-indexing.
- Wire format: MessagePack (`rmp-serde`) for compact, fast binary encoding.

## 3. Non-Goals

- Incremental / watch-mode updates (post-RFC-0006 follow-up).
- Snapshot versioning / migration (v0.1 snapshots are not guaranteed stable).
- Distributed or shared storage.
- Encryption of the snapshot.

## 4. Design

### 4.1 Wire Format

[MessagePack](https://msgpack.org/) via `rmp-serde`. Rationale:

- Already a workspace dependency.
- 5–10× smaller and 3–10× faster than JSON for binary-heavy data.
- Schemaless: adding new fields (edge kinds, node attributes) doesn't break
  existing readers in the same snapshot generation.

Snapshot path: `<workspace_root>/.mycelium/index.rmp`.

### 4.2 Type Derivations

All types that compose `Store` gain `#[derive(Serialize, Deserialize)]`:
`NodeId`, `EdgeKind`, `NodeKind`, `AdjacencyList`, `Synapse`, `Trunk`, `Store`.

`hashbrown` must be configured with its `serde` feature for `HashMap`
serialization.

### 4.3 `Store::save` / `Store::load`

```rust
impl Store {
    /// Write the store to `path` in MessagePack format.
    pub fn save(&self, path: &Path) -> anyhow::Result<()>

    /// Load a previously saved store from `path`.
    pub fn load(path: &Path) -> anyhow::Result<Store>
}
```

`save` creates parent directories if they don't exist.

### 4.4 CLI Auto-Save

`mycelium index <path>` calls `store.save(<path>/.mycelium/index.rmp)` after
a successful run and prints "Index saved to .mycelium/index.rmp".

### 4.5 MCP Tools

`mycelium_index_workspace` auto-saves after successful indexing (no change to
the response schema).

New tool `mycelium_load_index`:

```
Input:  { "path": "/abs/path/to/workspace" }
Output: { "nodes": N, "loaded_from": ".mycelium/index.rmp" }
        | { "error": "..." }
```

Loads the `.mycelium/index.rmp` snapshot instead of re-indexing. Lets
long-running MCP sessions reuse a previously built index on reconnect.

## 5. Acceptance Criteria

- `cargo test --all` passes.
- `cargo clippy --all-targets --all-features -- -D warnings` clean.
- Round-trip test: `store.save(path)` then `Store::load(path)` produces
  equal `Store` (same lookup results, same edge results).
- CLI test: `mycelium index <tmp>` creates `<tmp>/.mycelium/index.rmp`.
- MCP test: `mycelium_load_index` on a saved workspace returns correct node count.
