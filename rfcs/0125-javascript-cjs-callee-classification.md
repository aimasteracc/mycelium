# RFC-0125: JavaScript CJS callee classification — close the require() gap

- **Status**: Draft
- **Author(s)**: orchestrator (Hive AI agent, PM dispatch v216)
- **Created**: 2026-06-12
- **Depends on**: [RFC-0113](0113-stdlib-callee-classification.md) (callee classification
  infrastructure — classify.rs + classify_typescript_import_gated already live),
  [RFC-0002](0002-extractor.md) (Pass 1 extractor + @reference.import convention)
- **Affected paths**: `packs/javascript/queries.scm`,
  `crates/mycelium-core/src/classify.rs` (Phase 2 only),
  `crates/mycelium-core/src/queries.rs` (Phase 2 routing only)
- **Supersedes**: nothing (additive)
- **Tracked by**: PM dispatch v216

---

## Summary

JavaScript callee classification sits at **53.8%** in the Mycelium self-corpus
(RFC-0113 corpus measurement, 2026-06-12) — the **worst Tier-1 language** (Rust
66.3%, Python 67.3%, TypeScript 66.0%). Root cause: `.js` files use CommonJS
`require()` for imports, but `packs/javascript/queries.scm` only captures
ESM `import` statements. The RFC-0113 Phase 2 classifier
(`classify_typescript_import_gated`) correctly gates on import evidence — but for
CJS files that evidence is absent, so gating always misses and callees fall
through to `unknown`.

Phase 1 (extractor fix) closes the largest gap: add tree-sitter patterns to
capture `require()` calls as `@reference.import` evidence.  Phase 2 (classifier
extension) adds browser-global classification for DOM / Fetch API calls that
never have import backing in any module system.

---

## Motivation

### Measured gap

RFC-0113 corpus measurement (`cargo run --release -- index . && classify`
against Mycelium itself, 1,026 callee edges sampled):

| Language | Classified | Unknown |
|---|---|---|
| Python   | 67.3%      | 32.7%   |
| TypeScript | 66.0%    | 34.0%   |
| Rust     | 66.3%      | 33.7%   |
| Go       | ~66%       | ~34%    |
| **JavaScript** | **53.8%** | **46.2%** |

JavaScript is 12–13 pp below the other Tier 1 languages. The gap is not random —
it is structural: Mycelium's own JS tooling (`npm/`, `scripts/`) uses CJS
`require()` exclusively, and those imports produce zero `@reference.import`
captures today.

### Root cause: `require()` calls are not `@reference.import` captures

`packs/javascript/queries.scm` captures ESM `import_statement` nodes as
`@reference.import`. CommonJS `require()` is a *function call*, not an
import statement — tree-sitter parses it as `call_expression`, not
`import_statement`, so Pass 1 never emits `@reference.import` for it.

`classify_typescript_import_gated` (RFC-0113 Phase 2) requires import evidence
in `caller_imports` to gate `stdlib` / `external` classification. With empty
`caller_imports`, **every classifiable CJS stdlib call falls through to
`unknown`** — regardless of the allowlist match.

Example: `packs/javascript/queries.scm`'s own `require()` calls in
`scripts/build-npm.mjs` hit this gap in the corpus.

### Secondary gap: browser globals have no import backing

`document`, `window`, `fetch`, `navigator`, `localStorage` are browser globals
— always in scope, never imported. `classify_typescript_import_gated` correctly
treats them as `unknown` because there is no import to gate on. A
`classify_javascript_browser_global` tier (Phase 2) can classify these without
import gating, at the cost of one small allowlist.

---

## Decision

Two-phase fix. Phase 1 is the high-value item; Phase 2 is additive.

### Phase 1 — CJS `require()` as `@reference.import` in extractor

Add tree-sitter capture patterns to `packs/javascript/queries.scm` for the
common CJS `require()` assignment forms. The extractor's Pass 1 already
processes `@reference.import` captures and feeds them into `caller_imports`
(used by `classify_typescript_import_gated`). Adding `require()` captures makes
CJS files first-class for the existing classifier — **zero changes to
`classify.rs` or `queries.rs`**.

Target patterns (tree-sitter-javascript grammar nodes):

```scheme
; `const fs = require('fs')` — whole-module CJS import
(variable_declaration
  (variable_declarator
    name: (identifier) @alias.local
    value: (call_expression
      function: (identifier) @_req
      arguments: (arguments (string (string_fragment) @name))))
  (#eq? @_req "require")) @reference.import

; `const { readFile } = require('fs')` — destructured CJS import
(variable_declaration
  (variable_declarator
    name: (object_pattern
            (shorthand_property_identifier_pattern) @alias.original_name)
    value: (call_expression
      function: (identifier) @_req
      arguments: (arguments (string (string_fragment) @name))))
  (#eq? @_req "require")) @reference.alias_binding
```

The `@name` in both patterns captures the module string (`'fs'`, `'path'`, etc.),
exactly matching what ESM `import_statement` emits — so `caller_imports` becomes
a unified set regardless of module system.

**No `cjs` / `mjs` distinction needed**: `queries.scm` is loaded for all `.js`
extensions. ESM files already emit `import_statement` captures; CJS patterns are
additive (no overlap). Mixed-mode files (ESM + `require()`) benefit from both.

