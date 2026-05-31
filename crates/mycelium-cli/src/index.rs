//! `mycelium index` — walk a directory, extract symbols, report statistics.
//!
//! This is the first end-user-visible command that produces real output.
//! It exercises the full RFC-0001 + RFC-0002 stack.
// Items here are `pub` so main.rs can use them, but this module is private to
// the binary.  `unreachable_pub` is a false positive in this pattern.
#![allow(unreachable_pub)]

use std::path::Path;

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use mycelium_core::{extractor::Extractor, store::Store};
use mycelium_pack::PackRegistry;

// ── embedded pack sources ─────────────────────────────────────────────────────

// Pack queries are copied into ../packs/ at publish time so the crate is
// self-contained on crates.io (matches the pattern PR #145 set up for mycelium-mcp
// and the rename PR applied to mycelium-core).
const JAVASCRIPT_QUERIES: &str = include_str!("../packs/javascript/queries.scm");
const PYTHON_QUERIES: &str = include_str!("../packs/python/queries.scm");
const TYPESCRIPT_QUERIES: &str = include_str!("../packs/typescript/queries.scm");
const RUST_QUERIES: &str = include_str!("../packs/rust/queries.scm");
const GO_QUERIES: &str = include_str!("../packs/go/queries.scm");
const JAVA_QUERIES: &str = include_str!("../packs/java/queries.scm");
const C_QUERIES: &str = include_str!("../packs/c/queries.scm");
const RUBY_QUERIES: &str = include_str!("../packs/ruby/queries.scm");
const CPP_QUERIES: &str = include_str!("../packs/cpp/queries.scm");
const CSHARP_QUERIES: &str = include_str!("../packs/csharp/queries.scm");

// ── public surface ────────────────────────────────────────────────────────────

/// Statistics returned from a successful index run.
#[derive(Debug, Default)]
pub struct IndexStats {
    /// Number of source files processed.
    pub files: usize,
    /// Files that could not be read or extracted (non-fatal).
    pub errors: usize,
}

/// Map a grammar string from `pack.toml` to the corresponding compiled-in
/// `tree_sitter::Language`. Only languages bundled in this binary are
/// supported; returns `None` for unknown grammars.
fn grammar_language_for_name(grammar: &str) -> Option<tree_sitter::Language> {
    if grammar.contains("tree-sitter-javascript") {
        Some(tree_sitter_javascript::LANGUAGE.into())
    } else if grammar.contains("tree-sitter-python") {
        Some(tree_sitter_python::LANGUAGE.into())
    } else if grammar.contains("tree-sitter-typescript") {
        Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
    } else if grammar.contains("tree-sitter-rust") {
        Some(tree_sitter_rust::LANGUAGE.into())
    } else if grammar.contains("tree-sitter-go") {
        Some(tree_sitter_go::LANGUAGE.into())
    } else if grammar.contains("tree-sitter-java") {
        Some(tree_sitter_java::LANGUAGE.into())
    } else if grammar.contains("tree-sitter-c-sharp") || grammar.contains("tree-sitter-c_sharp") {
        Some(tree_sitter_c_sharp::LANGUAGE.into())
    } else if grammar.contains("tree-sitter-ruby") {
        Some(tree_sitter_ruby::LANGUAGE.into())
    } else if grammar.contains("tree-sitter-cpp") {
        Some(tree_sitter_cpp::LANGUAGE.into())
    } else if grammar.contains("tree-sitter-c") {
        Some(tree_sitter_c::LANGUAGE.into())
    } else {
        None
    }
}

/// Source-language extensions used by compound-extension detection (Issue #294).
const SOURCE_EXTS: &[&str] = &[
    "js", "jsx", "ts", "tsx", "py", "pyi", "rs", "go", "java", "c", "h", "cpp", "cc", "cxx", "hpp",
    "rb", "cs",
];

/// The 11 compiled-in static extractors, built once and shared by `&` across
/// indexing threads. `Extractor`'s fields (`tree_sitter::Language`, `Query`)
/// are `Send + Sync`, so `&Extractors` is safe to hand to many threads; each
/// `extract` call builds its own `Parser` internally (Issue #342 / R1).
struct Extractors {
    js: Extractor,
    python: Extractor,
    ts: Extractor,
    tsx: Extractor,
    rs: Extractor,
    go: Extractor,
    java: Extractor,
    c: Extractor,
    ruby: Extractor,
    cpp: Extractor,
    csharp: Extractor,
}

