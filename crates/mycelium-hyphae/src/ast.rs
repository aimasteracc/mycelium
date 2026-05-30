//! Abstract Syntax Tree types for Hyphae queries.
//!
//! See [RFC-0003](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0003-hyphae-query-language.md)
//! and [RFC-0091](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0091-hyphae-jquery-selectors.md)
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
///         attributes: vec![],
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

/// A simple (non-combined) selector with optional attribute filters and
/// pseudo-classes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimpleSelector {
    /// The base selector shape.
    pub base: BaseSelector,
    /// Attribute filters such as `[language=python]`. Empty when none.
    pub attributes: Vec<AttributeSelector>,
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

/// `[attr=value]` — matches nodes whose attribute equals the given value.
///
/// Supported attribute names (RFC-0091):
///
/// - `language` — node's source language (e.g. `rust`, `python`).
/// - `kind` — node kind wire string (e.g. `function`, `class`).
/// - `file` — file path component (e.g. `src/lib.rs`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttributeSelector {
    /// Attribute name (left of `=`).
    pub name: String,
    /// Attribute value (right of `=`).
    pub value: String,
}

/// A pseudo-class filter such as `:calls()`, `:not(#x)`, or `:nth-child(2)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PseudoClass {
    /// The pseudo-class name, e.g. `"calls"`, `"not"`, `"in"`, `"nth-child"`.
    pub name: String,
    /// An optional argument. Pseudo-classes that take no argument
    /// (`first-child`, `last-child`, `only-child`) leave this `None`.
    pub argument: Option<PseudoArg>,
}

/// The kinds of arguments a [`PseudoClass`] may carry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PseudoArg {
    /// A nested selector list — `:not(.kind)`, `:has(#name)`, `:calls(X)`.
    Selector(Box<Ast>),
    /// A non-negative integer — `:nth-child(2)`.
    Number(usize),
    /// A bare path-shaped string — `:in(src/auth/)`.
    Path(String),
}