### Phase 2 — Browser-global tier in `classify.rs`

Add `classify_javascript_browser_global(name: &str) -> CalleeClass` in
`classify.rs` — a pure name lookup with no import gating:

```rust
pub fn classify_javascript_browser_global(name: &str) -> CalleeClass {
    if JS_BROWSER_GLOBALS.contains(name) {
        CalleeClass::Stdlib
    } else {
        CalleeClass::Unknown
    }
}
```

Allowlist seed (from MDN Web API global objects, browser-only):
`document`, `window`, `navigator`, `location`, `history`, `fetch`, `XMLHttpRequest`,
`localStorage`, `sessionStorage`, `IndexedDB`, `Worker`, `WebSocket`,
`requestAnimationFrame`, `cancelAnimationFrame`, `alert`, `confirm`, `prompt`,
`addEventListener`, `removeEventListener`, `dispatchEvent`, `CustomEvent`.

Wire in `callees_payload` (`crates/mycelium-core/src/queries.rs`) after the
existing `classify_typescript_import_gated` call for `.js` files — only when
the primary classification returned `Unknown`:

```rust
} else if is_ts_js {
    let cls = classify_typescript_import_gated(path, &caller_imports);
    if cls != CalleeClass::Unknown || ext != Some("js") {
        cls.as_str()
    } else {
        classify_javascript_browser_global(path).as_str()
    }
}
```

---

## Acceptance criteria

### Phase 1 — CJS extractor fix

- [ ] **AC-1**: Tree-sitter pattern `const X = require('Y')` emits `@reference.import`
      with `@name = "Y"` in `packs/javascript/queries.scm`.
- [ ] **AC-2**: Pattern `const { A, B } = require('Y')` emits `@reference.alias_binding`
      with `@name = "Y"` (module) + `@alias.original_name = "A"` / `"B"`.
- [ ] **AC-3**: A `.js` fixture file using CJS `require('fs')` + `readFileSync()` call —
      extractor produces `caller_imports` containing `"fs"` → `classify_typescript_import_gated`
      returns `Stdlib` for `readFileSync`. TDD: test is RED before the queries.scm
      change and GREEN after.
- [ ] **AC-4**: ESM files (`import fs from 'fs'`) are **not affected** — their
      `@reference.import` captures are unchanged and no duplicate entries are emitted.
- [ ] **AC-5**: All three embedded pack copies synced (`mycelium-core`,
      `mycelium-cli`, `mycelium-mcp` — per anti-patterns.jsonl `packs/` parity rule).

### Phase 2 — Browser-global classifier

- [ ] **AC-6**: `classify_javascript_browser_global("fetch")` returns `CalleeClass::Stdlib`.
- [ ] **AC-7**: `classify_javascript_browser_global("document")` returns `CalleeClass::Stdlib`.
- [ ] **AC-8**: `classify_javascript_browser_global("myCustomFn")` returns `CalleeClass::Unknown`.
- [ ] **AC-9**: In `callees_payload`, a `.js` file calling `fetch()` with no imports
      returns `"stdlib"` (Phase 2 tier fires). A `.ts` file calling `fetch()` also
      returns `"stdlib"` via the existing TS classifier (no regression). A `.rs` file
      with a local function named `fetch` is not affected.

---

## Implementation plan

| Phase | Scope | Paths | Est. effort |
|---|---|---|---|
| 1 | queries.scm CJS patterns | `packs/javascript/queries.scm` + 3 pack copies synced | ½ session |
| 2 | browser-global classifier | `crates/mycelium-core/src/classify.rs` + `queries.rs` | ½ session |

Both phases follow the Charter §5.1 TDD workflow (RED → GREEN → refactor → clippy → fmt).
Phase 1 may be implemented independently; Phase 2 requires Phase 1 to be on develop first.

---

## Expected impact

Phase 1 alone should lift JavaScript from **53.8% → 65%+** in the Mycelium
self-corpus (conservative estimate — Mycelium's own scripts use CJS `require()`
almost exclusively). Phase 2 adds browser-global coverage relevant for any
front-end codebase (React/Vue/vanilla JS apps are a primary Mycelium target).

Combined target: **≥ 70% JavaScript classification** — bringing JS to parity
with the other Tier 1 languages.

---

## Alternatives considered

**Alt A: Add a separate `classify_javascript_cjs_gated` function** — Would duplicate
`classify_typescript_import_gated` logic with CJS-specific import parsing. Rejected:
the extractor fix (Phase 1) makes CJS imports first-class, so the existing
`classify_typescript_import_gated` already handles the gating correctly with no
duplication.

**Alt B: Detect CJS vs ESM at classification time** — Parse `require()` calls in
`classify.rs` at query time. Rejected: the classification layer should be pure
data → `CalleeClass`, not a parser. Import parsing belongs in Pass 1 (extractor).

**Alt C: Separate JS and TS classification entirely** — Duplicate the entire
TypeScript allowlist for JavaScript. Rejected: Node.js stdlib is shared between TS
and JS; a shared allowlist is correct. JS-specific divergence (browser globals) is
additive, not a split.
