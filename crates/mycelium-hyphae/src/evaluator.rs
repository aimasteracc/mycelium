//! Hyphae Evaluator.
//!
//! See [RFC-0003](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0003-hyphae-query-language.md)
//! for the original semantics and
//! [RFC-0091](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0091-hyphae-jquery-selectors.md)
//! for the jQuery-inspired extensions.

use mycelium_core::{
    Store,
    types::{EdgeKind, NodeKind},
};

use crate::ast::{
    Ast, AttributeSelector, BaseSelector, Combinator, PseudoArg, PseudoClass, Selector,
    SimpleSelector,
};

/// Returns the parent path of a `>` -separated symbol path, or `None` for
/// file-level nodes (no `>` in the path).
fn parent_path(path: &str) -> Option<&str> {
    path.rfind('>').map(|i| &path[..i])
}

/// Extract the file portion of a symbol path (everything before the first `>`).
fn file_of(path: &str) -> &str {
    path.find('>').map_or(path, |i| &path[..i])
}

/// Attribute names the evaluator implements (RFC-0003 / RFC-0091).
///
/// Kept in sync with the `match` arms in [`Evaluator::apply_attribute`].
const SUPPORTED_ATTRIBUTES: &[&str] = &["language", "kind", "file"];

/// Pseudo-class names the evaluator implements (RFC-0003 / RFC-0091).
///
/// Kept in sync with the `match` arms in [`Evaluator::apply_pseudo`].
const SUPPORTED_PSEUDOS: &[&str] = &[
    "calls",
    "callers",
    "imports",
    "extends",
    "implements",
    "not",
    "has",
    "in",
    "first-child",
    "last-child",
    "only-child",
    "nth-child",
];

/// Kind tokens the `.kind` base selector accepts — the domain of
/// [`node_kind_from_str`], which is the exact function [`Evaluator::eval_base`]
/// matches kinds with. Kept in sync by
/// `tests::eval_checked_accepts_every_supported_kind`.
const SUPPORTED_KINDS: &[&str] = &[
    "function",
    "method",
    "class",
    "struct",
    "enum",
    "interface",
    "trait",
    "module",
    "variable",
    "constant",
    "const",
    "type",
    "import",
];

/// Error returned when a query parses but names a selector the evaluator does
/// not implement.
///
/// The Hyphae grammar (a public API; see RFC-0003) accepts the *shape*
/// `[attr=value]` and `:pseudo(arg)` for any identifier, but the evaluator
/// only implements a fixed set of attribute and pseudo-class names. Names
/// outside that set used to evaluate to an *empty* result — silently
/// indistinguishable from "no symbols match". `EvalError` makes the failure
/// explicit and actionable instead.
#[derive(thiserror::Error, Clone, Debug, PartialEq, Eq)]
pub enum EvalError {
    /// `[attr=value]` named an attribute the evaluator does not implement.
    #[error(
        "unsupported selector: attribute filter `[{0}=...]` is not implemented (RFC-0003).\n  \
         Supported attributes: [language=...], [kind=...], [file=...].\n  \
         (Did you mean `[language=...]`? — the language filter is `language`, not `lang`.)"
    )]
    UnsupportedAttribute(String),

    /// `:pseudo(arg)` named a pseudo-class the evaluator does not implement.
    #[error(
        "unsupported selector: pseudo-class `:{0}` is not implemented (RFC-0003).\n  \
         Supported pseudo-classes: :calls, :callers, :imports, :extends, :implements, \
         :not, :has, :in, :first-child, :last-child, :only-child, :nth-child."
    )]
    UnsupportedPseudo(String),

    /// `.kind` named a node kind the evaluator does not implement.
    ///
    /// `hint` is either empty or a pre-rendered `\n  (Did you mean ...?)`
    /// line; see [`EvalError::unsupported_kind`]. Kept in sync with
    /// [`SUPPORTED_KINDS`] (asserted by
    /// `tests::eval_checked_rejects_unknown_kind_and_suggests_function`).
    #[error(
        "unsupported selector: kind `.{name}` is not implemented (RFC-0003).\n  \
         Supported kinds: .function, .method, .class, .struct, .enum, \
         .interface (alias .trait), .module, .variable, .constant (alias .const), \
         .type, .import.{hint}"
    )]
    UnsupportedKind {
        /// The unknown kind token as written (without the leading dot).
        name: String,
        /// Pre-rendered did-you-mean line, or empty when no near-miss exists.
        hint: String,
    },
}

