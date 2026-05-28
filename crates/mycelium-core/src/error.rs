//! Error type for `mycelium-core`.
//!
//! All fallible operations return [`Result<T>`].

use thiserror::Error;

/// Errors emitted by the `mycelium-core` engine.
///
/// Per Charter §5.4 we use `thiserror` for library errors. Binary
/// crates (CLI, MCP server) may wrap these in `anyhow::Error` at their
/// outer boundary.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// A `TrunkPath` failed to parse: empty, contains an empty segment, or contains a forbidden character.
    #[error("invalid trunk path: {reason}")]
    InvalidPath {
        /// Human-readable explanation of what made the path invalid.
        reason: String,
    },

    /// A lookup found no node at the given path.
    #[error("path not found: {path}")]
    NotFound {
        /// The path that was looked up but not present.
        path: String,
    },

    /// I/O failure while reading or writing the on-disk store.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Shorthand for `core::result::Result<T, Error>`.
pub type Result<T, E = Error> = core::result::Result<T, E>;
