//! # Cortex — Salsa 3 reactive layer  (RFC-0003 / Phase 1)
//!
//! The Cortex wraps Mycelium's extraction pipeline in a
//! [Salsa 0.18](https://github.com/salsa-rs/salsa) incremental-computation
//! database, enabling **on-demand memoisation** of file parsing.
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │ Cortex (this module)                                         │
//! │                                                              │
//! │  InputFile ──► index_file() ──► Arc<FileIndex>               │
//! │  (Salsa input)  (tracked fn)    (extracted symbol list)      │
//! │                                                              │
//! │  Salsa manages memoisation + invalidation automatically.     │
//! │  The MCP watch loop applies FileIndex to the main Store.     │
//! └──────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Phase 1 scope
//!
//! * Define [`CortexDb`] trait and [`Cortex`] database struct.
//! * Define [`InputFile`] as a Salsa *input* (path + content bytes).
//! * Define [`FileIndex`] as a lightweight, `Eq`-able snapshot of a file's
//!   extracted symbols (paths + node kinds).
//! * Define [`index_file()`] as a Salsa *tracked* query that runs the
//!   language-aware [`Extractor`] and returns an `Arc<FileIndex>`.
//! * Wire the watch loop in `mycelium-mcp` to call
//!   [`Cortex::set_file`]/[`Cortex::query_file`] instead of calling the
//!   extractor directly.
//!
//! Phase 2 (separate RFC) will propagate Salsa invalidation signals to
//! full-graph [`Store`] mutations.
//!
//! ## Guarantees
//!
//! * Re-parsing the same file content is **free** — Salsa returns the
//!   cached `Arc<FileIndex>` without calling the extractor again.
//! * Changing a file's content (via [`Cortex::set_file`]) marks the
//!   `index_file` result as *stale*; the next call to [`Cortex::query_file`]
//!   re-runs the extractor for that file only.

// Salsa macros generate doc-less items; allow them in this module.
#![allow(missing_docs)]

use std::path::PathBuf;
use std::sync::Arc;

use salsa::Storage;
use tracing::warn;

use crate::extractor::Extractor;
use crate::store::Store;
use crate::types::{NodeKind, SourceSpan};

// ── embedded pack query constants ─────────────────────────────────────────────
//
// Duplicated from `mycelium-mcp` so that `mycelium-core` can be tested
// independently of the MCP crate.

// Pack queries are copied into ../packs/ at publish time so the crate is
// self-contained on crates.io (matches the pattern PR #145 set up for mycelium-mcp).
// During workspace development, ../packs/ is a copy of the workspace-root packs/.
const JAVASCRIPT_QUERIES: &str = include_str!("../packs/javascript/queries.scm");
const PYTHON_QUERIES: &str = include_str!("../packs/python/queries.scm");
const TYPESCRIPT_QUERIES: &str = include_str!("../packs/typescript/queries.scm");
const RUST_QUERIES: &str = include_str!("../packs/rust/queries.scm");
const GO_QUERIES: &str = include_str!("../packs/go/queries.scm");

// ── extracted symbol record ───────────────────────────────────────────────────

/// A single symbol extracted from a source file.
///
/// Lightweight, `Clone + Eq`-able record produced by [`index_file()`] and
/// stored in [`FileIndex`].  Designed for cheap comparison by Salsa's
/// backdating logic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractedSymbol {
    /// Repository-relative path of this symbol, e.g. `"src/main.rs>main"`.
    pub path: String,
    /// The kind of code element.
    pub kind: NodeKind,
    /// Source location.  Defaults to `SourceSpan::default()` when unavailable.
    pub span: SourceSpan,
}

// ── file index (return type of the tracked query) ─────────────────────────────

/// The result of indexing a single source file.
///
/// Returned (wrapped in [`Arc`]) by [`index_file()`].  Salsa uses `Eq` to
/// decide whether the result has changed and whether to propagate
/// invalidation to dependents.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileIndex {
    /// All symbols extracted from the file, in encounter order.
    pub symbols: Vec<ExtractedSymbol>,
}

