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
}
