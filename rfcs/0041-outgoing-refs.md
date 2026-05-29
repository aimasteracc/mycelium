# RFC-0041 — `Store::outgoing_refs` + `mycelium_get_outgoing_refs` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0041                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0039 (cross_refs)              |

## Summary

Add `Store::outgoing_refs(id)` — all outgoing edges from a symbol grouped by
edge kind — and expose it as `mycelium_get_outgoing_refs`.  This is the
symmetric complement to RFC-0039 `cross_refs` / `mycelium_get_cross_refs`:

| Method | Direction | Question answered |
|---|---|---|
| `cross_refs` | incoming | Who references this symbol? |
| `outgoing_refs` | outgoing | What does this symbol reference? |

Together they give a complete reference picture for any symbol in one call
each.

## Design

### Store type

```rust
pub struct OutgoingRefs {
    pub callees: Vec<String>,
    pub imports: Vec<String>,
    pub extends: Vec<String>,
    pub implements: Vec<String>,
}
```

All lists sorted lexicographically.  Empty lists always present.

### Store method

```rust
pub fn outgoing_refs(&self, id: NodeId) -> OutgoingRefs
```

Collects outgoing `Calls`, `Imports`, `Extends`, `Implements` edges and
resolves them to path strings.

### MCP tool — `mycelium_get_outgoing_refs`

Request: `{ "path": "src/app.ts>App" }`

Response:
```json
{
  "callees": ["src/lib.ts>helper"],
  "imports": ["src/utils.ts>utils"],
  "extends": ["src/base.ts>Base"],
  "implements": []
}
```

Unknown path returns `{ "error": "path not found: ..." }`.

## Acceptance Criteria

- [ ] `OutgoingRefs` struct with `callees`, `imports`, `extends`, `implements`.
- [ ] All four lists sorted lexicographically.
- [ ] Empty lists present (not omitted).
- [ ] `mycelium_get_outgoing_refs`: known path returns `{ callees, imports, extends, implements }`.
- [ ] `mycelium_get_outgoing_refs`: unknown path returns `{ error }`.
- [ ] All prior tests pass.
