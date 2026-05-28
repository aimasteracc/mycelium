//! # mycelium-hyphae
//!
//! Hyphae — the query language for Mycelium.
//!
//! **Status: not yet designed.** This crate is scaffolded so the
//! workspace builds. RFC-0003 will define the grammar and semantics.
//! Until then, the only types here are placeholders.
//!
//! See [RFC-0001](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0001-trunk-and-synapse.md)
//! for the storage layer Hyphae will sit on top of.

#![doc(html_root_url = "https://docs.rs/mycelium-hyphae")]

/// Placeholder. The real `Query` type lands with RFC-0003.
#[derive(Clone, Debug, Default)]
pub struct Query {
    _placeholder: (),
}

impl Query {
    /// Create an empty query placeholder. Not useful yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
