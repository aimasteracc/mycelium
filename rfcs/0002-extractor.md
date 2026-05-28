# RFC-0002: Symbol Extractor

- **Status**: accepted
- **Author(s)**: @aimasteracc (Hive rust-implementer agent)
- **Created**: 2026-05-29
- **Last updated**: 2026-05-29
- **Tracking issue**: TBD
- **Affected source paths**:
  - `crates/mycelium-core/src/extractor/mod.rs`
  - `crates/mycelium-core/src/extractor/tests.rs`
  - `crates/mycelium-core/src/lib.rs`
  - `crates/mycelium-core/Cargo.toml`
  - `Cargo.toml` (workspace.dependencies)

## Summary

Add a `Extractor` type to `mycelium-core` that accepts a tree-sitter `Language`
and a `LanguagePack`'s query source, then populates a `Store` from a source file.
This is the tree-sitter→Store bridge that makes Mycelium actually useful.

## Motivation

RFC-0001 defined `Store` (Trunk + Synapse) and the language pack format.
RFC-0001's P6 added the Python pack. Neither produces symbols yet —
the extractor is the missing step between "grammar + queries" and "graph nodes."

Without the extractor, the Store is empty. No analysis, no queries, no AI value.

Target: after this RFC, a caller can do:

```rust
let pack = LanguagePack::load(pack_dir)?;
let language = tree_sitter_python::LANGUAGE.into();
let extractor = Extractor::new(language, &pack.queries)?;
let mut store = Store::new();
extractor.extract("src/main.py", source_bytes, &mut store)?;
// store now has nodes for all classes/functions/methods + Imports edges
```

## Detailed design

### Capture → Symbol mapping

The extractor processes tree-sitter query captures with these conventions
(matching `queries.scm`):

| Capture prefix    | Effect                                          | EdgeKind     |
|-------------------|-------------------------------------------------|--------------|
| `definition.*`    | `store.upsert_node(built_path)` + `Contains` edges to ancestors | — |
| `reference.import` | `store.upsert_node` for module, `store.upsert_edge(Imports, ...)` | `Imports` |
| `reference.import_from` | same as above                              | `Imports` |

The `@name` capture provides the symbol's identifier text.

#### Path construction

Paths follow the `TrunkPath` `>` separator convention established in RFC-0001.

| Match kind              | Path                                    |
|-------------------------|-----------------------------------------|
| `definition.module`     | `{file_path}`                           |
| `definition.function`   | `{file_path}>{name}`                    |
| `definition.class`      | `{file_path}>{name}`                    |
| `definition.method`     | `{file_path}>{class_chain...}>{name}`   |
| nested functions        | `{file_path}>{outer_fn}>{name}`         |

`class_chain` is built by walking the tree-sitter AST upward from the anchor
node, collecting `class_definition` ancestors.  Depth ≥ 4 is capped (v0.1 limit).

#### Contains edges

For every non-module definition, the extractor also inserts a `Contains` edge
from the parent node to the child:

```
file → class         (Contains)
class → method       (Contains)
file → function      (Contains)
```

Parent nodes are created by `upsert_node` — already idempotent.

### Public API

```rust
/// Errors from the extraction pipeline.
#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    /// The tree-sitter language could not be loaded into the parser.
    #[error("failed to set language on parser: {0}")]
    Language(String),

    /// The query source is invalid for this language.
    #[error("invalid query: {0}")]
    Query(String),

    /// The parser returned no tree (e.g., language not set, or timeout).
    #[error("parser returned no parse tree")]
    ParseFailed,
}

/// Extracts symbols from source files into a [`Store`].
///
/// Build once (reuse across many files); the internal [`Query`] is compiled
/// at construction time.
///
/// # Example
///
/// ```no_run
/// # use mycelium_core::{extractor::Extractor, Store};
/// # use tree_sitter::Language;
/// # fn example(language: Language, query_src: &str) {
/// let mut store = Store::new();
/// let extractor = Extractor::new(language, query_src).unwrap();
/// extractor.extract("src/foo.py", b"def hello(): pass", &mut store).unwrap();
/// # }
/// ```
pub struct Extractor {
    language: tree_sitter::Language,
    query: tree_sitter::Query,
}

impl Extractor {
    /// Create an extractor for `language` using the query source from a
    /// language pack's `queries.scm`.
    pub fn new(language: tree_sitter::Language, query_src: &str) -> Result<Self, ExtractError>;

    /// Parse `source` as a file at `file_path` and populate `store` with
    /// all extracted symbols and relationships.
    ///
    /// `file_path` is used as the first path segment (the module root).
    /// It should be the repository-relative path, e.g. `"src/main.py"`.
    pub fn extract(
        &self,
        file_path: &str,
        source: &[u8],
        store: &mut Store,
    ) -> Result<(), ExtractError>;
}
```

### Error handling

`ExtractError` covers three cases:

1. **Language**: wraps the string from `Parser::set_language` — happens only
   if the `Language` object is invalid (extremely rare in practice).
2. **Query**: compilation error from `Query::new` — caught at `Extractor::new`
   time, not at extract time.
3. **ParseFailed**: `parser.parse()` returned `None` — can happen if `timeout`
   is set or if the language is not set (guarded by `new()`).

### Concurrency

`Extractor` is `Send + Sync` (after `Sync` is confirmed by the compiler). Each
call to `extract` creates a fresh `Parser` (not stored), so parallel extraction
requires only a `&Extractor`.

## Drawbacks

- Adding `tree-sitter` as a `[dependencies]` (not `[dev-dependencies]`) of
  `mycelium-core` means every downstream gets the C parser compile time.
  Mitigation: tree-sitter has no runtime overhead when unused; it's already
  a workspace dependency.
- Grammar crates (`tree-sitter-python`, etc.) go in `[dev-dependencies]` of
  `mycelium-core` for tests, and in `[dependencies]` of the CLI/MCP crate.
  This keeps `mycelium-core` language-agnostic.

## Alternatives

- **Separate `mycelium-extractor` crate**: cleaner dependency graph, but adds
  a crate for a module-sized abstraction. Deferred to v0.2 if the extractor
  grows significantly.

- **Full tree walker instead of query-based**: walk the entire CST manually.
  Rejects the language-pack philosophy — the `.scm` file would be unused.

## Prior art

- **rust-analyzer**: uses tree-sitter queries (via `queries.scm` files
  per language) for symbol extraction in `ra-ap-syntax`. We follow the
  same pattern.
- **GitHub Monakai / Sourcegraph ctags**: similar capture-based extraction.
  Our approach adds graph edges, not just symbol lists.

## Migration

First extraction RFC — no prior format to migrate.

## Testing strategy

All tests use `tree-sitter-python` in `[dev-dependencies]`:

1. Unit: parse fixed Python snippets, assert specific node paths exist in Store.
2. Property (proptest): round-trip — extract then re-extract same source,
   assert Store contents are identical (idempotent).
3. SLA gate: extract 1 000-line Python file (generated), assert wall-time < 100 ms.

## Open questions

| # | Question | Status |
|---|----------|--------|
| 1 | Should the extractor emit `SourceSpan` attributes on nodes (line/col ranges)? | deferred to RFC-0002b |
| 2 | When is a `Contains` edge redundant with Trunk ancestry? | Keep both for v0.1 — Synapse edge enables cross-file queries without Trunk walk |
