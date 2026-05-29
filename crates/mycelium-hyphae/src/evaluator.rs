//! Hyphae Evaluator — RFC-0004 Phase 2.
//!
//! Executes a parsed [`Ast`] against a [`Store`] and returns the matching
//! symbol paths as a sorted, deduplicated `Vec<String>`.

use mycelium_core::{
    Store,
    types::{EdgeKind, NodeKind},
};

use crate::ast::{Ast, BaseSelector, Combinator, PseudoClass, Selector, SimpleSelector};

/// Returns the parent path of a `>` -separated symbol path, or `None` for
/// file-level nodes (no `>` in the path).
fn parent_path(path: &str) -> Option<&str> {
    path.rfind('>').map(|i| &path[..i])
}

/// Evaluates Hyphae queries against a [`Store`].
pub struct Evaluator<'s> {
    store: &'s Store,
}

impl<'s> Evaluator<'s> {
    /// Create an evaluator bound to `store`.
    #[must_use]
    pub fn new(store: &'s Store) -> Self {
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

    // ── selector dispatch ─────────────────────────────────────────────────────

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
            Combinator::Child => {
                // right paths whose direct parent segment is in left_paths
                all_right
                    .into_iter()
                    .filter(|p| parent_path(p).is_some_and(|parent| left_set.contains(parent)))
                    .collect()
            }
            Combinator::Descendant => {
                // right paths that have any ancestor in left_paths
                all_right
                    .into_iter()
                    .filter(|p| {
                        // walk up the path chain
                        let mut cur: &str = p.as_str();
                        while let Some(parent) = parent_path(cur) {
                            if left_set.contains(parent) {
                                return true;
                            }
                            cur = parent;
                        }
                        false
                    })
                    .collect()
            }
            Combinator::Sibling => {
                // right paths that share a direct parent with any left path
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

    // ── base selectors ────────────────────────────────────────────────────────

    fn eval_base(&self, base: &BaseSelector) -> Vec<String> {
        match base {
            BaseSelector::Universal => self.store.all_symbols(None, None),
            BaseSelector::Name(name) => self
                .store
                .all_symbols(None, None)
                .into_iter()
                .filter(|p| p.rsplit('>').next().is_some_and(|seg| seg == name.as_str()))
                .collect(),
            BaseSelector::Kind(kind_str) => {
                if let Some(nk) = node_kind_from_str(kind_str) {
                    self.store.all_symbols(None, Some(nk))
                } else {
                    Vec::new()
                }
            }
        }
    }

    // ── pseudo-class filters ──────────────────────────────────────────────────

    fn apply_pseudo(&self, candidates: Vec<String>, pseudo: &PseudoClass) -> Vec<String> {
        match pseudo.name.as_str() {
            "calls" => {
                let target_ids = self.pseudo_arg_ids(pseudo);
                candidates
                    .into_iter()
                    .filter(|p| {
                        self.store.lookup(p).is_some_and(|id| {
                            self.store
                                .outgoing(id, EdgeKind::Calls)
                                .iter()
                                .any(|t| target_ids.contains(t))
                        })
                    })
                    .collect()
            }
            "callers" => {
                let target_ids = self.pseudo_arg_ids(pseudo);
                candidates
                    .into_iter()
                    .filter(|p| {
                        self.store.lookup(p).is_some_and(|id| {
                            self.store
                                .incoming(id, EdgeKind::Calls)
                                .iter()
                                .any(|t| target_ids.contains(t))
                        })
                    })
                    .collect()
            }
            "imports" => {
                let target_ids = self.pseudo_arg_ids(pseudo);
                candidates
                    .into_iter()
                    .filter(|p| {
                        self.store.lookup(p).is_some_and(|id| {
                            self.store
                                .outgoing(id, EdgeKind::Imports)
                                .iter()
                                .any(|t| target_ids.contains(t))
                        })
                    })
                    .collect()
            }
            "extends" => {
                let target_ids = self.pseudo_arg_ids(pseudo);
                candidates
                    .into_iter()
                    .filter(|p| {
                        self.store.lookup(p).is_some_and(|id| {
                            self.store
                                .outgoing(id, EdgeKind::Extends)
                                .iter()
                                .any(|t| target_ids.contains(t))
                        })
                    })
                    .collect()
            }
            _ => Vec::new(), // unknown pseudo-class → strict empty
        }
    }

    /// Evaluate the pseudo-class argument and collect matching NodeIds.
    fn pseudo_arg_ids(
        &self,
        pseudo: &PseudoClass,
    ) -> std::collections::HashSet<mycelium_core::types::NodeId> {
        let paths = match &pseudo.argument {
            Some(arg_ast) => self.eval(arg_ast),
            None => self.store.all_symbols(None, None),
        };
        paths.iter().filter_map(|p| self.store.lookup(p)).collect()
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

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

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use mycelium_core::{Store, trunk::TrunkPath, types::EdgeKind};

    use super::Evaluator;
    use crate::parse;

    fn ev(store: &Store, query: &str) -> Vec<String> {
        let ast = parse(query).expect("parse error");
        Evaluator::new(store).eval(&ast)
    }

    // ── base selectors ────────────────────────────────────────────────────────

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
    fn eval_name_no_match_returns_empty() {
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>foo").unwrap());
        assert!(ev(&store, "#nonexistent").is_empty());
    }

    #[test]
    fn eval_empty_store_returns_empty() {
        let store = Store::new();
        assert!(ev(&store, "*").is_empty());
        assert!(ev(&store, "#foo").is_empty());
    }

    #[test]
    fn eval_results_sorted_and_deduped() {
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/z.rs>z_sym").unwrap());
        store.upsert_node(TrunkPath::parse("src/a.rs>a_sym").unwrap());
        assert_eq!(ev(&store, "*"), vec!["src/a.rs>a_sym", "src/z.rs>z_sym"]);
    }

    // ── pseudo-classes ────────────────────────────────────────────────────────

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
    fn eval_callers_pseudo_finds_callees() {
        let mut store = Store::new();
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>caller").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>callee").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        let result = ev(&store, "*:callers(#caller)");
        assert_eq!(result, vec!["src/b.rs>callee"]);
    }

    #[test]
    fn eval_calls_no_arg_returns_any_caller() {
        let mut store = Store::new();
        let a = store.upsert_node(TrunkPath::parse("src/a.rs>calls_something").unwrap());
        let b = store.upsert_node(TrunkPath::parse("src/b.rs>target").unwrap());
        let c = store.upsert_node(TrunkPath::parse("src/c.rs>quiet").unwrap());
        store.upsert_edge(EdgeKind::Calls, a, b);
        let _ = c;
        let result = ev(&store, "*:calls()");
        assert_eq!(result, vec!["src/a.rs>calls_something"]);
    }

    // ── comma list ────────────────────────────────────────────────────────────

    #[test]
    fn eval_comma_union_deduped() {
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>foo").unwrap());
        store.upsert_node(TrunkPath::parse("src/b.rs>bar").unwrap());
        let result = ev(&store, "#foo, #bar");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn eval_comma_overlap_deduped() {
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>foo").unwrap());
        let result = ev(&store, "#foo, *");
        assert_eq!(result, vec!["src/a.rs>foo"]);
    }

    // ── combinators ───────────────────────────────────────────────────────────

    #[test]
    fn eval_child_combinator() {
        let mut store = Store::new();
        // parent>child hierarchy in paths
        store.upsert_node(TrunkPath::parse("src/a.rs>Foo").unwrap());
        store.upsert_node(TrunkPath::parse("src/a.rs>Foo>bar").unwrap());
        store.upsert_node(TrunkPath::parse("src/a.rs>Other>bar").unwrap());
        // "#Foo > #bar" should only match src/a.rs>Foo>bar
        let result = ev(&store, "#Foo > #bar");
        assert_eq!(result, vec!["src/a.rs>Foo>bar"]);
    }

    #[test]
    fn eval_descendant_combinator() {
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>Mod").unwrap());
        store.upsert_node(TrunkPath::parse("src/a.rs>Mod>Sub").unwrap());
        store.upsert_node(TrunkPath::parse("src/a.rs>Mod>Sub>deep").unwrap());
        store.upsert_node(TrunkPath::parse("src/b.rs>Other").unwrap());
        // "#Mod *" should match all descendants of Mod
        let result = ev(&store, "#Mod *");
        assert!(result.contains(&"src/a.rs>Mod>Sub".to_owned()));
        assert!(result.contains(&"src/a.rs>Mod>Sub>deep".to_owned()));
        assert!(!result.contains(&"src/b.rs>Other".to_owned()));
    }

    // ── unknown pseudo-class ─────────────────────────────────────────────────

    #[test]
    fn eval_unknown_pseudo_returns_empty() {
        let mut store = Store::new();
        store.upsert_node(TrunkPath::parse("src/a.rs>foo").unwrap());
        assert!(ev(&store, "*:unknown_pseudo()").is_empty());
    }
}
