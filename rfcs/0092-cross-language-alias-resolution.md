# RFC-0092: Cross-Language Alias Resolution

- **Status**: Implemented
- **Author(s)**: @aimasteracc (orchestrator dispatch)
- **Created**: 2026-05-30
- **Last updated**: 2026-05-30 (status updated to Implemented; PRs #277 TS, #278 JS, #283 Pattern 2)
- **Tracking issue**: #205 (Python concrete case), umbrella #200, #206
- **Affected source paths**:
  - `crates/mycelium-core/src/extractor/mod.rs` — two-pass extraction shape
  - `packs/python/queries.scm`, `packs/typescript/queries.scm`,
    `packs/javascript/queries.scm`, `packs/ruby/queries.scm` — alias captures
  - `crates/mycelium-core/src/store/mod.rs` — alias-table type (if exposed)

## Summary

Extend the per-file extraction pipeline with an **alias-table pass** that
maps locally-bound identifiers (`Y` in `import X as Y`) back to their
real symbol paths, then rewrites call-targets in the reference pass so
that `Y.func()` resolves to `X.func` instead of being lost.

This RFC unblocks issue #205 (the Python concrete case where 73
functions in an alias-dispatched module appear as false-positive dead
code) and lays the foundation for the same fix in TypeScript,
JavaScript, and Ruby — every language whose import system can rebind a
module to a local name.

## Motivation

### The bug (real-world signal from #200 / #205)

`tree-sitter-analyzer` (1600 files) ran against `mycelium v0.1.4` and
reported:

> All ~73 functions in `_ast_cache_query.py` show `callers: []`. A naive
> dead-code cleanup pass would delete actively-used production code.

Root cause: every caller goes through the alias `_query`:

```python
# ast_cache.py
from . import _ast_cache_query as _query

class ASTCache:
    def fts_search_ranked(self, ...):
        return _query.fts_search_ranked(...)  # bound to _query, not _ast_cache_query
```

The extractor sees the call as `_query.fts_search_ranked` and emits an
edge to the unresolved bare path `_query>fts_search_ranked`. The actual
definition is at `_ast_cache_query.py>fts_search_ranked`. The two never
join, so callers list is empty.

### Why this generalises beyond Python

| Language | Alias pattern |
|---|---|
| Python | `import X as Y`, `from X import Y as Z`, `from . import M as N` |
| TypeScript / JavaScript | `import { foo as bar } from 'mod'`, `import * as ns from 'mod'` |
| Ruby | `Foo = require 'foo'`, constant aliasing |
| Go | `import foo "external/path"` |

The same pattern recurs in every language pack we ship. A
language-specific patch wins one battle; an alias-resolution layer in
`mycelium-core` wins the war.

### Outcome we want

After this RFC ships:

- `mycelium get-callers <real path>` returns every site that calls
  through any alias, in any of the affected languages.
- `mycelium get-dead-symbols` no longer flags alias-dispatched modules.
- Bug 3 of #200 (inconsistent caller counts) closes — the partial leak
  was the alias resolution working for some sites and not others.

## Detailed design

### Three-phase extraction (new pass between Pass 1 and Pass 2)

Current pipeline (`extractor/mod.rs`):

1. **Pass 1**: walk the parse tree, capture all `@definition.*`,
   populate Trunk nodes + Contains edges.
2. **Pass 2**: walk again, capture all `@reference.*`, emit Calls /
   Imports / Extends edges.

Proposed pipeline:

1. **Pass 1** (unchanged): definitions → Trunk + Contains.
2. **Pass 1b** (new): walk import / import_from / aliased_import
   captures. Build a `HashMap<local_name, resolved_path>` for the file.
3. **Pass 2**: same as today, except every reference looks up its
   leftmost identifier in the alias table before emitting the edge.

### The alias table

```rust
/// Local-name → resolved-path mapping for a single file.
///
/// Built once per file in Pass 1b. Consumed by Pass 2 reference
/// resolution. Lifetime is the duration of a single
/// `Extractor::extract()` call — never persisted in `Store`.
#[derive(Debug, Default)]
struct AliasTable {
    /// Local binding → resolved symbol or module path.
    ///
    /// For `import X as Y`: `{"Y": "X"}`.
    /// For `from M import X`: `{"X": "M>X"}` (or the resolved file path
    /// when bug #204's resolver applies — see below).
    /// For `from M import X as Z`: `{"Z": "M>X"}`.
    /// For `from . import M as N`: `{"N": "<resolved-file-path>"}`.
    bindings: hashbrown::HashMap<String, String>,
}

impl AliasTable {
    /// Look up `local_name`. Returns `Some(resolved_path)` if there's a
    /// binding, `None` for normal (non-aliased) identifiers.
    fn resolve(&self, local_name: &str) -> Option<&str> {
        self.bindings.get(local_name).map(String::as_str)
    }
}
```

### Capture additions (per language pack)

Each language pack `queries.scm` gains capture nodes that expose the
LOCAL name AND the SOURCE in one match. Python example:

