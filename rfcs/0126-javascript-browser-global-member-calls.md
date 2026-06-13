# RFC-0126: JavaScript browser-global member-call receiver classification (Phase 3)

- **Status**: Implemented
- **Author(s)**: orchestrator (Hive AI agent, PM dispatch v221–v222)
- **Created**: 2026-06-13
- **Depends on**: [RFC-0125](0125-javascript-cjs-callee-classification.md) (Phase 2 —
  browser-global bare-call classifier, Implemented as `a6c83af3`)
- **Affected paths**: `crates/mycelium-core/src/classify.rs` (dot-qualified receiver path),
  `crates/mycelium-core/src/extractor/mod.rs` (receiver synthesis),
  `crates/mycelium-core/src/extractor/tests.rs`,
  `crates/mycelium-core/src/queries.rs`
- **Supersedes**: nothing (additive)
- **Tracked by**: Issue #819, PM dispatch v221–v222

---

## Summary

RFC-0125 Phase 2 classifies **bare** browser-global calls (`fetch()`, `alert()`,
`addEventListener()`) as `Stdlib`. Member-expression calls such as
`document.querySelector()` or `window.open()` were **not** classified because the
extractor captured only the property name (`querySelector`, `open`) as
`callee_name` — the receiver (`document`, `window`) was discarded.

This RFC (Phase 3) extends the classifier to cover member calls where the
**object** is a known browser global, by synthesizing a `receiver.method` callee
name at extraction time (Rust-side, no queries.scm change needed).

---

## Motivation

### Observed gap

```javascript
// ✅ bare call — classified as Stdlib since RFC-0125 Phase 2
fetch('/api/data');
alert('hello');

// ❌ member call — fell through to Unknown before this RFC
document.querySelector('.foo');  // callee_name was "querySelector"
window.open(url);               // callee_name was "open"
localStorage.getItem('key');    // callee_name was "getItem"
```

### Root cause

The extractor's Pass 2 captures the **final identifier** in a call's function
position. For `document.querySelector(…)`, tree-sitter gives
`call_expression → member_expression → property_identifier = "querySelector"`.
The receiver `document` was not threaded into `callee_name`, so
`classify_javascript_browser_global` saw `"querySelector"` — a name too generic
to classify without receiver context.

---

## Decision

**Rust-side receiver synthesis (no queries.scm change needed).**

The extractor already captures `@call.receiver` for all depth-1 member calls
(existing `reference.call` capture in `packs/javascript/queries.scm` already
populates the `receiver` field for all calls). The synthesis is purely in Rust:

### Step 1 — Extractor (`extractor/mod.rs`)

When a `.js`/`.jsx` call node has a `receiver` whose root is a known
browser global, synthesize:

```rust
let bg_callee: Option<String> =
    if /* file is .js/.jsx */ {
        receiver
            .filter(|r| classify_javascript_browser_global(r) == CalleeClass::Stdlib)
            .map(|r| format!("{r}.{callee_name}"))
    } else { None };
let effective_callee = bg_callee.as_deref().unwrap_or(callee_name);
```

`effective_callee` is used only for the bare stub creation path; `callee_name`
is kept unmodified for `ReceiverContext.method` and alias-table resolution.

### Step 2 — `classify.rs`

Extend `classify_javascript_browser_global` to handle dot-qualified names:

```rust
pub fn classify_javascript_browser_global(name: &str) -> CalleeClass {
    // Split on first dot so "document.querySelector" → root = "document"
    let root = name.split('.').next().unwrap_or(name);
    if JS_BROWSER_GLOBALS.contains(root) {
        CalleeClass::Stdlib
    } else {
        CalleeClass::Unknown
    }
}
```

Backward-compatible: bare calls have no `.`, so `root == name`, hitting the
existing `JS_BROWSER_GLOBALS` set directly.

---

## Acceptance criteria

- [x] **AC-1**: `document.querySelector('.foo')` in a `.js` file → `callee_name =
  "document.querySelector"` in the graph, classified `Stdlib`. TDD: test RED
  before fix, GREEN after.
- [x] **AC-2**: `window.open(url)` → `callee_name = "window.open"`, classified
  `Stdlib`.
- [x] **AC-3**: `localStorage.getItem('key')` → `callee_name =
  "localStorage.getItem"`, classified `Stdlib`.
- [x] **AC-4**: `myObj.myMethod()` (unknown receiver) → NOT matched by the new
  pattern; `callee_name` and classification unchanged (no false positives).
- [x] **AC-5**: `fetch('/api')` bare call (RFC-0125 Phase 2) → still classified
  `Stdlib` (no regression).
- [x] **AC-6**: `navigator.sendBeacon(url, data)` — single-level receiver
  `navigator` matches; `callee_name = "navigator.sendBeacon"`, classified
  `Stdlib`. (Two-level chains such as `navigator.geolocation.getCurrentPosition`
  are **out of scope for Phase 3** — the call's immediate object is a nested
  member_expression, not an identifier, so the receiver capture does not
  fire for the outer call. Chained-receiver support is tracked for Phase 4.)
- [x] **AC-7**: All embedded pack copies synced (anti-patterns.jsonl
  `packs/` parity rule): no `queries.scm` change was needed in this Phase 3
  implementation, so no sync is required.

---

## Implementation plan

| Step | Scope | Paths | Est. effort |
|---|---|---|---|
| 1 | Extractor receiver synthesis | `crates/mycelium-core/src/extractor/mod.rs` | ¼ session |
| 2 | classify.rs dot-qualified path | `crates/mycelium-core/src/classify.rs` | ¼ session |
| 3 | Tests (TDD) | `extractor/tests.rs`, `queries.rs`, `classify.rs` | ¼ session |

Prerequisite: RFC-0125 Phase 2 on develop ✅ (`a6c83af3`).

---

## Expected impact

Covers `document.*`, `window.*`, `localStorage.*`, `navigator.*`, `XMLHttpRequest.*`
patterns ubiquitous in browser-facing `.js` code. Combined with Phases 1 + 2,
JavaScript callee classification is expected to reach **≥ 72%** (up from ~65%+
post-Phase 2; member-call patterns are structurally dense in front-end codebases).

---

## Alternatives considered

**Expanding the Phase 2 allowlist with method names** — Add `querySelector`,
`getElementById`, `getItem`, `setItem`, `open`, … to `JS_BROWSER_GLOBALS`.
Rejected: these names are too generic (`open`, `send`, `get`, `set` appear
ubiquitously in user code), causing false-positive `Stdlib` classifications.
Receiver disambiguation is necessary.

**Separate `receiver_path` stored attribute** — Persist the full receiver chain
as a new field on call nodes in redb. Overkill for Phase 3: the
`callee_name = "receiver.method"` synthesis achieves the same classification
goal with zero storage-format change.

**queries.scm predicate pattern** — Use `#match?` predicate in the JS pack to
filter at the tree-sitter level. Not needed: the existing `@call.receiver`
capture already supplies the receiver text; the Rust-side filter is cleaner and
requires no pack file modification.
