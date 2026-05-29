//! # mycelium-hyphae
//!
//! Hyphae — the CSS-selector-inspired query language for Mycelium.
//!
//! See [RFC-0003](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0003-hyphae-query-language.md)
//! for the full grammar specification.
//!
//! ## Quick start
//!
//! ```
//! use mycelium_hyphae::parser::parse;
//! use mycelium_hyphae::ast::Ast;
//!
//! let ast = parse("#login").unwrap();
//! if let Ast::SelectorList(selectors) = &ast {
//!     assert_eq!(selectors.len(), 1);
//! }
//! ```
//!
//! ## Modules
//!
//! - [`ast`] — AST types produced by the parser.
//! - [`lexer`] — token types and the `tokenise` function.
//! - [`parser`] — entry point: the `parse` function.

#![doc(html_root_url = "https://docs.rs/mycelium-hyphae")]

pub mod ast;
pub mod evaluator;
pub mod lexer;
pub mod parser;

/// Parse a Hyphae query string into an [`ast::Ast`].
///
/// This is a convenience re-export of [`parser::parse`].
///
/// # Errors
///
/// Returns [`parser::ParseError`] on invalid input.
///
/// # Examples
///
/// ```
/// let ast = mycelium_hyphae::parse("#login").unwrap();
/// # let _ = ast;
/// ```
pub fn parse(input: &str) -> Result<ast::Ast, parser::ParseError> {
    parser::parse(input)
}
