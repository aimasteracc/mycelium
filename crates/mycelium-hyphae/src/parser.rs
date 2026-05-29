//! Recursive-descent parser for the Hyphae query language.
//!
//! Consumes a flat token stream (produced by [`crate::lexer::tokenise`]) and
//! produces an [`Ast`].
//!
//! See [RFC-0003](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0003-hyphae-query-language.md)
//! for the grammar this parser implements.

use crate::{
    ast::{Ast, BaseSelector, Combinator, PseudoClass, Selector, SimpleSelector},
    lexer::{Token, tokenise},
};

/// Error type for Hyphae parse failures.
#[derive(thiserror::Error, Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// A token was encountered that does not fit the grammar at that position.
    #[error("unexpected token `{0}` at position {1}")]
    UnexpectedToken(String, usize),

    /// The input ended before the grammar was satisfied.
    #[error("unexpected end of input")]
    UnexpectedEof,

    /// The lexer encountered an unrecognised character.
    #[error("lexer error at position {0}")]
    LexError(usize),
}

/// Parse a Hyphae query string into an [`Ast`].
///
/// # Errors
///
/// Returns a [`ParseError`] if the input cannot be parsed as a valid Hyphae
/// query.
///
/// # Examples
///
/// ```
/// use mycelium_hyphae::parser::parse;
/// use mycelium_hyphae::ast::{Ast, Selector, SimpleSelector, BaseSelector};
///
/// let ast = parse("#login").unwrap();
/// assert_eq!(
///     ast,
///     Ast::SelectorList(vec![
///         Selector::Simple(SimpleSelector {
///             base: BaseSelector::Name("login".to_owned()),
///             pseudo_classes: vec![],
///         })
///     ])
/// );
/// ```
pub fn parse(input: &str) -> Result<Ast, ParseError> {
    let raw = tokenise(input).map_err(ParseError::LexError)?;
    // Filter leading/trailing whitespace; keep internal whitespace for
    // descendant combinator detection.
    let tokens: Vec<(Token<'_>, usize)> =
        raw.into_iter().map(|(t, span)| (t, span.start)).collect();
    let mut parser = Parser { tokens, pos: 0 };
    let ast = parser.parse_query()?;
    // Skip any trailing whitespace
    parser.skip_ws();
    if parser.pos < parser.tokens.len() {
        let (tok, pos) = &parser.tokens[parser.pos];
        return Err(ParseError::UnexpectedToken(format!("{tok:?}"), *pos));
    }
    Ok(ast)
}

// ── Internal parser state ────────────────────────────────────────────────────

struct Parser<'src> {
    tokens: Vec<(Token<'src>, usize)>,
    pos: usize,
}

impl<'src> Parser<'src> {
    // ── Helpers ──────────────────────────────────────────────────────────

    fn peek(&self) -> Option<&Token<'src>> {
        self.tokens.get(self.pos).map(|(t, _)| t)
    }

    fn advance(&mut self) -> Option<(Token<'src>, usize)> {
        let item = self.tokens.get(self.pos).cloned();
        if item.is_some() {
            self.pos += 1;
        }
        item
    }

    /// Consume a `Ws` token if present, returning whether one was consumed.
    fn skip_ws(&mut self) -> bool {
        if matches!(self.peek(), Some(Token::Ws)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn current_pos(&self) -> usize {
        self.tokens.get(self.pos).map_or(usize::MAX, |(_, p)| *p)
    }

    // ── Grammar productions ───────────────────────────────────────────────

    /// `query ::= selector_list EOF`
    fn parse_query(&mut self) -> Result<Ast, ParseError> {
        self.skip_ws();
        if self.peek().is_none() {
            return Err(ParseError::UnexpectedEof);
        }
        self.parse_selector_list()
    }

    /// `selector_list ::= selector ( ',' selector )*`
    fn parse_selector_list(&mut self) -> Result<Ast, ParseError> {
        let mut selectors = vec![self.parse_selector()?];
        loop {
            // Skip optional whitespace around comma
            self.skip_ws();
            if matches!(self.peek(), Some(Token::Comma)) {
                self.advance();
                self.skip_ws();
                selectors.push(self.parse_selector()?);
            } else {
                break;
            }
        }
        Ok(Ast::SelectorList(selectors))
    }

    /// `selector ::= simple ( combinator simple )*`
    fn parse_selector(&mut self) -> Result<Selector, ParseError> {
        let mut left = Selector::Simple(self.parse_simple()?);
        loop {
            // Determine the next combinator, if any.
            // The combinator may be:
            //   - `>` or `~` (explicit, possibly surrounded by Ws)
            //   - Ws followed immediately by a simple selector (descendant)
            let had_ws = self.skip_ws();

            let combinator = match self.peek() {
                Some(Token::Gt) => {
                    self.advance();
                    self.skip_ws();
                    Combinator::Child
                }
                Some(Token::Tilde) => {
                    self.advance();
                    self.skip_ws();
                    Combinator::Sibling
                }
                Some(Token::Hash(_) | Token::Dot(_) | Token::Star) if had_ws => {
                    Combinator::Descendant
                }
                _ => break,
            };

            let right = self.parse_simple()?;
            left = Selector::Combined {
                left: Box::new(left),
                combinator,
                right: Box::new(Selector::Simple(right)),
            };
        }
        Ok(left)
    }

    /// `simple ::= ( name_selector | kind_selector | universal ) pseudo_class*`
    fn parse_simple(&mut self) -> Result<SimpleSelector, ParseError> {
        let base = match self.peek() {
            Some(Token::Hash(_)) => {
                let (tok, _) = self.advance().unwrap();
                let Token::Hash(name) = tok else {
                    unreachable!()
                };
                BaseSelector::Name(name.to_owned())
            }
            Some(Token::Dot(_)) => {
                let (tok, _) = self.advance().unwrap();
                let Token::Dot(kind) = tok else {
                    unreachable!()
                };
                BaseSelector::Kind(kind.to_owned())
            }
            Some(Token::Star) => {
                self.advance();
                BaseSelector::Universal
            }
            Some(other) => {
                return Err(ParseError::UnexpectedToken(
                    format!("{other:?}"),
                    self.current_pos(),
                ));
            }
            None => return Err(ParseError::UnexpectedEof),
        };

        let mut pseudo_classes = Vec::new();
        while matches!(self.peek(), Some(Token::Colon(_))) {
            pseudo_classes.push(self.parse_pseudo_class()?);
        }

        Ok(SimpleSelector {
            base,
            pseudo_classes,
        })
    }

    /// `pseudo_class ::= ':' IDENT ( '(' selector_list ')' )?`
    fn parse_pseudo_class(&mut self) -> Result<PseudoClass, ParseError> {
        let (tok, pos) = self.advance().ok_or(ParseError::UnexpectedEof)?;
        let name = if let Token::Colon(n) = tok {
            n.to_owned()
        } else {
            return Err(ParseError::UnexpectedToken(format!("{tok:?}"), pos));
        };

        let argument = if matches!(self.peek(), Some(Token::LParen)) {
            self.advance(); // consume `(`
            self.skip_ws();
            // Empty parens `()` means "match everything" — argument is None.
            if matches!(self.peek(), Some(Token::RParen)) {
                self.advance(); // consume `)`
                None
            } else {
                let inner = self.parse_selector_list()?;
                self.skip_ws();
                match self.advance() {
                    Some((Token::RParen, _)) => {}
                    Some((tok, pos)) => {
                        return Err(ParseError::UnexpectedToken(format!("{tok:?}"), pos));
                    }
                    None => return Err(ParseError::UnexpectedEof),
                }
                Some(Box::new(inner))
            }
        } else {
            None
        };

        Ok(PseudoClass { name, argument })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{BaseSelector, Combinator, PseudoClass, Selector, SimpleSelector};

    // ── Helpers ───────────────────────────────────────────────────────────

    fn simple_name(name: &str) -> Selector {
        Selector::Simple(SimpleSelector {
            base: BaseSelector::Name(name.to_owned()),
            pseudo_classes: vec![],
        })
    }

    fn simple_kind(kind: &str) -> Selector {
        Selector::Simple(SimpleSelector {
            base: BaseSelector::Kind(kind.to_owned()),
            pseudo_classes: vec![],
        })
    }

    fn simple_universal() -> Selector {
        Selector::Simple(SimpleSelector {
            base: BaseSelector::Universal,
            pseudo_classes: vec![],
        })
    }

    fn combined(left: Selector, combinator: Combinator, right: Selector) -> Selector {
        Selector::Combined {
            left: Box::new(left),
            combinator,
            right: Box::new(right),
        }
    }

    fn selector_list(selectors: Vec<Selector>) -> Ast {
        Ast::SelectorList(selectors)
    }

    // ── Tests ─────────────────────────────────────────────────────────────

    #[test]
    fn name_selector() {
        let ast = parse("#login").unwrap();
        assert_eq!(ast, selector_list(vec![simple_name("login")]));
    }

    #[test]
    fn kind_selector() {
        let ast = parse(".function").unwrap();
        assert_eq!(ast, selector_list(vec![simple_kind("function")]));
    }

    #[test]
    fn universal_selector() {
        let ast = parse("*").unwrap();
        assert_eq!(ast, selector_list(vec![simple_universal()]));
    }

    #[test]
    fn child_combinator() {
        let ast = parse("#Foo>.bar").unwrap();
        let expected = selector_list(vec![combined(
            simple_name("Foo"),
            Combinator::Child,
            simple_kind("bar"),
        )]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn child_combinator_with_spaces() {
        let ast = parse("#Foo > .bar").unwrap();
        let expected = selector_list(vec![combined(
            simple_name("Foo"),
            Combinator::Child,
            simple_kind("bar"),
        )]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn sibling_combinator() {
        let ast = parse("#a~#b").unwrap();
        let expected = selector_list(vec![combined(
            simple_name("a"),
            Combinator::Sibling,
            simple_name("b"),
        )]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn descendant_combinator() {
        let ast = parse("#parent .child").unwrap();
        let expected = selector_list(vec![combined(
            simple_name("parent"),
            Combinator::Descendant,
            simple_kind("child"),
        )]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn selector_list_two_items() {
        let ast = parse("#foo, .bar").unwrap();
        assert_eq!(
            ast,
            selector_list(vec![simple_name("foo"), simple_kind("bar")])
        );
    }

    #[test]
    fn selector_list_three_items() {
        let ast = parse("#a, #b, #c").unwrap();
        assert_eq!(
            ast,
            selector_list(vec![simple_name("a"), simple_name("b"), simple_name("c"),])
        );
    }

    #[test]
    fn pseudo_class_no_arg() {
        let ast = parse("#foo:calls").unwrap();
        let expected = selector_list(vec![Selector::Simple(SimpleSelector {
            base: BaseSelector::Name("foo".to_owned()),
            pseudo_classes: vec![PseudoClass {
                name: "calls".to_owned(),
                argument: None,
            }],
        })]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn pseudo_class_with_arg() {
        let ast = parse("#foo:calls(#bar)").unwrap();
        let expected = selector_list(vec![Selector::Simple(SimpleSelector {
            base: BaseSelector::Name("foo".to_owned()),
            pseudo_classes: vec![PseudoClass {
                name: "calls".to_owned(),
                argument: Some(Box::new(selector_list(vec![simple_name("bar")]))),
            }],
        })]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn pseudo_class_imports() {
        let ast = parse(".function:imports").unwrap();
        let expected = selector_list(vec![Selector::Simple(SimpleSelector {
            base: BaseSelector::Kind("function".to_owned()),
            pseudo_classes: vec![PseudoClass {
                name: "imports".to_owned(),
                argument: None,
            }],
        })]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn multiple_pseudo_classes() {
        let ast = parse("#foo:calls:imports").unwrap();
        let expected = selector_list(vec![Selector::Simple(SimpleSelector {
            base: BaseSelector::Name("foo".to_owned()),
            pseudo_classes: vec![
                PseudoClass {
                    name: "calls".to_owned(),
                    argument: None,
                },
                PseudoClass {
                    name: "imports".to_owned(),
                    argument: None,
                },
            ],
        })]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn complex_chain() {
        // #Foo > .bar ~ #baz
        let ast = parse("#Foo > .bar ~ #baz").unwrap();
        let inner = combined(simple_name("Foo"), Combinator::Child, simple_kind("bar"));
        let expected = selector_list(vec![combined(
            inner,
            Combinator::Sibling,
            simple_name("baz"),
        )]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn universal_with_pseudo() {
        let ast = parse("*:calls").unwrap();
        let expected = selector_list(vec![Selector::Simple(SimpleSelector {
            base: BaseSelector::Universal,
            pseudo_classes: vec![PseudoClass {
                name: "calls".to_owned(),
                argument: None,
            }],
        })]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn leading_whitespace_ignored() {
        let a = parse("#foo").unwrap();
        let b = parse("  #foo  ").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn empty_input_error() {
        assert_eq!(parse(""), Err(ParseError::UnexpectedEof));
    }

    #[test]
    fn whitespace_only_error() {
        assert_eq!(parse("   "), Err(ParseError::UnexpectedEof));
    }

    #[test]
    fn invalid_char_error() {
        assert!(matches!(parse("@bad"), Err(ParseError::LexError(_))));
    }

    #[test]
    fn unclosed_paren_error() {
        assert!(parse("#foo:calls(#bar").is_err());
    }

    #[test]
    fn hyphenated_name() {
        let ast = parse("#my-symbol").unwrap();
        assert_eq!(ast, selector_list(vec![simple_name("my-symbol")]));
    }

    #[test]
    fn hyphenated_kind() {
        let ast = parse(".arrow-function").unwrap();
        assert_eq!(ast, selector_list(vec![simple_kind("arrow-function")]));
    }

    #[test]
    fn nested_pseudo_class_arg() {
        // #foo:calls(.function:imports)
        let ast = parse("#foo:calls(.function:imports)").unwrap();
        let inner_selector = Selector::Simple(SimpleSelector {
            base: BaseSelector::Kind("function".to_owned()),
            pseudo_classes: vec![PseudoClass {
                name: "imports".to_owned(),
                argument: None,
            }],
        });
        let expected = selector_list(vec![Selector::Simple(SimpleSelector {
            base: BaseSelector::Name("foo".to_owned()),
            pseudo_classes: vec![PseudoClass {
                name: "calls".to_owned(),
                argument: Some(Box::new(selector_list(vec![inner_selector]))),
            }],
        })]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn kind_descendant_name() {
        // .class #method — any method named "method" under a class
        let ast = parse(".class #method").unwrap();
        let expected = selector_list(vec![combined(
            simple_kind("class"),
            Combinator::Descendant,
            simple_name("method"),
        )]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn selector_list_no_spaces_around_comma() {
        let ast = parse("#a,#b").unwrap();
        assert_eq!(ast, selector_list(vec![simple_name("a"), simple_name("b")]));
    }
}

// ── Snapshot tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod snapshot_tests {
    use super::*;

    macro_rules! snap {
        ($name:ident, $input:expr) => {
            #[test]
            fn $name() {
                let result = parse($input);
                insta::assert_debug_snapshot!(result);
            }
        };
    }

    snap!(snap_name_selector, "#login");
    snap!(snap_kind_selector, ".function");
    snap!(snap_universal, "*");
    snap!(snap_child, "#Foo>.bar");
    snap!(snap_child_spaces, "#Foo > .bar");
    snap!(snap_sibling, "#a~#b");
    snap!(snap_descendant, "#parent .child");
    snap!(snap_comma_list, "#foo, .bar");
    snap!(snap_three_list, "#a, #b, #c");
    snap!(snap_pseudo_no_arg, "#foo:calls");
    snap!(snap_pseudo_with_arg, "#foo:calls(#bar)");
    snap!(snap_imports, ".function:imports");
    snap!(snap_multi_pseudo, "#foo:calls:imports");
    snap!(snap_complex_chain, "#Foo > .bar ~ #baz");
    snap!(snap_universal_pseudo, "*:calls");
    snap!(snap_hyphenated_name, "#my-symbol");
    snap!(snap_hyphenated_kind, ".arrow-function");
    snap!(snap_nested_pseudo, "#foo:calls(.function:imports)");
    snap!(snap_kind_descendant_name, ".class #method");
    snap!(snap_no_space_comma, "#a,#b");
    snap!(snap_empty_err, "");
    snap!(snap_invalid_char_err, "@bad");
    snap!(snap_unclosed_paren_err, "#foo:calls(#bar");
}
