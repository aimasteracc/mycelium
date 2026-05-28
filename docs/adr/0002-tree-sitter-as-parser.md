# 0002. tree-sitter as the parser framework

- **Status**: accepted
- **Date**: 2026-05-28
- **RFC**: founders' decision

## Context

Mycelium needs to extract symbols and references from source code in 20+
programming languages. We need to:

- Handle each language with one consistent abstraction.
- Onboard new languages with ≤ 3 files (Charter §4 hard constraint).
- Recover from syntax errors gracefully (developers query incomplete code).
- Run at native performance.

## Decision

Use **tree-sitter** (https://tree-sitter.github.io) as the universal parser,
and use its declarative `.scm` query language for symbol/reference extraction.
Per-language code is forbidden in the engine core.

## Consequences

### Positive

- Single uniform AST API across all languages.
- Mature grammars exist for every Tier-1 and Tier-2 language we plan to support.
- Error recovery is built into the parser.
- Incremental reparsing is supported, enabling sub-second updates after edits.
- The `.scm` query language is expressive enough to extract symbols and edges declaratively for almost every language.

### Negative

- Quality of extraction depends on quality of the upstream grammar.
- Some niche languages (Pascal, ObjC) have grammars but require `hooks.wasm` for quirks.
- tree-sitter's `.scm` queries cannot express truly procedural extraction; we accept this as the boundary case for `hooks.wasm`.

### Neutral / Trade-offs

- We are committed to tree-sitter's stability and continued maintenance. Migration off tree-sitter would be a multi-year undertaking. We accept the dependency risk.

## Alternatives considered

### Alternative A: Hand-written parsers per language
- Pros: maximum control, can extract anything.
- Cons: violates Charter §4; we would need a major engineering investment per language.
- **Rejected**: scales linearly with language count; non-starter.

### Alternative B: ANTLR
- Pros: mature, well-known.
- Cons: Java runtime dependency, slower than tree-sitter, no incremental reparsing.
- **Rejected**: dependency and performance.

### Alternative C: LSP servers as parsers
- Pros: high-quality per-language analysis.
- Cons: heavyweight (a real language server per file), per-language process management, not designed for batch indexing.
- **Rejected**: orchestration cost dwarfs the engineering savings.

### Alternative D: Roslyn-style multi-language framework
- Pros: structured analysis.
- Cons: only exists for a few languages; not portable; not the right abstraction.
- **Rejected**.
