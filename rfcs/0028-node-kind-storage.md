# RFC-0028 — NodeKind Storage + Symbol Kind Query Tools

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0028                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0001 (Store design), RFC-0002 (Extractor) |

## Summary

Persist `NodeKind` alongside each node in the `Store` and expose two new MCP
tools — `mycelium_get_node_kind` (query the kind of a single path) and
`mycelium_get_symbols_by_kind` (enumerate all symbols of a given kind).

## Motivation

The `NodeKind` enum has been defined since the beginning but was never stored
anywhere reachable at query time. Agents that want to "find all classes" or
"find all functions" currently have no way to do so — `mycelium_search_symbol`
returns any matching name regardless of kind, and there is no filter.

Storing kind during extraction is zero-cost at query time (a single `HashMap`
lookup) and unlocks queries that would otherwise require full source re-parsing.

## Design

### Store changes

```rust
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Store {
    trunk: Trunk,
    synapse: Synapse,
    kind_map: HashMap<NodeId, NodeKind>,   // ← new
}
```

New methods:
```rust
pub fn set_kind(&mut self, id: NodeId, kind: NodeKind)
pub fn kind_of(&self, id: NodeId) -> Option<NodeKind>
pub fn symbols_of_kind(&self, kind: NodeKind, prefix: Option<&str>) -> Vec<String>
```

`symbols_of_kind` iterates `kind_map`, filters by `kind`, optionally filters
by `prefix`, looks up the path string, and returns sorted paths.

### Extractor changes

After `store.upsert_node(path)` in Pass 1, call `store.set_kind(id, kind)`
using this mapping from capture-name suffix:

| Capture suffix | NodeKind |
|---|---|
| `module` | `File` (for the file node) |
| `mod` | `Module` |
| `function` | `Function` |
| `class` | `Class` |
| `method` | `Method` |
| `interface` | `Interface` |
| `trait` | `Interface` |
| `type_alias` | `TypeAlias` |
| `const` | `Constant` |
| `struct` | `Struct` |
| `enum` | `Enum` |
| other | `NodeKind::try_from_wire(suffix)`, skip if `None` |

### MCP request structs

```rust
pub struct GetNodeKindRequest { pub path: String }
pub struct GetSymbolsByKindRequest { pub kind: String, pub path_prefix: Option<String> }
```

### MCP tools

#### `mycelium_get_node_kind`

Request: `{ "path": "src/auth.rs>login" }`

Response (found + kind known): `{ "path": "src/auth.rs>login", "kind": "function" }`

Response (found, kind unknown): `{ "path": "src/auth.rs>login", "kind": null }`

Response (not found): `{ "error": "path not found: ..." }`

#### `mycelium_get_symbols_by_kind`

Request: `{ "kind": "class", "path_prefix": "src/" }`

Response: `{ "symbols": ["src/auth.rs>AuthService", ...] }` — sorted lexicographically.

`kind` must be a valid wire string (e.g. `"function"`, `"class"`, `"method"`).
Unknown kind returns `{ "error": "unknown kind: ..." }`.

## Acceptance Criteria

- [x] `Store::set_kind` stores kind; `kind_of` retrieves it.
- [x] `Store::symbols_of_kind` returns only matching paths, sorted.
- [x] `Store::symbols_of_kind` with `prefix` filters correctly.
- [x] Extractor populates `kind_map` for file, function, class, method, and other definition captures.
- [x] `mycelium_get_node_kind`: known path+kind returns `{ path, kind }`.
- [x] `mycelium_get_node_kind`: known path, unknown kind returns `{ path, kind: null }`.
- [x] `mycelium_get_node_kind`: unknown path returns `{ error }`.
- [x] `mycelium_get_symbols_by_kind`: returns all matching symbols sorted.
- [x] `mycelium_get_symbols_by_kind`: `path_prefix` filter works.
- [x] `mycelium_get_symbols_by_kind`: unknown kind returns `{ error }`.
- [x] All prior tests pass.
