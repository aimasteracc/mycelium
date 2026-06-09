# RFC-0122: Phase 2b — Function-return-type receiver inference (rule f)

- **Status**: Draft
- **Created**: 2026-06-09
- **Revised**: 2026-06-09 (v2 — architect review, PM dispatch v149/v150)
- **Author**: orchestrator (PM dispatch v148); revised by architect (v149)
- **Depends on**: RFC-0118 (Phase 2a — `resolve_call_site_contexts`, `ReceiverContext`,
  `infer_receiver_type` a–e already on develop)
- **Tracked by**: Issue #612 Item 1
- **Blocked by**: PR #568 back-merge to develop (branch baseline prerequisite)

---

## Summary

`infer_receiver_type` (RFC-0118) has rules a–e covering `self`, param annotations,
constructor locals, field annotations, and import aliases.  Rule f is the **one
missing case**: a local variable whose RHS is a plain function call rather than a
constructor.

```rust
// rule c fires  (ctor_type = Some("Store"))
let s = Store::new();
s.upsert_node();       // ✅ resolved by RFC-0118

// rule f MISSING  (ctor_type = None; fn_call_hint = Some("get_store"))
let s = get_store();
s.upsert_node();       // ❌ returns None → bare stub → callers 0
```

This RFC adds rule f as a **pure-resolver extension** — no new redb table, no
schema migration, no new store pass.  The change is:

1. Extend `LocalBinding` with `fn_call_hint: Option<String>` — captured at
   parse time when the RHS is a function call (not a constructor).
2. Add a pre-enrichment step in `resolve_call_site_contexts()` that looks up
   `fn_call_hint` in the store to find the callee's declared return type, then
   synthesises a `ctor_type` value on a cloned context.
3. `infer_receiver_type` rule c fires on the enriched context — no change to
   the pure function itself.

---

## v1 → v2 revision rationale

The v1 Draft proposed a new `TABLE_CALL_SITE_CONTEXT` persisted in redb, a new
`CallSiteContext` struct with codec, a `Store::resolve_deferred_call_sites()` pass,
and watch-engine integration.

**Architect review (PM v149) found that this is over-engineered:**

- The existing `call_site_contexts: Vec<ReceiverContext>` field collected by the
  extractor **already is** the deferred per-call-site mechanism; `resolve_call_site_contexts()`
  **already is** the post-merge pass.
- The actual gap is narrower: `infer_receiver_type` returns `None` for
  function-call initialisers because `LocalBinding.ctor_type` is always `None`
  for `let s = get_store()`.
- No new redb table is needed.  The fix is a store-aware pre-enrichment step in
  the *caller* of `infer_receiver_type`, keeping the pure function's interface
  unchanged.

---

## Problem

### Observed symptom

```
$ mycelium get-callers Store::upsert_node --format json
{"caller_paths": [], ...}
```

Expected: callers from all files, including those where `upsert_node` is reached
via a function-return binding (`let s = get_store(); s.upsert_node()`).

### Root cause

`LocalBinding` stores `ctor_type: Option<String>` which is `Some("Store")` for
`Store::new()` but `None` for `get_store()`.  The extractor does not currently
capture the function name of a non-constructor call-initialiser.  As a result
rule c in `infer_receiver_type` does not fire, the method call stays unattributed,
and the receiver type inference returns `None` → conservative bare-stub fallback.

This happens even when both files are already fully extracted (same-file or any
extraction order), because the missing information is the callee return type, not
ordering.  It is a precision gap in the resolver, not a deferred/cross-file
ordering problem.

The pack captures required are already present (verified PM v144):
- `@call.receiver` — the receiver identifier at the call site
- `@binding.local` / `@binding.ctor` — local variable initialisers
- `@param.type` — function parameter annotations

The gap is in `LocalBinding` (no `fn_call_hint` field) and in the caller of
`infer_receiver_type` (no return-type lookup).

---

## Proposed solution

### 1. Extend `LocalBinding`

```rust
/// A local variable binding whose RHS is a constructor or function call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalBinding {
    pub name: String,
    /// `Some("Store")` when RHS is a constructor: `Store::new()` or `Store(…)`.
    pub ctor_type: Option<String>,
    /// `Some("get_store")` when RHS is a plain function call: `let s = get_store()`.
    /// Mutually exclusive with `ctor_type` — at most one is `Some`.
    pub fn_call_hint: Option<String>,
}
```

### 2. Extractor update

In `crates/mycelium-core/src/extractor/mod.rs`, where `@binding.local` captures
are turned into `LocalBinding` values: if the node matched by `@binding.ctor`
is absent but `@binding.local` is a call-expression (tree-sitter node kind
`call_expression`), set `fn_call_hint` to the function-path identifier.

Existing `ctor_type` logic is unchanged; `fn_call_hint` is only populated when
`ctor_type` is `None`.

### 3. Rule f in `resolve_call_site_contexts`

`infer_receiver_type` remains a **pure function** (no store parameter).  Rule f
is implemented in its caller, `resolve_call_site_contexts()`:

