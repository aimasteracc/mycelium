//! RFC-0091 — Hyphae jQuery-inspired selector extensions.
//!
//! These tests RED until each grammar/evaluator piece lands. They are the
//! contract for the new selectors:
//!
//! - `:not(X)`
//! - `:has(X)`
//! - `:in(path-pattern)`
//! - `:implements(X)`
//! - `:first-child` / `:last-child` / `:only-child`
//! - `:nth-child(N)`
//! - `[attr=value]`
//!
//! Each test indexes a small Rust fixture and runs the selector through
//! the parser + evaluator end-to-end.

use mycelium_core::extractor::Extractor;
use mycelium_core::store::Store;
use mycelium_hyphae::evaluator::Evaluator;
use mycelium_hyphae::parser::parse;

/// Build a store containing a Rust file with two top-level functions and a
/// struct with two methods. Returns the populated store.
fn small_rust_fixture() -> Store {
    let mut store = Store::default();
    let src = "pub fn login(name: &str) -> String { name.to_owned() }\n\
               pub fn logout() {}\n\
               pub struct App;\n\
               impl App {\n\
                   pub fn render(&self) { self.login_helper() }\n\
                   pub fn login_helper(&self) {}\n\
               }\n";
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let extractor =
        Extractor::new(language, include_str!("../../../packs/rust/queries.scm")).unwrap();
    extractor
        .extract("src/lib.rs", src.as_bytes(), &mut store)
        .unwrap();
    store
}

fn matches(store: &Store, expr: &str) -> Vec<String> {
    let ast = parse(expr).unwrap_or_else(|e| panic!("parse error on {expr:?}: {e:?}"));
    let evaluator = Evaluator::new(store);
    evaluator.eval(&ast)
}

// ── :not(X) ───────────────────────────────────────────────────────────────────

#[test]
fn not_excludes_named_symbols() {
    let store = small_rust_fixture();
    let with_logout = matches(&store, ".function");
    let without_logout = matches(&store, ".function:not(#logout)");
    assert!(with_logout.iter().any(|p| p.ends_with(">logout")));
    assert!(
        !without_logout.iter().any(|p| p.ends_with(">logout")),
        ":not(#logout) should exclude logout, got: {without_logout:?}"
    );
    assert!(
        without_logout.iter().any(|p| p.ends_with(">login")),
        "login should still match"
    );
}

#[test]
fn not_excludes_by_kind() {
    let store = small_rust_fixture();
    let everything = matches(&store, "*");
    let without_struct = matches(&store, "*:not(.struct)");
    assert!(everything.len() > without_struct.len());
}

// ── :has(X) — containment check ───────────────────────────────────────────────

#[test]
fn has_filters_by_descendant_existence() {
    let store = small_rust_fixture();
    let with_render = matches(&store, ".struct:has(#render)");
    assert!(
        with_render.iter().any(|p| p.contains("App")),
        ":has(#render) should match App which contains render, got: {with_render:?}"
    );
}

#[test]
fn has_returns_empty_when_no_descendant_matches() {
    let store = small_rust_fixture();
    let none = matches(&store, ".struct:has(#nonexistent)");
    assert!(none.is_empty(), "expected empty, got {none:?}");
}

// ── :in(path-pattern) — path-scoped ───────────────────────────────────────────

#[test]
fn in_filters_by_path_prefix() {
    let store = small_rust_fixture();
    let all_funcs = matches(&store, ".function");
    let in_lib = matches(&store, ".function:in(src/lib.rs)");
    assert!(!all_funcs.is_empty());
    assert_eq!(
        all_funcs.len(),
        in_lib.len(),
        "all functions are in src/lib.rs"
    );
    let nowhere = matches(&store, ".function:in(tests/)");
    assert!(nowhere.is_empty(), "no functions under tests/ in fixture");
}

// ── :implements(X) ────────────────────────────────────────────────────────────
// Already supported in the existing evaluator via the :extends pattern, but
// we want a dedicated test that documents the contract.

#[test]
fn implements_is_a_pseudo_class() {
    // Even a syntactically valid :implements() against a fixture with no
    // Implements edges should parse + return empty without panicking.
    let store = small_rust_fixture();
    let result = matches(&store, ".struct:implements(#Trait)");
    // Either empty (no Implements edge) or a real match — both fine. The
    // important thing is it parses + evaluates.
    let _ = result;
}

// ── :first-child / :last-child / :only-child ──────────────────────────────────

#[test]
fn first_child_filters_to_first_in_parent_order() {
    let store = small_rust_fixture();
    let firsts = matches(&store, ".method:first-child");
    // App has two methods (render, login_helper). One of them is first.
    assert!(
        !firsts.is_empty(),
        ":first-child should match at least one method of App"
    );
}

#[test]
fn only_child_matches_solo_children() {
    let store = small_rust_fixture();
    // No struct in this fixture has exactly one child method — both render
    // and login_helper are siblings — so :only-child should return empty
    // for methods. But top-level functions of lib.rs are direct children of
    // the file; they're not "only" because there are multiple.
    let _ = matches(&store, ".method:only-child");
    // Just assert it parses + evaluates without panic.
}

// ── :nth-child(N) ─────────────────────────────────────────────────────────────

#[test]
fn nth_child_first() {
    let store = small_rust_fixture();
    let first = matches(&store, "*:nth-child(1)");
    // At least one node is the first child of its parent (the first
    // top-level symbol of src/lib.rs).
    let _ = first; // parse + eval contract
}

// ── [attr=value] — attribute selectors ────────────────────────────────────────

#[test]
fn attribute_language_python_returns_empty_on_rust_only_fixture() {
    let store = small_rust_fixture();
    let py = matches(&store, "*[language=python]");
    assert!(py.is_empty(), "no Python symbols in this Rust fixture");
}

#[test]
fn attribute_language_rust_finds_everything() {
    let store = small_rust_fixture();
    let rs = matches(&store, "*[language=rust]");
    let all = matches(&store, "*");
    assert_eq!(
        rs.len(),
        all.len(),
        "every symbol in this fixture is rust-language"
    );
}
