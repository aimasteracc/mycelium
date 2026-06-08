//! # Symbol extractor
//!
//! Parses source files with tree-sitter using a `LanguagePack`'s query
//! source, then populates a [`Store`] with nodes and edges.
//!
//! See RFC-0002 for the full design.

use std::collections::HashMap;

use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator as _};

use crate::{
    resolver::receiver::{LocalBinding, ParamBinding, ReceiverContext},
    store::Store,
    trunk::TrunkPath,
    types::{EdgeKind, NodeKind, SourceSpan},
};

// ── error type ────────────────────────────────────────────────────────────────

/// Errors from the symbol extraction pipeline.
#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    /// The tree-sitter language could not be loaded into the parser.
    #[error("failed to set language on parser: {0}")]
    Language(String),

    /// The query source is invalid for this grammar.
    #[error("invalid query: {0}")]
    Query(String),

    /// The parser returned no parse tree.
    #[error("parser returned no parse tree")]
    ParseFailed,
}

// ── extractor ─────────────────────────────────────────────────────────────────

/// Extracts symbols from source files into a [`Store`].
///
/// Build once and reuse across many files; the internal [`Query`] is compiled
/// at construction time.
///
/// # Example
///
/// ```no_run
/// # use mycelium_core::{extractor::Extractor, Store};
/// # use tree_sitter::Language;
/// # fn example(language: Language, query_src: &str) {
/// let mut store = Store::new();
/// let extractor = Extractor::new(language, query_src).unwrap();
/// extractor.extract("src/foo.py", b"def hello(): pass", &mut store).unwrap();
/// # }
/// ```
pub struct Extractor {
    language: Language,
    query: Query,
}

impl Extractor {
    /// Create an extractor for `language` using the query source from a
    /// language pack's `queries.scm`.
    ///
    /// # Errors
    ///
    /// Returns [`ExtractError::Query`] if `query_src` is not valid for
    /// the given `language`.
    pub fn new(language: Language, query_src: &str) -> Result<Self, ExtractError> {
        let query =
            Query::new(&language, query_src).map_err(|e| ExtractError::Query(e.to_string()))?;
        Ok(Self { language, query })
    }

