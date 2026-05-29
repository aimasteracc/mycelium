//! Abstract Syntax Tree types for Hyphae queries.
//!
//! See [RFC-0003](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0003-hyphae-query-language.md)
//! for the full grammar specification.

/// A complete Hyphae query: a comma-separated list of selectors.
///
/// # Examples
///
/// ```
/// # use mycelium_hyphae::ast::{Ast, Selector, SimpleSelector, BaseSelector};
/// let ast = Ast::SelectorList(vec![
///     Selector::Simple(SimpleSelector {
///         base: BaseSelector::Name("login".to_owned()),
///         pseudo_classes: vec![],
///     }),
/// ]);
/// # let _ = ast;
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ast {
    /// A comma-separated list of selectors such as `#foo, .bar`.
    SelectorList(Vec<Selector>),
}

/// A single selector within a [`Ast::SelectorList`].
///
/// Selectors may be simple (`#login`) or two selectors joined by a
/// [`Combinator`] (`#Foo > .bar`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selector {
    /// A single simple selector, optionally decorated with pseudo-classes.
    Simple(SimpleSelector),
    /// Two selectors connected by a combinator.
    Combined {
        /// The left-hand selector.
        left: Box<Self>,
        /// The combinator joining left and right.
        combinator: Combinator,
        /// The right-hand selector.
        right: Box<Self>,
    },
}

/// Combinators that express structural relationships between selectors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Combinator {
    /// `>` — direct parent-to-child edge.
    Child,
    /// `~` — following sibling in the same container.
    Sibling,
    /// ` ` (whitespace) — any ancestor-to-descendant path.
    Descendant,
}

/// A simple (non-combined) selector with an optional list of pseudo-classes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimpleSelector {
    /// The base selector shape.
    pub base: BaseSelector,
    /// Pseudo-classes applied to the base, in order.
    pub pseudo_classes: Vec<PseudoClass>,
}

/// The fundamental matching criterion for a simple selector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BaseSelector {
    /// `#ident` — matches a symbol whose name equals `ident`.
    Name(String),
    /// `.ident` — matches all symbols whose kind equals `ident`.
    Kind(String),
    /// `*` — matches any symbol.
    Universal,
}

/// A pseudo-class filter such as `:calls()` or `:imports(#Foo)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PseudoClass {
    /// The pseudo-class name, e.g. `"calls"` or `"imports"`.
    pub name: String,
    /// An optional nested selector list argument, e.g. `#Foo` in `:calls(#Foo)`.
    pub argument: Option<Box<Ast>>,
}