impl Extractors {
    /// Compile all 11 static extractors. Done once per index run.
    #[allow(clippy::similar_names)]
    fn build() -> Result<Self> {
        let js_lang: tree_sitter::Language = tree_sitter_javascript::LANGUAGE.into();
        let python_lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
        let ts_lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        let tsx_lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TSX.into();
        let rs_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let go_lang: tree_sitter::Language = tree_sitter_go::LANGUAGE.into();
        let java_lang: tree_sitter::Language = tree_sitter_java::LANGUAGE.into();
        let c_lang: tree_sitter::Language = tree_sitter_c::LANGUAGE.into();
        let ruby_lang: tree_sitter::Language = tree_sitter_ruby::LANGUAGE.into();
        let cpp_lang: tree_sitter::Language = tree_sitter_cpp::LANGUAGE.into();
        let csharp_lang: tree_sitter::Language = tree_sitter_c_sharp::LANGUAGE.into();
        Ok(Self {
            js: Extractor::new(js_lang, JAVASCRIPT_QUERIES)
                .context("failed to compile JavaScript extractor")?,
            python: Extractor::new(python_lang, PYTHON_QUERIES)
                .context("failed to compile Python extractor")?,
            ts: Extractor::new(ts_lang, TYPESCRIPT_QUERIES)
                .context("failed to compile TypeScript extractor")?,
            tsx: Extractor::new(tsx_lang, TYPESCRIPT_QUERIES)
                .context("failed to compile TSX extractor")?,
            rs: Extractor::new(rs_lang, RUST_QUERIES)
                .context("failed to compile Rust extractor")?,
            go: Extractor::new(go_lang, GO_QUERIES).context("failed to compile Go extractor")?,
            java: Extractor::new(java_lang, JAVA_QUERIES)
                .context("failed to compile Java extractor")?,
            c: Extractor::new(c_lang, C_QUERIES).context("failed to compile C extractor")?,
            ruby: Extractor::new(ruby_lang, RUBY_QUERIES)
                .context("failed to compile Ruby extractor")?,
            cpp: Extractor::new(cpp_lang, CPP_QUERIES)
                .context("failed to compile C++ extractor")?,
            csharp: Extractor::new(csharp_lang, CSHARP_QUERIES)
                .context("failed to compile C# extractor")?,
        })
    }

    /// Pick the static extractor for a file extension, if any.
    fn pick(&self, ext: &str) -> Option<&Extractor> {
        match ext {
            "js" | "jsx" => Some(&self.js),
            "py" | "pyi" => Some(&self.python),
            "ts" => Some(&self.ts),
            "tsx" => Some(&self.tsx),
            "rs" => Some(&self.rs),
            "go" => Some(&self.go),
            "java" => Some(&self.java),
            "c" | "h" => Some(&self.c),
            "rb" => Some(&self.ruby),
            "cpp" | "cc" | "cxx" | "hpp" => Some(&self.cpp),
            "cs" => Some(&self.csharp),
            _ => None,
        }
    }
}

/// Outcome of indexing one file. Counts roll up into [`IndexStats`].
enum FileOutcome {
    /// Successfully extracted into the store.
    Indexed,
    /// Not a source file we handle (no extractor / compound extension). No-op.
    Skipped,
    /// A real error occurred (read failure, `strip_prefix`, extraction error).
    Errored,
}

/// Index a single file into `store`. Pure with respect to `(path, contents)`:
/// the extractor only resolves same-file definitions; cross-file calls become
/// bare stubs resolved after all files are merged. This is what makes parallel
/// extract-then-merge equivalent to the serial single-store build (Issue #342).
fn index_file_into(
    store: &mut Store,
    exts: &Extractors,
    registry: Option<&PackRegistry>,
    root: &Path,
    path: &Path,
) -> FileOutcome {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return FileOutcome::Skipped;
    };

    // Issue #294: skip compound source-language extensions like `module.ts.py`.
    if let Some(stem_ext) = path
        .file_stem()
        .and_then(|s| std::path::Path::new(s).extension())
        .and_then(|e| e.to_str())
    {
        if SOURCE_EXTS.contains(&stem_ext) && stem_ext != ext {
            tracing::debug!("skipping compound-extension file: {}", path.display());
            return FileOutcome::Skipped;
        }
    }

    // Static built-in extractor first; fall through to the registry for unknown
    // extensions when --packs-dir is provided.
    let dynamic_ext: Option<Extractor>;
    let extractor: &Extractor = if let Some(e) = exts.pick(ext) {
        e
    } else if let Some(reg) = registry {
        let dotted = format!(".{ext}");
        if let Some(pack) = reg.lookup_by_ext(&dotted) {
            if let Some(lang) = grammar_language_for_name(&pack.manifest.meta.grammar) {
                dynamic_ext = Extractor::new(lang, &pack.queries).ok();
                match dynamic_ext.as_ref() {
                    Some(e) => e,
                    None => return FileOutcome::Skipped,
                }
            } else {
                return FileOutcome::Skipped;
            }
        } else {
            return FileOutcome::Skipped;
        }
    } else {
        return FileOutcome::Skipped;
    };

    // Issue #294: skip rather than store an absolute path if strip_prefix fails.
    let Ok(rel_path) = path.strip_prefix(root) else {
        tracing::warn!(
            "could not relativize path {} against root {}; skipping",
            path.display(),
            root.display()
        );
        return FileOutcome::Errored;
    };
    let rel = rel_path.to_string_lossy().replace('\\', "/");

    let source = match std::fs::read(path) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("could not read {}: {e}", path.display());
            return FileOutcome::Errored;
        }
    };

    if let Err(e) = extractor.extract(&rel, &source, store) {
        tracing::warn!("extraction failed for {}: {e}", path.display());
        return FileOutcome::Errored;
    }

    FileOutcome::Indexed
}

