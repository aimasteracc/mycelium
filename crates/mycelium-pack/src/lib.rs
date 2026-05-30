//! # mycelium-pack
//!
//! Loads language packs from `packs/<lang>/{pack.toml, queries.scm}` and
//! provides the runtime surface the extractor uses.
//!
//! ## Pack directory layout (Charter §4)
//!
//! ```text
//! packs/<lang>/
//! ├── pack.toml     — metadata (name, extensions, grammar source)
//! └── queries.scm   — tree-sitter capture queries for symbol extraction
//! ```
//!
//! No more than 3 files per pack. No modifications to core code are ever
//! needed to add a language — the Charter §4 hard constraint.
//!
//! ## Quick example
//!
//! ```no_run
//! use std::path::Path;
//! use mycelium_pack::LanguagePack;
//!
//! let pack = LanguagePack::load(Path::new("packs/python")).unwrap();
//! assert_eq!(pack.manifest.meta.name, "python");
//! println!("{} extensions: {:?}", pack.manifest.meta.name, pack.manifest.meta.extensions);
//! ```

#![doc(html_root_url = "https://docs.rs/mycelium-pack")]

use std::path::{Path, PathBuf};

use serde::Deserialize;

// ── error type ────────────────────────────────────────────────────────────────

/// Errors that can occur while loading a language pack.
#[derive(Debug, thiserror::Error)]
pub enum PackError {
    /// A required file in the pack directory could not be read.
    #[error("failed to read `{path}`: {source}")]
    Io {
        /// The path that failed.
        path: PathBuf,
        /// The underlying I/O error.
        source: std::io::Error,
    },

    /// `pack.toml` is malformed.
    #[error("failed to parse pack.toml: {0}")]
    Toml(#[from] toml::de::Error),
}

// ── manifest schema ───────────────────────────────────────────────────────────

/// Parsed `pack.toml`.
#[derive(Clone, Debug, Deserialize)]
pub struct PackManifest {
    /// `[meta]` section — required.
    pub meta: Meta,
}

/// `[meta]` block of `pack.toml`.
#[derive(Clone, Debug, Deserialize)]
pub struct Meta {
    /// Human-readable language name, e.g. `"python"`.
    pub name: String,
    /// All file extensions this pack can handle (union of primary and secondary).
    /// For packs with ambiguous extensions (e.g. C++ shares `.h` with C) prefer
    /// setting [`Meta::primary_extensions`] and [`Meta::secondary_extensions`] instead.
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Extensions that unambiguously belong to this language (e.g. `.cpp`, `.cc`).
    /// When present the indexer uses this list for automatic dispatch instead of
    /// [`Meta::extensions`].
    #[serde(default)]
    pub primary_extensions: Vec<String>,
    /// Extensions shared with other languages (e.g. `.h` for both C and C++).
    /// The indexer only uses these when an explicit language hint is provided.
    #[serde(default)]
    pub secondary_extensions: Vec<String>,
    /// Grammar source reference, e.g. `"npm:tree-sitter-python@^0.21"`.
    pub grammar: String,
    /// Optional human-readable description.
    #[serde(default)]
    pub description: Option<String>,
}

impl Meta {
    /// Return the extensions used for automatic file-extension dispatch.
    ///
    /// If `primary_extensions` is non-empty it is used; otherwise falls back to
    /// `extensions`.  This lets packs with ambiguous extensions (C++/C sharing
    /// `.h`) declare only their unambiguous extensions for indexer dispatch.
    #[must_use]
    pub fn dispatch_extensions(&self) -> &[String] {
        if self.primary_extensions.is_empty() {
            &self.extensions
        } else {
            &self.primary_extensions
        }
    }
}

// ── loaded pack ───────────────────────────────────────────────────────────────

/// A fully loaded language pack ready for use by the extractor.
pub struct LanguagePack {
    /// Parsed `pack.toml` metadata.
    pub manifest: PackManifest,
    /// Raw tree-sitter query source from `queries.scm`.
    pub queries: String,
}

impl LanguagePack {
    /// Load a language pack from `pack_dir`.
    ///
    /// Reads `pack.toml` and `queries.scm` from `pack_dir`. Returns an error
    /// if either file is missing, unreadable, or `pack.toml` is malformed.
    ///
    /// # Errors
    ///
    /// Returns [`PackError::Io`] if a file cannot be read, or
    /// [`PackError::Toml`] if `pack.toml` is malformed TOML.
    pub fn load(pack_dir: &Path) -> Result<Self, PackError> {
        let manifest_path = pack_dir.join("pack.toml");
        let toml_src = std::fs::read_to_string(&manifest_path).map_err(|source| PackError::Io {
            path: manifest_path,
            source,
        })?;
        let manifest: PackManifest = toml::from_str(&toml_src)?;

        let queries_path = pack_dir.join("queries.scm");
        let queries = std::fs::read_to_string(&queries_path).map_err(|source| PackError::Io {
            path: queries_path,
            source,
        })?;

        Ok(Self { manifest, queries })
    }

