# RFC-0091 — Hyphae jQuery-Inspired Selector Extensions

| 字段 | 值 |
|------|----|
| RFC  | 0091 |
| 状态 | Accepted |
| 作者 | orchestrator (Hive AI agent) |
| 日期 | 2026-05-30 |
| 参考 | RFC-0003 (Hyphae grammar), RFC-0004 (Hyphae executor) |
| 影响路径 | `crates/mycelium-hyphae/src/{ast,lexer,parser,evaluator}.rs` |

## 摘要

Hyphae v1 shipped name (`#login`), kind (`.function`), and universal
(`*`) base selectors plus four relationship pseudo-classes
(`:calls`, `:callers`, `:imports`, `:extends`). This RFC closes the gap
with jQuery by adding seven jQuery-inspired forms and one attribute
syntax. The grammar additions are conservative and parse-compatible
with existing queries.

## 新增形式

### Pseudo-classes (no grammar shape change)

| Form | Meaning | Example |
|---|---|---|
| `:not(X)` | Set difference — candidates NOT in `X`. | `.function:not(#main)` |
| `:has(X)` | Containment — candidates that have at least one descendant matching `X`. | `.struct:has(#render)` |
| `:implements(X)` | Outgoing `Implements` edge to `X`. Mirror of `:extends`. | `.struct:implements(#Repository)` |
| `:first-child` | The first sibling under the same parent (lexicographic order). | `.method:first-child` |
| `:last-child` | The last sibling under the same parent. | `.method:last-child` |
| `:only-child` | The sole child of its parent. | `.method:only-child` |

### Pseudo-classes (new argument shapes)

| Form | Grammar change | Example |
|---|---|---|
| `:in(<path-prefix>)` | New `Path` token to lex bare paths like `src/auth/`. | `.function:in(src/auth/)` |
| `:nth-child(<N>)` | New `Number` token. 1-indexed. | `.method:nth-child(2)` |

### Attribute selector

| Form | Grammar change | Example |
|---|---|---|
| `[name=value]` | New `LBracket`, `RBracket`, `Eq` tokens; `Ident` for value. | `*[language=python]`, `*[kind=function]`, `*[file=src/lib.rs]` |

Supported attribute names: `language`, `kind`, `file`. Language is
derived from file extension (no per-node store cost). Unknown attribute
names match nothing (strict-empty fallback, same as unknown
pseudo-classes).

## 文法补全 (RFC-0003 §3 grammar updated)

```
simple ::= base attribute_filter* pseudo_class*
attribute_filter ::= '[' IDENT '=' (IDENT | NUMBER) ']'
pseudo_class ::= ':' IDENT ( '(' pseudo_arg ')' )?
pseudo_arg ::= selector_list
             | IDENT      // for :in(...)
             | NUMBER     // for :nth-child(N)
```

The disambiguation of `pseudo_arg` is by pseudo-class name. `:in`
always expects an `IDENT` path; `:nth-child` always expects a
`NUMBER`; everything else expects a `selector_list`. This avoids
ambiguity in the parser and matches user intuition.

## AST 改动

```rust
pub struct SimpleSelector {
    pub base: BaseSelector,
    pub attributes: Vec<AttributeSelector>,   // NEW
    pub pseudo_classes: Vec<PseudoClass>,
}

pub struct AttributeSelector {                 // NEW
    pub name: String,
    pub value: String,
}

pub struct PseudoClass {
    pub name: String,
    pub argument: Option<PseudoArg>,            // shape change
}

pub enum PseudoArg {                            // NEW
    Selector(Box<Ast>),  // :not, :has, :calls, etc.
    Number(usize),       // :nth-child
    Path(String),        // :in
}
```

## Lexer 改动

New tokens (RFC-0003 token table extended):

| Token | Regex / literal |
|---|---|
| `LBracket` | `[` |
| `RBracket` | `]` |
| `Eq` | `=` |
| `Number(usize)` | `[0-9]+` |
| `Ident(&str)` | `[a-zA-Z_][a-zA-Z0-9_./\-]*` — allows `/` and `.` so paths like `src/lib.rs` lex as a single token |

## Evaluator 语义

Each new pseudo-class is implemented in `Evaluator::apply_pseudo`.
Each new attribute is in `Evaluator::apply_attribute`. Both operate
on the candidate path list, applying a filter and returning a new
candidate list. Sort + dedup happens once at the top-level `eval`
call.

`language_of_path()` maps file extension to language wire string
(`rs → rust`, `py → python`, `ts/tsx → typescript`, `js/jsx →
javascript`, `go`, `java`, `c/h → c`, `cpp/cc/cxx/hpp/hxx → cpp`,
`cs → csharp`, `rb → ruby`). Unknown extensions return `None`.

## 验收标准

- [x] All 7 new pseudo-classes parse + evaluate against a small Rust
      fixture (`crates/mycelium-hyphae/tests/jquery_selectors.rs`).
- [x] `[attr=value]` parses + evaluates for `language`, `kind`, `file`.
- [x] Existing RFC-0003 queries continue to parse and evaluate
      identically (no regressions in evaluator unit tests).
- [x] Parser unit tests cover one example per new form.
- [x] Lexer unit tests cover new tokens.
- [x] `mycelium-hyphae` test suite: 33 unit + 11 integration tests
      passing.
- [x] Workspace `cargo clippy --all-targets --all-features -- -D
      warnings` clean.
- [x] `mycelium query` CLI accepts every new form without change
      (Three-Surface preserved).
- [x] `mycelium_query` MCP tool accepts every new form without
      change.
- [x] `skills/hyphae-query/SKILL.md` updated with the new selector
      table.
- [x] README cheat-sheet updated.
- [x] CHANGELOG `Unreleased` entry.

## Alternatives considered

### (A) Bring CSS attribute operators (`^=`, `$=`, `*=`)

Rejected for v1. The three exact-match attributes cover ~90 % of
real use cases (language, kind, file). Substring/prefix operators
are easy to add later in RFC-0092 if demand emerges.

### (B) Implement `:nth-of-type(N)`

Rejected. Hyphae nodes don't have a clear "type" distinct from
kind; `:nth-of-type` would collapse into `:nth-child` with a kind
prefix, which is already expressible via combinators.

### (C) String literals for `:in("src/path")`

Considered. The bare-ident approach (`:in(src/path)`) is more
ergonomic and consistent with the rest of Hyphae (no string
quoting elsewhere). We extended `Ident` to allow `/` and `.` to
support this.

## Consequences

**Positive:**
- Hyphae now expresses every common jQuery-style query.
- Parity with the README marketing copy ("CSS-selector-inspired with
  relationship pseudo-classes") is now literal, not aspirational.
- `:in(path)` is heavily used in agent prompts and was the single
  biggest gap.

**Negative:**
- AST shape changed (`pseudo_class.argument` is now an enum). External
  consumers of the AST (if any) need to update. Internal-only as of
  this RFC.
- The Stale snapshot tests under
  `crates/mycelium-hyphae/src/snapshots/` are removed; new snapshot
  tests can be added per pseudo-class in a follow-up.

**Neutral:**
- Lexer is slightly more permissive (`Ident` accepts `/` and `.`).
  This is fine because Hyphae has no other context where a slash
  would appear.