```rust
// Before calling infer_receiver_type, enrich any LocalBinding that has
// fn_call_hint but no ctor_type:
fn enrich_context(ctx: &ReceiverContext, store: &Store) -> ReceiverContext {
    let locals: Vec<LocalBinding> = ctx.locals.iter().map(|l| {
        if l.ctor_type.is_none() {
            if let Some(hint) = &l.fn_call_hint {
                // Look up the called function's declared return type.
                if let Some(ret_type) = store.return_type_of(hint) {
                    return LocalBinding {
                        name: l.name.clone(),
                        ctor_type: Some(ret_type),
                        fn_call_hint: l.fn_call_hint.clone(),
                    };
                }
            }
        }
        l.clone()
    }).collect();
    ReceiverContext { locals, ..ctx.clone() }
}
```

`store.return_type_of(fn_name)` is a new `Store` helper that looks up the
declared return type of a node by its simple function name (exact-match scan of
trunk paths ending in `>fn_name`).  It returns the first match or `None`.  No
cross-crate resolution — same-crate precision is sufficient for the primary use
case.

### 4. No new redb table

No schema change.  No migration.  Existing index files are fully compatible.

### API surface (Three-Surface Rule)

No new CLI/MCP tool.  `get-callers` / `mycelium_get_callers` returns more results
automatically.  No parity obligation.

---

## Implementation plan

### Step 1 — Extend `LocalBinding` and update `Default`/tests
- `crates/mycelium-core/src/resolver/receiver.rs`: add `fn_call_hint` field.
- Update all `LocalBinding { … }` struct literals in tests and extractor.

### Step 2 — Extractor: populate `fn_call_hint`
- `crates/mycelium-core/src/extractor/mod.rs`: detect call-expression initialiser
  in the `@binding.local` handler; set `fn_call_hint`.
- TDD: write RED test `fn_call_hint_populated_for_return_binding` before changing
  the extractor.

### Step 3 — Store helper: `return_type_of`
- `crates/mycelium-core/src/store/mod.rs`: add
  `pub fn return_type_of(&self, fn_name: &str) -> Option<String>`.
- Walk trunk paths; if a node path ends in `>fn_name` and the node has a
  `return_type` attribute, return it.
- TDD: write RED test `return_type_of_known_function` first.

### Step 4 — Resolver: `enrich_context` + rule f
- `crates/mycelium-core/src/extractor/mod.rs` (or a new
  `resolver/enrichment.rs`): implement `enrich_context`.
- Call `enrich_context` in `resolve_call_site_contexts` before
  `infer_receiver_type`.
- TDD: write RED test `rule_f_resolves_return_binding_caller` first.

### Step 5 — Quality gate
- `cargo test --all` green.
- Dogfood: `mycelium get-callers Store::upsert_node` returns ≥ 1 caller
  (currently 0 in the regression fixture).

---

## Acceptance criteria

- [ ] AC-1: `LocalBinding.fn_call_hint` field present; all existing struct
  literals compile without change (field is `None` by default via `Option`).
- [ ] AC-2: Extractor populates `fn_call_hint` for `let s = get_store()` style
  bindings (call-expression initialisers, non-constructor).
- [ ] AC-3: `Store::return_type_of` implemented and unit-tested.
- [ ] AC-4: `enrich_context` synthesises `ctor_type` from `fn_call_hint` +
  `return_type_of`; `infer_receiver_type` fires rule c on the enriched context.
- [ ] AC-5: Integration test `rule_f_resolves_return_binding_caller` GREEN — a
  two-symbol fixture (function returning a type + method call via return binding)
  resolved by `get-callers`.
- [ ] AC-6: `cargo test --all` green; coverage ≥ 90%; clippy clean.
- [ ] AC-7: No new redb table, no schema migration, no new unsafe.

---

## Alternatives considered

**Alt A (v1 Draft): New redb `TABLE_CALL_SITE_CONTEXT` + post-merge pass.**
Superseded by this revision.  Over-engineered: the existing `call_site_contexts`
Vec + `resolve_call_site_contexts()` already provides the deferred mechanism.
The additional redb table would have been a schema-break requiring a migration
and adding complexity without benefit.

**Alt B: Make `infer_receiver_type` store-aware.**
Breaks the pure-function contract that makes it easily testable.  The pre-enrichment
approach (`enrich_context`) achieves the same result while keeping the pure function.

**Alt C: Topological sort / re-run full extraction.**
O(N) per change; violates `< 10 ms` reactive SLA.  Rejected.

**Alt D: LSP-based resolution.**
Rejected by ADR-0010 (no live LSP; cold-start latency violates Charter §2).

---

## Risks

| Risk | Mitigation |
|---|---|
| `return_type_of` is imprecise (multiple fns with same name) | Return first match; document same-crate-precision scope; no worse than current stub fallback |
| Extractor change regresses same-file ctor detection | Existing rule c tests stay green; `fn_call_hint` only set when `ctor_type` is absent |
| Extraction-order sensitivity for `return_type_of` | `return_type_of` is called in `resolve_call_site_contexts` (post-extraction phase), not during per-file extraction; full store is available |

---

## Status history

| Date | Status | Note |
|---|---|---|
| 2026-06-09 | Draft (v1) | Authored by PM dispatch v148; proposed new redb table |
| 2026-06-09 | Draft (v2) | Revised by architect review (PM v149): scope narrowed to pure-resolver extension; new redb table removed; rule f via `enrich_context` pre-enrichment; acceptance criteria simplified |

*Next step: rust-implementer TDD implementation (after PR #568 back-merges to develop).*