```scheme
; from X import Y     → local=Y, source=X.Y
(import_from_statement
  module_name: (_) @import.source
  name: (dotted_name) @import.local) @reference.import_alias

; from X import Y as Z → local=Z, source=X.Y
(import_from_statement
  module_name: (_) @import.source
  name: (aliased_import
    name: (_) @import.original_name
    alias: (_) @import.local)) @reference.import_alias
```

Extractor reads `@import.source` + `@import.local` (and optionally
`@import.original_name` for the `as` form) into the alias table.

### Reference rewriting (Pass 2 change)

Current `reference.call` handling:

```rust
let callee_name = name_text.unwrap_or("_unknown");
let intra = format!("{file_path}>{callee_name}");
let callee_id = if let Some(id) = store.lookup(&intra) {
    id
} else if let Ok(bare) = TrunkPath::parse(callee_name) {
    store.upsert_node(bare)
} else {
    continue;
};
```

Add alias lookup BEFORE the intra/bare fallback:

```rust
// For Y.func() patterns: leftmost identifier is the alias.
let (alias_prefix, method) = split_attribute_chain(callee_name);
let resolved_prefix = aliases.resolve(alias_prefix).unwrap_or(alias_prefix);
let qualified = if let Some(method) = method {
    format!("{resolved_prefix}>{method}")
} else {
    resolved_prefix.to_string()
};
let callee_id = store.lookup(&qualified)
    .unwrap_or_else(|| /* existing intra/bare fallback */);
```

### Interaction with #204 (relative-import resolution)

Bug #204's resolver (`resolve_python_relative_import`) already produces
the actual file path for `from .X import Y`. Pass 1b reuses that
resolver when building the alias table — so `from . import M as N`
binds `N` to the resolved sibling file path, and `N.func()` correctly
resolves to `<file>>func`.

### Error handling

Alias table operations are infallible. Lookup miss = caller is a normal
(non-aliased) identifier = existing behaviour. No new error variants in
`ExtractError`.

### Concurrency

The alias table lives entirely on the stack of one
`Extractor::extract()` call. No shared state, no locks, no async.

## Drawbacks

- **Per-language pack work.** Each language pack must add the new
  capture nodes. ≤ 3 files per language per Charter §5.7, but it is
  real per-language effort.
- **Imperfect for dynamic dispatch.** Python's runtime-dynamic patterns
  (`getattr(mod, name)()`, `globals()['_query'].func()`) still defeat
  static resolution. We will document this as a known limitation in
  CHANGELOG and the relevant Skill examples.
- **Doesn't solve transitive aliases.** `import A as B; B as C = B; C.f()`
  needs alias-of-alias resolution. Scope-out for this RFC; track as a
  follow-up if real-world signal demands it.

## Alternatives

1. **Heuristic suffix-match in `get-callers`** — when a symbol shows
   zero callers, search for any `*.<symbol_name>()` call site and
   include those. Rejected: false-positive prone; conflates `foo.bar()`
   for unrelated `foo`s.
2. **Persist alias edges in `Store`** — emit an `Aliases` edge between
   the alias's anchor and the real symbol; resolve queries by walking
   that edge. Rejected: explodes the edge count for marginal queryable
   value; alias resolution is an extraction-time concern, not a graph
   query concern.
3. **Defer to a language-server-protocol bridge** — outsource symbol
   resolution to pyright / tsserver. Rejected: contradicts mycelium's
   self-contained, no-server architectural promise (Charter §1).

## Acceptance criteria

- [x] `AliasTable` type lives in `crates/mycelium-core/src/extractor/`
  (implemented as `alias_table: HashMap<String,String>` — named-type deferred)
- [x] Python `queries.scm` gains `@alias.source` / `@alias.local` /
  `@alias.original_name` captures (names differ from RFC spec; functionally equivalent)
- [x] `Extractor::extract()` runs Pass 1 alias build → Pass 2 alias lookup
- [x] Integration test in `crates/mycelium-core/src/extractor/tests.rs`:
  multiple alias-resolution scenarios covering relative, absolute, `from . import M as N`,
  `from .M import X`, TypeScript named/namespace/default imports. (PR #277 / PR #278)
- [x] Regression test: real-projects CI `requests` and `ripgrep` fixtures pass;
  dogfood job indexes mycelium itself. (Regression against tree-sitter-analyzer
  fixture deferred — fixture not in-repo)
- [x] TypeScript pack gains the same captures (PR #277); JavaScript pack (PR #278)
- [x] CHANGELOG `[Unreleased]` entries added for alias resolution work
- [x] Issue #205 closes via PR #277; bug 3 of #200 resolved

## Rollout plan

Single PR per language pack. Python first (closes #205 + closes bug 3
of #200). TypeScript next. Others as community PRs once the pattern is
proven in two packs.

Target release: **v0.2.0**. Cross-language correctness for AI-agent
workloads is the v0.2.0 narrative.
