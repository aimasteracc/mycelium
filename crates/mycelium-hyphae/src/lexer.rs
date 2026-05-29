//! Lexer for the Hyphae query language.
//!
//! Converts a raw query string into a flat sequence of [`Token`]s using
//! the [`logos`] crate for fast DFA-based tokenisation.
//!
//! See [RFC-0003](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0003-hyphae-query-language.md)
//! for the original tokens; [RFC-0091](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0091-hyphae-jquery-selectors.md)
//! adds `Number`, `Path`, `LBracket`, `RBracket`, `Eq`, and `Ident`.

use logos::Logos;

/// A single Hyphae token.
///
/// Whitespace is emitted as [`Token::Ws`] because a space between two
/// simple selectors is the *descendant* combinator (`a b` means "b inside
/// a"). Leading and trailing whitespace is stripped by the parser.
#[derive(Logos, Clone, Debug, PartialEq, Eq)]
#[logos(skip r"")]
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

    /// `[` — opens an attribute selector.
    #[token("[")]
    LBracket,

    /// `]` — closes an attribute selector.
    #[token("]")]
    RBracket,

    /// `=` — attribute equality.
    #[token("=")]
    Eq,

    /// `,` — separates selectors in a selector list.
    #[token(",")]
    Comma,

    /// `*` — universal selector.
    #[token("*")]
    Star,

    // ── Literals ─────────────────────────────────────────────────────────
    /// A non-negative integer literal — `:nth-child(2)`.
    #[regex(r"[0-9]+", |lex| lex.slice().parse::<usize>().ok())]
    Number(usize),

    /// A bare identifier (no sigil) — used as attribute name/value and
    /// pseudo-class argument body for `:in(...)`.
    ///
    /// The token's slice intentionally accepts `/`, `.`, and `-` so that
    /// `:in(src/auth/session.rs)` and `[file=src/lib.rs]` lex as a single
    /// token.
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_./\-]*", |lex| lex.slice())]
    Ident(&'src str),

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
/// # Errors
///
/// Returns the byte position of the first unrecognised character.
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
        let toks = tokenise("#a>#b").unwrap();
        assert_eq!(toks[1].0, Token::Gt);
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
    fn brackets_and_eq() {
        let toks = tokenise("[a=b]").unwrap();
        assert_eq!(toks[0].0, Token::LBracket);
        assert_eq!(toks[1].0, Token::Ident("a"));
        assert_eq!(toks[2].0, Token::Eq);
        assert_eq!(toks[3].0, Token::Ident("b"));
        assert_eq!(toks[4].0, Token::RBracket);
    }

    #[test]
    fn number_literal() {
        let toks = tokenise("42").unwrap();
        assert_eq!(toks[0].0, Token::Number(42));
    }

    #[test]
    fn ident_with_slash_and_dot() {
        let toks = tokenise("src/lib.rs").unwrap();
        assert_eq!(toks[0].0, Token::Ident("src/lib.rs"));
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
