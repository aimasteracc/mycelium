# RFC-0124 — Hyphae: Attribute Filters After Pseudo-Classes

| 字段 | 值 |
|------|----|
| RFC  | 0124 |
| 状态 | Implemented |
| 作者 | rust-implementer (Hive AI agent) |
| 日期 | 2026-06-11 |
| 参考 | RFC-0003 (Hyphae grammar), RFC-0091 (jQuery selector extensions) |
| 影响路径 | `crates/mycelium-hyphae/src/parser.rs` (grammar), CLI `query` help, MCP `mycelium_query` description |

## 摘要

Allow attribute filters (`[attr=value]`) and pseudo-classes (`:pseudo(arg)`)
to appear in **any order** after the base of a simple selector. Strictly
additive grammar extension: every query that parses today keeps parsing with
identical semantics.

## Motivation (live agent QA finding)

An agent's most natural "filter callers by file" query hard-errors at parse
time:

```
*:calls(#upsert_node)[file=crates/mycelium-cli/src/index.rs]
→ hyphae parse error: UnexpectedToken("LBracket", ...)
```

All three attribute filters (`[language=…]`, `[file=…]`, `[kind=…]`) are
implemented and working — but RFC-0091's grammar only accepted them **before**
pseudo-classes. CSS allows attribute selectors and pseudo-classes in any
order after the base; agents trained on CSS write the pseudo-class first.
The fixed ordering was an arbitrary expressiveness gap in the headline DSL.

## Grammar change

RFC-0091 §文法补全 declared:

```
simple ::= base attribute_filter* pseudo_class*
```

This RFC amends it to:

```
simple ::= base (attribute_filter | pseudo_class)*
```

`attribute_filter` and `pseudo_class` productions are unchanged. The AST is
unchanged (`SimpleSelector { base, attributes, pseudo_classes }`): the parser
collects each kind into its own list in source order. No lexer change.

## Semantics (normative)

**Filters compose by set intersection and are therefore ORDER-INDEPENDENT:**

```
a[x]:p  ≡  a:p[x]
```

for every attribute filter `[x]` and every pseudo-class `:p`. This is the
normative rule; the evaluator already satisfies it because every filter is a
per-node predicate computed against the store, never against the running
candidate set:

- Edge pseudos (`:calls`, `:callers`, `:imports`, `:extends`, `:implements`)
  test a node's edges against targets evaluated independently over the whole
  store.
- `:not(X)` / `:has(X)` evaluate `X` against the whole store, then test each
  candidate; set difference/containment commutes with intersection.
- **Structural pseudos** (`:first-child`, `:last-child`, `:only-child`,
  `:nth-child(N)`) rank a node against **ALL of its siblings in the store**
  (`Evaluator::apply_pseudo` filters `store.all_symbols(None, None)` by
  parent), **never against the attribute-filtered candidate set**. This is
  CSS semantics: structural position is a property of the tree, independent
  of the other parts of the simple selector. So an attribute filter before or
  after a structural pseudo cannot change which node is "first".

Because the AST is order-erasing and the evaluator applies attributes then
pseudos as successive per-node predicates, order-independence falls out with
**zero evaluator changes** — it is pinned by tests rather than re-implemented.

## Backward compatibility

Pure superset: the old grammar's sentences (`base attribute* pseudo*`) are a
subset of the new one's (`base (attribute | pseudo)*`), and identical inputs
produce identical ASTs. `eval_checked` validation (unknown attribute / pseudo
/ kind rejection, #703 / #749) operates on the AST and therefore covers the
new positions unchanged.

## Surfaces (Charter §5.13 / RFC-0090)

The example `*:calls(#Foo)[file=src/x.rs]` ("callers of Foo defined in
src/x.rs") is added in lockstep to:

- CLI `mycelium query` help (`crates/mycelium-cli/src/main.rs`)
- MCP `mycelium_query` tool description (`crates/mycelium-mcp/src/lib.rs`)
- `crates/mycelium-hyphae/tests/documented_examples_parse.rs` (regression guard)

## Acceptance criteria

- [x] `*:calls(#Foo)[file=src/x.rs]` parses (previously
      `UnexpectedToken("LBracket")`).
- [x] Order-independence pinned by test: `*:calls(#Foo)[file=a]` ≡
      `*[file=a]:calls(#Foo)` over a store with callers in two files.
- [x] Structural-pseudo order-independence pinned by test:
      `*[file=a]:first-child` ≡ `*:first-child[file=a]`.
- [x] Interleavings parse and evaluate:
      `.function[language=rust]:calls(#Foo)[file=src/x.rs]`.
- [x] `eval_checked` rejects unknown attribute / pseudo names in the new
      positions.
- [x] No regression: all existing parser / evaluator / documented-examples
      tests stay green; new example added to `documented_examples_parse.rs`.
- [x] CLI help and MCP description updated in lockstep with the same example
      string.
