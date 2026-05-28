//! # mycelium-pack
//!
//! Loads language packs from `packs/<lang>/{pack.toml, queries.scm, [hooks.wasm]}`
//! and provides the runtime surface the extractor uses.
//!
//! **Status: scaffold only.** RFC for the pack format will accompany the
//! first real language pack PR.

#![doc(html_root_url = "https://docs.rs/mycelium-pack")]

use serde::Deserialize;

/// Top-level `pack.toml` shape (subset, expanded incrementally).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PackManifest {
    /// `[meta]` section.
    pub meta: Meta,
}

/// `[meta]` block of `pack.toml`.
#[derive(Clone, Debug, Deserialize)]
pub struct Meta {
    /// Human-readable language name, e.g. `"python"`.
    pub name: String,
    /// File extensions, leading dot included, e.g. `[".py", ".pyi"]`.
    pub extensions: Vec<String>,
    /// Grammar source, e.g. `"npm:tree-sitter-python@^0.21"`.
    pub grammar: String,
}
