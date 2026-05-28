//! # Symbol extractor
//!
//! Parses source files with tree-sitter using a [`LanguagePack`]'s query
//! source, then populates a [`Store`] with nodes and edges.
//!
//! See RFC-0002 for the full design.

use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator as _};

use crate::{store::Store, trunk::TrunkPath, types::EdgeKind};

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

        let names = self.query.capture_names();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&self.query, root, source);

        while let Some(m) = matches.next() {
            // Identify the "definition" or "reference" capture in this match.
            let def_cap = m.captures.iter().find(|c| {
                let n = names[c.index as usize];
                n.starts_with("definition.") || n.starts_with("reference.")
            });

            let Some(def_cap) = def_cap else { continue };
            let cap_name = names[def_cap.index as usize];
            let anchor = def_cap.node;

            // Find the @name capture (identifier text), if present.
            let name_text: Option<&str> = m.captures.iter().find_map(|c| {
                if names[c.index as usize] == "name" {
                    c.node.utf8_text(source).ok()
                } else {
                    None
                }
            });

            match cap_name {
                "definition.module" => {
                    // File node already created above.
                    let _ = file_id;
                }

                "definition.function" | "definition.class" => {
                    let name = name_text.unwrap_or("_unknown");
                    let path_str = format!("{file_path}>{name}");
                    if let Ok(path) = TrunkPath::parse(&path_str) {
                        let child_id = store.upsert_node(path);
                        store.upsert_edge(EdgeKind::Contains, file_id, child_id);
                    }
                }

                "definition.method" => {
                    let method_name = name_text.unwrap_or("_unknown");
                    // anchor is the class_definition node.
                    let class_chain = build_class_chain(anchor, source);
                    let chain_str = class_chain.join(">");
                    let path_str = format!("{file_path}>{chain_str}>{method_name}");
                    if let Ok(path) = TrunkPath::parse(&path_str) {
                        let method_id = store.upsert_node(path);
                        // Upsert the class node (may already exist).
                        let class_path_str = format!("{file_path}>{chain_str}");
                        if let Ok(cls_path) = TrunkPath::parse(&class_path_str) {
                            let cls_id = store.upsert_node(cls_path);
                            store.upsert_edge(EdgeKind::Contains, file_id, cls_id);
                            store.upsert_edge(EdgeKind::Contains, cls_id, method_id);
                        }
                    }
                }

                "reference.import" | "reference.import_from" => {
                    let mod_name = name_text.unwrap_or("_unknown");
                    // Module path may be dotted (e.g., "os.path") — use as-is.
                    if let Ok(mod_path) = TrunkPath::parse(mod_name) {
                        let mod_id = store.upsert_node(mod_path);
                        store.upsert_edge(EdgeKind::Imports, file_id, mod_id);
                    }
                }

                _ => {}
            }
        }

        Ok(())
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Walk ancestor nodes of `node`, collecting names of enclosing
/// `class_definition` nodes in outermost→innermost order.
///
/// `node` itself is the *innermost* class; this function returns the chain of
/// classes that contain it (its ancestors), NOT including `node` itself.
/// Callers combine the result with `node`'s own name to get the full chain.
fn build_class_chain(node: tree_sitter::Node<'_>, source: &[u8]) -> Vec<String> {
    // anchor IS the innermost class_definition.  Get ITS name first.
    let own_name = node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(source).ok())
        .unwrap_or("_Unknown")
        .to_owned();

    // Then collect any enclosing class_definition ancestors.
    let mut ancestors: Vec<String> = Vec::new();
    let mut cur = node;
    while let Some(parent) = cur.parent() {
        if parent.kind() == "class_definition" {
            if let Some(name_node) = parent.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source) {
                    ancestors.push(name.to_owned());
                }
            }
        }
        cur = parent;
    }
    ancestors.reverse(); // outermost first

    ancestors.push(own_name);
    ancestors
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
