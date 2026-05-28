# RFC-0023 — `mycelium_get_imports` MCP Tool

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0023                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0002 (Extractor, Imports edges), RFC-0012 (get_callees/callers) |

## Summary

Add `mycelium_get_imports` — an MCP tool that returns both the direct
outgoing `Imports` edges (what does this node import?) and incoming
`Imports` edges (what imports this node?) for a given trunk path.

Completes the direct-neighbor query surface for all three populated edge
kinds: `Calls` (covered by RFC-0012 `get_callees`/`get_callers`),
`Contains` (covered by RFC-0005 `get_descendants`/`get_ancestors`), and
`Imports` (this RFC).

## Motivation

Agents performing module dependency analysis, refactoring impact
assessment, or dead-module detection need to know which files import a
given module and which modules a given file pulls in.  Currently there is
no way to query `Imports` edges from MCP.

## Design

### Request

```json
{ "path": "src/auth.rs" }
```

### Response (found)

```json
{
  "imports": ["os", "hashlib"],
  "imported_by": ["src/main.rs", "src/tests/auth_test.rs"]
}
```

Both lists are sorted lexicographically and contain resolved path strings
(or `"<unknown>"` if a NodeId has no materialized path).

### Response (unknown path)

```json
{ "error": "path not found: src/auth.rs" }
```

### Implementation

Two new Store helpers:

```rust
pub fn imports_of(&self, id: NodeId) -> Vec<String>     // outgoing Imports
pub fn imported_by(&self, id: NodeId) -> Vec<String>    // incoming Imports
```

Both resolve NodeIds to path strings via `path_of`, sort, and return.

## Acceptance Criteria

- [ ] `imports` contains all outgoing `Imports` edge targets for the path.
- [ ] `imported_by` contains all incoming `Imports` edge sources for the path.
- [ ] Both lists sorted lexicographically.
- [ ] Unknown path returns `{ error }`.
- [ ] All prior tests pass.
