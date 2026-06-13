# RFC-0126: JavaScript browser-global member-call receiver classification (Phase 3)

- **Status**: Draft
- **Author(s)**: orchestrator (Hive AI agent, PM dispatch v221)
- **Created**: 2026-06-13
- **Depends on**: [RFC-0125](0125-javascript-cjs-callee-classification.md) (Phase 2 —
  browser-global bare-call classifier, Implemented as `a6c83af3`)
- **Affected paths**: `packs/javascript/queries.scm`,
  `crates/mycelium-core/src/classify.rs` (dot-qualified receiver path),
  `crates/mycelium-core/src/` extractor pass-1 (receiver synthesis)
- **Supersedes**: nothing (additive)
- **Tracked by**: Issue #819, PM dispatch v221

---

## Summary

RFC-0125 Phase 2 classifies **bare** browser-global calls (`fetch()`, `alert()`,
`addEventListener()`) as `Stdlib`. Member-expression calls such as
`document.querySelector()` or `window.open()` are **not** classified because the
extractor captures only the property name (`querySelector`, `open`) as
`callee_name` — the receiver (`document`, `window`) is discarded.

This RFC (Phase 3) extends the classifier to cover member calls where the
**object** is a known browser global, by synthesizing a `receiver.method` callee
name at extraction time.

---

## Motivation

### Observed gap

```javascript
// ✅ bare call — classified as Stdlib since RFC-0125 Phase 2
fetch('/api/data');
alert('hello');

// ❌ member call — falls through to Unknown today
document.querySelector('.foo');  // callee_name = "querySelector"
window.open(url);               // callee_name = "open"
navigator.geolocation.getCurrentPosition(cb);
localStorage.getItem('key');
```

### Root cause

The extractor's Pass 1 captures the **final identifier** in a call's function
position. For `document.querySelector(…)`, tree-sitter gives
`call_expression → member_expression → property_identifier = "querySelector"`.
The receiver `document` is not threaded into `callee_name`, so
`classify_javascript_browser_global` sees `"querySelector"` — a name too generic
to classify without receiver context.

---

## Decision

**Option A (recommended):** Pack-level predicate capture + extractor receiver
synthesis.

### Step 1 — `packs/javascript/queries.scm`

Add a capture pattern using a `#match?` predicate to identify member calls whose
object is a well-known browser global:

```scheme
; member calls on known browser globals: document.X(), window.X(), etc.
; Produces @_bg_receiver + @_bg_method captures consumed by the extractor.
(call_expression
  function: (member_expression
    object: (identifier) @_bg_receiver
    property: (property_identifier) @_bg_method)
  (#match? @_bg_receiver
    "^(document|window|navigator|location|history|localStorage|sessionStorage|indexedDB|XMLHttpRequest|Worker|WebSocket|screen|performance|crypto)$"))
```

Pack constraint (Charter §4): this file is 1 of ≤ 3 allowed under
`packs/javascript/` — additive patterns only, no file count increase.

### Step 2 — Extractor (Pass 1)

When a call node has both `@_bg_receiver` and `@_bg_method` captures, synthesize:

```
callee_name = format!("{}.{}", receiver, method)
// e.g. "document.querySelector", "window.open", "localStorage.getItem"
```

Existing call nodes without these captures are unaffected.

### Step 3 — `classify.rs`

Extend `classify_javascript_browser_global` to handle dot-qualified names:

```rust
pub fn classify_javascript_browser_global(name: &str) -> CalleeClass {
    // bare name: "fetch", "alert" — existing path
    // dot-qualified: "document.querySelector" — new path (prefix is receiver)
    let root = name.split('.').next().unwrap_or(name);
    if JS_BROWSER_GLOBALS.contains(root) || JS_BROWSER_GLOBALS.contains(name) {
        CalleeClass::Stdlib
    } else {
        CalleeClass::Unknown
    }
}
```

Backward-compatible: bare calls hit `JS_BROWSER_GLOBALS.contains(name)` directly
since `root == name` when there is no `.`.

**Option B (not recommended):** Classifier-level store lookup — resolve the callee
back to a graph node and check if its containing module is a browser global.
Requires store access in `classify.rs`, breaking the pure-function contract, and
adds non-trivial latency to the classification hot path.

---

## Acceptance criteria

- [ ] **AC-1**: `document.querySelector('.foo')` in a `.js` file → `callee_name =
  "document.querySelector"` in the graph, classified `Stdlib`. TDD: test RED
  before fix, GREEN after.
- [ ] **AC-2**: `window.open(url)` → `callee_name = "window.open"`, classified
  `Stdlib`.
- [ ] **AC-3**: `localStorage.getItem('key')` → `callee_name =
  "localStorage.getItem"`, classified `Stdlib`.
- [ ] **AC-4**: `myObj.myMethod()` (unknown receiver) → NOT matched by the new
  pattern; `callee_name` and classification unchanged (no false positives).
- [ ] **AC-5**: `fetch('/api')` bare call (RFC-0125 Phase 2) → still classified
  `Stdlib` (no regression).
- [ ] **AC-6**: `navigator.sendBeacon(url, data)` — single-level receiver
  `navigator` matches; `callee_name = "navigator.sendBeacon"`, classified
  `Stdlib`. (Two-level chains such as `navigator.geolocation.getCurrentPosition`
  are **out of scope for Phase 3** — the call's immediate object is a nested
  member_expression, not an identifier, so the query pattern does not fire.
  Chained-receiver support is tracked for Phase 4.)
- [ ] **AC-7**: All embedded pack copies synced (anti-patterns.jsonl
  `packs/` parity rule): `mycelium-core`, `mycelium-cli`, `mycelium-mcp`.

---

## Implementation plan

| Step | Scope | Paths | Est. effort |
|---|---|---|---|
| 1 | queries.scm predicate pattern | `packs/javascript/queries.scm` (+ 3 embedded copies) | ¼ session |
| 2 | Extractor receiver synthesis | `crates/mycelium-core/src/<extractor-pass1>.rs` | ¼ session |
| 3 | classify.rs dot-qualified path | `crates/mycelium-core/src/classify.rs` | ¼ session |

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
