//! # Symbol extractor
//!
//! Parses source files with tree-sitter using a `LanguagePack`'s query
//! source, then populates a [`Store`] with nodes and edges.
//!
//! See RFC-0002 for the full design.

use std::collections::HashMap;

use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator as _};

use crate::{
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
                        let class_chain = build_class_chain(anchor, source);
                        let chain_str = class_chain.join(">");
                        let path_str = format!("{file_path}>{chain_str}>{method_name}");
                        if let Ok(path) = TrunkPath::parse(&path_str) {
                            let method_id = store.upsert_node(path);
                            store.set_kind(method_id, NodeKind::Method);
                            store.set_span(method_id, node_to_span(anchor));
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
                alias_table.insert(effective_local.to_string(), resolved);
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
                        // Issue #227: imports inside `if TYPE_CHECKING:` are
                        // type-annotation-only (TYPE_CHECKING is always False
                        // at runtime). Including them causes false-positive
                        // cycle reports; skip them entirely.
                        if is_inside_type_checking_block(anchor, source) {
                            continue;
                        }
                        let mod_name = name_text.unwrap_or("_unknown");
                        // Issue #204: Python relative imports (`.X` / `..X`)
                        // resolve to actual file paths relative to the
                        // importing file. Absolute imports keep the symbolic
                        // node (resolution requires package discovery, out of
                        // scope for #204; tracked in #205 alias-table work).
                        let resolved = resolve_python_relative_import(file_path, mod_name);
                        let edge_target = resolved.as_deref().unwrap_or(mod_name);
                        if let Ok(mod_path) = TrunkPath::parse(edge_target) {
                            let mod_id = store.upsert_node(mod_path);
                            store.upsert_edge(EdgeKind::Imports, file_id, mod_id);
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
                        let caller_path =
                            enclosing_function_path(anchor, source).and_then(|suffix| {
                                TrunkPath::parse(&format!("{file_path}>{suffix}")).ok()
                            });
                        let caller_id = caller_path.map_or(file_id, |p| store.upsert_node(p));

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

                        let callee_id = if let Some(qualified) = resolved_target {
                            if let Some(id) = store.lookup(&qualified) {
                                id
                            } else if let Ok(path) = TrunkPath::parse(&qualified) {
                                store.upsert_node(path)
                            } else {
                                continue;
                            }
                        } else {
                            let intra = format!("{file_path}>{callee_name}");
                            if let Some(id) = store.lookup(&intra) {
                                id
                            } else if let Ok(bare) = TrunkPath::parse(callee_name) {
                                store.upsert_node(bare)
                            } else {
                                continue;
                            }
                        };
                        store.upsert_edge(EdgeKind::Calls, caller_id, callee_id);
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
        if kind == "class_definition" || kind == "class_declaration" || kind == "impl_item" {
            chain.push(container_name(parent, source).to_owned());
        }
        cur = parent;
    }
    chain.reverse();
    chain
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
        // `from . import M` (no `as`)  →  pkg-dir/M.py
        (true, None) => format!("{resolved_prefix}/{local}.py"),
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
fn enclosing_function_path(node: tree_sitter::Node<'_>, source: &[u8]) -> Option<String> {
    const FUNCTION_KINDS: &[&str] = &[
        "function_definition",  // Python
        "function_declaration", // TS/JS
        "function_expression",  // JS/TS
        "method_definition",    // TS/JS
        "function_item",        // Rust
    ];

    let mut cur = node;
    while let Some(parent) = cur.parent() {
        if FUNCTION_KINDS.contains(&parent.kind()) {
            let fn_name = parent
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source).ok())
                .unwrap_or("_unknown")
                .to_owned();

            // Collect enclosing class/impl containers (outermost first).
            let mut containers: Vec<String> = Vec::new();
            let mut scan = parent;
            while let Some(ancestor) = scan.parent() {
                let kind = ancestor.kind();
                if kind == "class_definition" || kind == "class_declaration" || kind == "impl_item"
                {
                    containers.push(container_name(ancestor, source).to_owned());
                }
                scan = ancestor;
            }
            containers.reverse();

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
        if kind == "class_definition" || kind == "class_declaration" || kind == "impl_item" {
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
        "mod" | "module" => Some(NodeKind::Module),
        "function" => Some(NodeKind::Function),
        "class" => Some(NodeKind::Class),
        "method" => Some(NodeKind::Method),
        "interface" | "trait" => Some(NodeKind::Interface),
        "type_alias" => Some(NodeKind::TypeAlias),
        "const" | "constant" => Some(NodeKind::Constant),
        "struct" => Some(NodeKind::Struct),
        "enum" => Some(NodeKind::Enum),
        other => NodeKind::try_from_wire(other),
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