impl EvalError {
    /// Build an [`EvalError::UnsupportedKind`] for `name`, attaching a
    /// did-you-mean hint when a supported kind is a near-miss.
    fn unsupported_kind(name: &str) -> Self {
        let hint = suggest_kind(name)
            .map_or_else(String::new, |s| format!("\n\n  (Did you mean `.{s}`?)"));
        Self::UnsupportedKind {
            name: name.to_owned(),
            hint,
        }
    }
}

/// Best-effort did-you-mean for an unknown kind token.
///
/// Suggests a supported kind that the input abbreviates (`fn` → `function`:
/// the input is a subsequence) or nearly spells (`clazz` → `class`:
/// Levenshtein distance ≤ 2). Ties resolve to the smallest edit distance.
///
/// The abbreviation branch additionally anchors on the FIRST character
/// (review finding on PR #749): without the anchor, a short input like `st`
/// subsequence-matches `const` out of the middle (c-o-n-**s**-**t**) and the
/// edit-distance tie-break would suggest `.const` where the user almost
/// certainly meant `.struct`. Anchoring keeps the advertised abbreviations
/// (`fn` → `function`, `st` → `struct`) and rejects mid-word accidents.
fn suggest_kind(input: &str) -> Option<&'static str> {
    let first = input.chars().next();
    SUPPORTED_KINDS
        .iter()
        .copied()
        .filter(|k| {
            (input.len() >= 2 && first == k.chars().next() && is_subsequence(input, k))
                || levenshtein(input, k) <= 2
        })
        .min_by_key(|k| levenshtein(input, k))
}

/// `true` if every char of `needle` appears in `hay` in order (abbreviation
/// check: `fn` ⊑ `function`).
fn is_subsequence(needle: &str, hay: &str) -> bool {
    let mut hay_chars = hay.chars();
    needle.chars().all(|c| hay_chars.any(|h| h == c))
}

/// Levenshtein edit distance — inputs are short kind tokens, so the simple
/// O(len a × len b) two-row DP is plenty.
fn levenshtein(a: &str, b: &str) -> usize {
    let b_chars: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b_chars.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = Vec::with_capacity(b_chars.len() + 1);
        cur.push(i + 1);
        for (j, cb) in b_chars.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b_chars.len()]
}

/// Map a file extension to a language wire string (`rust`, `python`, ...).
fn language_of_path(path: &str) -> Option<&'static str> {
    let file = file_of(path);
    let ext = file.rsplit('.').next()?;
    Some(match ext {
        "rs" => "rust",
        "py" => "python",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "go" => "go",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        _ => return None,
    })
}

/// Evaluates Hyphae queries against a [`Store`].
pub struct Evaluator<'s> {
    store: &'s Store,
}

impl<'s> Evaluator<'s> {
    /// Create an evaluator bound to `store`.
    #[must_use]
    pub const fn new(store: &'s Store) -> Self {
        Self { store }
    }

    /// Evaluate `ast` and return sorted, deduplicated symbol paths.
    #[must_use]
    pub fn eval(&self, ast: &Ast) -> Vec<String> {
        let Ast::SelectorList(selectors) = ast;
        let mut result: Vec<String> = selectors
            .iter()
            .flat_map(|sel| self.eval_selector(sel))
            .collect();
        result.sort();
        result.dedup();
        result
    }

    /// Validate `ast`, then evaluate it.
    ///
    /// Identical to [`eval`](Self::eval) on success, but returns an
    /// [`EvalError`] when the query names an attribute or pseudo-class the
    /// evaluator does not implement — instead of [`eval`](Self::eval)'s silent
    /// empty result. Entry points that surface results to a user (the CLI
    /// `query` command and its MCP twin `mycelium_query`) route through this
    /// method so an unsupported selector is reported as an error, never as
    /// "no matches".
    ///
    /// # Errors
    ///
    /// Returns [`EvalError::UnsupportedAttribute`] or
    /// [`EvalError::UnsupportedPseudo`] if any (possibly nested) selector names
    /// a filter the evaluator cannot apply.
    pub fn eval_checked(&self, ast: &Ast) -> Result<Vec<String>, EvalError> {
        validate(ast)?;
        Ok(self.eval(ast))
    }