/// Walk `root` and collect candidate source-file paths, honoring `.gitignore`,
/// `.myceliumignore`, and the hard-coded `target/` + `.mycelium/` exclusions.
/// Shared by the serial and parallel index paths so they see the same files.
fn collect_source_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut walk_builder = WalkBuilder::new(root);
    walk_builder
        .follow_links(false)
        .add_custom_ignore_filename(".myceliumignore");
    for name in &[".gitignore", ".myceliumignore"] {
        let p = root.join(name);
        if p.exists() {
            walk_builder.add_ignore(&p);
        }
    }
    walk_builder
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !matches!(name.as_ref(), "target" | ".mycelium")
        })
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_some_and(|ft| ft.is_file()))
        .map(|e| e.path().to_path_buf())
        .collect()
}

/// Walk `root`, extract all recognised source files, and return stats.
///
/// Supported languages: JavaScript (`.js`, `.jsx`), Python (`.py`, `.pyi`),
/// TypeScript (`.ts`, `.tsx`), Rust (`.rs`), Go (`.go`),
/// Java (`.java`), C (`.c`, `.h`), Ruby (`.rb`),
/// C++ (`.cpp`, `.cc`, `.cxx`, `.hpp`), C# (`.cs`).
///
/// If `packs_dir` is provided, additional language packs are loaded from
/// that directory via [`PackRegistry`]. Unknown file extensions are dispatched
/// to the registry; packs whose grammar does not match a compiled-in
/// tree-sitter grammar are silently skipped.
///
/// # Errors
///
/// Returns an error only if `root` cannot be accessed. Individual file errors
/// are counted in [`IndexStats::errors`] but do not stop the run.
pub fn index_path(root: &Path, packs_dir: Option<&Path>) -> Result<(Store, IndexStats)> {
    let extractors = Extractors::build()?;

    // Load runtime pack registry when --packs-dir is provided.
    let registry: Option<PackRegistry> = packs_dir.and_then(|dir| match PackRegistry::load(dir) {
        Ok(r) => Some(r),
        Err(e) => {
            tracing::warn!("failed to load pack registry from {}: {e}", dir.display());
            None
        }
    });

    let mut store = Store::new();
    let mut stats = IndexStats::default();

    for path in collect_source_files(root) {
        match index_file_into(&mut store, &extractors, registry.as_ref(), root, &path) {
            FileOutcome::Indexed => stats.files += 1,
            FileOutcome::Errored => stats.errors += 1,
            FileOutcome::Skipped => {}
        }
    }

    // Resolve cross-file call stubs after all files are processed.
    store.resolve_bare_call_stubs();

    Ok((store, stats))
}

