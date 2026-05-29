# RFC-0045 — `Store::siblings` + `mycelium_get_siblings` Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0045                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0001 (Trunk), RFC-0005 (descendants) |

## Summary

Add `Store::siblings(id)` — return all sibling paths (other direct children
of the same parent in the containment tree) — and expose it as
`mycelium_get_siblings`.

Answers: "what other symbols share the same container as this one?"
  - `src/app.rs>App>render` → siblings are other methods of `App`
  - `src/app.rs>App` → siblings are other top-level symbols in `src/app.rs`
  - A file node → siblings are other file nodes in the same directory

Results sorted lexicographically.  If the node has no parent (root),
returns an empty list.

## Design

### Store method

```rust
pub fn siblings(&self, id: NodeId) -> Vec<String>
```

1. Look up the `TrunkPath` for `id`.
2. If the path has a parent, collect all children of that parent from the
   trunk, excluding `id` itself.
3. Sort lexicographically and return as path strings.

### MCP tool — `mycelium_get_siblings`

Request: `{ "path": "src/app.rs>App>render" }`

Response:
```json
{
  "siblings": ["src/app.rs>App>init", "src/app.rs>App>destroy"],
  "count": 2
}
```

Unknown path returns `{ "error": "path not found: ..." }`.
Root node (no parent) returns `{ "siblings": [], "count": 0 }`.

## Acceptance Criteria

- [ ] `Store::siblings(id)` returns direct siblings (other children of the same parent).
- [ ] The node itself is excluded from results.
- [ ] Root nodes (no parent) return empty `Vec`.
- [ ] Results sorted lexicographically.
- [ ] `mycelium_get_siblings`: known path returns `{ siblings, count }`.
- [ ] `mycelium_get_siblings`: unknown path returns `{ error }`.
- [ ] All prior tests pass.