    fn eval_selector(&self, sel: &Selector) -> Vec<String> {
        match sel {
            Selector::Simple(simple) => self.eval_simple(simple),
            Selector::Combined {
                left,
                combinator,
                right,
            } => self.eval_combined(left, combinator, right),
        }
    }

    fn eval_simple(&self, simple: &SimpleSelector) -> Vec<String> {
        let mut candidates = self.eval_base(&simple.base);
        for attr in &simple.attributes {
            candidates = self.apply_attribute(candidates, attr);
        }
        for pseudo in &simple.pseudo_classes {
            candidates = self.apply_pseudo(candidates, pseudo);
        }
        candidates
    }

    fn eval_combined(
        &self,
        left: &Selector,
        combinator: &Combinator,
        right: &Selector,
    ) -> Vec<String> {
        let left_paths = self.eval_selector(left);
        let all_right = self.eval_selector(right);
        let left_set: std::collections::HashSet<&str> =
            left_paths.iter().map(String::as_str).collect();

        match combinator {
            Combinator::Child => all_right
                .into_iter()
                .filter(|p| parent_path(p).is_some_and(|parent| left_set.contains(parent)))
                .collect(),
            Combinator::Descendant => all_right
                .into_iter()
                .filter(|p| {
                    let mut cur: &str = p.as_str();
                    while let Some(parent) = parent_path(cur) {
                        if left_set.contains(parent) {
                            return true;
                        }
                        cur = parent;
                    }
                    false
                })
                .collect(),
            Combinator::Sibling => {
                let left_parents: std::collections::HashSet<&str> = left_paths
                    .iter()
                    .filter_map(|p| parent_path(p.as_str()))
                    .collect();
                all_right
                    .into_iter()
                    .filter(|p| parent_path(p).is_some_and(|parent| left_parents.contains(parent)))
                    .collect()
            }
        }
    }

    fn eval_base(&self, base: &BaseSelector) -> Vec<String> {
        match base {
            BaseSelector::Universal => self.store.all_symbols(None, None),
            BaseSelector::Name(name) => self
                .store
                .all_symbols(None, None)
                .into_iter()
                .filter(|p| p.rsplit('>').next().is_some_and(|seg| seg == name.as_str()))
                .collect(),
            BaseSelector::Kind(kind_str) => node_kind_from_str(kind_str)
                .map_or_else(Vec::new, |nk| self.store.all_symbols(None, Some(nk))),
        }
    }

    // ── attribute selector ────────────────────────────────────────────────────

    fn apply_attribute(&self, candidates: Vec<String>, attr: &AttributeSelector) -> Vec<String> {
        match attr.name.as_str() {
            "language" => candidates
                .into_iter()
                .filter(|p| language_of_path(p).is_some_and(|lang| lang == attr.value))
                .collect(),
            "kind" => candidates
                .into_iter()
                .filter(|p| {
                    self.store
                        .lookup(p)
                        .and_then(|id| self.store.kind_of(id))
                        .is_some_and(|k| k.as_str() == attr.value)
                })
                .collect(),
            "file" => candidates
                .into_iter()
                .filter(|p| file_of(p) == attr.value)
                .collect(),
            // unknown attribute → strict empty
            _ => Vec::new(),
        }
    }

    // ── pseudo-class filters ──────────────────────────────────────────────────

