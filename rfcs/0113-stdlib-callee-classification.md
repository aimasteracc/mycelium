# RFC-0113: stdlib/builtin callee classification — rescue the `unknown` tail (design)

- **Status**: **Partially Implemented** (Phase 1 criteria 1/2/3/5 done; corpus measurement pending; Phase 2 TypeScript tables shipped; Phase 3 Go tables shipped; Phase 3b Go qualified-call fix shipped; Phase 4 Rust tables shipped; Phase 5 Rust qualified-call fix shipped)
- **Author(s)**: orchestrator (Hive AI agent)
- **Created**: 2026-06-06 (UTC)
- **Depends on**: [RFC-0103](0103-import-aware-cross-file-resolution.md) +
  [RFC-0092](0092-cross-language-alias-resolution.md) (existing import-aware /
  cross-file resolution — this RFC adds the *last* tier after them),
  [ADR-0010](../docs/adr/0010-no-live-lsp-prefer-scip-ingestion.md) (no live LSP;
  prefer static precision — **this RFC is exactly the sanctioned static lever**),
  Charter §4 (≤3-file language packs), Charter §5.13 (Three-Surface Rule)
- **Affected paths** (when implemented): `crates/mycelium-core/src/store/`
  (resolver), `packs/<lang>/` (the per-language allowlist data)
- **Reuses**: tree-sitter-analyzer (TSA) `synapse_resolver/_constants.py`
  (`_FALLBACK_STDLIB` + builtin/external tables) and `__init__.py` (the
  classification cascade). Same founder, MIT — the **data tables and the
  algorithm** are reused; TSA is Python, Mycelium is Rust, so this is a
  *port-concept + copy-data*, not a source dependency.

## Summary

Add a deterministic **final classification tier** to callee resolution that
rescues the large `unknown` tail tree-sitter leaves behind. When the existing
resolver (RFC-0092/0103) cannot resolve a call like `p.write_text()` or
`pytest.raises(...)`, classify the bare name against **curated, import-gated
stdlib / builtin / well-known-external allowlists** instead of giving up. TSA
proved this lifts callee classification **83.9% → 95.9%** with **zero LSP** — it
is pure static name-table matching, exactly the precision lever ADR-0010 calls
for.

## Motivation

Tree-sitter is syntactic: it cannot type-resolve a method on a receiver
(`obj.method()`), a stdlib call routed through an import, or a third-party
library call. Today those land as **`unknown` bare-stub callee nodes**
(`getcwd`, `dumps`, `print`), which:

- **pollute the graph with spurious nodes** — a stdlib call materializes a
  meaningless stub symbol that then shows up in `get-isolated-symbols` /
  leaf analysis and inflates node counts,
- make **`get-callees` ambiguous** — a caller's callee set mixes real
  project callees with anonymous stubs, with no way to tell "this is a stdlib
  call" from "this is a genuinely-unresolved *project* call", and
- weaken every downstream consumer (context bundles, impact/blast-radius).

*(Note: this does **not** touch `get-dead-symbols`, which keys on a symbol's
**incoming** edges — whether a function itself calls stdlib is irrelevant to
whether anything calls *it*. This RFC is about classifying **outgoing** callee
edges, not liveness.)*

TSA hit the same wall and solved it without LSP: a final tier that says "this
bare name is a Python stdlib/builtin/known-external call" using frozen
allowlists, gated by import evidence. The result was the 83.9% → 95.9% jump. The
same data + algorithm port directly onto Mycelium's resolved graph.

## Decision: a last-resort, import-gated classification tier

Mirror TSA's cascade (TSA `synapse_resolver/__init__.py` documents tiers 1–6).
Mycelium already implements the precise tiers; this RFC adds the final two:

```
existing (RFC-0092/0103):  local → project(import-backed) → single-def
NEW (this RFC):            → stdlib/builtin/external (allowlist, import-gated)
                           → unknown            (only if nothing matched)
```

Rules (ported from TSA `_try_stdlib` + `_constants.py`):

1. **Import-gated, not blind.** `from <stdlib_mod> import X` makes a later `X()`
   classify as **stdlib**; `import <mod>` makes `<mod>.X()` classify as
   **stdlib/external**. A bare `X()` with no import backing stays `unknown` —
   the table never fires on names the file didn't import. (Prevents
   false-classifying a project function that merely shares a stdlib name.)