    /// Parse `source` as a file at `file_path` and populate `store`.
    ///
    /// `file_path` should be the repository-relative path, e.g. `"src/main.py"`.
    ///
    /// # Errors
    ///
    /// Returns [`ExtractError::Language`] or [`ExtractError::ParseFailed`] on
    /// parser failure; these are extremely rare for well-formed grammars.
    ///
    /// # Panics
    ///
    /// Will not panic in practice. If `file_path` is an invalid `TrunkPath`
    /// the implementation falls back to the compile-time literal `"_unknown"`,
    /// which is always a valid path.
    // caller_id / callee_id are semantically distinct despite the shared prefix.
    #[allow(clippy::similar_names)]
    #[allow(clippy::too_many_lines)]
    pub fn extract(
        &self,
        file_path: &str,
        source: &[u8],
        store: &mut Store,
    ) -> Result<(), ExtractError> {
        let mut parser = Parser::new();
        parser
            .set_language(&self.language)
            .map_err(|e| ExtractError::Language(e.to_string()))?;

        let tree = parser
            .parse(source, None)
            .ok_or(ExtractError::ParseFailed)?;

        let root = tree.root_node();

        // Always create the file/module node.
        let file_id = store.upsert_node(
            TrunkPath::parse(file_path)
                .unwrap_or_else(|_| TrunkPath::parse("_unknown").expect("fallback path is valid")),
        );
        store.set_kind(file_id, NodeKind::File);
        store.set_span(file_id, node_to_span(root));

        let names = self.query.capture_names();

        // ─── Pass 1: definitions ─────────────────────────────────────────
        // Populate all Trunk nodes and Contains edges first so that Pass 2
        // can resolve forward-reference call targets to definition nodes.
        {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&self.query, root, source);
            while let Some(m) = matches.next() {
                let cap = m
                    .captures
                    .iter()
                    .find(|c| names[c.index as usize].starts_with("definition."));
                let Some(cap) = cap else { continue };
                let cap_name = names[cap.index as usize];
                let anchor = cap.node;
                let name_text: Option<&str> = m.captures.iter().find_map(|c| {
                    if names[c.index as usize] == "name" {
                        c.node.utf8_text(source).ok()
                    } else {
                        None
                    }
                });

                match cap_name {
                    "definition.module" => {
                        let _ = file_id;
                    }
                    other
                        if other.starts_with("definition.")
                            && other != "definition.module"
                            && other != "definition.method" =>
                    {
                        let suffix = other.trim_start_matches("definition.");
                        let name = name_text.unwrap_or("_unknown");
                        let path_str = format!("{file_path}>{name}");
                        if let Ok(path) = TrunkPath::parse(&path_str) {
                            let child_id = store.upsert_node(path);
                            store.upsert_edge(EdgeKind::Contains, file_id, child_id);
                            if let Some(kind) = cap_suffix_to_kind(suffix) {
                                store.set_kind(child_id, kind);
                            }
                            store.set_span(child_id, node_to_span(anchor));
                        }
                    }
                    "definition.method" => {
                        let method_name = name_text.unwrap_or("_unknown");
                        // Go methods path under their RECEIVER type (not a lexical
                        // chain); every other language uses the lexical container
                        // chain via build_class_chain.
                        let class_chain = go_receiver_type(anchor, source)
                            .map_or_else(|| build_class_chain(anchor, source), |recv| vec![recv]);
                        let chain_str = class_chain.join(">");
                        let path_str = format!("{file_path}>{chain_str}>{method_name}");
                        if let Ok(path) = TrunkPath::parse(&path_str) {
                            let method_id = store.upsert_node(path);
                            store.set_kind(method_id, NodeKind::Method);
                            // When the anchor IS a type-container node (e.g. class_definition
                            // in Python, class_declaration in TS/JS/Java/C#), the anchor span
                            // covers the whole class — wrong for jump-to-definition. Walk up
                            // from the @name identifier to find the precise method-declaration
                            // node (Issue #657).
                            let span_node = if is_type_container(anchor.kind()) {
                                m.captures
                                    .iter()
                                    .find(|c| names[c.index as usize] == "name")
                                    .and_then(|c| method_span_node(c.node, anchor))
                                    .unwrap_or(anchor)
                            } else {
                                anchor
                            };
                            store.set_span(method_id, node_to_span(span_node));
                            let class_path_str = format!("{file_path}>{chain_str}");
                            if let Ok(cls_path) = TrunkPath::parse(&class_path_str) {
                                let cls_id = store.upsert_node(cls_path);
                                store.upsert_edge(EdgeKind::Contains, file_id, cls_id);
                                store.upsert_edge(EdgeKind::Contains, cls_id, method_id);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // ─── Pass 1b: per-file alias table (RFC-0092) ─────────────────────
        // Walk `@reference.alias_binding` captures, build a
        // `local_name → resolved_path` map. Pass 2's `reference.call`
        // handler uses this to rewrite `_query.foo()` style calls back
        // to their real symbol path.
        let mut alias_table: HashMap<String, String> = HashMap::new();
        {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&self.query, root, source);
            while let Some(m) = matches.next() {
                let is_alias = m
                    .captures
                    .iter()
                    .any(|c| names[c.index as usize] == "reference.alias_binding");
                if !is_alias {
                    continue;
                }
                let local = m.captures.iter().find_map(|c| {
                    if names[c.index as usize] == "alias.local" {
                        c.node.utf8_text(source).ok()
                    } else {
                        None
                    }
                });
                // Primary: @alias.source in the match (Python-style patterns).
                // Fallback: read source: field from the @reference.alias_binding
                // anchor node at runtime. This covers TypeScript/JavaScript where
                // the query validator rejects combining `source:` + `import_clause`
                // in one pattern (inline-rule visibility limitation in ts 0.26).
                let captured_src = m.captures.iter().find_map(|c| {
                    if names[c.index as usize] == "alias.source" {
                        c.node.utf8_text(source).ok()
                    } else {
                        None
                    }
                });
                let anchor_src: Option<&str> = if captured_src.is_none() {
                    m.captures
                        .iter()
                        .find(|c| names[c.index as usize] == "reference.alias_binding")
                        .and_then(|c| c.node.child_by_field_name("source"))
                        .and_then(|string_node| {
                            // string_fragment is the first named child of the string node.
                            let count = string_node.named_child_count();
                            (0..count).find_map(|i| {
                                string_node
                                    .named_child(i.try_into().unwrap_or(u32::MAX))
                                    .filter(|n| n.kind() == "string_fragment")
                                    .and_then(|n| n.utf8_text(source).ok())
                            })
                        })
                } else {
                    None
                };
                let src = captured_src.or(anchor_src);
                let original = m.captures.iter().find_map(|c| {
                    if names[c.index as usize] == "alias.original_name" {
                        c.node.utf8_text(source).ok()
                    } else {
                        None
                    }
                });
                // `from M import X` (no `as`): local=None, original=Some("X").
                // Treat X as both the original name and the local binding.
                let effective_local = local.or(original);
                let (Some(effective_local), Some(src)) = (effective_local, src) else {
                    continue;
                };
                let resolved = build_alias_target(file_path, src, original, effective_local);
                // Issue #286: when the resolved target is a symbol-level path
                // (contains '>'), create both the Trunk node and an Imports edge
                // from the importing file so that get-dead-symbols does not flag
                // the imported symbol as dead just because it has no Calls edges.
                if resolved.contains('>') {
                    if let Ok(sym_path) = TrunkPath::parse(&resolved) {
                        let sym_id = store.upsert_node(sym_path);
                        store.upsert_edge(EdgeKind::Imports, file_id, sym_id);
                    }
                }
                alias_table.insert(effective_local.to_string(), resolved);
            }
        }

        // ─── Pass 1c: per-function local constructor bindings (RFC-0118 B) ──
        // `let x = Type::new()` → record (x → Type) under the enclosing function,
        // so Pass 2's method-call handler can attach a ReceiverContext and the
        // post-merge pass can bind `x.method()` to `…>Type>method`. Pure
        // capture-driven (pack data), language-agnostic core.
        //
        // Keyed by the enclosing function NODE's start byte (a unique scope id),
        // NOT its name — two distinct functions that happen to share a name (e.g.
        // nested `fn inner` in different outer fns) must NOT share a binding scope,
        // or a binding could leak across functions and mis-bind (independent
        // reviewer finding). The matching Pass-2 call uses the same node key.
        let mut local_bindings: HashMap<usize, Vec<LocalBinding>> = HashMap::new();
        // Per-scope count of ALL assignment targets (`@binding.rebind`), used to
        // invalidate a constructor binding whose name was later reassigned to a
        // NON-constructor (Codex P1 #647). If a name's total assignment count
        // exceeds its recognized constructor-binding count, the extra assignment
        // was a non-ctor (or differently-shaped) RHS whose type we can't know, so
        // inference must DECLINE — preserving "never mis-bind" under dynamic /
        // structural typing. Languages whose pack emits no `@binding.rebind`
        // (e.g. Rust today) leave this empty, so the drop pass is inert for them.
        let mut assign_counts: HashMap<usize, HashMap<String, usize>> = HashMap::new();
        {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&self.query, root, source);
            while let Some(m) = matches.next() {
                if let Some((rebind_node, Some(rebind_name))) = m.captures.iter().find_map(|c| {
                    (names[c.index as usize] == "binding.rebind")
                        .then(|| (c.node, c.node.utf8_text(source).ok()))
                }) {
                    if let Some(scope) = enclosing_function_node(rebind_node) {
                        *assign_counts
                            .entry(scope.start_byte())
                            .or_default()
                            .entry(rebind_name.to_owned())
                            .or_insert(0) += 1;
                    }
                }
                let local = m.captures.iter().find_map(|c| {
                    (names[c.index as usize] == "binding.local")
                        .then(|| (c.node, c.node.utf8_text(source).ok()))
                });
                let Some((local_node, Some(local_name))) = local else {
                    continue;
                };
                let ctor = m.captures.iter().find_map(|c| {
                    (names[c.index as usize] == "binding.ctor")
                        .then(|| c.node.utf8_text(source).ok())
                        .flatten()
                });
                let Some(ctor_type) = ctor else { continue };
                // Only treat title-case RHS paths as constructor TYPES. This drops
                // utility-module calls like `mem::take` / `io::stdin` (lowercase
                // module → not a type), preventing a wrong-bind if a struct ever
                // shared that name (reviewer finding). Cross-language: types are
                // title-case in Rust/Python/TS/Go-exported/Java/C#.
                if !ctor_type.chars().next().is_some_and(char::is_uppercase) {
                    continue;
                }
                // Scope to the enclosing function node (file-level lets are skipped).
                if let Some(scope) = enclosing_function_node(local_node) {
                    local_bindings
                        .entry(scope.start_byte())
                        .or_default()
                        .push(LocalBinding {
                            name: local_name.to_owned(),
                            ctor_type: Some(ctor_type.to_owned()),
                        });
                }
            }
        }

        // De-shadow: a name bound to DIFFERENT constructor types within one
        // function (Rust block shadowing, e.g. `let s = Store::new(); { let s =
        // Trunk::new(); s.m(); }`) is ambiguous — we don't track block scopes, so
        // we must not guess which binding is visible. Drop every entry for a
        // name that has conflicting types so inference declines rather than
        // mis-binds (Codex P2 #635). Same-name same-type repeats are harmless.
        for bindings in local_bindings.values_mut() {
            let mut seen: HashMap<String, String> = HashMap::new();
            let mut conflicting: Vec<String> = Vec::new();
            for b in bindings.iter() {
                let ty = b.ctor_type.clone().unwrap_or_default();
                if let Some(prev) = seen.insert(b.name.clone(), ty.clone()) {
                    if prev != ty {
                        conflicting.push(b.name.clone());
                    }
                }
            }
            if !conflicting.is_empty() {
                bindings.retain(|b| !conflicting.contains(&b.name));
            }
        }

        // Rebind invalidation (Codex P1 #647): drop any ctor binding whose name
        // was assigned MORE times than it was bound to a recognized constructor —
        // the surplus assignment was a non-ctor RHS, so the declared type is stale
        // at the call site and inference must decline (never mis-bind). Inert for
        // packs that emit no `@binding.rebind` (assign_counts is empty).
        for (scope, bindings) in &mut local_bindings {
            let Some(counts) = assign_counts.get(scope) else {
                continue;
            };
            let drop_names: Vec<String> = {
                let mut ctor_per_name: HashMap<&str, usize> = HashMap::new();
                for b in bindings.iter() {
                    *ctor_per_name.entry(b.name.as_str()).or_insert(0) += 1;
                }
                ctor_per_name
                    .iter()
                    .filter(|(name, ctor_n)| counts.get(**name).copied().unwrap_or(0) > **ctor_n)
                    .map(|(name, _)| (*name).to_owned())
                    .collect()
            };
            if !drop_names.is_empty() {
                bindings.retain(|b| !drop_names.contains(&b.name));
            }
        }

        // ─── Pass 1d: per-function parameter bindings (RFC-0118 B rule-b) ──
        // `fn f(s: &mut Store)` → param `s : Store` so `s.method()` resolves. The
        // type is normalized (strips &/mut/lifetime/generics, keeps Title-case
        // base). Scoped to the enclosing function, like local bindings.
        let mut param_bindings: HashMap<usize, Vec<ParamBinding>> = HashMap::new();
        {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&self.query, root, source);
            while let Some(m) = matches.next() {
                let pname = m.captures.iter().find_map(|c| {
                    (names[c.index as usize] == "param.name")
                        .then(|| (c.node, c.node.utf8_text(source).ok()))
                });
                let Some((pname_node, Some(pname_text))) = pname else {
                    continue;
                };
                let ptype = m.captures.iter().find_map(|c| {
                    (names[c.index as usize] == "param.type")
                        .then(|| c.node.utf8_text(source).ok())
                        .flatten()
                });
                let Some(normalized) = ptype.and_then(normalize_param_type) else {
                    continue;
                };
                if let Some(scope) = enclosing_function_node(pname_node) {
                    param_bindings
                        .entry(scope.start_byte())
                        .or_default()
                        .push(ParamBinding {
                            name: pname_text.to_owned(),
                            declared_type: Some(normalized),
                        });
                }
            }
        }

        // Cross param/local conflict: a name that is BOTH a param and a local of a
        // DIFFERENT type is ambiguous (the local shadows the param, but we don't
        // track which is visible at the call site) — drop it from BOTH tables so
        // inference declines rather than guessing (never mis-bind).
        let param_local_conflicts: HashMap<usize, Vec<String>> = param_bindings
            .iter()
            .filter_map(|(scope, params)| {
                let locals = local_bindings.get(scope)?;
                let names: Vec<String> = params
                    .iter()
                    .filter(|p| {
                        let pt = p.declared_type.clone().unwrap_or_default();
                        locals.iter().any(|l| {
                            l.name == p.name && l.ctor_type.clone().unwrap_or_default() != pt
                        })
                    })
                    .map(|p| p.name.clone())
                    .collect();
                (!names.is_empty()).then_some((*scope, names))
            })
            .collect();
        for (scope, conflict) in &param_local_conflicts {
            if let Some(p) = param_bindings.get_mut(scope) {
                p.retain(|b| !conflict.contains(&b.name));
            }
            if let Some(l) = local_bindings.get_mut(scope) {
                l.retain(|b| !conflict.contains(&b.name));
            }
        }

        // ─── Pass 2: references ──────────────────────────────────────────
        // All definitions are now in the store; intra-file callee lookup
        // will succeed for both backward and forward references.
        {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&self.query, root, source);
            while let Some(m) = matches.next() {
                let cap = m
                    .captures
                    .iter()
                    .find(|c| names[c.index as usize].starts_with("reference."));
                let Some(cap) = cap else { continue };
                let cap_name = names[cap.index as usize];
                let anchor = cap.node;
                let name_text: Option<&str> = m.captures.iter().find_map(|c| {
                    if names[c.index as usize] == "name" {
                        c.node.utf8_text(source).ok()
                    } else {
                        None
                    }
                });

                match cap_name {
                    "reference.import" | "reference.import_from" => {
                        // RFC-0096: type-annotation-only imports emit TypeImports
                        // edges so they are queryable but excluded from the
                        // default Imports graph, keeping detect-cycles clean.
                        //   • Python: `if TYPE_CHECKING:` block (Issue #227)
                        //   • TypeScript: `import type { Foo }` (Phase 2)
                        let edge_kind = if is_inside_type_checking_block(anchor, source)
                            || is_typescript_type_import_statement(anchor)
                        {
                            EdgeKind::TypeImports
                        } else {
                            EdgeKind::Imports
                        };
                        let mod_name = name_text.unwrap_or("_unknown");
                        // Issue #204 / RFC-0096: dispatch to the correct
                        // relative-import resolver per language:
                        //   • TypeScript / JavaScript → resolve_typescript_import
                        //   • Python (and others)     → resolve_python_relative_import
                        // Using the Python resolver for TS files produced wrong
                        // nodes (e.g. `/foo.py` instead of `foo.ts`).
                        let resolved = if matches!(
                            std::path::Path::new(file_path)
                                .extension()
                                .and_then(|e| e.to_str()),
                            Some("ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs")
                        ) {
                            resolve_typescript_import(file_path, mod_name)
                        } else {
                            resolve_python_relative_import(file_path, mod_name)
                        };
                        let edge_target = resolved.as_deref().unwrap_or(mod_name);
                        if let Ok(mod_path) = TrunkPath::parse(edge_target) {
                            let mod_id = store.upsert_node(mod_path);
                            store.upsert_edge(edge_kind, file_id, mod_id);
                        }
                    }
                    "reference.call" => {
                        let callee_name = name_text.unwrap_or("_unknown");
                        let receiver = m.captures.iter().find_map(|c| {
                            if names[c.index as usize] == "call.receiver" {
                                c.node.utf8_text(source).ok()
                            } else {
                                None
                            }
                        });
                        // Full enclosing-function path string (RFC-0118 B: keys the
                        // local-binding table for receiver-type inference).
                        let caller_full = enclosing_function_path(anchor, source)
                            .map(|s| format!("{file_path}>{s}"));
                        let caller_id = caller_full
                            .as_deref()
                            .and_then(|p| TrunkPath::parse(p).ok())
                            .map_or(file_id, |p| store.upsert_node(p));

                        // Issue #220: self.method() / cls.method() inside a
                        // class must resolve to the sibling method in the same
                        // class. Otherwise every method called only via `self`
                        // appears isolated (533 false positives reported in
                        // the tree-sitter-analyzer dogfood). Note: anchor here
                        // is the call site, not a container — we need a walker
                        // that only collects enclosing class-like ancestors,
                        // not build_class_chain which assumes anchor IS a
                        // container.
                        let self_method_target = receiver
                            .filter(|r| matches!(*r, "self" | "cls"))
                            .and_then(|_| {
                                let class_chain = enclosing_class_chain(anchor, source);
                                if class_chain.is_empty() {
                                    None
                                } else {
                                    Some(format!(
                                        "{file_path}>{}>{callee_name}",
                                        class_chain.join(">")
                                    ))
                                }
                            });

                        // RFC-0092: alias-aware dispatch for receiver.method() calls.
                        // If the receiver is in the alias table, rewrite to the
                        // resolved path. Otherwise fall back to intra-file lookup
                        // then bare-symbol upsert.
                        let receiver_target = receiver
                            .and_then(|r| alias_table.get(r))
                            .map(|prefix| chain_resolve(&alias_table, prefix))
                            .map(|prefix| format!("{prefix}>{callee_name}"));

                        // Issue #229: when the call is a bare identifier
                        // (no receiver) and that identifier is in the alias
                        // table, the callsite is `local()` where earlier we
                        // saw `local = mod.fn`. Walk the chain so multi-hop
                        // aliases (e.g. `local = _h.fn` and `_h` was bound
                        // by `from . import helpers as _h`) resolve to the
                        // real path.
                        let bare_alias_target = if receiver.is_none() {
                            alias_table
                                .get(callee_name)
                                .map(|prefix| chain_resolve(&alias_table, prefix))
                        } else {
                            None
                        };

                        let resolved_target =
                            self_method_target.or(receiver_target).or(bare_alias_target);

                        // RFC-0118 Part A: a callee we cannot resolve to an existing
                        // definition is minted as a `NodeKind::Unresolved` placeholder, so
                        // `is_real_symbol` keeps it out of the symbol universe (all_symbols /
                        // page_rank / rank_symbols). `upsert_node_with_kind` only runs in the
                        // `lookup == None` branch, so it never overwrites an existing real
                        // node; and if the real definition is indexed later, its own
                        // definition-extraction `set_kind` corrects this node's kind.
                        // `resolved` tracks whether we bound to an existing
                        // definition (true) or minted an Unresolved stub (false).
                        let (callee_id, resolved) = if let Some(qualified) = resolved_target {
                            if let Some(id) = store.lookup(&qualified) {
                                (id, true)
                            } else if let Ok(path) = TrunkPath::parse(&qualified) {
                                (
                                    store.upsert_node_with_kind(path, NodeKind::Unresolved),
                                    false,
                                )
                            } else {
                                continue;
                            }
                        } else {
                            let intra = format!("{file_path}>{callee_name}");
                            if let Some(id) = store.lookup(&intra) {
                                (id, true)
                            } else if let Ok(bare) = TrunkPath::parse(callee_name) {
                                (
                                    store.upsert_node_with_kind(bare, NodeKind::Unresolved),
                                    false,
                                )
                            } else {
                                continue;
                            }
                        };
                        store.upsert_edge(EdgeKind::Calls, caller_id, callee_id);

                        // RFC-0118 Part B: for an UNRESOLVED method call with a
                        // plain-identifier receiver (not self/cls/this), capture a
                        // ReceiverContext so the post-merge pass can infer the
                        // receiver's type and bind the precise `…>Type>method` edge.
                        // The `resolved` flag only distinguishes "new stub" from
                        // "existing node in the store"; a prior call to the same
                        // method name may have already created an Unresolved stub
                        // (resolved=true), so we check the node's actual kind.
                        let callee_is_unresolved = !resolved
                            || matches!(store.kind_of(callee_id), Some(NodeKind::Unresolved));
                        if callee_is_unresolved {
                            if let Some(recv) = receiver {
                                if !matches!(recv, "self" | "cls" | "this") {
                                    let locals = scope_chain_bindings(anchor, &local_bindings);
                                    let params = scope_chain_params(anchor, &param_bindings);
                                    store.record_call_site(
                                        caller_id,
                                        callee_id,
                                        ReceiverContext {
                                            receiver: recv.to_owned(),
                                            method: callee_name.to_owned(),
                                            imports: Vec::new(),
                                            locals,
                                            self_type: None,
                                            params,
                                            fields: Vec::new(),
                                        },
                                    );
                                }
                            } else {
                                // RFC-0118 Part B: a BARE implicit-receiver call
                                // (`speak()` with no `this.`/explicit receiver) inside
                                // a class/type body is an intra-type method call. When
                                // `speak` is defined on more than one type the bare
                                // stub stays Ambiguous and get-callers returns a
                                // confident-but-wrong EMPTY set. Record a synthetic
                                // `self` context whose `self_type` is the innermost
                                // enclosing type, so `infer_receiver_type` rule (a)
                                // returns that type and `disambiguate` matches the
                                // unique `…>{EnclosingType}>{method}` candidate.
                                //
                                // CONSERVATISM: `enclosing_self_type` fires ONLY for
                                // the four languages with implicit receiver dispatch
                                // (Java/C#/C++/Ruby) — see IMPLICIT_SELF_SCOPES. In
                                // Python, Rust, TS/JS, and Go a bare call is a
                                // free/local identifier, so it records no self-context
                                // and resolves exactly as before (Codex P2 on PR #680).
                                // A bare call at file scope also returns None. If the
                                // enclosing type does not define the method, the
                                // candidate set omits `…>{EnclosingType}>{method}` →
                                // disambiguate returns Ambiguous → the stub stays (no
                                // mis-bind).
                                if let Some(enclosing_type) = enclosing_self_type(anchor, source) {
                                    store.record_call_site(
                                        caller_id,
                                        callee_id,
                                        ReceiverContext {
                                            receiver: "self".to_owned(),
                                            method: callee_name.to_owned(),
                                            imports: Vec::new(),
                                            locals: Vec::new(),
                                            self_type: Some(enclosing_type),
                                            params: Vec::new(),
                                            fields: Vec::new(),
                                        },
                                    );
                                }
                            }
                        }
                    }
                    "reference.arg_callback" => {
                        // Issue #247: identifier passed as a function argument
                        // (positional or keyword-value). The identifier may be
                        // a callback / higher-order function that is never
                        // called with `()` syntax directly. Create a Calls
                        // edge from the enclosing function to the identifier
                        // so that `get-isolated-symbols` does not report it
                        // as dead code.
                        let cb_name = name_text.unwrap_or("_unknown");
                        let caller_path =
                            enclosing_function_path(anchor, source).and_then(|suffix| {
                                TrunkPath::parse(&format!("{file_path}>{suffix}")).ok()
                            });
                        let caller_id = caller_path.map_or(file_id, |p| store.upsert_node(p));
                        // Prefer intra-file definition; fall back to bare stub.
                        let callee_id = {
                            let intra = format!("{file_path}>{cb_name}");
                            if let Some(id) = store.lookup(&intra) {
                                id
                            } else if let Some(resolved) = alias_table
                                .get(cb_name)
                                .map(|p| chain_resolve(&alias_table, p))
                                .and_then(|q| {
                                    store.lookup(&q).or_else(|| {
                                        TrunkPath::parse(&q).ok().map(|p| store.upsert_node(p))
                                    })
                                })
                            {
                                resolved
                            } else if let Ok(bare) = TrunkPath::parse(cb_name) {
                                store.upsert_node(bare)
                            } else {
                                continue;
                            }
                        };
                        store.upsert_edge(EdgeKind::Calls, caller_id, callee_id);
                    }
                    "reference.extends" => {
                        // anchor = class_definition node; @name = base class identifier.
                        let base_name = name_text.unwrap_or("_unknown");
                        let Some(subclass_name) = anchor
                            .child_by_field_name("name")
                            .and_then(|n| n.utf8_text(source).ok())
                        else {
                            continue;
                        };
                        let Ok(subclass_path) =
                            TrunkPath::parse(&format!("{file_path}>{subclass_name}"))
                        else {
                            continue;
                        };
                        let subclass_id = store.upsert_node(subclass_path);
                        // Prefer the intra-file definition; fall back to a bare stub.
                        let base_id = {
                            let intra = format!("{file_path}>{base_name}");
                            if let Some(id) = store.lookup(&intra) {
                                // Same-file base class: resolved immediately.
                                id
                            } else if let Some(id) = alias_table
                                .get(base_name)
                                .map(|t| alias_target_to_file_path(t))
                                .and_then(|qualified| {
                                    store.lookup(&qualified).or_else(|| {
                                        TrunkPath::parse(&qualified)
                                            .ok()
                                            .map(|p| store.upsert_node(p))
                                    })
                                })
                            {
                                // Cross-file: alias table resolved the import to a file path.
                                id
                            } else if let Ok(bare) = TrunkPath::parse(base_name) {
                                // Fallback: bare stub (will be resolved post-extraction if unambiguous).
                                store.upsert_node(bare)
                            } else {
                                continue;
                            }
                        };
                        store.upsert_edge(EdgeKind::Extends, subclass_id, base_id);
                    }
                    "reference.implements" => {
                        // anchor = class_declaration; @name = implemented interface identifier.
                        let iface_name = name_text.unwrap_or("_unknown");
                        let Some(class_name) = anchor
                            .child_by_field_name("name")
                            .and_then(|n| n.utf8_text(source).ok())
                        else {
                            continue;
                        };
                        let Ok(class_path) = TrunkPath::parse(&format!("{file_path}>{class_name}"))
                        else {
                            continue;
                        };
                        let class_id = store.upsert_node(class_path);
                        let iface_id = {
                            let intra = format!("{file_path}>{iface_name}");
                            if let Some(id) = store.lookup(&intra) {
                                id
                            } else if let Some(id) = alias_table
                                .get(iface_name)
                                .map(|t| alias_target_to_file_path(t))
                                .and_then(|qualified| {
                                    store.lookup(&qualified).or_else(|| {
                                        TrunkPath::parse(&qualified)
                                            .ok()
                                            .map(|p| store.upsert_node(p))
                                    })
                                })
                            {
                                id
                            } else if let Ok(bare) = TrunkPath::parse(iface_name) {
                                store.upsert_node(bare)
                            } else {
                                continue;
                            }
                        };
                        store.upsert_edge(EdgeKind::Implements, class_id, iface_id);
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Chain-resolve a path through the alias table. If the leading segment of
/// `path` (everything before the first `>`) is itself a key in the table,
/// substitute and repeat — up to 8 hops to guard against pathological
/// self-referencing patterns. Required for issue #229's
/// `local = _h.fn; local()` chain where `_h` itself was bound by an earlier
/// `from . import helpers as _h`.
fn chain_resolve(table: &HashMap<String, String>, path: &str) -> String {
    let mut current = path.to_string();
    for _ in 0..8 {
        let head = current.split('>').next().unwrap_or("");
        if head.is_empty() || head == current {
            break;
        }
        match table.get(head) {
            Some(deeper) if deeper != head => {
                let rest = &current[head.len()..];
                current = format!("{deeper}{rest}");
            }
            _ => break,
        }
    }
    current
}

/// Convert an alias-table target to a file-path-qualified symbol path.
///
/// Relative imports already produce file-path targets (e.g. `pkg/sub.py>Sym`).
/// Absolute imports produce dotted targets (e.g. `pkg.sub>Sym`). This function
/// converts the latter to file-path format so that `Store::lookup` can find
/// the definition that was indexed under its on-disk path.
///
/// Only the module prefix (everything before the first `>`) is converted;
/// the symbol suffix is kept as-is. A prefix that already contains `/` is
/// assumed to be a file path and is returned unchanged.
fn alias_target_to_file_path(alias_target: &str) -> String {
    if let Some(sep) = alias_target.find('>') {
        let mod_part = &alias_target[..sep];
        let sym_part = &alias_target[sep..]; // includes '>'
        if !mod_part.contains('/') {
            // Dotted Python module path: `pkg.sub` → `pkg/sub.py`.
            return format!("{}.py{}", mod_part.replace('.', "/"), sym_part);
        }
    }
    alias_target.to_string()
}

/// Walk ancestors of `node` (a reference site, NOT a definition) and collect
/// the names of enclosing class-like containers, outermost first. Used by
/// `self.method()` / `cls.method()` resolution (issue #220) to qualify the
/// call target with the class chain.
///
/// Returns `vec!["App"]` for a call inside `class App: def m(self): self.f()`.
/// Returns `vec![]` for a free function call.
fn enclosing_class_chain(node: tree_sitter::Node<'_>, source: &[u8]) -> Vec<String> {
    let mut chain: Vec<String> = Vec::new();
    let mut cur = node;
    while let Some(parent) = cur.parent() {
        let kind = parent.kind();
        if is_type_container(kind) {
            chain.push(container_name(parent, source).to_owned());
        }
        cur = parent;
    }
    chain.reverse();
    chain
}

/// `(enclosing-method kind, enclosing-type-container kind)` pairs for the FOUR
/// languages with IMPLICIT receiver dispatch, where a bare unqualified
/// `speak()` inside a method body is sugar for `this.speak()` / `self.speak()`:
///
/// - **Java / C#**: `method_declaration` | `constructor_declaration` nested in a
///   `class`/`interface`/`enum`/`record`/`struct` *declaration*.
/// - **C++**: `function_definition` nested in a class/struct/union *specifier*.
/// - **Ruby**: `method` | `singleton_method` nested in a `class`/`module`.
///
/// Every OTHER language requires an explicit qualifier (`self.`, `this.`,
/// `Self::`, or a named receiver), so a bare call there is a free/local
/// identifier lookup, NOT an intra-type method call — binding it to the
/// enclosing type would manufacture a FALSE caller edge (Codex P2 on PR #680;
/// e.g. TS `class A { speak(){} run(){ speak(); } }` must NOT bind
/// `A>run -> A>speak`).
///
/// The pair is matched STRUCTURALLY because node kinds collide across languages
/// and neither half alone disambiguates:
/// - `class_declaration` is Java/C# (implicit `this`) AND TS/JS (no implicit).
/// - `method_declaration` is Java/C# (nested in a type) AND top-level Go (no
///   enclosing type container at all → no match → Go correctly excluded).
/// - `function_definition` is C++ (in a `*_specifier`) AND Python (in a
///   `class_definition` → not a pair here → Python correctly excluded).
const IMPLICIT_SELF_SCOPES: &[(&str, &str)] = &[
    // Java / C#
    ("method_declaration", "class_declaration"),
    ("method_declaration", "interface_declaration"),
    ("method_declaration", "enum_declaration"),
    ("method_declaration", "record_declaration"),
    ("method_declaration", "struct_declaration"),
    ("constructor_declaration", "class_declaration"),
    ("constructor_declaration", "record_declaration"),
    ("constructor_declaration", "struct_declaration"),
    // C++
    ("function_definition", "class_specifier"),
    ("function_definition", "struct_specifier"),
    ("function_definition", "union_specifier"),
    // Ruby
    ("method", "class"),
    ("method", "module"),
    ("singleton_method", "class"),
    ("singleton_method", "module"),
];

/// The enclosing type that a bare implicit-receiver call belongs to, i.e. the
/// type a synthetic `self` receiver would resolve to (RFC-0118 Part B).
///
/// Only the four languages with implicit receiver dispatch (Java, C#, C++,
/// Ruby) participate. We identify the language family by the
/// `(innermost enclosing method kind, innermost enclosing type-container kind)`
/// pair and accept ONLY combinations in [`IMPLICIT_SELF_SCOPES`]. This excludes
/// Python, Rust, and TypeScript/JavaScript (where a bare call is a free/local
/// identifier lookup) and Go (whose receiver methods have no enclosing type
/// container), so no false caller edge is manufactured (Codex P2 on PR #680).
///
/// Returns `None` at file scope, in a non-implicit language, or when the
/// container name is unknown — a bare call there records no self-context and
/// resolves exactly as before.
fn enclosing_self_type(node: tree_sitter::Node<'_>, source: &[u8]) -> Option<String> {
    let mut cur = node;
    let mut method_kind: Option<&str> = None;
    let mut container: Option<tree_sitter::Node<'_>> = None;
    while let Some(parent) = cur.parent() {
        let kind = parent.kind();
        if method_kind.is_none() && FUNCTION_KINDS.contains(&kind) {
            method_kind = Some(kind);
        }
        // Ruby's `class`/`module` are excluded from `is_type_container` (their
        // kinds collide with JS class-expressions / Python's module root), so
        // accept them here too — the IMPLICIT_SELF_SCOPES pair gate keeps the
        // collision harmless (a JS `class` expr would need a Ruby `method`
        // ancestor to match, which never happens).
        if container.is_none() && (is_type_container(kind) || kind == "class" || kind == "module") {
            container = Some(parent);
        }
        if method_kind.is_some() && container.is_some() {
            break;
        }
        cur = parent;
    }
    let method_kind = method_kind?;
    let container = container?;
    let is_implicit_self = IMPLICIT_SELF_SCOPES
        .iter()
        .any(|&(m, c)| m == method_kind && c == container.kind());
    if !is_implicit_self {
        return None;
    }
    let name = container_name(container, source);
    if name != "_Unknown" {
        return Some(name.to_owned());
    }
    None
}

/// Build the resolved target path that an alias binding points to
/// (RFC-0092). Handles four import shapes:
///
/// | Capture pattern                          | source     | original | local | Returns                  |
/// |------------------------------------------|------------|----------|-------|--------------------------|
/// | `import X as Y`                          | `X`        | None     | `Y`   | `X`                      |
/// | `from M import X as Y` (M absolute)      | `M`        | `X`      | `Y`   | `M>X`                    |
/// | `from .M import X as Y` (M relative)     | `.M`       | `X`      | `Y`   | `<resolved>>X`           |
/// | `from . import M as N` (bare relative)   | `.`        | `M`      | `N`   | `<pkg-dir>/M.py`         |
/// | `from . import M` (no `as`)              | `.`        | None     | `M`   | `<pkg-dir>/M.py`         |
///
/// The `<resolved>` is whatever [`resolve_python_relative_import`] yields
/// for the source. Returns the symbolic source as a fallback when the
/// resolver returns None (purely absolute paths with no leading dots).
fn build_alias_target(file_path: &str, src: &str, original: Option<&str>, local: &str) -> String {
    // TypeScript / JavaScript: specifiers are `./path`, `../path`, or bare package names.
    // We only resolve relative specifiers; package imports stay symbolic.
    if matches!(
        std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str()),
        Some("ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs")
    ) {
        let resolved = resolve_typescript_import(file_path, src).unwrap_or_else(|| src.to_string());
        return match original {
            Some(orig) => format!("{resolved}>{orig}"),
            None => resolved,
        };
    }
    // Python: existing logic unchanged.
    let is_relative = src.starts_with('.');
    let resolved_prefix =
        resolve_python_relative_import(file_path, src).unwrap_or_else(|| src.to_string());
    match (is_relative, original) {
        // `from . import M` (bare dots, no `as`) → local is a sibling module  →  pkg-dir/M.py
        // `from .submod import X` (non-bare, no `as`) → local is a symbol in submod  →  submod.py>X
        (true, None) => {
            if src.trim_start_matches('.').is_empty() {
                format!("{resolved_prefix}/{local}.py")
            } else {
                format!("{resolved_prefix}>{local}")
            }
        }
        // `from . import M as N`  →  pkg-dir/M.py (use `original`, not `local`)
        (true, Some(orig)) if src == "." => format!("{resolved_prefix}/{orig}.py"),
        // `from .M import X as Y`  →  resolved>X
        (true, Some(orig)) => format!("{resolved_prefix}>{orig}"),
        // `from M import X as Y`  →  M>X
        (false, Some(orig)) => format!("{src}>{orig}"),
        // `import X as Y`  →  X
        (false, None) => src.to_string(),
    }
}

/// Resolve a Python relative import (`.X` or `..X.Y`) to the importing file's
/// sibling/ancestor file path. Returns `None` for absolute imports (no leading
/// dots) — those keep the existing symbolic-node behaviour because resolving
/// them requires package-discovery logic out of scope for issue #204.
///
/// Examples (importing file = `pkg/sub/foo.py`):
/// - `.models`  → `pkg/sub/models.py`
/// - `..utils`  → `pkg/utils.py`
/// - `typing`   → `None` (absolute import)
///
/// Note: this purely computes the file-path string from syntax. It does NOT
/// check whether the target file exists on disk — the Trunk node is created
/// upserted whether the file is present or not, matching the existing
/// behaviour for absolute-import symbolic nodes.
fn resolve_python_relative_import(importing_file: &str, mod_name: &str) -> Option<String> {
    let dot_count = mod_name.chars().take_while(|&c| c == '.').count();
    if dot_count == 0 {
        // Absolute import — bail out, let the caller use the symbolic name.
        return None;
    }
    let rest = &mod_name[dot_count..];

    // Importing file's directory is the "current package". One dot = stay
    // here; each additional dot pops one parent.
    let mut current = std::path::Path::new(importing_file).parent()?.to_path_buf();
    for _ in 1..dot_count {
        current = current.parent()?.to_path_buf();
    }

    if rest.is_empty() {
        // Bare `from . import X` — Trunk node is the package dir itself.
        // Convert backslashes (Windows path semantics never appear in our
        // virtual TrunkPath; we always emit forward slashes).
        return Some(current.to_string_lossy().replace('\\', "/"));
    }

    // `.models` → models.py;  `..pkg.mod` → pkg/mod.py (dotted suffix).
    let suffix = rest.replace('.', "/");
    let target = current.join(format!("{suffix}.py"));
    Some(target.to_string_lossy().replace('\\', "/"))
}

/// Resolve a TypeScript/JavaScript relative import specifier (`./foo`,
/// `../bar`) to a file path relative to the workspace root.  When the
/// specifier has no extension, appends the same extension as the importing
/// file so that `.ts` consumers resolve to `.ts` modules and `.js` consumers
/// resolve to `.js` modules (matching the source-extension convention).
///
/// Returns `None` for bare package imports (`react`, `lodash`, etc.) that
/// have no leading `./` or `../` — those remain symbolic nodes.
///
/// Examples (importing file = `src/consumer.ts`):
/// - `./module`      → `src/module.ts`
/// - `../lib/util`   → `lib/util.ts`
/// - `react`         → `None` (package import — stays symbolic)
///
/// Examples (importing file = `src/consumer.js`):
///
/// - `./module`      → `src/module.js`
fn resolve_typescript_import(importing_file: &str, specifier: &str) -> Option<String> {
    if !specifier.starts_with("./") && !specifier.starts_with("../") {
        return None;
    }
    let dir = std::path::Path::new(importing_file).parent()?;
    // If the specifier already carries an extension, don't double-append one.
    let has_ext = std::path::Path::new(specifier)
        .extension()
        .is_some_and(|e| !e.is_empty());
    let target = if has_ext {
        dir.join(specifier)
    } else {
        // Use the importer's own extension so .js files resolve to .js targets,
        // .ts files to .ts targets, etc.  Fall back to "ts" if the importer
        // has no extension (should not happen in practice).
        let ext = std::path::Path::new(importing_file)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("ts");
        dir.join(specifier).with_extension(ext)
    };
    // Normalise `a/b/../c` → `a/c` using component iteration (no fs access).
    let mut components: Vec<_> = Vec::new();
    for comp in target.components() {
        match comp {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    let normalized: std::path::PathBuf = components.into_iter().collect();
    Some(normalized.to_string_lossy().replace('\\', "/"))
}

/// Walk ancestors of `node` looking for the nearest enclosing function-like
/// definition. Returns a path suffix like `"fn_name"` or `"ClassName>method_name"`
/// that can be appended to `file_path>` to build the full caller path.
///
/// Returns `None` if the call is at module level (no enclosing function).
/// Function/method node kinds across the supported grammars. A single source of
/// truth for "what counts as an enclosing function scope".
const FUNCTION_KINDS: &[&str] = &[
    "function_definition",     // Python / C / C++
    "function_declaration",    // TS/JS
    "function_expression",     // JS/TS
    "method_definition",       // TS/JS
    "function_item",           // Rust
    "method_declaration",      // Java/C#/Go
    "constructor_declaration", // Java/C#
    "method",                  // Ruby
    "singleton_method",        // Ruby (def self.x)
];

/// Node kinds that introduce a NESTED LEXICAL SCOPE for local bindings, in
/// ADDITION to [`FUNCTION_KINDS`]. Used only for RFC-0118 Part B receiver-binding
/// scoping (NOT for caller-path naming, which stays on named functions). Without
/// these, a binding inside an arrow/lambda/closure folds into the enclosing named
/// function and can leak to a sibling closure or the outer body where the receiver
/// is actually a free variable — manufacturing a FALSE caller (Codex P2 #653).
/// Treating them as their own binding scope keeps the "never mis-bind" invariant;
/// the call-site lookup walks the scope CHAIN so legitimate outer-scope closure
/// captures still resolve (no recall loss).
const BINDING_SCOPE_KINDS: &[&str] = &[
    "function_definition",     // Python / C / C++
    "function_declaration",    // TS/JS
    "function_expression",     // JS/TS
    "method_definition",       // TS/JS
    "function_item",           // Rust
    "method_declaration",      // Java/C#/Go
    "constructor_declaration", // Java/C#
    "method",                  // Ruby
    "singleton_method",        // Ruby
    "arrow_function",          // JS/TS
    "lambda",                  // Python
    "lambda_expression",       // Java/C#
    "closure_expression",      // Rust
];

/// Return the nearest enclosing binding SCOPE node (function/method/arrow/
/// lambda/closure), or `None` at file scope.
///
/// Its `start_byte()` is a stable, collision-free scope identifier — unlike the
/// scope *name*, two distinct same-named scopes (e.g. nested `fn inner` in
/// different outer fns, or sibling arrows) get different start bytes, so
/// per-scope binding tables never leak into each other (RFC-0118 Part B; Codex
/// P2 #653 for the arrow/closure case). Lexical lookup across enclosing scopes is
/// handled by walking the chain at the call site, see [`scope_chain_bindings`].
fn enclosing_function_node(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    let mut cur = node;
    while let Some(parent) = cur.parent() {
        if BINDING_SCOPE_KINDS.contains(&parent.kind()) {
            return Some(parent);
        }
        cur = parent;
    }
    None
}

/// Collect the local bindings visible at `node` by walking the enclosing binding
/// scope CHAIN from innermost outward, so a call inside an arrow/closure sees its
/// own scope AND all enclosing scopes (lexical closure capture). For a given
/// name the INNERMOST binding wins (shadowing); a name bound only in a sibling
/// scope is never visible (no leak). De-shadow/rebind invalidation already ran
/// per scope, so a name dropped there simply doesn't appear.
fn scope_chain_bindings(
    node: tree_sitter::Node<'_>,
    local_bindings: &HashMap<usize, Vec<LocalBinding>>,
) -> Vec<LocalBinding> {
    let mut out: Vec<LocalBinding> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut scope = enclosing_function_node(node);
    while let Some(s) = scope {
        if let Some(bindings) = local_bindings.get(&s.start_byte()) {
            for b in bindings {
                if seen.insert(b.name.clone()) {
                    out.push(b.clone());
                }
            }
        }
        scope = s.parent().and_then(enclosing_function_node);
    }
    out
}

/// Like [`scope_chain_bindings`] but for parameter bindings (RFC-0118 rule-b).
fn scope_chain_params(
    node: tree_sitter::Node<'_>,
    param_bindings: &HashMap<usize, Vec<ParamBinding>>,
) -> Vec<ParamBinding> {
    let mut out: Vec<ParamBinding> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut scope = enclosing_function_node(node);
    while let Some(s) = scope {
        if let Some(params) = param_bindings.get(&s.start_byte()) {
            for p in params {
                if seen.insert(p.name.clone()) {
                    out.push(p.clone());
                }
            }
        }
        scope = s.parent().and_then(enclosing_function_node);
    }
    out
}

/// Normalize a parameter's declared-type text to a base type name for receiver
/// inference: strips leading references (`&`), a lifetime (`'a`), `mut`, generics
/// (`<…>`), and a leading path (`crate::Store` → `Store`). Returns `None` unless
/// the result is a Title-case base type (so primitives/`impl Trait`/`dyn …`
/// decline). Smart pointers (`Arc<Store>` → `Arc`) are intentionally NOT
/// unwrapped — they yield a non-matching type and decline (conservative).
fn normalize_param_type(raw: &str) -> Option<String> {
    let mut s = raw.trim();
    s = s.trim_start_matches('&').trim_start();
    if s.starts_with('\'') {
        // Strip a leading lifetime token (`'a`), up to the next whitespace.
        match s.find(char::is_whitespace) {
            Some(pos) => s = s[pos..].trim_start(),
            None => return None,
        }
    }
    s = s.strip_prefix("mut ").unwrap_or(s).trim_start();
    let base = s.split('<').next().unwrap_or(s).trim();
    let base = base.rsplit("::").next().unwrap_or(base).trim();
    if base.is_empty() || !base.chars().next().is_some_and(char::is_uppercase) {
        return None;
    }
    Some(base.to_owned())
}

fn enclosing_function_path(node: tree_sitter::Node<'_>, source: &[u8]) -> Option<String> {
    let mut cur = node;
    while let Some(parent) = cur.parent() {
        if FUNCTION_KINDS.contains(&parent.kind()) {
            let fn_name = parent
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source).ok())
                .map(str::to_owned)
                .or_else(|| {
                    // Anonymous function_expression assigned to a variable:
                    // `const localize = function(...) {...}` — name lives in
                    // the enclosing variable_declarator, not the function node.
                    parent
                        .parent()
                        .filter(|p| p.kind() == "variable_declarator")
                        .and_then(|p| p.child_by_field_name("name"))
                        .and_then(|n| n.utf8_text(source).ok())
                        .map(str::to_owned)
                })
                .or_else(|| descend_declarator_name(parent, source))
                .unwrap_or_else(|| "_unknown".to_owned());

            // Collect enclosing class/impl containers (outermost first).
            // Ruby's `class`/`module` are excluded from the shared
            // `is_type_container` (their kind names collide with JS
            // class-EXPRESSIONS and Python's module root). But when the enclosing
            // FUNCTION is itself a Ruby `method`/`singleton_method`, the
            // `class`/`module` ancestors are unambiguously Ruby type containers,
            // so a call inside a Ruby method is attributed to `Class>method`
            // (matching the method's `@definition.method` path) instead of the
            // flat `method` path (RFC-0118 Part B: previously a known limitation).
            let ruby_method = matches!(parent.kind(), "method" | "singleton_method");
            let mut containers: Vec<String> = Vec::new();
            let mut scan = parent;
            while let Some(ancestor) = scan.parent() {
                let kind = ancestor.kind();
                if is_type_container(kind) || (ruby_method && matches!(kind, "class" | "module")) {
                    containers.push(container_name(ancestor, source).to_owned());
                }
                scan = ancestor;
            }
            containers.reverse();

            // Go methods: the receiver type is the container (not a lexical
            // ancestor), so a call inside `func (s *Server) Run()` is attributed
            // to `Server>Run` — matching the method's definition path.
            if containers.is_empty() {
                if let Some(recv) = go_receiver_type(parent, source) {
                    containers.push(recv);
                }
            }

            return Some(if containers.is_empty() {
                fn_name
            } else {
                format!("{}>{}", containers.join(">"), fn_name)
            });
        }
        cur = parent;
    }
    None
}

/// Walk ancestor nodes of `node`, collecting names of enclosing class/impl
/// nodes in outermost→innermost order.
///
/// `node` itself is the *innermost* such node; this function returns the chain
/// of containers that contain it, then appends `node`'s own name.
///
/// Recognised container node kinds and their name sources:
/// - `class_definition` (Python) → `"name"` field
/// - `class_declaration` (TypeScript/JavaScript) → `"name"` field
/// - `impl_item` (Rust) → `"type"` field, base identifier only (strips
///   generics like `<T>`)
fn build_class_chain(node: tree_sitter::Node<'_>, source: &[u8]) -> Vec<String> {
    // anchor IS the innermost container node.  Get ITS name first.
    let own_name = container_name(node, source).to_owned();

    // Then collect any enclosing class-like ancestors.
    let mut ancestors: Vec<String> = Vec::new();
    let mut cur = node;
    while let Some(parent) = cur.parent() {
        let kind = parent.kind();
        if is_type_container(kind) {
            ancestors.push(container_name(parent, source).to_owned());
        }
        cur = parent;
    }
    ancestors.reverse(); // outermost first

    ancestors.push(own_name);
    ancestors
}

/// Extract the identifier name from a container node.
///
/// Uses `"name"` field for class nodes and `"type"` field for Rust `impl_item`
/// (taking only the base type name before any `<` to strip generics).
fn container_name<'a>(node: tree_sitter::Node<'_>, source: &'a [u8]) -> &'a str {
    let field = if node.kind() == "impl_item" {
        "type"
    } else {
        "name"
    };
    let text = node
        .child_by_field_name(field)
        .and_then(|n| n.utf8_text(source).ok())
        .unwrap_or("_Unknown");
    // Strip generic parameters (e.g. "Vec<T>" → "Vec").
    text.split('<').next().unwrap_or(text)
}

/// Extract a function/method name from a C/C++ `function_definition`, which has
/// no `name` field — the name lives at the end of the `declarator` chain
/// (`function_definition → declarator (function_declarator) → declarator →
/// identifier` for a free function, `field_identifier` for a method,
/// `qualified_identifier` for an out-of-line `Foo::bar`). Returns `None` for
/// non-C++ nodes (which have a `name` field handled by the caller). Without this,
/// every C++ caller was attributed to `_unknown`.
/// For a Go `method_declaration`, return its RECEIVER TYPE — the container the
/// method should be pathed under (`Server` for `func (s *Server) Run()`). Go
/// methods aren't lexically nested in their type; the type lives in the
/// `receiver` parameter list, so `build_class_chain` (a lexical walk) can't find
/// it. Returns `None` for non-method nodes / non-Go (no `receiver` field).
fn go_receiver_type(node: tree_sitter::Node<'_>, source: &[u8]) -> Option<String> {
    if node.kind() != "method_declaration" {
        return None;
    }
    let recv = node.child_by_field_name("receiver")?;
    let mut cursor = recv.walk();
    for child in recv.children(&mut cursor) {
        if child.kind() != "parameter_declaration" {
            continue;
        }
        let Some(ty) = child.child_by_field_name("type") else {
            continue;
        };
        // `*Server` (pointer_type) → unwrap to the inner type_identifier.
        let type_node = if ty.kind() == "pointer_type" {
            ty.named_child(0).unwrap_or(ty)
        } else {
            ty
        };
        if let Ok(t) = type_node.utf8_text(source) {
            // Strip generics (`Server[T]`/`Server<T>` → `Server`).
            let base = t.split(['<', '[']).next().unwrap_or(t).trim();
            if !base.is_empty() {
                return Some(base.to_owned());
            }
        }
    }
    None
}

fn descend_declarator_name(node: tree_sitter::Node<'_>, source: &[u8]) -> Option<String> {
    let mut cur = node.child_by_field_name("declarator")?;
    for _ in 0..16 {
        match cur.kind() {
            "identifier" | "field_identifier" | "type_identifier" => {
                return cur.utf8_text(source).ok().map(str::to_owned);
            }
            "qualified_identifier" => {
                return cur
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source).ok())
                    .map(str::to_owned);
            }
            _ => {
                cur = cur.child_by_field_name("declarator")?;
            }
        }
    }
    None
}

/// Whether `kind` is a type-container node whose name participates in a member's
/// dotted path (e.g. `Class>method`). Single source of truth so a member's chain
/// is consistent across `build_class_chain`, `enclosing_class_chain`, and
/// `enclosing_function_path`. Covers Python `class_definition`, TS/JS/Java/C#
/// `class_declaration`, Rust `impl_item`, and Java `enum_declaration` /
/// `record_declaration` / `interface_declaration` (so enum/record/interface
/// methods nest correctly instead of landing at file scope).
fn is_type_container(kind: &str) -> bool {
    matches!(
        kind,
        "class_definition"
            | "class_declaration"
            | "impl_item"
            | "enum_declaration"
            | "record_declaration"
            | "interface_declaration"
            | "struct_declaration" // C# (also Rust struct_item is handled via impl_item)
            | "class_specifier" // C++
            | "struct_specifier" // C++
            | "union_specifier" // C++
                                // NOTE: Ruby's container kinds (`class`/`module`) are intentionally
                                // NOT listed here — `module` collides with Python's file-root node
                                // kind, and `class` collides with JS/TS class-EXPRESSION nodes
                                // (`const C = class {}`), which would manufacture orphan
                                // `_Unknown>method` caller paths if matched unconditionally. Ruby
                                // method DEFINITIONS path correctly via the @definition.method anchor
                                // (which reads the class/module `name` field). Ruby CALLER attribution
                                // and bare implicit-self resolution are handled with a Ruby-scoped
                                // `class`/`module` walk in `enclosing_function_path` and
                                // `enclosing_self_type` (gated on a Ruby `method`/`singleton_method`
                                // ancestor), so the collision concern never applies there
                                // (RFC-0118 Part B).
    )
}

/// Return `true` if `node` is a TypeScript `import type { ... }` statement.
///
/// TypeScript's `import type` syntax (`import type { Foo } from 'mod'`) is
/// purely a type-annotation construct — the module is never loaded at runtime.
/// It maps to `EdgeKind::TypeImports` (RFC-0096 Phase 2).
///
/// Detection: an `import_statement` node that has a direct anonymous child
/// with kind `"type"` (the `type` keyword that follows `import`).
fn is_typescript_type_import_statement(node: tree_sitter::Node<'_>) -> bool {
    // Only import_statement nodes can be `import type`; skip anything else.
    if node.kind() != "import_statement" {
        return false;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // The "type" keyword appears as an anonymous node (is_named() == false)
        // with kind "type" — distinct from named nodes like `type_identifier`.
        if !child.is_named() && child.kind() == "type" {
            return true;
        }
    }
    false
}

/// Return `true` if `node` is a direct child of an `if TYPE_CHECKING:` block.
///
/// Python's `TYPE_CHECKING` constant (from `typing`) is always `False` at
/// runtime; imports guarded by it are annotation-only and must not contribute
/// to the Imports graph (issue #227). We check immediate context only —
/// nested `if TYPE_CHECKING:` inside a function is unusual and treated the
/// same way.
fn is_inside_type_checking_block(node: tree_sitter::Node<'_>, source: &[u8]) -> bool {
    let mut cur = node;
    while let Some(parent) = cur.parent() {
        if parent.kind() == "if_statement" {
            let cond_is_type_checking = parent
                .child_by_field_name("condition")
                .and_then(|cond| {
                    if cond.kind() == "identifier" {
                        cond.utf8_text(source).ok()
                    } else {
                        None
                    }
                })
                .is_some_and(|text| text == "TYPE_CHECKING");
            if cond_is_type_checking {
                return true;
            }
        }
        cur = parent;
    }
    false
}

/// Method-declaration node kinds, ordered from most- to least-specific so the
/// first match against `parent.kind()` wins.
const METHOD_DECL_KINDS: &[&str] = &[
    "method_definition",       // TypeScript / JavaScript
    "function_definition",     // Python / C++ (inside a specifier)
    "method_declaration",      // Java / C# / Go
    "constructor_declaration", // Java / C#
    "method",                  // Ruby
    "singleton_method",        // Ruby
];

/// Given the `@name` identifier node from a `@definition.method` capture whose
/// anchor (`container`) is a type container, walk up the CST to find the
/// nearest enclosing method/function declaration — which has the precise span
/// we want to store. Returns `None` when no method-kind ancestor is found
/// before reaching `container` itself (fall back to `container`).
fn method_span_node<'a>(
    name_node: tree_sitter::Node<'a>,
    container: tree_sitter::Node<'a>,
) -> Option<tree_sitter::Node<'a>> {
    let mut cur = name_node;
    while let Some(parent) = cur.parent() {
        if parent.id() == container.id() {
            return None;
        }
        if METHOD_DECL_KINDS.contains(&parent.kind()) {
            return Some(parent);
        }
        cur = parent;
    }
    None
}

/// Convert a tree-sitter node's position to a [`SourceSpan`].
///
/// Tree-sitter rows are 0-indexed; `start_line`/`end_line` are stored 1-indexed
/// to match LSP and editor conventions. Columns remain 0-indexed.
fn node_to_span(node: tree_sitter::Node<'_>) -> SourceSpan {
    let start = node.start_position();
    let end = node.end_position();
    SourceSpan {
        start_line: u32::try_from(start.row)
            .unwrap_or(u32::MAX)
            .saturating_add(1),
        start_col: u32::try_from(start.column).unwrap_or(u32::MAX),
        end_line: u32::try_from(end.row).unwrap_or(u32::MAX).saturating_add(1),
        end_col: u32::try_from(end.column).unwrap_or(u32::MAX),
        start_byte: u32::try_from(node.start_byte()).unwrap_or(u32::MAX),
        end_byte: u32::try_from(node.end_byte()).unwrap_or(u32::MAX),
    }
}

/// Map the suffix of a `definition.*` capture name to a [`NodeKind`].
///
/// Returns `None` for capture names that have no meaningful `NodeKind` mapping
/// (e.g. `definition.module` which represents the whole file — callers should
/// handle that case separately using `NodeKind::File`).
fn cap_suffix_to_kind(suffix: &str) -> Option<NodeKind> {
    match suffix {
        // `namespace` (C++/C#) is a module-like container.
        "mod" | "module" | "namespace" => Some(NodeKind::Module),
        // `template_function` (C++) is a function with type parameters.
        "function" | "template_function" => Some(NodeKind::Function),
        // `template_class` (C++) is a class with type parameters.
        "class" | "template_class" => Some(NodeKind::Class),
        // `constructor` (C#) is a method.
        "method" | "constructor" => Some(NodeKind::Method),
        "interface" | "trait" => Some(NodeKind::Interface),
        // Generic named-type declaration: `type` is one `@definition.type` capture
        // covering Go `type X struct/interface/…`, C struct/union/typedef and TS
        // `type` — map to the neutral `TypeAlias` so they are categorized and stay
        // navigable/searchable rather than landing kind-less (PR #651 review).
        "type_alias" | "associated_type" | "type" => Some(NodeKind::TypeAlias),
        // `static FOO: T = ...` and `impl T { const X: ... }` are both
        // compile-time-constant kinds at the language level; reuse
        // `NodeKind::Constant` so they participate in `get-symbols-by-kind`,
        // `symbol-count-by-kind`, and the Salsa `FileIndex` round-trip.
        // (Codex P2 catch on PR #492, 2026-06-04.)
        "const" | "constant" | "static" | "associated_const" => Some(NodeKind::Constant),
        "struct" => Some(NodeKind::Struct),
        "enum" => Some(NodeKind::Enum),
        other => NodeKind::try_from_wire(other),
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