    #[allow(clippy::too_many_lines)]
    fn apply_pseudo(&self, candidates: Vec<String>, pseudo: &PseudoClass) -> Vec<String> {
        match pseudo.name.as_str() {
            "calls" => self.filter_by_edge(candidates, pseudo, EdgeKind::Calls, true),
            "callers" => self.filter_by_edge(candidates, pseudo, EdgeKind::Calls, false),
            "imports" => self.filter_by_edge(candidates, pseudo, EdgeKind::Imports, true),
            "extends" => self.filter_by_edge(candidates, pseudo, EdgeKind::Extends, true),
            "implements" => self.filter_by_edge(candidates, pseudo, EdgeKind::Implements, true),
            "not" => {
                // Exclude every path that matches the inner selector.
                let exclude: std::collections::HashSet<String> =
                    self.eval_selector_arg(pseudo).into_iter().collect();
                candidates
                    .into_iter()
                    .filter(|p| !exclude.contains(p))
                    .collect()
            }
            "has" => {
                // Keep every candidate that has at least one descendant
                // matching the inner selector. A descendant is a path
                // whose containment chain includes the candidate.
                let inner = self.eval_selector_arg(pseudo);
                if inner.is_empty() {
                    return Vec::new();
                }
                let inner_set: std::collections::HashSet<&str> =
                    inner.iter().map(String::as_str).collect();
                candidates
                    .into_iter()
                    .filter(|cand| {
                        // Any inner path whose containment chain passes through cand?
                        inner_set.iter().any(|inner| {
                            let mut cur: &str = inner;
                            while let Some(parent) = parent_path(cur) {
                                if parent == cand.as_str() {
                                    return true;
                                }
                                cur = parent;
                            }
                            false
                        })
                    })
                    .collect()
            }
            "in" => {
                // Path-scoped: keep candidates whose path starts with the
                // given prefix.
                let Some(PseudoArg::Path(prefix)) = pseudo.argument.as_ref() else {
                    return Vec::new();
                };
                candidates
                    .into_iter()
                    .filter(|p| {
                        p == prefix
                            || p.starts_with(prefix.as_str())
                                && p[prefix.len()..].starts_with(['>', '/'])
                    })
                    .collect()
            }
            "first-child" => candidates
                .into_iter()
                .filter(|p| {
                    let Some(parent) = parent_path(p) else {
                        return true; // file-level node has no parent → counts as first
                    };
                    let mut siblings: Vec<String> = self
                        .store
                        .all_symbols(None, None)
                        .into_iter()
                        .filter(|q| parent_path(q) == Some(parent))
                        .collect();
                    siblings.sort();
                    siblings.first() == Some(p)
                })
                .collect(),
            "last-child" => candidates
                .into_iter()
                .filter(|p| {
                    let Some(parent) = parent_path(p) else {
                        return true;
                    };
                    let mut siblings: Vec<String> = self
                        .store
                        .all_symbols(None, None)
                        .into_iter()
                        .filter(|q| parent_path(q) == Some(parent))
                        .collect();
                    siblings.sort();
                    siblings.last() == Some(p)
                })
                .collect(),
            "only-child" => candidates
                .into_iter()
                .filter(|p| {
                    let Some(parent) = parent_path(p) else {
                        return true;
                    };
                    let siblings = self
                        .store
                        .all_symbols(None, None)
                        .into_iter()
                        .filter(|q| parent_path(q) == Some(parent))
                        .count();
                    siblings == 1
                })
                .collect(),
            "nth-child" => {
                let Some(PseudoArg::Number(n)) = pseudo.argument.as_ref() else {
                    return Vec::new();
                };
                // 1-indexed per CSS convention.
                if *n == 0 {
                    return Vec::new();
                }
                let idx = n - 1;
                candidates
                    .into_iter()
                    .filter(|p| {
                        let Some(parent) = parent_path(p) else {
                            return idx == 0;
                        };
                        let mut siblings: Vec<String> = self
                            .store
                            .all_symbols(None, None)
                            .into_iter()
                            .filter(|q| parent_path(q) == Some(parent))
                            .collect();
                        siblings.sort();
                        siblings.get(idx) == Some(p)
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    fn filter_by_edge(
        &self,
        candidates: Vec<String>,
        pseudo: &PseudoClass,
        kind: EdgeKind,
        outgoing: bool,
    ) -> Vec<String> {
        let target_ids = self.pseudo_arg_ids(pseudo);
        candidates
            .into_iter()
            .filter(|p| {
                self.store.lookup(p).is_some_and(|id| {
                    let edges = if outgoing {
                        self.store.outgoing(id, kind)
                    } else {
                        self.store.incoming(id, kind)
                    };
                    edges.iter().any(|t| target_ids.contains(t))
                })
            })
            .collect()
    }

    /// Evaluate the selector-style argument of a pseudo-class to paths.
    fn eval_selector_arg(&self, pseudo: &PseudoClass) -> Vec<String> {
        match pseudo.argument.as_ref() {
            Some(PseudoArg::Selector(ast)) => self.eval(ast),
            // No argument or non-selector argument → match nothing.
            _ => Vec::new(),
        }
    }

    fn pseudo_arg_ids(
        &self,
        pseudo: &PseudoClass,
    ) -> std::collections::HashSet<mycelium_core::types::NodeId> {
        let paths = match pseudo.argument.as_ref() {
            Some(PseudoArg::Selector(ast)) => self.eval(ast),
            // No argument → match any (used by `:calls()` with empty parens
            // or bare `:calls`).
            None => self.store.all_symbols(None, None),
            // Number / Path arguments don't make sense for edge-kind
            // pseudos; fall through to empty.
            _ => Vec::new(),
        };
        paths.iter().filter_map(|p| self.store.lookup(p)).collect()
    }
}

// ── AST validation (silent-empty guard) ─────────────────────────────────────

/// Walk `ast` and reject any attribute or pseudo-class name the evaluator does
/// not implement.
///
/// Recurses through combinators and into nested pseudo-class selector
/// arguments (`:not(...)`, `:has(...)`, `:calls(...)`, ...), so an unsupported
/// name buried inside an argument is still caught.
///
/// # Errors
///
/// Returns the first [`EvalError`] encountered, or `Ok(())` if every selector
/// names only supported filters.
fn validate(ast: &Ast) -> Result<(), EvalError> {
    let Ast::SelectorList(selectors) = ast;
    for sel in selectors {
        validate_selector(sel)?;
    }
    Ok(())
}

fn validate_selector(sel: &Selector) -> Result<(), EvalError> {
    match sel {
        Selector::Simple(simple) => validate_simple(simple),
        Selector::Combined { left, right, .. } => {
            validate_selector(left)?;
            validate_selector(right)
        }
    }
}

fn validate_simple(simple: &SimpleSelector) -> Result<(), EvalError> {
    // Validity check is `node_kind_from_str` itself — the exact function
    // `eval_base` matches `.kind` tokens with — so the validator can NEVER
    // reject a kind that would have matched (kinds are open-ended-ish across
    // language families; a hand-maintained allow-list here would drift).
    if let BaseSelector::Kind(kind) = &simple.base {
        if node_kind_from_str(kind).is_none() {
            return Err(EvalError::unsupported_kind(kind));
        }
    }
    for attr in &simple.attributes {
        if !SUPPORTED_ATTRIBUTES.contains(&attr.name.as_str()) {
            return Err(EvalError::UnsupportedAttribute(attr.name.clone()));
        }
    }
    for pseudo in &simple.pseudo_classes {
        if !SUPPORTED_PSEUDOS.contains(&pseudo.name.as_str()) {
            return Err(EvalError::UnsupportedPseudo(pseudo.name.clone()));
        }
        // Recurse into nested selector arguments (`:not(...)`, `:has(...)`, ...).
        if let Some(PseudoArg::Selector(inner)) = pseudo.argument.as_ref() {
            validate(inner)?;
        }
    }
    Ok(())
}

fn node_kind_from_str(s: &str) -> Option<NodeKind> {
    match s {
        "function" => Some(NodeKind::Function),
        "method" => Some(NodeKind::Method),
        "class" => Some(NodeKind::Class),
        "struct" => Some(NodeKind::Struct),
        "enum" => Some(NodeKind::Enum),
        "interface" | "trait" => Some(NodeKind::Interface),
        "module" => Some(NodeKind::Module),
        "variable" => Some(NodeKind::Variable),
        "constant" | "const" => Some(NodeKind::Constant),
        "type" => Some(NodeKind::TypeAlias),
        "import" => Some(NodeKind::Import),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use mycelium_core::{Store, trunk::TrunkPath, types::EdgeKind};

    use super::Evaluator;
    use crate::parse;

    fn ev(store: &Store, query: &str) -> Vec<String> {
        let ast = parse(query).expect("parse error");
        Evaluator::new(store).eval(&ast)
    }

    #[test]
    fn eval_universal_returns_all_symbols() {
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>foo").unwrap());
        store.upsert_node(TrunkPath::parse("src/a.rs>bar").unwrap());
        let mut result = ev(&store, "*");
        result.sort();
        assert_eq!(result, vec!["src/a.rs>bar", "src/a.rs>foo"]);
    }

    #[test]
    fn eval_name_selector_exact_match() {
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>login").unwrap());
        store.upsert_node(TrunkPath::parse("src/b.rs>logout").unwrap());
        store.upsert_node(TrunkPath::parse("src/c.rs>login").unwrap());
        let result = ev(&store, "#login");
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|p| p.ends_with(">login")));
    }

    #[test]
    fn eval_calls_pseudo_finds_callers() {
        let mut store = Store::new();
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>caller").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>callee").unwrap());
        store.upsert_node(TrunkPath::parse("src/c.rs>unrelated").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        let result = ev(&store, "*:calls(#callee)");
        assert_eq!(result, vec!["src/a.rs>caller"]);
    }

    #[test]
    fn eval_unknown_pseudo_returns_empty() {
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>foo").unwrap());
        assert!(ev(&store, "*:unknown_pseudo()").is_empty());
    }

    #[test]
    fn eval_not_excludes() {
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>login").unwrap());
        store.upsert_node(TrunkPath::parse("src/a.rs>logout").unwrap());
        let result = ev(&store, "*:not(#logout)");
        assert!(result.iter().any(|p| p.ends_with(">login")));
        assert!(!result.iter().any(|p| p.ends_with(">logout")));
    }

    #[test]
    fn eval_in_filters_by_prefix() {
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/auth/session.rs>login").unwrap());
        store.upsert_node(TrunkPath::parse("src/db.rs>find").unwrap());
        let result = ev(&store, "*:in(src/auth)");
        assert!(result.iter().any(|p| p.contains("session.rs")));
        assert!(!result.iter().any(|p| p.contains("db.rs")));
    }

    // ── Silent-empty → explicit error guard (RFC-0003) ──────────────────────
    //
    // An unsupported attribute name (`[lang=rust]` instead of the supported
    // `[language=rust]`) or an unsupported pseudo-class (`:frobnicate()`)
    // previously evaluated to an *empty* result set — indistinguishable, to an
    // AI agent, from "no symbols match". `eval_checked` must instead return an
    // explicit, actionable error so the query is reported as unsupported.

    use super::EvalError;

    fn ev_checked(store: &Store, query: &str) -> Result<Vec<String>, EvalError> {
        let ast = parse(query).expect("parse error");
        Evaluator::new(store).eval_checked(&ast)
    }

    #[test]
    fn eval_checked_rejects_unsupported_attribute_name() {
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>foo").unwrap());
        // `lang` is NOT a supported attribute name — `language` is.
        let err = ev_checked(&store, ".function[lang=rust]").expect_err("must be an error");
        let msg = err.to_string();
        assert!(msg.contains("lang"), "names the offending attribute: {msg}");
        assert!(
            msg.contains("language"),
            "suggests the supported `language` name: {msg}"
        );
        assert!(
            msg.to_lowercase().contains("rfc") || msg.contains("attribute"),
            "describes the failure and points at the grammar: {msg}"
        );
    }

    #[test]
    fn eval_checked_rejects_unsupported_pseudo_class() {
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>foo").unwrap());
        let err = ev_checked(&store, "*:frobnicate()").expect_err("must be an error");
        let msg = err.to_string();
        assert!(
            msg.contains("frobnicate"),
            "names the offending pseudo: {msg}"
        );
        assert!(
            msg.contains("calls") || msg.contains(":has") || msg.contains("supported"),
            "lists supported pseudo-classes: {msg}"
        );
    }

    #[test]
    fn eval_checked_accepts_supported_attribute_and_returns_matches() {
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>foo").unwrap());
        // `language` is supported; `.rs` → rust → should keep the match.
        let result = ev_checked(&store, "*[language=rust]").expect("supported, must be Ok");
        assert_eq!(result, vec!["src/a.rs>foo"]);
    }

    #[test]
    fn eval_checked_accepts_supported_pseudo() {
        let mut store = Store::new();
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>caller").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>callee").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        let result = ev_checked(&store, "*:calls(#callee)").expect("supported, must be Ok");
        assert_eq!(result, vec!["src/a.rs>caller"]);
    }

    #[test]
    fn eval_checked_validates_nested_pseudo_argument() {
        // The unsupported pseudo lives INSIDE a `:not(...)` argument — the
        // validator must recurse into nested selector arguments, not just the
        // top level.
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>foo").unwrap());
        let err = ev_checked(&store, "*:not(*:frobnicate())").expect_err("must be an error");
        assert!(err.to_string().contains("frobnicate"), "{err}");
    }

    // ── Unknown `.kind` selectors → explicit error (extends the #703 guard) ──
    //
    // `.fn` / `.clazz` previously evaluated to a silent `{matches:[], count:0}`
    // while `[nmae=foo]` correctly errored: the #703 validation covered
    // attribute and pseudo-class names but not `.kind` tokens. The validator
    // must reject unknown kinds with the valid-kind list and a did-you-mean —
    // and it must NEVER reject a kind the evaluator would actually match
    // (the validity check is `node_kind_from_str`, the exact function
    // `eval_base` matches with).

    #[test]
    fn eval_checked_rejects_unknown_kind_and_suggests_function() {
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>foo").unwrap());
        let err = ev_checked(&store, ".fn").expect_err("`.fn` must be an error, not Ok-empty");
        let msg = err.to_string();
        assert!(msg.contains("fn"), "names the offending kind: {msg}");
        assert!(
            msg.contains(".function"),
            "suggests the supported `.function` kind: {msg}"
        );
        // Lists every supported kind token so the agent can self-correct.
        for kind in super::SUPPORTED_KINDS {
            assert!(
                msg.contains(kind),
                "valid-kind list must include `{kind}`: {msg}"
            );
        }
    }

    #[test]
    fn eval_checked_suggests_class_for_clazz() {
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>foo").unwrap());
        let err = ev_checked(&store, ".clazz").expect_err("`.clazz` must be an error");
        let msg = err.to_string();
        assert!(msg.contains("clazz"), "names the offending kind: {msg}");
        assert!(msg.contains(".class"), "suggests `.class`: {msg}");
        // Nested inside a pseudo argument: the validator must recurse.
        let nested = ev_checked(&store, "*:not(.clazz)").expect_err("nested unknown kind");
        assert!(nested.to_string().contains("clazz"), "{nested}");
    }

    #[test]
    fn suggest_kind_anchors_on_first_char() {
        // Review finding (PR #749): without a first-char anchor, `st`
        // subsequence-matches `const` out of the middle and the edit-distance
        // tie-break suggests `.const` where `.struct` is clearly meant.
        assert_eq!(super::suggest_kind("st"), Some("struct"), "st -> struct");
        // The advertised abbreviation + typo cases must keep working.
        assert_eq!(
            super::suggest_kind("fn"),
            Some("function"),
            "fn -> function"
        );
        assert_eq!(
            super::suggest_kind("clazz"),
            Some("class"),
            "clazz -> class"
        );
    }

    #[test]
    fn eval_checked_accepts_every_supported_kind() {
        // No-false-rejection guard: every kind token the evaluator can match
        // (the domain of `node_kind_from_str`, advertised via SUPPORTED_KINDS)
        // must validate cleanly across all language families.
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>foo").unwrap());
        for kind in super::SUPPORTED_KINDS {
            assert!(
                super::node_kind_from_str(kind).is_some(),
                "SUPPORTED_KINDS entry `{kind}` must be matchable by node_kind_from_str"
            );
            assert!(
                ev_checked(&store, &format!(".{kind}")).is_ok(),
                "supported kind `.{kind}` must not be rejected"
            );
        }
    }

    // ── RFC-0124: attribute filters after pseudo-classes ─────────────────────
    //
    // Normative rule: attribute filters and pseudo-classes compose by set
    // intersection, so source order is NOT semantically significant —
    // `a[x]:p` ≡ `a:p[x]`. Structural pseudos (`:first-child`, `:nth-child`,
    // ...) rank against ALL store siblings, never the attribute-filtered
    // candidate set, so they too are order-independent (CSS semantics).

    #[test]
    fn attr_after_pseudo_filters_and_is_order_independent() {
        let mut store = Store::new();
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>caller_a").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>caller_b").unwrap());
        let target = store.upsert_node(TrunkPath::parse("src/t.rs>Foo").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, target);
        store.upsert_edge(EdgeKind::Calls, b, target);

        // Unfiltered: callers in both files.
        assert_eq!(ev(&store, "*:calls(#Foo)").len(), 2);
        // Attribute AFTER the pseudo-class narrows to one file.
        let after = ev(&store, "*:calls(#Foo)[file=src/a.rs]");
        assert_eq!(after, vec!["src/a.rs>caller_a"]);
        // Order-independence: identical to the attribute-first spelling.
        let before = ev(&store, "*[file=src/a.rs]:calls(#Foo)");
        assert_eq!(after, before, "a[x]:p must equal a:p[x]");
    }

