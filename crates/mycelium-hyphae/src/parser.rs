//! Recursive-descent parser for the Hyphae query language.
//!
//! See [RFC-0003](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0003-hyphae-query-language.md)
//! for the original grammar and
//! [RFC-0091](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0091-hyphae-jquery-selectors.md)
//! for the jQuery-inspired selector extensions (`:not`, `:has`, `:in`,
//! `:implements`, `:first-child` / `:last-child` / `:only-child`,
//! `:nth-child(N)`, and `[attr=value]`).
//! [RFC-0124](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0124-hyphae-attr-after-pseudo.md)
//! allows attribute filters and pseudo-classes in any order after the base
//! (`simple ::= base (attribute_filter | pseudo_class)*`).

use crate::{
    ast::{
        Ast, AttributeSelector, BaseSelector, Combinator, PseudoArg, PseudoClass, Selector,
        SimpleSelector,
    },
    lexer::{Token, tokenise},
};

/// The teaching text appended to every position-carrying parse error: what a
/// valid Hyphae selector looks like, plus a docs pointer. One renderer for
/// all variants — no raw lexer/parser internals (Debug formatting) may leak
/// into the message a user or agent sees.
const GRAMMAR_HINT: &str = "A Hyphae simple selector is `#Name` (by symbol name), `.kind` (e.g. \
     `.function`, `.class`), or `*` (any) — combined with `>` (child), \
     `:pseudo(arg)`, and `[attr=value]` (pseudo-classes and attribute filters \
     may appear in any order after the base). To find a symbol named `Foo`, write \
     `#Foo` (NOT `Foo`, `class.Foo`, or `class:name(Foo)`).\n  \
     Grammar: rfcs/0003-hyphae-query-language.md and rfcs/0091-hyphae-jquery-selectors.md";

/// Render `tok` the way the user wrote it (`` `div` ``, `` `:calls` ``,
/// `` `>` ``, "whitespace") — never the internal Debug form
/// (`Ident("div")`). Used to build [`ParseError::UnexpectedToken`].
fn describe_token(tok: &Token<'_>) -> String {
    match tok {
        Token::Hash(s) => format!("`#{s}`"),
        Token::Dot(s) => format!("`.{s}`"),
        Token::Colon(s) => format!("`:{s}`"),
        Token::Gt => "`>`".to_owned(),
        Token::Tilde => "`~`".to_owned(),
        Token::LParen => "`(`".to_owned(),
        Token::RParen => "`)`".to_owned(),
        Token::LBracket => "`[`".to_owned(),
        Token::RBracket => "`]`".to_owned(),
        Token::Eq => "`=`".to_owned(),
        Token::Comma => "`,`".to_owned(),
        Token::Star => "`*`".to_owned(),
        Token::Number(n) => format!("number `{n}`"),
        Token::Ident(s) => format!("`{s}`"),
        Token::Ws => "whitespace".to_owned(),
    }
}

/// Error type for Hyphae parse failures.
#[derive(thiserror::Error, Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// A token was encountered that does not fit the grammar at that position.
    ///
    /// The `String` is the human rendering from [`describe_token`] (already
    /// backtick-quoted), never a Debug-formatted internal token.
    #[error("unexpected token {0} at position {1}\n  {GRAMMAR_HINT}")]
    UnexpectedToken(String, usize),

    /// The input ended before the grammar was satisfied.
    #[error("unexpected end of input\n  {GRAMMAR_HINT}")]
    UnexpectedEof,

    /// The lexer encountered an unrecognised character.
    #[error("unrecognized character at position {0}\n  {GRAMMAR_HINT}")]
    LexError(usize),
}

