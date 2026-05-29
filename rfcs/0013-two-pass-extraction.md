# RFC-0013 — Two-Pass Extraction for Accurate Intra-File Call Resolution

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0013                               |
| Status   | Implemented                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0002 (Extractor), RFC-0011 (Call graph) |

## Summary

Refactor `Extractor::extract` from a single-pass AST traversal to a two-pass
approach so that call edges resolve to definition nodes even when the callee
is defined after the caller in source order (forward references).

## Motivation

Single-pass extraction processes AST matches in document order. When a function
calls another function defined later in the same file, the callee lookup fails
and falls back to a bare stub node. The caller's `Calls` edge then points to a
stub instead of the actual definition node, breaking `mycelium_get_callers` for
those symbols.

This is very common in practice: JavaScript/TypeScript files often call hoisted
functions; Python files frequently have a `main()` call at the bottom that calls
helpers defined below.

## Design

Separate the single query run into two explicit passes:

**Pass 1 — Definitions only**:  
Run `QueryCursor` over all matches where `cap_name.starts_with("definition.")`.
Populate Trunk nodes and `Contains` edges. After pass 1, the store has all
symbols from the file.

**Pass 2 — References only**:  
Run `QueryCursor` again over all matches where `cap_name.starts_with("reference.")`.
Process `Imports`, `Calls`, and any future reference types. At this point all
definitions are already in the store, so intra-file callee lookup always succeeds.

### Implementation

The `QueryCursor` can be run twice since it borrows the tree and source
immutably. Total cost: two linear AST traversals per file (same asymptotic
complexity, small constant factor increase).

No public API changes — `Extractor::extract` signature is unchanged.

## Acceptance Criteria

- [x] Forward-reference call: `def foo(): bar()\ndef bar(): pass` — `foo -> Calls -> bar` using the definition node (not a stub).
- [x] All existing call-graph tests still pass.
- [x] `node_count` is identical between single-pass and two-pass for the same input.
- [x] All prior tests pass.