    #[test]
    fn interleaved_filters_evaluate_as_intersection() {
        use mycelium_core::types::NodeKind;
        let mut store = Store::new();
        let a = store.upsert_node_with_kind(
            TrunkPath::parse("src/a.rs>caller_a").unwrap(),
            NodeKind::Function,
        );
        let b = store.upsert_node_with_kind(
            TrunkPath::parse("src/b.rs>caller_b").unwrap(),
            NodeKind::Function,
        );
        let target = store.upsert_node(TrunkPath::parse("src/t.rs>Foo").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, target);
        store.upsert_edge(EdgeKind::Calls, b, target);

        let result = ev(
            &store,
            ".function[language=rust]:calls(#Foo)[file=src/a.rs]",
        );
        assert_eq!(result, vec!["src/a.rs>caller_a"]);
    }

    #[test]
    fn structural_pseudo_is_order_independent_of_attribute() {
        // `:first-child` ranks against ALL siblings in the store — an
        // attribute filter before or after it must not change which node is
        // "first" (CSS semantics: structural position is a property of the
        // tree, not of the filtered candidate set).
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>Outer>alpha").unwrap());
        store.upsert_node(TrunkPath::parse("src/a.rs>Outer>beta").unwrap());
        // Discriminating sibling in ANOTHER file (review pin): gamma is the
        // first-child of its own parent, but the attribute filter excludes it.
        // If a future change ranked structural pseudos within the filtered
        // candidate set instead of the full store, the two orderings would
        // diverge here and this test would catch it.
        store.upsert_node(TrunkPath::parse("src/b.rs>Other>gamma").unwrap());
        let pre = ev(&store, "*[file=src/a.rs]:first-child");
        let post = ev(&store, "*:first-child[file=src/a.rs]");
        assert_eq!(pre, post, "structural pseudo must commute with attributes");
        assert!(
            !pre.iter().any(|p| p.contains("gamma")),
            "gamma (first-child in src/b.rs) must be excluded by [file=src/a.rs]"
        );
        assert!(
            pre.iter().any(|p| p.contains("alpha")),
            "alpha (first-child in src/a.rs) must survive both orderings"
        );
    }