impl FileIndex {
    /// Apply this index to a mutable [`Store`].
    ///
    /// Removes any existing nodes for `file_path` then re-inserts all
    /// extracted symbols.  Call this from the watch loop after
    /// [`Cortex::query_file`] returns a fresh index.
    pub fn apply_to_store(&self, file_path: &str, store: &mut Store) {
        store.remove_file(file_path);
        for sym in &self.symbols {
            use crate::trunk::TrunkPath;
            if let Ok(path) = TrunkPath::parse(&sym.path) {
                let id = store.upsert_node(path);
                store.set_kind(id, sym.kind);
                if !sym.span.is_empty() {
                    store.set_span(id, sym.span);
                }
            }
        }
    }
}

// ── Salsa database trait ──────────────────────────────────────────────────────

/// Salsa database trait for the Cortex reactive layer.
///
/// This is the *dyn-safe* surface that tracked functions receive as
/// `&dyn CortexDb`.  Concrete databases implement it alongside
/// [`salsa::Database`].
#[salsa::db]
pub trait CortexDb: salsa::Database {}

// ── concrete database ─────────────────────────────────────────────────────────

/// The Cortex Salsa database.
///
/// Construct with [`Cortex::default`] and then call [`Cortex::set_file`]
/// to feed file content into the reactive pipeline.
///
/// The database is [`Clone`] (Salsa requires this for `#[salsa::db]`)
/// and can be shared across threads via `Arc<tokio::sync::Mutex<Cortex>>`.
#[salsa::db]
#[derive(Clone)]
pub struct Cortex {
    storage: Storage<Self>,
}

impl std::fmt::Debug for Cortex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cortex").finish_non_exhaustive()
    }
}

#[allow(clippy::derivable_impls)]
impl Default for Cortex {
    fn default() -> Self {
        Self {
            storage: Storage::default(),
        }
    }
}

#[salsa::db]
impl salsa::Database for Cortex {
    fn salsa_event(&self, event: &dyn Fn() -> salsa::Event) {
        let ev = event();
        tracing::trace!(event = ?ev, "salsa event");
    }
}

#[salsa::db]
impl CortexDb for Cortex {}

// ── input ─────────────────────────────────────────────────────────────────────

/// A source file tracked as a Salsa *input*.
///
/// Fields are updated via the Salsa-generated setter API:
///
/// ```ignore
/// // Initial population:
/// let file = InputFile::new(&db, "src/main.rs".into(), Arc::new(src_bytes));
/// // Later, when the file changes on disk:
/// file.set_content(&mut db).to(Arc::new(new_bytes));
/// ```
///
/// Changing `content` automatically marks all downstream tracked queries
/// (including [`index_file()`]) as stale for that file.
#[salsa::input]
pub struct InputFile {
    /// Repository-relative path, e.g. `"src/main.rs"`.
    pub path: PathBuf,
    /// Raw source bytes.  Wrapped in [`Arc`] for cheap Salsa cloning.
    pub content: Arc<Vec<u8>>,
}

// ── tracked query ─────────────────────────────────────────────────────────────

/// Extract symbols from `file` and return a [`FileIndex`] snapshot.
///
/// This is a Salsa *tracked* function: the result is memoised per
/// `(db_revision, file)` key.  When `file.content` changes the old result is
/// invalidated and the extractor is re-run on the next call.
///
/// The return value is wrapped in [`Arc`] so Salsa can perform cheap
/// pointer-equality checks when deciding whether to propagate invalidation.
///
/// # Unsupported extensions
///
/// Files with unrecognised extensions return an empty [`FileIndex`].
/// Supported: `js`, `jsx`, `py`, `pyi`, `ts`, `tsx`, `rs`, `go`.
#[salsa::tracked]
pub fn index_file(db: &dyn CortexDb, file: InputFile) -> Arc<FileIndex> {
    let path = file.path(db);
    let content = file.content(db);

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let rel = path.to_string_lossy();

    let Some(extractor) = build_extractor(ext) else {
        return Arc::new(FileIndex::default());
    };

    let mut tmp_store = Store::new();
    if let Err(e) = extractor.extract(&rel, &content, &mut tmp_store) {
        warn!(path = %rel, error = %e, "cortex: extraction failed");
        return Arc::new(FileIndex::default());
    }

    // Collect extracted symbols from the temporary store.
    let symbols = tmp_store
        .all_paths()
        .map(|p| {
            let id = tmp_store.lookup(p).unwrap_or(crate::types::NodeId::NULL);
            ExtractedSymbol {
                path: p.to_owned(),
                kind: tmp_store.kind_of(id).unwrap_or(NodeKind::File),
                span: tmp_store.span_of(id).unwrap_or_default(),
            }
        })
        .collect();

    Arc::new(FileIndex { symbols })
}