/// Parse a Hyphae query string into an [`Ast`].
///
/// # Errors
///
/// Returns a [`ParseError`] if the input cannot be parsed as a valid Hyphae
/// query.
pub fn parse(input: &str) -> Result<Ast, ParseError> {
    let raw = tokenise(input).map_err(ParseError::LexError)?;
    let tokens: Vec<(Token<'_>, usize)> =
        raw.into_iter().map(|(t, span)| (t, span.start)).collect();
    let mut parser = Parser { tokens, pos: 0 };
    let ast = parser.parse_query()?;
    parser.skip_ws();
    if parser.pos < parser.tokens.len() {
        let (tok, pos) = &parser.tokens[parser.pos];
        return Err(ParseError::UnexpectedToken(describe_token(tok), *pos));
    }
    Ok(ast)
}

// ── Internal parser state ────────────────────────────────────────────────────

struct Parser<'src> {
    tokens: Vec<(Token<'src>, usize)>,
    pos: usize,
}

impl<'src> Parser<'src> {
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

    fn parse_query(&mut self) -> Result<Ast, ParseError> {
        self.skip_ws();
        if self.peek().is_none() {
            return Err(ParseError::UnexpectedEof);
        }
        self.parse_selector_list()
    }

    fn parse_selector_list(&mut self) -> Result<Ast, ParseError> {
        let mut selectors = vec![self.parse_selector()?];
        loop {
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

    fn parse_selector(&mut self) -> Result<Selector, ParseError> {
        let mut left = Selector::Simple(self.parse_simple()?);
        loop {
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
                    describe_token(other),
                    self.current_pos(),
                ));
            }
            None => return Err(ParseError::UnexpectedEof),
        };

        // Attribute filters `[attr=value]` and pseudo-classes `:pseudo(arg)`
        // may appear in any order after the base (RFC-0124). Filters compose
        // by set intersection, so source order carries no semantics —
        // `a[x]:p` ≡ `a:p[x]`. Each kind is collected into its own list in
        // source order; the AST shape is unchanged from RFC-0091.
        let mut attributes = Vec::new();
        let mut pseudo_classes = Vec::new();
        loop {
            match self.peek() {
                Some(Token::LBracket) => attributes.push(self.parse_attribute()?),
                Some(Token::Colon(_)) => pseudo_classes.push(self.parse_pseudo_class()?),
                _ => break,
            }
        }

        Ok(SimpleSelector {
            base,
            attributes,
            pseudo_classes,
        })
    }

    /// `attribute ::= '[' IDENT '=' (IDENT | NUMBER) ']'`
    fn parse_attribute(&mut self) -> Result<AttributeSelector, ParseError> {
        // consume `[`
        self.advance();
        let (name_tok, name_pos) = self.advance().ok_or(ParseError::UnexpectedEof)?;
        let name = match name_tok {
            Token::Ident(s) => s.to_owned(),
            other => {
                return Err(ParseError::UnexpectedToken(
                    describe_token(&other),
                    name_pos,
                ));
            }
        };
        match self.advance() {
            Some((Token::Eq, _)) => {}
            Some((tok, pos)) => return Err(ParseError::UnexpectedToken(describe_token(&tok), pos)),
            None => return Err(ParseError::UnexpectedEof),
        }
        let (value_tok, value_pos) = self.advance().ok_or(ParseError::UnexpectedEof)?;
        let value = match value_tok {
            Token::Ident(s) => s.to_owned(),
            Token::Number(n) => n.to_string(),
            other => {
                return Err(ParseError::UnexpectedToken(
                    describe_token(&other),
                    value_pos,
                ));
            }
        };
        match self.advance() {
            Some((Token::RBracket, _)) => {}
            Some((tok, pos)) => return Err(ParseError::UnexpectedToken(describe_token(&tok), pos)),
            None => return Err(ParseError::UnexpectedEof),
        }
        Ok(AttributeSelector { name, value })
    }

    /// `pseudo_class ::= ':' IDENT ( '(' pseudo_arg ')' )?`
    ///
    /// `pseudo_arg` shape depends on the pseudo-class name:
    /// - `nth-child` → integer literal
    /// - `in` → bare path identifier
    /// - everything else → nested selector list
    fn parse_pseudo_class(&mut self) -> Result<PseudoClass, ParseError> {
        let (tok, pos) = self.advance().ok_or(ParseError::UnexpectedEof)?;
        let name = if let Token::Colon(n) = tok {
            n.to_owned()
        } else {
            return Err(ParseError::UnexpectedToken(describe_token(&tok), pos));
        };

        let argument = if matches!(self.peek(), Some(Token::LParen)) {
            self.advance(); // consume `(`
            self.skip_ws();
            if matches!(self.peek(), Some(Token::RParen)) {
                // Empty parens — argument-less.
                self.advance();
                None
            } else {
                let arg = match name.as_str() {
                    "nth-child" => {
                        let (t, p) = self.advance().ok_or(ParseError::UnexpectedEof)?;
                        match t {
                            Token::Number(n) => PseudoArg::Number(n),
                            other => {
                                return Err(ParseError::UnexpectedToken(describe_token(&other), p));
                            }
                        }
                    }
                    "in" => {
                        let (t, p) = self.advance().ok_or(ParseError::UnexpectedEof)?;
                        match t {
                            Token::Ident(s) => PseudoArg::Path(s.to_owned()),
                            other => {
                                return Err(ParseError::UnexpectedToken(describe_token(&other), p));
                            }
                        }
                    }
                    _ => PseudoArg::Selector(Box::new(self.parse_selector_list()?)),
                };
                self.skip_ws();
                match self.advance() {
                    Some((Token::RParen, _)) => {}
                    Some((t, p)) => {
                        return Err(ParseError::UnexpectedToken(describe_token(&t), p));
                    }
                    None => return Err(ParseError::UnexpectedEof),
                }
                Some(arg)
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
    use crate::ast::{
        AttributeSelector, BaseSelector, Combinator, PseudoClass, Selector, SimpleSelector,
    };

    fn simple_name(name: &str) -> Selector {
        Selector::Simple(SimpleSelector {
            base: BaseSelector::Name(name.to_owned()),
            attributes: vec![],
            pseudo_classes: vec![],
        })
    }

    fn simple_kind(kind: &str) -> Selector {
        Selector::Simple(SimpleSelector {
            base: BaseSelector::Kind(kind.to_owned()),
            attributes: vec![],
            pseudo_classes: vec![],
        })
    }

    fn simple_universal() -> Selector {
        Selector::Simple(SimpleSelector {
            base: BaseSelector::Universal,
            attributes: vec![],
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
    fn pseudo_class_no_arg() {
        let ast = parse("#foo:calls").unwrap();
        let expected = selector_list(vec![Selector::Simple(SimpleSelector {
            base: BaseSelector::Name("foo".to_owned()),
            attributes: vec![],
            pseudo_classes: vec![PseudoClass {
                name: "calls".to_owned(),
                argument: None,
            }],
        })]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn pseudo_class_with_selector_arg() {
        let ast = parse("#foo:calls(#bar)").unwrap();
        let expected = selector_list(vec![Selector::Simple(SimpleSelector {
            base: BaseSelector::Name("foo".to_owned()),
            attributes: vec![],
            pseudo_classes: vec![PseudoClass {
                name: "calls".to_owned(),
                argument: Some(PseudoArg::Selector(Box::new(selector_list(vec![
                    simple_name("bar"),
                ])))),
            }],
        })]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn pseudo_class_with_number_arg() {
        let ast = parse("*:nth-child(3)").unwrap();
        let expected = selector_list(vec![Selector::Simple(SimpleSelector {
            base: BaseSelector::Universal,
            attributes: vec![],
            pseudo_classes: vec![PseudoClass {
                name: "nth-child".to_owned(),
                argument: Some(PseudoArg::Number(3)),
            }],
        })]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn pseudo_class_with_path_arg() {
        let ast = parse(".function:in(src/auth/)").unwrap();
        let expected = selector_list(vec![Selector::Simple(SimpleSelector {
            base: BaseSelector::Kind("function".to_owned()),
            attributes: vec![],
            pseudo_classes: vec![PseudoClass {
                name: "in".to_owned(),
                argument: Some(PseudoArg::Path("src/auth/".to_owned())),
            }],
        })]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn attribute_selector() {
        let ast = parse("*[language=python]").unwrap();
        let expected = selector_list(vec![Selector::Simple(SimpleSelector {
            base: BaseSelector::Universal,
            attributes: vec![AttributeSelector {
                name: "language".to_owned(),
                value: "python".to_owned(),
            }],
            pseudo_classes: vec![],
        })]);
        assert_eq!(ast, expected);
    }

    // ── RFC-0124: attribute filters after pseudo-classes ────────────────────

    #[test]
    fn attribute_after_pseudo_class() {
        // Live agent QA: `*:calls(#upsert_node)[file=...]` — the natural
        // "filter callers by file" query — hard-errored with
        // UnexpectedToken("LBracket"). RFC-0124 makes attribute filters and
        // pseudo-classes accepted in any order after the base.
        let ast = parse("*:calls(#Foo)[file=src/x.rs]").unwrap();
        let expected = selector_list(vec![Selector::Simple(SimpleSelector {
            base: BaseSelector::Universal,
            attributes: vec![AttributeSelector {
                name: "file".to_owned(),
                value: "src/x.rs".to_owned(),
            }],
            pseudo_classes: vec![PseudoClass {
                name: "calls".to_owned(),
                argument: Some(PseudoArg::Selector(Box::new(selector_list(vec![
                    simple_name("Foo"),
                ])))),
            }],
        })]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn interleaved_attributes_and_pseudo_classes() {
        // attr, pseudo, attr — all interleavings parse into the same AST
        // shape (attributes and pseudo-classes each collected in source
        // order into their own list).
        let ast = parse(".function[language=rust]:calls(#Foo)[file=src/x.rs]").unwrap();
        let expected = selector_list(vec![Selector::Simple(SimpleSelector {
            base: BaseSelector::Kind("function".to_owned()),
            attributes: vec![
                AttributeSelector {
                    name: "language".to_owned(),
                    value: "rust".to_owned(),
                },
                AttributeSelector {
                    name: "file".to_owned(),
                    value: "src/x.rs".to_owned(),
                },
            ],
            pseudo_classes: vec![PseudoClass {
                name: "calls".to_owned(),
                argument: Some(PseudoArg::Selector(Box::new(selector_list(vec![
                    simple_name("Foo"),
                ])))),
            }],
        })]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn attr_after_pseudo_then_combinator_still_parses() {
        // The any-order loop must not eat the `>` combinator that follows.
        let ast = parse("*:first-child[file=src/x.rs]>.method").unwrap();
        let expected = selector_list(vec![combined(
            Selector::Simple(SimpleSelector {
                base: BaseSelector::Universal,
                attributes: vec![AttributeSelector {
                    name: "file".to_owned(),
                    value: "src/x.rs".to_owned(),
                }],
                pseudo_classes: vec![PseudoClass {
                    name: "first-child".to_owned(),
                    argument: None,
                }],
            }),
            Combinator::Child,
            simple_kind("method"),
        )]);
        assert_eq!(ast, expected);
    }

    #[test]
    fn empty_input_error() {
        assert_eq!(parse(""), Err(ParseError::UnexpectedEof));
    }

    #[test]
    fn invalid_char_error() {
        assert!(matches!(parse("@bad"), Err(ParseError::LexError(_))));
    }

    #[test]
    fn lex_error_is_human_readable_and_teaches() {
        // Live QA: `#a + #b` (a CSS adjacent-sibling guess) surfaced as the
        // raw Debug `LexError(3)` — an internal variant name plus a bare byte
        // offset. The rendered error must instead say, in human words, what
        // was found and where, and teach the grammar.
        let err = parse("#a + #b").expect_err("`+` is not a Hyphae combinator");
        let msg = err.to_string();
        assert!(!msg.contains("LexError("), "no Debug noise: {msg}");
        assert!(msg.contains("position 3"), "names the byte position: {msg}");
        assert!(
            msg.to_lowercase().contains("character"),
            "describes the failure in human words: {msg}"
        );
        assert!(
            msg.contains("#Name") || msg.contains("Hyphae"),
            "teaches the grammar: {msg}"
        );
    }

    #[test]
    fn unexpected_token_error_has_no_debug_noise() {
        // Live QA: `div` surfaced as `UnexpectedToken("Ident(\"div\")", 0)` —
        // the offending token was embedded in its Debug form. The rendered
        // error must name the token as the user wrote it, not as an enum.
        let err = parse("div").expect_err("bare ident is not a selector");
        let msg = err.to_string();
        assert!(!msg.contains("Ident(\""), "no Debug-formatted token: {msg}");
        assert!(msg.contains("div"), "names the offending token: {msg}");
        assert!(msg.contains("position 0"), "names the position: {msg}");
    }

    #[test]
    fn unexpected_token_error_teaches_the_grammar() {
        // Dogfood F6: an agent improvising `class:name(Store)` (CSS-ish but wrong)
        // got "unexpected token Ident(...)" with no guidance. The rendered error
        // must now name the token, show the grammar shape, suggest the `.Name`
        // correction, and point at the docs.
        let err = parse("class:name(Store)").expect_err("should not parse");
        let msg = err.to_string();
        assert!(msg.contains("class"), "names the offending token: {msg}");
        // Must suggest syntax the grammar ACTUALLY accepts — `#Name`, the id
        // selector (verified working) — and explicitly steer away from the
        // invalid `class.Name` / `class:name(...)` forms (Codex P2, PR #600).
        assert!(
            msg.contains("#Foo"),
            "suggests the working #Name form: {msg}"
        );
        assert!(
            msg.contains("#Name") || msg.contains("`#Name`") || msg.contains("by symbol name"),
            "describes #Name as the by-name selector: {msg}"
        );
        assert!(
            !msg.contains("`class.Name`"),
            "must NOT teach the invalid class.Name form: {msg}"
        );
        assert!(
            msg.to_lowercase().contains("rfc") || msg.contains("hyphae"),
            "points at the grammar/docs: {msg}"
        );
    }
}
