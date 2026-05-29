# RFC-0014 — Cross-File Call Stub Resolution

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0014                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0011 (Call graph), RFC-0013 (Two-pass extraction) |

## Summary

After a full workspace index, resolve bare call stub nodes (created when a
callee is defined in a different file) to their actual definition nodes.

## Motivation

`Extractor::extract` processes files in isolation. When `a.py` calls `bar()`
but `bar` is defined in `b.py`, Pass 2 cannot find `a.py>bar` in the store
(not yet indexed) and falls back to creating a bare stub node `bar`. After
RFC-0013 forward references within a file are resolved; cross-file references
still produce stubs.

**Consequences**:
- `mycelium_get_callers("b.py>bar")` returns an empty list even though
  `a.py>foo` calls it — the edge points to stub `bar`, not `b.py>bar`.
- The graph has duplicate representations: stub `bar` and definition `b.py>bar`
  are unrelated nodes holding contradictory semantics.

## Design

After all files are indexed, run a single post-processing pass:

### `Store::resolve_bare_call_stubs() -> usize`

1. Collect all "bare" nodes — paths with no `>` separator (single name segment).
2. For each bare node `stub` with name `name`:
   a. Search all nodes whose path ends with `>name` (i.e. last segment == `name`).
   b. If exactly 1 match (`def_id`):
      - Call `synapse.redirect_node(stub_id, def_id)` — rewires all edges.
      - Remove `stub` from `Trunk`.
      - Increment resolved counter.
   c. If 0 or 2+ matches: leave unchanged (unresolvable or ambiguous).
3. Return count of resolved stubs.

### `Synapse::redirect_node(from: NodeId, to: NodeId)`

For each `EdgeKind`:
- **Redirect incoming edges**: for each `src` in `reverse[from]`, remove `(src→from)` and upsert `(src→to)`.
- **Redirect outgoing edges**: for each `dst` in `forward[from]`, remove `(from→dst)` and upsert `(to→dst)`.
- Remove `from` from both `forward` and `reverse` maps.

### `Trunk::remove_node(id: NodeId)`

Remove the NodeId↔path mapping from both forward and reverse maps. (The
NodeId is derived deterministically from the path, so removal is safe.)

### Integration points

Call `store.resolve_bare_call_stubs()` after the full walk completes:
- **CLI** (`run_index` in `mycelium-cli`): after the walking loop.
- **MCP** (`run_index` on `MyceliumServer`): after the walking loop.
- **Watch mode**: after each batch of changed files.

The return value (resolved count) is exposed in `mycelium_index_workspace`
response as `"stubs_resolved"` for observability.

## Scope Limitation

This RFC resolves **unambiguous** stubs only (exactly one definition with the
matching name). Ambiguous stubs (multiple definitions with the same simple name
across files) require import-path analysis, which is out of scope here. Import
analysis is deferred to a future RFC.

## Acceptance Criteria

- [x] `a.py` calling `bar()` defined in `b.py`: after `resolve_bare_call_stubs`, `mycelium_get_callers("b.py>bar")` returns `["a.py>foo"]`.
- [x] Bare stub `bar` no longer exists as a separate node after resolution.
- [x] Ambiguous stubs (same name in multiple files) are left unchanged.
- [x] `stubs_resolved` field added to `mycelium_index_workspace` response.
- [x] All prior tests pass.