/// Build a language [`Extractor`] for the given file extension, or `None`
/// if the extension is not supported by any embedded language pack.
fn build_extractor(ext: &str) -> Option<Extractor> {
    match ext {
        "js" | "jsx" => {
            let lang: tree_sitter::Language = tree_sitter_javascript::LANGUAGE.into();
            Extractor::new(lang, JAVASCRIPT_QUERIES).ok()
        }
        "py" | "pyi" => {
            let lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
            Extractor::new(lang, PYTHON_QUERIES).ok()
        }
        "ts" => {
            let lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
            Extractor::new(lang, TYPESCRIPT_QUERIES).ok()
        }
        "tsx" => {
            let lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TSX.into();
            Extractor::new(lang, TYPESCRIPT_QUERIES).ok()
        }
        "rs" => {
            let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
            Extractor::new(lang, RUST_QUERIES).ok()
        }
        "go" => {
            let lang: tree_sitter::Language = tree_sitter_go::LANGUAGE.into();
            Extractor::new(lang, GO_QUERIES).ok()
        }
        _ => None,
    }
}

// ── high-level helpers ────────────────────────────────────────────────────────

impl Cortex {
    /// Insert or update a file in the Cortex database.
    ///
    /// Returns an [`InputFile`] handle that can be used with
    /// [`Cortex::query_file`] to retrieve the per-file [`FileIndex`].
    ///
    /// To update an existing handle (and preserve Salsa memoisation for
    /// content that has not changed), keep the [`InputFile`] returned by the
    /// first call and use its setter directly:
    ///
    /// ```ignore
    /// file.set_content(&mut db).to(Arc::new(new_bytes));
    /// ```
    pub fn set_file(&mut self, path: PathBuf, content: Vec<u8>) -> InputFile {
        InputFile::new(self, path, Arc::new(content))
    }

