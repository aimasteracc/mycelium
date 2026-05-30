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
                let src = m.captures.iter().find_map(|c| {
                    if names[c.index as usize] == "alias.source" {
                        c.node.utf8_text(source).ok()
                    } else {
                        None
                    }
                });
                let original = m.captures.iter().find_map(|c| {
                    if names[c.index as usize] == "alias.original_name" {
                        c.node.utf8_text(source).ok()
                    } else {
                        None
                    }
                });
                let (Some(local), Some(src)) = (local, src) else {
                    continue;
                };
                let resolved = build_alias_target(file_path, src, original, local);
                alias_table.insert(local.to_string(), resolved);
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

                        // RFC-0092: alias-aware dispatch for receiver.method() calls.
                        // If the receiver is in the alias table, rewrite to the
                        // resolved path. Otherwise fall back to intra-file lookup
                        // then bare-symbol upsert.
                        let resolved_target = receiver
                            .and_then(|r| alias_table.get(r))
                            .map(|prefix| format!("{prefix}>{callee_name}"));

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
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

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