2. **Project ownership shadows everything.** If the project defines a symbol with
   that name (the existing `single`/`project` tiers matched), that wins — the
   allowlist tier is only reached when project resolution already returned
   nothing. (TSA's shadowing gate.)
3. **Curated frozen tables, per language.** Start with Python (TSA ships
   `_FALLBACK_STDLIB` + `sys.stdlib_module_names`, builtins, and a curated
   external set). The table is **language data**, not core logic.

### Edge classification, not a new edge

The tier does not invent a new capability — it **reclassifies** what is already
a (currently-`unknown`) callee. The resolved callee gains a `class`:
`project | stdlib | builtin | external | unknown`. This flows through the
**existing** tools — `get-callees`, `get-dead-symbols`, `get-isolated-symbols`,
context — making them more accurate, and is exposed as an **additive JSON field**
on callee entries. No new CLI/MCP tool ⇒ no Three-Surface surface to add (the
field is byte-identical across CLI/MCP by construction).

## Where the data lives (Charter §4 — ≤3 files per pack)

The **algorithm** is core (shared, language-agnostic cascade). The **allowlist
tables** are per-language **data** shipped with the pack, not core code, so a new
language adds precision by adding *data*, never by editing core. Candidate
homes (settled in the implementation PR): an `allowlist.scm`/`allowlist.toml`
adjacent to the pack's `queries.scm`, or a section in `pack.toml`. The ≤3-file
rule is respected by treating the allowlist as pack data co-located with the
existing pack files (the core resolver loads it the way it loads `queries.scm`).

## Acceptance criteria (when promoted to implementation)

**Phase 1 — Python:**
- [x] Port TSA's Python stdlib/builtin/external allowlists into pack data;
      confirm MIT provenance noted. *(Phase 1 `classify.rs`)*
- [x] Core resolver: a final `classify_callee` tier after RFC-0092/0103, import-
      gated, with the project-ownership shadow guard. TDD with fixtures for:
      `from os import getcwd; getcwd()` → stdlib; `import json; json.dumps()` →
      stdlib; bare `helper()` defined in-project → project (shadow wins); bare
      `frobnicate()` with no import → unknown (table must NOT fire).
      *(Phase 2 + Phase 3 import gate — `classify_python_import_gated`, 8 unit tests)*
- [x] Additive `class` field on callee JSON, so `get-callees` distinguishes
      stdlib/builtin/external from genuinely-unresolved project callees; spurious
      stdlib bare-stub nodes drop out of `get-isolated-symbols` / leaf analysis.
      (No change to `get-dead-symbols` — it keys on incoming edges.) Snapshot tests.
      *(Phase 2 `callees_payload` + Phase 3 import gate, 6+2 TDD tests)*
- [ ] Measure the `unknown`-tail reduction on the dogfood corpus (target: a
      material drop, reported in the PR — TSA's reference is 83.9%→95.9%).
- [x] CLI ↔ MCP parity preserved (additive field identical on both).
      *(shared `callees_payload` builder — byte-identical across surfaces)*

**Phase 2:** TypeScript/JS — `classify_typescript`, `classify_typescript_import_gated`,
`classify_typescript_qualified` + 21 TDD tests. ✅ Shipped in `classify.rs` alongside the
Python tables. Global builtins (`parseInt`, `Error`, …), Node.js modules (`fs`, `path`, …),
stdlib methods (Array/String/Promise), Node.js module-level functions (`readFileSync`, …),
test-framework matchers (jest/vitest/mocha/chai). Import-gated with `node:` prefix
tolerance. `callees_payload` dispatches `.ts/.tsx/.js/.jsx/.mjs/.cjs` callers here.

**Phase 3:** Go — `classify_go`, `classify_go_import_gated`, `classify_go_qualified`. ✅ Shipped.
Go builtins (`make`, `len`, `append`, `cap`, `copy`, `delete`, `close`, `panic`, `recover`,
`new`, `real`, `imag`, `complex`, `min`, `max`, `clear`). Stdlib package local names covering
all common standard library packages (`fmt`, `os`, `io`, `http`, `json`, `filepath`, `sync`,
`context`, `regexp`, `testing`, …). Import-gated via last-component matching: `"net/http"` →
local name `"http"`, `"encoding/json"` → local name `"json"`. `callees_payload` dispatches
`.go` callers here. 11 TDD tests. Other Tier-1 packs (Java, C/C++) remain pending.

**Phase 4:** Rust — `classify_rust`, `classify_rust_import_gated`, `classify_rust_qualified`. ✅ Shipped.
Builtin macros (`println`, `panic`, `assert`, `vec`, `dbg`, …) + `drop` classify unconditionally as
`Builtin`. Stdlib module local names (`fs`, `io`, `env`, `sync`, `thread`, `collections`, …) are
import-gated: `use std::<name>` or any sub-import (`std::fs::File`) enables the module. Qualified
paths (`fs>read_to_string`, `std::io>stdout`) classified via `classify_rust_qualified`. `callees_payload`
dispatches `.rs` callers here. 21 new TDD tests (14 in `classify::rust_tests`, 7 in `queries::tests`).

**Phase 3b:** Go qualified-call fix (Issue #795). ✅ Shipped.
- [x] `packs/go/queries.scm` — removed the `selector_expression` arm from the first
      `@reference.call` pattern; RFC-0118 Part B (`@call.receiver` + `@name`) is now
      the sole handler of qualified calls (`pkg.Func()`), eliminating duplicate bare-stub edges.
- [x] `extractor/mod.rs` — added **Pass 1b-go**: after the alias-binding loop, iterate
      `@reference.import` captures for `.go` files, strip surrounding double-quotes from
      `interpreted_string_literal`, take the last path component as the local name, and
      insert `local → local` into `alias_table`. This ensures `http.Get()` resolves to
      callee path `"http>Get"` (and then `classify_go_qualified("http","Get")` → `Stdlib`).
- [x] `queries.rs` — `callees_payload` dispatches `path.contains('>')` Go paths to
      `classify_go_qualified`; `Unknown` falls through to `Project` (safe default).
- [x] 4 new TDD tests (2 extractor integration, 2 `callees_payload` unit tests) all GREEN.
- [x] Embedded pack copy `crates/mycelium-core/packs/go/queries.scm` synced; verified by
      `cortex::tests::embedded_core_pack_queries_match_canonical_root`.

**Phase 5:** Rust qualified-call fix (Issue #800). ✅ Shipped (PR #802).
- [x] `packs/rust/queries.scm` + `extractor/mod.rs` — single-segment Rust scoped calls
      (e.g., `fs::read_to_string()`, `io::stdout()`) now emit `scope>name` stubs
      instead of bare unresolved stubs, eliminating the duplicate bare-stub edge class
      for qualified Rust callees.
- [x] `callees_payload` in `queries.rs` dispatches `scope>name` Rust paths to
      `classify_rust_qualified`, enabling precise stdlib/builtin classification for all
      scoped calls (previously only handled when the scope was pre-resolved).
- [x] Embedded pack copy `crates/mycelium-core/packs/rust/queries.scm` synced; verified
      by `cortex::tests::embedded_core_pack_queries_match_canonical_root`.
- [x] 3 new TDD tests (extractor integration + `callees_payload` unit tests), all GREEN.
      Total: 957/957 tests pass. Issue #800 CLOSED.

## Alternatives considered

- **Live LSP for type-resolved callees.** Rejected by **ADR-0010**. This tier
  gets most of the precision LSP would, statically, with no resident server.
- **Static SCIP/LSIF ingestion (ADR-0010's reserved path).** Complementary, not
  competing: SCIP is the *full* type-precision answer if/when prioritized; this
  allowlist tier is the **cheap 80%** that ships now with no new dependency and
  no per-repo index-generation step. SCIP can later supersede the heuristic for
  languages where it's available.
- **Heuristic receiver-type inference** (guess the type of `obj` in
  `obj.method()`). Rejected for v1: even TSA defers it; it edges toward the
  type-inference ADR-0010 routes to SCIP and would strain the ≤3-file pack rule.
- **Classify blindly by name (no import gate).** Rejected: would mis-classify
  project functions sharing a stdlib name. The import gate is what makes the
  tables safe.

## Conflicts with binding constraints

- **ADR-0010 (no live LSP):** ✅ fully compliant — static allowlists + import
  evidence, no server, no subprocess. This is the precise "improve precision
  without LSP" path ADR-0010 endorses.
- **Charter §4 (≤3-file packs):** respected — tables are pack *data*, core holds
  only the language-agnostic cascade; new languages add data, not core edits.
- **Charter §5.13 (Three-Surface):** no new capability — an additive
  classification field on existing callee output, identical across CLI/MCP.