    #[test]
    fn eval_checked_rejects_unknown_names_in_post_pseudo_positions() {
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>foo").unwrap());
        // Unknown attribute AFTER a pseudo-class.
        let err = ev_checked(&store, "*:calls(#foo)[lang=rust]").expect_err("must error");
        assert!(err.to_string().contains("lang"), "{err}");
        // Unknown pseudo AFTER an attr-after-pseudo sequence.
        let err = ev_checked(&store, "*:calls(#foo)[file=src/a.rs]:frobnicate()")
            .expect_err("must error");
        assert!(err.to_string().contains("frobnicate"), "{err}");
    }

    #[test]
    fn eval_checked_passes_working_combinators() {
        // No-regression: child + descendant combinators must validate cleanly.
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>Outer").unwrap());
        store.upsert_node(TrunkPath::parse("src/a.rs>Outer>inner").unwrap());
        assert!(ev_checked(&store, ".class>.method").is_ok());
        assert!(ev_checked(&store, ".class .method").is_ok());
        // `.inner` (previously used here) is NOT a real kind token — under the
        // unknown-kind guard it now correctly errors; use a valid kind.
        assert!(ev_checked(&store, "#Outer .method").is_ok());
        assert!(ev_checked(&store, "#Outer #inner").is_ok());
    }
}
