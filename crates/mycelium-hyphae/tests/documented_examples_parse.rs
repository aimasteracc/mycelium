//! Regression guard: every Hyphae example shipped in user-facing docs and tool
//! descriptions MUST parse.
//!
//! Background: the documented examples in README.md and the SDK snippets used
//! the dot-less form `function:calls(#Foo)`, which is NOT valid Hyphae — a kind
//! selector requires a leading dot (`.function`). The dot-less form produces
//! `hyphae parse error: unexpected token Ident("function") at position 0`, so
//! every copy-pasted example failed. See `fix/hyphae-doc-examples-parse`.
//!
//! PURPOSE: this test fails if anyone reintroduces a non-parsing documented
//! example. When you add or change an example in any of these places, add the
//! exact string to `DOCUMENTED_EXAMPLES` below and keep them in sync:
//!   - README.md (Hyphae + Node SDK + Python SDK snippets)
//!   - the `mycelium_query` MCP tool description (crates/mycelium-mcp/src/lib.rs)
//!   - the CLI `query` subcommand help (crates/mycelium-cli/src/main.rs)
//!
//! Only list forms that actually parse. Grammar reference:
//! rfcs/0003-hyphae-query-language.md.

use mycelium_hyphae::parse;

/// The canonical set of Hyphae expressions advertised to users. Every entry
/// here is copy-pasteable and MUST parse successfully.
const DOCUMENTED_EXAMPLES: &[&str] = &[
    "#Foo",                  // by symbol name
    "*:calls(#Foo)",         // callers of Foo (any kind)
    ".function:calls(#Foo)", // functions that call Foo
    ".class:has(.method)",   // classes that contain a method
    ".function",             // all function symbols
    ".class>.method",        // methods directly nested in a class
    // RFC-0124: attribute filters may follow pseudo-classes (any order).
    "*:calls(#Foo)[file=src/x.rs]", // callers of Foo defined in src/x.rs
    // skills/hyphae-query/SKILL.md composition example — advertised since
    // RFC-0091 but only parseable since RFC-0124 (attr after pseudo).
    ".class:has(.method:calls(#log))[language=python]",
];

#[test]
fn every_documented_example_parses() {
    for expr in DOCUMENTED_EXAMPLES {
        assert!(
            parse(expr).is_ok(),
            "documented Hyphae example failed to parse: {expr:?}\n\
             A kind selector needs a leading dot (`.function`, not `function`). \
             Keep README / tool descriptions in sync with DOCUMENTED_EXAMPLES.",
        );
    }
}

/// Negative guard: the dot-less kind-selector form that caused the original bug
/// must NOT parse. This pins the grammar so a future change that silently
/// accepted `function:calls(...)` would surface here for review.
#[test]
fn dotless_kind_selector_is_rejected() {
    assert!(
        parse("function:calls(#Foo)").is_err(),
        "dot-less kind selector unexpectedly parsed — the documented form \
         requires a leading dot (`.function`)",
    );
}
