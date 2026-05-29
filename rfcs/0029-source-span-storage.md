# RFC-0029 — Source Span Storage + `mycelium_get_source_span` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0029                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0028 (NodeKind storage), RFC-0002 (Extractor) |

## Summary

Persist the tree-sitter `SourceSpan` (start/end line, column, byte offset)
alongside each node in the `Store` and expose a new MCP tool
`mycelium_get_source_span` that returns the exact source location of any
indexed symbol.

## Motivation

Agents that want to "open this symbol in an editor" or "read the
source of this function" have no way to locate it without re-parsing the
file. Storing spans during extraction is free (tree-sitter already
computes them) and unlocks editor-navigation and inline-source queries
without re-reading any file.

## Design

### `SourceSpan` serde

`SourceSpan` in `types.rs` currently lacks `Serialize`/`Deserialize`.
Add them so the `span_map` persists to the MessagePack snapshot:

```rust
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct SourceSpan { … }
```

### Store changes

```rust
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Store {
    trunk: Trunk,
    synapse: Synapse,
    kind_map: HashMap<NodeId, NodeKind>,
    span_map: HashMap<NodeId, SourceSpan>,   // ← new
}
```

New methods:
```rust
pub fn set_span(&mut self, id: NodeId, span: SourceSpan)
pub fn span_of(&self, id: NodeId) -> Option<SourceSpan>
```

`remove_node` and `remove_file` must also call `self.span_map.remove(&id)`.

### Extractor changes

After `store.upsert_node` and `store.set_kind` in each branch:

- **File node**: use `root.start_position()` / `root.end_position()` /
  `root.start_byte()` / `root.end_byte()`.
- **Definition nodes**: use the capture node's position and byte range.

Tree-sitter `Point.row` is 0-indexed; store as `row + 1` in
`start_line`/`end_line` (1-indexed, matching LSP convention). Columns
remain 0-indexed.

### MCP request struct

```rust
pub struct GetSourceSpanRequest { pub path: String }
```

### MCP tool — `mycelium_get_source_span`

Request: `{ "path": "src/auth.rs>login" }`

Response (found + span known):
```json
{
  "path": "src/auth.rs>login",
  "start_line": 42,
  "start_col": 0,
  "end_line": 55,
  "end_col": 1,
  "start_byte": 1024,
  "end_byte": 1280
}
```

Response (found, span unknown):
```json
{ "path": "src/auth.rs>login", "span": null }
```

Response (not found):
```json
{ "error": "path not found: src/auth.rs>login" }
```

## Acceptance Criteria

- [x] `SourceSpan` derives `Serialize` + `Deserialize`.
- [x] `Store::set_span` stores a span; `span_of` retrieves it.
- [x] `span_map` is cleaned up by `remove_node` and `remove_file`.
- [x] Extractor populates spans for file, function, class, method, and other definition nodes.
- [x] `mycelium_get_source_span`: known path + span returns the six fields.
- [x] `mycelium_get_source_span`: known path, span unknown returns `{ path, span: null }`.
- [x] `mycelium_get_source_span`: unknown path returns `{ error }`.
- [x] All prior tests pass.