    /// Run (or return the cached result of) [`index_file()`] for `file`.
    ///
    /// This is the primary read path for the watch loop.
    #[must_use]
    pub fn query_file(&self, file: InputFile) -> Arc<FileIndex> {
        index_file(self, file)
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use salsa::Setter;

    // ── helpers ───────────────────────────────────────────────────────────────

    fn make_cortex() -> Cortex {
        Cortex::default()
    }

    const PYTHON_SRC: &[u8] = b"def hello():\n    pass\n";
    const RUST_SRC: &[u8] = b"pub fn greet() {}\n";

    fn symbol_paths(idx: &FileIndex) -> Vec<&str> {
        idx.symbols.iter().map(|s| s.path.as_str()).collect()
    }

    // ── construction ─────────────────────────────────────────────────────────

    #[test]
    fn cortex_constructs_without_panic() {
        let _db = make_cortex();
    }

    #[test]
    fn input_file_stores_path_and_content() {
        let mut db = make_cortex();
        let file = db.set_file("src/hello.py".into(), PYTHON_SRC.to_vec());
        assert_eq!(file.path(&db), PathBuf::from("src/hello.py"));
        assert_eq!(file.content(&db).as_ref(), PYTHON_SRC);
    }

    // ── extraction ───────────────────────────────────────────────────────────

    #[test]
    fn index_file_extracts_python_function() {
        let mut db = make_cortex();
        let file = db.set_file("src/hello.py".into(), PYTHON_SRC.to_vec());
        let idx = db.query_file(file);

        let paths = symbol_paths(&idx);
        assert!(
            paths.iter().any(|p| p.contains("src/hello.py")),
            "expected file node, got: {paths:?}",
        );
        assert!(
            paths.iter().any(|p| p.contains("hello")),
            "expected `hello` symbol, got: {paths:?}",
        );
    }

    #[test]
    fn index_file_extracts_rust_function() {
        let mut db = make_cortex();
        let file = db.set_file("lib.rs".into(), RUST_SRC.to_vec());
        let idx = db.query_file(file);

        let paths = symbol_paths(&idx);
        assert!(
            paths.iter().any(|p| p.contains("greet")),
            "expected `greet` symbol, got: {paths:?}",
        );
    }

    #[test]
    fn index_file_unknown_extension_returns_empty_index() {
        let mut db = make_cortex();
        let file = db.set_file("README.md".into(), b"# hi".to_vec());
        let idx = db.query_file(file);
        assert!(
            idx.symbols.is_empty(),
            "expected empty index for unknown extension",
        );
    }

    // ── memoisation ──────────────────────────────────────────────────────────

    #[test]
    fn index_file_is_memoised_for_same_content() {
        let mut db = make_cortex();
        let file = db.set_file("src/hello.py".into(), PYTHON_SRC.to_vec());
        let idx1 = db.query_file(file);
        let idx2 = db.query_file(file);
        // Salsa must return the same Arc (pointer equality) on second call.
        assert!(
            Arc::ptr_eq(&idx1, &idx2),
            "index_file must return cached result for unchanged content",
        );
    }

    #[test]
    fn index_file_invalidates_on_content_change() {
        let mut db = make_cortex();
        let file = db.set_file("src/hello.py".into(), PYTHON_SRC.to_vec());
        let _idx1 = db.query_file(file);

        // Mutate the content — bumps the Salsa revision.
        let new_src = b"def goodbye():\n    pass\n";
        file.set_content(&mut db).to(Arc::new(new_src.to_vec()));

        let idx2 = db.query_file(file);
        let paths2 = symbol_paths(&idx2);

        assert!(
            paths2.iter().any(|p| p.contains("goodbye")),
            "expected `goodbye` after content update, got: {paths2:?}",
        );
        assert!(
            !paths2.iter().any(|p| p.ends_with(">hello")),
            "`hello` must be gone after content update, got: {paths2:?}",
        );
    }

    // ── multi-file ───────────────────────────────────────────────────────────

    #[test]
    fn multiple_files_tracked_independently() {
        let mut db = make_cortex();
        let py = db.set_file("a.py".into(), PYTHON_SRC.to_vec());
        let rs = db.set_file("b.rs".into(), RUST_SRC.to_vec());

        let py_store = db.query_file(py);
        let py_paths = symbol_paths(&py_store);
        let rs_store = db.query_file(rs);
        let rs_paths = symbol_paths(&rs_store);

        assert!(
            py_paths.iter().any(|p| p.contains("hello")),
            "python index must contain `hello`",
        );
        assert!(
            rs_paths.iter().any(|p| p.contains("greet")),
            "rust index must contain `greet`",
        );
        // Cross-contamination check.
        assert!(
            !py_paths.iter().any(|p| p.contains("greet")),
            "python index must not contain `greet`",
        );
    }

    // ── apply_to_store ────────────────────────────────────────────────────────

    #[test]
    fn file_index_apply_to_store_populates_store() {
        let mut db = make_cortex();
        let file = db.set_file("src/hello.py".into(), PYTHON_SRC.to_vec());
        let idx = db.query_file(file);

        let mut store = Store::new();
        idx.apply_to_store("src/hello.py", &mut store);

        let store_paths: Vec<String> = store.all_paths().map(ToOwned::to_owned).collect();
        assert!(
            store_paths.iter().any(|p| p.contains("hello")),
            "store must contain `hello` after apply, got: {store_paths:?}",
        );
    }
}
