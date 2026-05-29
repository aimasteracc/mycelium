# RFC-0026 — `mycelium_get_extends` and `mycelium_get_implements` MCP Tools

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0026                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0023 (get_imports) |

## Summary

Add `mycelium_get_extends` and `mycelium_get_implements` — two MCP tools that
expose the existing `EdgeKind::Extends` and `EdgeKind::Implements` synapse edges.
Mirrors the design of `mycelium_get_imports` (RFC-0023) exactly.

## Motivation

`EdgeKind::Extends` (class inheritance / trait bounds) and
`EdgeKind::Implements` (class-implements-interface) are already tracked by the
synapse layer. Without these tools, agents analysing OOP hierarchies must
call `mycelium_get_symbol_info` and infer relationships by inspecting caller/callee
trees, which is neither accurate nor efficient for inheritance analysis.

## Design

### `mycelium_get_extends`

Exposes:
- `extends`: symbols this path *directly* extends (outgoing `Extends` edges).
- `extended_by`: symbols that extend this path (incoming `Extends` edges).

#### Request

```json
{ "path": "src/shapes.py>Rectangle" }
```

#### Response (found)

```json
{
  "extends":     ["src/shapes.py>Shape"],
  "extended_by": ["src/shapes.py>Square"]
}
```

Both lists sorted lexicographically.

#### Response (unknown path)

```json
{ "error": "path not found: src/shapes.py>Rectangle" }
```

---

### `mycelium_get_implements`

Exposes:
- `implements`: symbols this path *directly* implements (outgoing `Implements` edges).
- `implemented_by`: symbols that implement this path (incoming `Implements` edges).

#### Request

```json
{ "path": "src/io.ts>FileReader" }
```

#### Response (found)

```json
{
  "implements":    ["src/io.ts>IReader"],
  "implemented_by": []
}
```

#### Response (unknown path)

```json
{ "error": "path not found: src/io.ts>FileReader" }
```

### Implementation

#### Store methods (no new store methods needed)

Re-use `Store::outgoing(id, EdgeKind::Extends)` / `Store::incoming(id, EdgeKind::Extends)`
and the `Implements` equivalents. Wrap into sorted `Vec<String>` in the MCP layer
exactly as RFC-0023 does for Imports.

#### MCP request structs

```rust
pub struct GetExtendsRequest    { pub path: String }
pub struct GetImplementsRequest { pub path: String }
```

#### MCP tools

```rust
async fn mycelium_get_extends(&self, ...)    -> String { ... }
async fn mycelium_get_implements(&self, ...) -> String { ... }
```

### Limits

None — these are simple edge lookups with no traversal.

## Acceptance Criteria

- [x] `mycelium_get_extends`: found path returns `{ extends, extended_by }` (both sorted).
- [x] `mycelium_get_extends`: unknown path returns `{ error }`.
- [x] `mycelium_get_implements`: found path returns `{ implements, implemented_by }` (both sorted).
- [x] `mycelium_get_implements`: unknown path returns `{ error }`.
- [x] Both lists are empty (not omitted) when no edges of that kind exist.
- [x] All prior tests pass.