    /// Return the language name from the manifest `[meta]` section.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.manifest.meta.name
    }

    /// Return the file extensions this pack handles.
    ///
    /// For dispatch purposes, prefer [`Meta::dispatch_extensions`] which
    /// returns only `primary_extensions` when present.
    #[must_use]
    pub fn extensions(&self) -> &[String] {
        &self.manifest.meta.extensions
    }
}

// ── registry ─────────────────────────────────────────────────────────────────

/// A collection of language packs indexed by file extension for fast dispatch.
///
/// Call [`PackRegistry::load`] once at startup, then use
/// [`PackRegistry::lookup_by_ext`] in the hot path.
pub struct PackRegistry {
    packs: Vec<LanguagePack>,
    /// Maps each extension (e.g. `".py"`) to an index into `packs`.
    by_ext: std::collections::HashMap<String, usize>,
}

impl PackRegistry {
    /// Load all language packs from `packs_dir`.
    ///
    /// Scans `packs_dir` for sub-directories that contain a `pack.toml`
    /// and loads each one.  Directories without `pack.toml` are silently
    /// skipped (allows plain files or future non-pack sub-dirs).
    ///
    /// # Errors
    ///
    /// Returns an error if `packs_dir` cannot be read (e.g. does not exist).
    pub fn load(packs_dir: &std::path::Path) -> Result<Self, PackError> {
        let mut packs: Vec<LanguagePack> = Vec::new();
        let mut by_ext: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        let entries = std::fs::read_dir(packs_dir).map_err(|source| PackError::Io {
            path: packs_dir.to_path_buf(),
            source,
        })?;

        for entry in entries {
            let entry = entry.map_err(|source| PackError::Io {
                path: packs_dir.to_path_buf(),
                source,
            })?;
            let sub = entry.path();
            if !sub.is_dir() {
                continue;
            }
            if !sub.join("pack.toml").exists() {
                continue;
            }
            match LanguagePack::load(&sub) {
                Ok(pack) => {
                    let idx = packs.len();
                    let exts: Vec<String> = pack.manifest.meta.dispatch_extensions().to_vec();
                    packs.push(pack);
                    for ext in exts {
                        by_ext.entry(ext).or_insert(idx);
                    }
                }
                Err(e) => {
                    tracing::warn!(path = %sub.display(), error = %e, "skipping malformed pack");
                }
            }
        }

        Ok(Self { packs, by_ext })
    }

    /// Look up a pack by file extension (e.g. `".py"`, `".ts"`).
    ///
    /// Returns `None` if no loaded pack handles this extension.
    #[must_use]
    pub fn lookup_by_ext(&self, ext: &str) -> Option<&LanguagePack> {
        self.by_ext.get(ext).map(|&idx| &self.packs[idx])
    }

    /// All loaded packs in load order.
    #[must_use]
    pub fn packs(&self) -> &[LanguagePack] {
        &self.packs
    }

