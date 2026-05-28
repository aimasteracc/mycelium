# RFC-0015 — Watch-Mode Stub Resolution

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0015                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0008 (Watch mode), RFC-0014 (Cross-file stub resolution) |

## Summary

Call `Store::resolve_bare_call_stubs()` at the end of each watch-mode
batch so that cross-file call edges stay accurate after incremental
re-indexes.

## Motivation

RFC-0014 shipped `resolve_bare_call_stubs()` and wires it into full
workspace indexes (CLI and MCP `mycelium_index_workspace`). However, the
background FSE watch loop processes changed files one by one without a
final resolution pass. After a batch of edits:

- A newly-added function `bar()` in `b.py` creates a definition node
  `b.py>bar`, but the existing bare stub `bar` (created when `a.py` was
  first indexed and called `bar()`) is never resolved.
- Existing bare stubs from incremental updates are not resolved until
  the next full `mycelium_index_workspace` call.

## Design

In `MyceliumServer::start_watch`, the batch-processing loop ends with:

```rust
store.resolve_bare_call_stubs();
```

This adds one linear-time scan after every debounced batch. The cost is
O(N) over all bare stubs (typically a tiny fraction of nodes), and is
bounded by the number of unique simple callee names used across the
workspace.

The `batches_processed` counter returned by `mycelium_watch_status` is
unchanged; no new MCP tools are added.

## Acceptance Criteria

- [ ] After watch-mode processes `b.py` (adding `def bar(): pass`), calling
  `mycelium_get_callers("b.py>bar")` returns the callers from `a.py` even
  without a fresh `mycelium_index_workspace`.
- [ ] All prior tests pass.
- [ ] No new clippy warnings.