/// Parallel variant of [`index_path`] (Issue #342 / R1).
///
/// Walks the tree serially (cheap), then extracts files **in parallel** across
/// `available_parallelism()` OS threads, each folding into a thread-local
/// [`Store`]. The sub-stores are reduced with `Store::merge`, an
/// order-independent union (`NodeId`s are content hashes), so the result is
/// **semantically identical** to [`index_path`] — same nodes, edges, kinds, and
/// spans — regardless of thread scheduling. (Byte-identity of the snapshot is
/// not a goal: even two serial runs differ byte-wise because `HashMap`
/// iteration order is unstable.)
///
/// `packs_dir` (runtime registry) is not yet thread-verified and falls back to
/// the serial [`index_path`]; the built-in languages cover the large-repo case
/// this optimization targets.
///
/// # Errors
///
/// Returns an error only if the extractors fail to compile. Per-file errors are
/// counted in [`IndexStats::errors`] and do not stop the run.
pub fn index_path_parallel(root: &Path, packs_dir: Option<&Path>) -> Result<(Store, IndexStats)> {
    if packs_dir.is_some() {
        return index_path(root, packs_dir);
    }

    let extractors = Extractors::build()?;
    let files = collect_source_files(root);

    let workers = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    if workers <= 1 || files.len() <= 1 {
        let mut store = Store::new();
        let mut stats = IndexStats::default();
        for path in &files {
            match index_file_into(&mut store, &extractors, None, root, path) {
                FileOutcome::Indexed => stats.files += 1,
                FileOutcome::Errored => stats.errors += 1,
                FileOutcome::Skipped => {}
            }
        }
        store.resolve_bare_call_stubs();
        return Ok((store, stats));
    }

    let chunk_size = files.len().div_ceil(workers);
    let extractors_ref = &extractors;

    // Each thread folds its chunk into a thread-local (Store, IndexStats).
    // `std::thread::scope` lets closures borrow `extractors_ref`, `root`, and
    // their `files` slice without `'static` bounds or `Arc`.
    let partials: Vec<(Store, IndexStats)> = std::thread::scope(|scope| {
        // The `collect` here is load-bearing, NOT needless: every thread must
        // be spawned (started) before we join any of them. Folding spawn+join
        // into one lazy iterator chain would serialize the work (spawn, join,
        // spawn, join, …), defeating the parallelism. Hence the explicit Vec.
        #[allow(clippy::needless_collect)]
        let handles: Vec<_> = files
            .chunks(chunk_size)
            .map(|chunk| {
                scope.spawn(move || {
                    let mut store = Store::new();
                    let mut stats = IndexStats::default();
                    for path in chunk {
                        match index_file_into(&mut store, extractors_ref, None, root, path) {
                            FileOutcome::Indexed => stats.files += 1,
                            FileOutcome::Errored => stats.errors += 1,
                            FileOutcome::Skipped => {}
                        }
                    }
                    (store, stats)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("index worker thread panicked"))
            .collect()
    });

    // Reduce: union all sub-stores, sum all stats. Order-independent (#345).
    let mut store = Store::new();
    let mut stats = IndexStats::default();
    for (sub, sub_stats) in partials {
        store.merge(&sub);
        stats.files += sub_stats.files;
        stats.errors += sub_stats.errors;
    }

    // Resolve cross-file call stubs once, after the full graph is merged —
    // exactly as the serial path does after its loop.
    store.resolve_bare_call_stubs();

    Ok((store, stats))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn write_temp_py(dir: &Path, name: &str, src: &str) {
        fs::write(dir.join(name), src).unwrap();
    }

    fn write_temp_ts(dir: &Path, name: &str, src: &str) {
        fs::write(dir.join(name), src).unwrap();
    }

    #[test]
    fn index_path_extracts_single_file() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_py(
            tmp.path(),
            "hello.py",
            "def greet(): pass\nclass World: pass",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1, "should process one file");
        assert!(stats.errors == 0);
        assert!(store.lookup("hello.py").is_some(), "file node should exist");
        assert!(store.lookup("hello.py>greet").is_some());
        assert!(store.lookup("hello.py>World").is_some());
    }

    #[test]
    fn index_path_skips_non_python_files() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_py(tmp.path(), "main.py", "x = 1");
        fs::write(tmp.path().join("README.md"), "# docs").unwrap();
        let (_, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1, "only .py file should be counted");
    }

    #[test]
    fn index_path_recurses_into_subdirectories() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub");
        fs::create_dir(&sub).unwrap();
        write_temp_py(&sub, "deep.py", "def deep(): pass");
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        // On Unix, the relative path uses forward slash
        assert!(store.lookup("sub/deep.py").is_some());
    }

    #[test]
    fn index_path_handles_empty_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let (_, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 0);
        assert_eq!(stats.errors, 0);
    }

    // ── TypeScript tests ─────────────────────────────────────────────

    #[test]
    fn index_path_extracts_typescript_function() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_ts(
            tmp.path(),
            "greet.ts",
            "function greet(name: string): void {}",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert_eq!(stats.errors, 0);
        assert!(store.lookup("greet.ts").is_some(), "module node must exist");
        assert!(
            store.lookup("greet.ts>greet").is_some(),
            "function node must exist"
        );
    }

    #[test]
    fn index_path_extracts_typescript_class_and_method() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_ts(
            tmp.path(),
            "greeter.ts",
            "class Greeter { greet(): string { return ''; } }",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert!(store.lookup("greeter.ts>Greeter").is_some());
        assert!(store.lookup("greeter.ts>Greeter>greet").is_some());
    }

    #[test]
    fn index_path_extracts_typescript_interface() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_ts(
            tmp.path(),
            "types.ts",
            "interface IGreeter { greet(): void; }",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert!(store.lookup("types.ts>IGreeter").is_some());
    }

    #[test]
    fn index_path_indexes_both_python_and_typescript() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_py(tmp.path(), "mod.py", "def foo(): pass");
        write_temp_ts(tmp.path(), "mod.ts", "function bar(): void {}");
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 2);
        assert!(store.lookup("mod.py>foo").is_some());
        assert!(store.lookup("mod.ts>bar").is_some());
    }

    #[test]
    fn index_path_indexes_tsx_as_typescript() {
        let tmp = tempfile::tempdir().unwrap();
        // TSX is treated as TypeScript; JSX nodes are transparent to symbol extraction.
        write_temp_ts(tmp.path(), "app.tsx", "function App() { return null; }");
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(
            stats.files, 1,
            "tsx should be indexed via the TypeScript extractor"
        );
        assert!(
            store.lookup("app.tsx>App").is_some(),
            "App function must be extracted"
        );
    }

    #[test]
    fn index_path_indexes_jsx_as_javascript() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_ts(tmp.path(), "app.jsx", "function App() { return null; }");
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(
            stats.files, 1,
            "jsx should be indexed via the JavaScript extractor"
        );
        assert!(
            store.lookup("app.jsx>App").is_some(),
            "App function must be extracted"
        );
    }

    #[test]
    fn index_path_indexes_js_function() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_ts(
            tmp.path(),
            "utils.js",
            "function add(a, b) { return a + b; }",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert!(store.lookup("utils.js>add").is_some());
    }

    // ── Rust tests ───────────────────────────────────────────────────

    fn write_temp_rs(dir: &Path, name: &str, src: &str) {
        fs::write(dir.join(name), src).unwrap();
    }

    #[test]
    fn index_path_extracts_rust_function() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_rs(tmp.path(), "lib.rs", "fn greet() {}");
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert_eq!(stats.errors, 0);
        assert!(store.lookup("lib.rs").is_some());
        assert!(store.lookup("lib.rs>greet").is_some());
    }

    #[test]
    fn index_path_extracts_rust_struct_and_impl_method() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_rs(
            tmp.path(),
            "model.rs",
            "struct Point { x: i32 } impl Point { fn new() -> Self { Point { x: 0 } } }",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert!(store.lookup("model.rs>Point").is_some());
        assert!(store.lookup("model.rs>Point>new").is_some());
    }

    #[test]
    fn index_path_extracts_rust_enum_and_trait() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_rs(
            tmp.path(),
            "types.rs",
            "enum Color { Red } trait Drawable { fn draw(&self); }",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert!(store.lookup("types.rs>Color").is_some());
        assert!(store.lookup("types.rs>Drawable").is_some());
    }

    // ── RFC-0009 ignore-filtering tests ──────────────────────────────

    #[test]
    fn index_path_skips_gitignored_directory() {
        let tmp = tempfile::tempdir().unwrap();
        // Write a .gitignore that excludes node_modules/
        fs::write(tmp.path().join(".gitignore"), "node_modules/\n").unwrap();
        let nm = tmp.path().join("node_modules");
        fs::create_dir(&nm).unwrap();
        fs::write(nm.join("dep.js"), "function dep() {}").unwrap();
        // Also write a non-ignored file
        write_temp_py(tmp.path(), "main.py", "def main(): pass");

        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1, "only main.py should be indexed");
        assert!(
            store.lookup("main.py>main").is_some(),
            "main.py symbols must be present"
        );
        assert!(
            store.lookup("node_modules/dep.js").is_none(),
            "node_modules must be skipped"
        );
    }

    #[test]
    fn index_path_always_skips_target_dir() {
        let tmp = tempfile::tempdir().unwrap();
        // No .gitignore — target/ must still be skipped by default.
        let target = tmp.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("artifact.rs"), "fn artifact() {}").unwrap();
        write_temp_rs(tmp.path(), "lib.rs", "fn real() {}");

        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1, "only lib.rs should be indexed");
        assert!(
            store.lookup("target/artifact.rs").is_none(),
            "target/ must always be skipped"
        );
    }

    #[test]
    fn index_path_always_skips_mycelium_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let snap_dir = tmp.path().join(".mycelium");
        fs::create_dir(&snap_dir).unwrap();
        fs::write(snap_dir.join("index.rmp"), b"not real").unwrap();
        write_temp_rs(tmp.path(), "lib.rs", "fn real() {}");

        let (_store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1, ".mycelium/ must not be indexed");
    }

    #[test]
    fn index_path_respects_myceliumignore() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join(".myceliumignore"), "vendor/\n").unwrap();
        let vendor = tmp.path().join("vendor");
        fs::create_dir(&vendor).unwrap();
        fs::write(vendor.join("third.py"), "def third(): pass").unwrap();
        write_temp_py(tmp.path(), "app.py", "def app(): pass");

        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(
            stats.files, 1,
            "vendor/ should be excluded via .myceliumignore"
        );
        assert!(store.lookup("app.py>app").is_some());
        assert!(store.lookup("vendor/third.py").is_none());
    }

    // ── Go tests ─────────────────────────────────────────────────────

    fn write_temp_go(dir: &Path, name: &str, src: &str) {
        fs::write(dir.join(name), src).unwrap();
    }

    #[test]
    fn index_path_extracts_go_function() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_go(tmp.path(), "main.go", "package main\n\nfunc greet() {}\n");
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1, "should process one .go file");
        assert_eq!(stats.errors, 0);
        assert!(store.lookup("main.go").is_some(), "module node must exist");
        assert!(
            store.lookup("main.go>greet").is_some(),
            "Go function must be extracted"
        );
    }

    #[test]
    fn index_path_extracts_go_type_declaration() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_go(
            tmp.path(),
            "types.go",
            "package main\n\ntype Point struct { X int; Y int }\n",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert_eq!(stats.errors, 0);
        assert!(
            store.lookup("types.go>Point").is_some(),
            "Go struct type must be extracted"
        );
    }

    #[test]
    fn index_path_extracts_go_method() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_go(
            tmp.path(),
            "geom.go",
            "package main\n\ntype Rect struct {}\nfunc (r Rect) Area() int { return 0 }\n",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert_eq!(stats.errors, 0);
        assert!(
            store.lookup("geom.go>Rect").is_some(),
            "Rect type must be extracted"
        );
        assert!(
            store.lookup("geom.go>Area").is_some(),
            "Go method must be extracted"
        );
    }

    #[test]
    fn index_path_indexes_go_alongside_other_languages() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_py(tmp.path(), "mod.py", "def foo(): pass");
        write_temp_rs(tmp.path(), "lib.rs", "fn bar() {}");
        write_temp_go(tmp.path(), "main.go", "package main\nfunc baz() {}");
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 3, "all three languages should be indexed");
        assert!(store.lookup("mod.py>foo").is_some());
        assert!(store.lookup("lib.rs>bar").is_some());
        assert!(store.lookup("main.go>baz").is_some());
    }

    #[test]
    fn index_path_extracts_go_interface() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_go(
            tmp.path(),
            "iface.go",
            "package main\n\ntype Stringer interface { String() string }\n",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert!(
            store.lookup("iface.go>Stringer").is_some(),
            "Go interface type must be extracted"
        );
    }
    // ── Java tests ───────────────────────────────────────────────────

    fn write_temp_java(dir: &Path, name: &str, src: &str) {
        fs::write(dir.join(name), src).unwrap();
    }

    #[test]
    fn index_path_extracts_java_class() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_java(
            tmp.path(),
            "Hello.java",
            "public class Hello { public void greet() {} }",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1, "should process one .java file");
        assert_eq!(stats.errors, 0);
        assert!(
            store.lookup("Hello.java>Hello").is_some(),
            "Java class must be extracted"
        );
        assert!(
            store.lookup("Hello.java>Hello>greet").is_some(),
            "Java method must be extracted"
        );
    }

    #[test]
    fn index_path_extracts_java_interface() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_java(
            tmp.path(),
            "Greeter.java",
            "public interface Greeter { void greet(); }",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert_eq!(stats.errors, 0);
        assert!(
            store.lookup("Greeter.java>Greeter").is_some(),
            "Java interface must be extracted"
        );
    }

    #[test]
    fn index_path_extracts_java_alongside_other_languages() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_py(tmp.path(), "mod.py", "def foo(): pass");
        write_temp_java(
            tmp.path(),
            "App.java",
            "public class App { public static void main(String[] args) {} }",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 2, "both languages should be indexed");
        assert!(store.lookup("mod.py>foo").is_some());
        assert!(store.lookup("App.java>App").is_some());
    }

    // ── C tests ──────────────────────────────────────────────────────

    fn write_temp_c(dir: &Path, name: &str, src: &str) {
        fs::write(dir.join(name), src).unwrap();
    }

    #[test]
    fn index_path_extracts_c_function() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_c(tmp.path(), "main.c", "int greet(void) { return 0; }");
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1, "should process one .c file");
        assert_eq!(stats.errors, 0);
        assert!(
            store.lookup("main.c>greet").is_some(),
            "C function must be extracted"
        );
    }

    #[test]
    fn index_path_extracts_c_struct() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_c(tmp.path(), "types.c", "struct Point { int x; int y; };");
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert_eq!(stats.errors, 0);
        assert!(
            store.lookup("types.c>Point").is_some(),
            "C struct must be extracted"
        );
    }

    #[test]
    fn index_path_extracts_c_header() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_c(tmp.path(), "api.h", "int compute(int a, int b);");
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1, "should process one .h file");
        assert_eq!(stats.errors, 0);
        assert!(
            store.lookup("api.h").is_some(),
            "header module node must exist"
        );
    }

    #[test]
    fn index_path_extracts_c_alongside_other_languages() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_rs(tmp.path(), "lib.rs", "fn bar() {}");
        write_temp_c(tmp.path(), "util.c", "void util(void) {}");
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 2, "both languages should be indexed");
        assert!(store.lookup("lib.rs>bar").is_some());
        assert!(store.lookup("util.c>util").is_some());
    }

    // ── Ruby tests ───────────────────────────────────────────────────

    fn write_temp_rb(dir: &Path, name: &str, src: &str) {
        fs::write(dir.join(name), src).unwrap();
    }

    #[test]
    fn index_path_extracts_ruby_class_and_method() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_rb(
            tmp.path(),
            "greeter.rb",
            "class Greeter\n  def greet\n  end\nend\n",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1, "should process one .rb file");
        assert_eq!(stats.errors, 0);
        assert!(
            store.lookup("greeter.rb>Greeter").is_some(),
            "Ruby class must be extracted"
        );
        assert!(
            store.lookup("greeter.rb>Greeter>greet").is_some(),
            "Ruby method must be extracted"
        );
    }

    #[test]
    fn index_path_extracts_ruby_module() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_rb(
            tmp.path(),
            "helpers.rb",
            "module Helpers\n  def help\n  end\nend\n",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert_eq!(stats.errors, 0);
        assert!(
            store.lookup("helpers.rb>Helpers").is_some(),
            "Ruby module must be extracted"
        );
    }

    #[test]
    fn index_path_extracts_ruby_alongside_other_languages() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_py(tmp.path(), "mod.py", "def foo(): pass");
        write_temp_rb(tmp.path(), "app.rb", "class App\ndef run\nend\nend\n");
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 2, "both languages should be indexed");
        assert!(store.lookup("mod.py>foo").is_some());
        assert!(store.lookup("app.rb>App").is_some());
    }

    // ── C++ tests ────────────────────────────────────────────────────

    fn write_temp_cpp(dir: &Path, name: &str, src: &str) {
        fs::write(dir.join(name), src).unwrap();
    }

    #[test]
    fn index_path_extracts_cpp_function() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_cpp(tmp.path(), "main.cpp", "int greet() { return 0; }");
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1, "should process one .cpp file");
        assert_eq!(stats.errors, 0);
        assert!(store.lookup("main.cpp").is_some(), "module node must exist");
        assert!(
            store.lookup("main.cpp>greet").is_some(),
            "C++ function must be extracted"
        );
    }

    #[test]
    fn index_path_extracts_cpp_class() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_cpp(
            tmp.path(),
            "shape.cpp",
            "class Shape { public: int area() { return 0; } };",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert_eq!(stats.errors, 0);
        assert!(
            store.lookup("shape.cpp>Shape").is_some(),
            "C++ class must be extracted"
        );
    }

    #[test]
    fn index_path_extracts_cpp_namespace() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_cpp(
            tmp.path(),
            "ns.cpp",
            "namespace MyNS { int helper() { return 0; } }",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert_eq!(stats.errors, 0);
        assert!(
            store.lookup("ns.cpp>MyNS").is_some(),
            "C++ namespace must be extracted"
        );
    }

    #[test]
    fn index_path_extracts_cpp_hpp_header() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_cpp(tmp.path(), "point.hpp", "struct Point { int x; int y; };");
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1, ".hpp file should be indexed as C++");
        assert!(
            store.lookup("point.hpp>Point").is_some(),
            "C++ struct from .hpp must be extracted"
        );
    }

    #[test]
    fn index_path_extracts_cpp_cc_extension() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_cpp(
            tmp.path(),
            "util.cc",
            "int add(int a, int b) { return a + b; }",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1, ".cc file should be indexed as C++");
        assert!(
            store.lookup("util.cc>add").is_some(),
            "C++ function from .cc must be extracted"
        );
    }

    #[test]
    fn index_path_indexes_cpp_alongside_other_languages() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_py(tmp.path(), "mod.py", "def foo(): pass");
        write_temp_rs(tmp.path(), "lib.rs", "fn bar() {}");
        write_temp_cpp(tmp.path(), "main.cpp", "int baz() { return 0; }");
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 3, "all three languages should be indexed");
        assert!(store.lookup("mod.py>foo").is_some());
        assert!(store.lookup("lib.rs>bar").is_some());
        assert!(store.lookup("main.cpp>baz").is_some());
    }

    // ── C# tests ─────────────────────────────────────────────────────

    fn write_temp_cs(dir: &Path, name: &str, src: &str) {
        fs::write(dir.join(name), src).unwrap();
    }

    #[test]
    fn index_path_extracts_csharp_class() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_cs(
            tmp.path(),
            "Hello.cs",
            "namespace App { public class Hello { public void Greet() {} } }",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1, "should process one .cs file");
        assert_eq!(stats.errors, 0);
        assert!(store.lookup("Hello.cs").is_some(), "module node must exist");
        assert!(
            store.lookup("Hello.cs>Hello").is_some(),
            "C# class must be extracted"
        );
    }

    #[test]
    fn index_path_extracts_csharp_interface() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_cs(
            tmp.path(),
            "IGreeter.cs",
            "public interface IGreeter { void Greet(); }",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert!(
            store.lookup("IGreeter.cs>IGreeter").is_some(),
            "C# interface must be extracted"
        );
    }

    #[test]
    fn index_path_extracts_csharp_namespace() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_cs(
            tmp.path(),
            "App.cs",
            "namespace MyApp { public class Service {} }",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert!(
            store.lookup("App.cs>MyApp").is_some(),
            "C# namespace must be extracted"
        );
        assert!(
            store.lookup("App.cs>Service").is_some(),
            "C# class within namespace must be extracted"
        );
    }

    #[test]
    fn index_path_extracts_csharp_method() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_cs(
            tmp.path(),
            "Calc.cs",
            "public class Calc { public int Add(int a, int b) { return a + b; } }",
        );
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 1);
        assert!(
            store.lookup("Calc.cs>Calc").is_some(),
            "C# class must be extracted"
        );
        assert!(
            store.lookup("Calc.cs>Calc>Add").is_some(),
            "C# method must be extracted under its class"
        );
    }

    #[test]
    fn index_path_indexes_csharp_alongside_other_languages() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp_py(tmp.path(), "mod.py", "def foo(): pass");
        write_temp_cpp(tmp.path(), "main.cpp", "int bar() { return 0; }");
        write_temp_cs(tmp.path(), "App.cs", "public class Baz {}");
        let (store, stats) = index_path(tmp.path(), None).unwrap();
        assert_eq!(stats.files, 3, "all three languages should be indexed");
        assert!(store.lookup("mod.py>foo").is_some());
        assert!(store.lookup("main.cpp>bar").is_some());
        assert!(store.lookup("App.cs>Baz").is_some());
    }

    // ── --packs-dir tests ─────────────────────────────────────────────

    fn write_custom_pack(packs_root: &Path, ext: &str, grammar: &str, queries_src: &str) {
        let pack_subdir = packs_root.join("customlang");
        fs::create_dir_all(&pack_subdir).unwrap();
        let toml = format!(
            "[meta]\nname = \"customlang\"\nextensions = [\"{ext}\"]\ngrammar = \"{grammar}\"\n"
        );
        fs::write(pack_subdir.join("pack.toml"), toml).unwrap();
        fs::write(pack_subdir.join("queries.scm"), queries_src).unwrap();
    }

    #[test]
    fn index_path_with_packs_dir_indexes_custom_extension() {
        let root = tempfile::tempdir().unwrap();
        let packs_dir = tempfile::tempdir().unwrap();

        // A custom pack for .mypy using the Python grammar + real Python queries.
        let python_queries = include_str!("../packs/python/queries.scm");
        write_custom_pack(
            packs_dir.path(),
            ".mypy",
            "npm:tree-sitter-python@^0.21",
            python_queries,
        );

        // A .mypy file with Python-compatible source.
        fs::write(root.path().join("hello.mypy"), b"def hello(): pass").unwrap();

        let (store, stats) =
            index_path(root.path(), Some(packs_dir.path())).expect("index must succeed");

        assert_eq!(stats.files, 1, "custom extension file must be indexed");
        assert_eq!(stats.errors, 0);
        assert!(
            store.lookup("hello.mypy>hello").is_some(),
            "function from custom-ext file must appear in the store"
        );
    }

    #[test]
    fn index_path_without_packs_dir_ignores_custom_extension() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("hello.mypy"), b"def hello(): pass").unwrap();

        let (_, stats) = index_path(root.path(), None).expect("index must succeed");

        assert_eq!(
            stats.files, 0,
            "unknown extension must not be indexed without packs-dir"
        );
    }

    // ── Issue #294: compound-extension / mangled-path guards ─────────────────

    #[test]
    fn index_path_skips_compound_extension_file() {
        // A file named `module.ts.py` has last extension `py` but stem `module.ts`
        // whose own extension is `ts` — a source-language extension that conflicts.
        // The indexer must skip this file rather than index it as Python, because
        // it is almost certainly a build artifact or cache file, not a real Python
        // file.  Issue #294.
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("module.ts.py"), b"x = 1").unwrap();

        let (store, stats) = index_path(root.path(), None).expect("index must succeed");

        assert_eq!(stats.files, 0, "compound-extension file must be skipped");
        assert!(
            store.lookup("module.ts.py").is_none(),
            "compound-extension file must not create a node in the store"
        );
    }

    #[test]
    fn index_path_does_not_skip_simple_named_py_file() {
        // A file named `js.py` (stem `js`, no extension in stem) is a legitimate
        // Python file — not a compound-extension artifact.  It must still be
        // indexed as Python (Issue #294 — non-regression).
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("js.py"), b"def handle(): pass").unwrap();

        let (store, stats) = index_path(root.path(), None).expect("index must succeed");

        assert_eq!(stats.files, 1, "js.py must be indexed as Python");
        assert!(
            store.lookup("js.py>handle").is_some(),
            "js.py>handle must be in the store"
        );
    }

    #[test]
    fn index_path_stored_paths_are_relative() {
        // All paths stored in the index must be relative to the root, never
        // absolute.  Absolute paths (mangled '///' prefix) indicate that
        // path.strip_prefix(root) failed and the raw OS path leaked through.
        // Issue #294.
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("app.py"), b"def run(): pass").unwrap();

        let (store, _) = index_path(root.path(), None).expect("index must succeed");

        for path in store.all_paths() {
            assert!(
                !path.starts_with('/'),
                "stored path must be relative, got absolute: {path}"
            );
        }
    }
}
