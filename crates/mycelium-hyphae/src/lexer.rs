//! Lexer for the Hyphae query language.
//!
//! Converts a raw query string into a flat sequence of [`Token`]s using
//! the [`logos`] crate for fast DFA-based tokenisation.
//!
//! See [RFC-0003](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0003-hyphae-query-language.md)
//! for the full token table.

use logos::Logos;

/// A single Hyphae token.
///
/// Whitespace is emitted as [`Token::Ws`] because a space between two
/// simple selectors is the *descendant* combinator (`a b` means "b inside
/// a"). Leading and trailing whitespace is stripped by the parser.
#[derive(Logos, Clone, Debug, PartialEq, Eq)]
#[logos(skip r"")] // nothing skipped; caller decides what to do with Ws
pub enum Token<'src> {
    // ── Sigil-prefixed selectors ─────────────────────────────────────────
    /// `#ident` — name selector.
    #[regex(r"#[a-zA-Z_][a-zA-Z0-9_\-]*", |lex| &lex.slice()[1..])]
    Hash(&'src str),

    /// `.ident` — kind selector.
    #[regex(r"\.[a-zA-Z_][a-zA-Z0-9_\-]*", |lex| &lex.slice()[1..])]
    Dot(&'src str),

    // ── Pseudo-class ─────────────────────────────────────────────────────
    /// `:ident` — start of a pseudo-class.
    #[regex(r":[a-zA-Z_][a-zA-Z0-9_\-]*", |lex| &lex.slice()[1..])]
    Colon(&'src str),

    // ── Combinators ──────────────────────────────────────────────────────
    /// `>` — child combinator.
    #[token(">")]
    Gt,

    /// `~` — sibling combinator.
    #[token("~")]
    Tilde,

    // ── Structure ────────────────────────────────────────────────────────
    /// `(` — opens a pseudo-class argument list.
    #[token("(")]
    LParen,

    /// `)` — closes a pseudo-class argument list.
    #[token(")")]
    RParen,

    /// `,` — separates selectors in a selector list.
    #[token(",")]
    Comma,

    /// `*` — universal selector.
    #[token("*")]
    Star,

    // ── Whitespace ───────────────────────────────────────────────────────
    /// One or more whitespace characters.
    ///
    /// Significant as a descendant combinator between two simple selectors;
    /// insignificant at the start/end or around explicit combinators.
    #[regex(r"[ \t\r\n]+")]
    Ws,
}

/// Tokenise `input` into a [`Vec`] of `(Token, span)` pairs.
///
/// Returns `Err(pos)` if a lexer error is encountered at byte offset `pos`.
///
/// # Errors
///
/// Returns the byte position of the first unrecognised character.
///
/// # Examples
///
/// ```
/// use mycelium_hyphae::lexer::{Token, tokenise};
///
/// let tokens = tokenise("#login").unwrap();
/// assert_eq!(tokens[0].0, Token::Hash("login"));
/// ```
pub fn tokenise(input: &str) -> Result<Vec<(Token<'_>, std::ops::Range<usize>)>, usize> {
    let mut out = Vec::new();
    let mut lex = Token::lexer(input);
    while let Some(result) = lex.next() {
        match result {
            Ok(tok) => out.push((tok, lex.span())),
            Err(()) => return Err(lex.span().start),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_selector() {
        let toks = tokenise("#login").unwrap();
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].0, Token::Hash("login"));
    }

    #[test]
    fn dot_selector() {
        let toks = tokenise(".function").unwrap();
        assert_eq!(toks[0].0, Token::Dot("function"));
    }

    #[test]
    fn star_selector() {
        let toks = tokenise("*").unwrap();
        assert_eq!(toks[0].0, Token::Star);
    }

    #[test]
    fn colon_pseudo() {
        let toks = tokenise(":calls").unwrap();
        assert_eq!(toks[0].0, Token::Colon("calls"));
    }

    #[test]
    fn combinators() {
        // Bare idents are not valid tokens in Hyphae; selectors must be prefixed.
        let toks = tokenise("#a>#b").unwrap();
        assert_eq!(toks[1].0, Token::Gt);
        // Verify the full token sequence: Hash("a"), Gt, Hash("b")
        let kinds = toks
            .iter()
            .map(|(t, _)| format!("{t:?}"))
            .collect::<Vec<_>>();
        assert_eq!(kinds, vec!["Hash(\"a\")", "Gt", "Hash(\"b\")"]);
    }

    #[test]
    fn tilde_combinator() {
        let toks = tokenise("#a~#b").unwrap();
        assert_eq!(toks[1].0, Token::Tilde);
    }

    #[test]
    fn whitespace_emitted() {
        let toks = tokenise("#a #b").unwrap();
        assert_eq!(toks[1].0, Token::Ws);
    }

    #[test]
    fn comma_separator() {
        let toks = tokenise("#a,#b").unwrap();
        assert_eq!(toks[1].0, Token::Comma);
    }

    #[test]
    fn parens() {
        let toks = tokenise("(#a)").unwrap();
        assert_eq!(toks[0].0, Token::LParen);
        assert_eq!(toks[2].0, Token::RParen);
    }

    #[test]
    fn invalid_char_returns_err() {
        assert!(tokenise("@bad").is_err());
    }

    #[test]
    fn hyphenated_ident() {
        let toks = tokenise("#my-symbol").unwrap();
        assert_eq!(toks[0].0, Token::Hash("my-symbol"));
    }
}
