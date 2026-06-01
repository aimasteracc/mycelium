//! `StorageBackend` trait — the abstraction layer between the high-level
//! `Store` API and the physical storage engine (in-memory or redb).
//!
//! RFC-0100 §3 / P1-T06.
//!
//! ## Design constraints
//!
//! - Object-safe: `Box<dyn StorageBackend>` must compile.
//! - `Send + Sync`: backends may be shared across threads.
//! - No associated types in method signatures (object-safety).
//! - `flush()` is a no-op for `InMemoryBackend`; a commit+checkpoint for `RedbBackend`.

use thiserror::Error;

use crate::types::{EdgeKind, NodeId, NodeKind, SourceSpan};

// ── error type ───────────────────────────────────────────────────────────────

/// Errors that can occur in a [`StorageBackend`] operation.
#[derive(Debug, Error)]
pub enum StorageError {
    /// The requested node or edge was not found.
    #[error("not found")]
    NotFound,

    /// The on-disk schema version is newer than this build understands.
    #[error("schema version mismatch: file={file}, supported={supported}")]
    SchemaVersion {
        /// Version recorded in the file.
        file: u32,
        /// Highest version this binary supports.
        supported: u32,
    },

    /// An encoding or decoding failure.
    #[error("encode/decode error: {0}")]
    Encode(String),

    /// An underlying backend I/O or engine error.
    #[error("backend error: {0}")]
    Backend(String),
}

// ── trait ────────────────────────────────────────────────────────────────────

/// The core storage contract implemented by both `InMemoryBackend` and
/// `RedbBackend`.
///
/// All methods that return collections return owned `Vec`s to stay
/// object-safe (no `impl Iterator` in signatures).
pub trait StorageBackend: Send + Sync {
    // ── node primitives ───────────────────────────────────────────────────

    /// Insert or retrieve the node for `path`, returning its stable [`NodeId`].
    ///
    /// Idempotent: calling with the same path twice yields the same id.
    fn upsert_node(&mut self, path: &str) -> NodeId;

    /// Remove a node and all its edges from the backend.
    fn remove_node(&mut self, id: NodeId);

    /// Materialize the full path for `id`. `None` if unknown.
    fn path_of(&self, id: NodeId) -> Option<String>;

    /// Look up `id` by exact path. `None` if not materialized.
    fn lookup_path(&self, path: &str) -> Option<NodeId>;

    /// Total number of materialized nodes.
    fn node_count(&self) -> usize;

    /// All materialized paths (order unspecified).
    fn all_paths(&self) -> Vec<String>;

    // ── kind / span ────────────────────────────────────────────────────────

    /// Record the `NodeKind` for an already-upserted node.
    fn set_kind(&mut self, id: NodeId, kind: NodeKind);

    /// Return the `NodeKind` for `id`, or `None` if not recorded.
    fn kind_of(&self, id: NodeId) -> Option<NodeKind>;

    /// Record the `SourceSpan` for an already-upserted node.
    fn set_span(&mut self, id: NodeId, span: SourceSpan);

    /// Return the `SourceSpan` for `id`, or `None` if not recorded.
    fn span_of(&self, id: NodeId) -> Option<SourceSpan>;

    // ── edge primitives ────────────────────────────────────────────────────

    /// Insert a directed edge `kind: src → dst`. Idempotent.
    fn upsert_edge(&mut self, kind: EdgeKind, src: NodeId, dst: NodeId);

    /// Remove all edges that involve `id` (as source or destination).
    fn remove_node_edges(&mut self, id: NodeId);

    /// Targets of `src` for the given edge `kind`. Sorted ascending.
    fn outgoing(&self, src: NodeId, kind: EdgeKind) -> Vec<NodeId>;

    /// Sources that point to `dst` for the given edge `kind`. Sorted ascending.
    fn incoming(&self, dst: NodeId, kind: EdgeKind) -> Vec<NodeId>;

    /// Total number of directed edges across all kinds.
    fn edge_count(&self) -> usize;

    /// All edges as `(kind, src, dst)` triples (order unspecified).
    fn all_edges(&self) -> Vec<(EdgeKind, NodeId, NodeId)>;

    // ── metrics ────────────────────────────────────────────────────────────

    /// Estimated heap bytes consumed by the backend's in-memory structures.
    ///
    /// For `InMemoryBackend` this is the same heuristic as `Store::heap_size_estimate`.
    /// For `RedbBackend` this reports the redb allocated mmap/storage page
    /// footprint so memory-curve tests can compare formula estimates with real
    /// backend accounting.
    fn heap_size_estimate(&self) -> usize;

    // ── persistence ────────────────────────────────────────────────────────

    /// Commit pending writes and sync to disk.
    ///
    /// For `InMemoryBackend` this is always `Ok(())`.
    /// For `RedbBackend` this finalises the current write transaction.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::Backend` if the underlying engine reports an error.
    fn flush(&mut self) -> Result<(), StorageError>;
}