    /// Number of loaded packs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.packs.len()
    }

    /// Returns `true` if no packs are loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.packs.is_empty()
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// Resolve the workspace root from this crate's `CARGO_MANIFEST_DIR`.
    fn workspace_root() -> PathBuf {
        // CARGO_MANIFEST_DIR = .../crates/mycelium-pack
        // two parents up = workspace root
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn pack_loader_reads_python_pack() {
        let pack_dir = workspace_root().join("packs/python");
        let pack = LanguagePack::load(&pack_dir).expect("python pack should load");

        assert_eq!(pack.name(), "python");
        assert!(
            pack.extensions().contains(&".py".to_string()),
            "python pack must list .py extension"
        );
        assert!(
            pack.extensions().contains(&".pyi".to_string()),
            "python pack must list .pyi stub extension"
        );
        assert!(!pack.queries.is_empty(), "queries.scm must be non-empty");
        assert!(
            pack.queries.contains("@definition"),
            "queries.scm must contain @definition captures"
        );
    }

    #[test]
    fn pack_loader_reads_go_pack() {
        let pack_dir = workspace_root().join("packs/go");
        let pack = LanguagePack::load(&pack_dir).expect("go pack should load");

        assert_eq!(pack.name(), "go");
        assert!(
            pack.extensions().contains(&".go".to_string()),
            "go pack must list .go extension"
        );
        assert!(!pack.queries.is_empty(), "queries.scm must be non-empty");
        assert!(
            pack.queries.contains("@definition"),
            "queries.scm must contain @definition captures"
        );
        assert!(
            pack.queries.contains("@reference"),
            "queries.scm must contain @reference captures"
        );
    }

    #[test]
    fn pack_loader_reads_cpp_pack() {
        let pack_dir = workspace_root().join("packs/cpp");
        let pack = LanguagePack::load(&pack_dir).expect("cpp pack should load");

        assert_eq!(pack.name(), "cpp");
        // C++ uses primary_extensions for dispatch (.cpp, .cc, .cxx, .hpp)
        let dispatch = pack.manifest.meta.dispatch_extensions();
        assert!(
            dispatch.contains(&".cpp".to_string()),
            "cpp pack must list .cpp as a dispatch extension"
        );
        assert!(
            dispatch.contains(&".cc".to_string()),
            "cpp pack must list .cc as a dispatch extension"
        );
        assert!(
            dispatch.contains(&".cxx".to_string()),
            "cpp pack must list .cxx as a dispatch extension"
        );
        assert!(
            dispatch.contains(&".hpp".to_string()),
            "cpp pack must list .hpp as a dispatch extension"
        );
        // .h is a secondary extension (shared with C)
        assert!(
            pack.manifest
                .meta
                .secondary_extensions
                .contains(&".h".to_string()),
            "cpp pack must list .h as a secondary extension"
        );
        // .h must NOT be in the dispatch extensions to avoid ambiguity with C
        assert!(
            !dispatch.contains(&".h".to_string()),
            ".h must not be in dispatch_extensions to avoid C/C++ ambiguity"
        );
        assert!(!pack.queries.is_empty(), "queries.scm must be non-empty");
        assert!(
            pack.queries.contains("@definition"),
            "queries.scm must contain @definition captures"
        );
        assert!(
            pack.queries.contains("@reference"),
            "queries.scm must contain @reference captures"
        );
    }

    #[test]
    fn pack_loader_reads_csharp_pack() {
        let pack_dir = workspace_root().join("packs/csharp");
        let pack = LanguagePack::load(&pack_dir).expect("csharp pack should load");

        assert_eq!(pack.name(), "csharp");
        assert!(
            pack.extensions().contains(&".cs".to_string()),
            "csharp pack must list .cs extension"
        );
        assert!(!pack.queries.is_empty(), "queries.scm must be non-empty");
        assert!(
            pack.queries.contains("@definition"),
            "queries.scm must contain @definition captures"
        );
        assert!(
            pack.queries.contains("@reference"),
            "queries.scm must contain @reference captures"
        );
    }

    #[test]
    fn dispatch_extensions_falls_back_to_extensions_when_no_primary() {
        // Python pack uses `extensions` only (no primary_extensions)
        let pack_dir = workspace_root().join("packs/python");
        let pack = LanguagePack::load(&pack_dir).expect("python pack should load");
        assert_eq!(
            pack.manifest.meta.dispatch_extensions(),
            pack.manifest.meta.extensions.as_slice(),
            "dispatch_extensions should return extensions when primary_extensions is empty"
        );
    }

    #[test]
    fn dispatch_extensions_returns_primary_when_present() {
        // C++ pack uses primary_extensions to avoid .h ambiguity
        let pack_dir = workspace_root().join("packs/cpp");
        let pack = LanguagePack::load(&pack_dir).expect("cpp pack should load");
        assert_eq!(
            pack.manifest.meta.dispatch_extensions(),
            pack.manifest.meta.primary_extensions.as_slice(),
            "dispatch_extensions should return primary_extensions when non-empty"
        );
    }

    #[test]
    fn pack_loader_errors_on_missing_dir() {
        let result = LanguagePack::load(Path::new("/nonexistent/does/not/exist"));
        assert!(result.is_err());
    }
    #[test]
    fn pack_loader_reads_java_pack() {
        let pack_dir = workspace_root().join("packs/java");
        let pack = LanguagePack::load(&pack_dir).expect("java pack should load");
        assert_eq!(pack.name(), "java");
        assert!(
            pack.extensions().contains(&".java".to_string()),
            "java pack must list .java extension"
        );
        assert!(!pack.queries.is_empty(), "queries.scm must be non-empty");
        assert!(
            pack.queries.contains("@definition"),
            "queries.scm must contain @definition"
        );
        assert!(
            pack.queries.contains("@reference"),
            "queries.scm must contain @reference"
        );
    }

    #[test]
    fn pack_loader_reads_c_pack() {
        let pack_dir = workspace_root().join("packs/c");
        let pack = LanguagePack::load(&pack_dir).expect("c pack should load");
        assert_eq!(pack.name(), "c");
        assert!(
            pack.extensions().contains(&".c".to_string()),
            "c pack must list .c extension"
        );
        assert!(
            pack.extensions().contains(&".h".to_string()),
            "c pack must list .h extension"
        );
        assert!(!pack.queries.is_empty(), "queries.scm must be non-empty");
        assert!(
            pack.queries.contains("@definition"),
            "queries.scm must contain @definition"
        );
        assert!(
            pack.queries.contains("@reference"),
            "queries.scm must contain @reference"
        );
    }

    #[test]
    fn pack_loader_reads_ruby_pack() {
        let pack_dir = workspace_root().join("packs/ruby");
        let pack = LanguagePack::load(&pack_dir).expect("ruby pack should load");
        assert_eq!(pack.name(), "ruby");
        assert!(
            pack.extensions().contains(&".rb".to_string()),
            "ruby pack must list .rb extension"
        );
        assert!(!pack.queries.is_empty(), "queries.scm must be non-empty");
        assert!(
            pack.queries.contains("@definition"),
            "queries.scm must contain @definition"
        );
        assert!(
            pack.queries.contains("@reference"),
            "queries.scm must contain @reference"
        );
    }

    // ── PackRegistry tests ────────────────────────────────────────────────────

    #[test]
    fn registry_loads_all_bundled_packs() {
        let packs_dir = workspace_root().join("packs");
        let registry = PackRegistry::load(&packs_dir).expect("registry should load");
        let names: Vec<&str> = registry.packs().iter().map(LanguagePack::name).collect();
        for expected in &["python", "javascript", "typescript", "rust", "go"] {
            assert!(
                names.contains(expected),
                "registry must contain pack '{expected}'"
            );
        }
    }

    #[test]
    fn registry_lookup_by_ext_returns_correct_pack() {
        let packs_dir = workspace_root().join("packs");
        let registry = PackRegistry::load(&packs_dir).expect("registry should load");

        let py = registry
            .lookup_by_ext(".py")
            .expect(".py must map to python pack");
        assert_eq!(py.name(), "python");

        let js = registry
            .lookup_by_ext(".js")
            .expect(".js must map to javascript pack");
        assert_eq!(js.name(), "javascript");

        let ts = registry
            .lookup_by_ext(".ts")
            .expect(".ts must map to typescript pack");
        assert_eq!(ts.name(), "typescript");
    }

    #[test]
    fn registry_lookup_unknown_ext_returns_none() {
        let packs_dir = workspace_root().join("packs");
        let registry = PackRegistry::load(&packs_dir).expect("registry should load");
        assert!(
            registry.lookup_by_ext(".brainfuck").is_none(),
            "unknown extension must return None"
        );
    }

    #[test]
    fn registry_errors_on_missing_dir() {
        let result = PackRegistry::load(Path::new("/nonexistent/packs/dir"));
        assert!(result.is_err(), "loading from a missing dir must fail");
    }
}
