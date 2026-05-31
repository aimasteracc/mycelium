//! `InMemoryBackend` — `StorageBackend` implementation backed by the existing
//! `Trunk` + `Synapse` + `HashMap` in-memory structures.
//!
//! This is the *oracle* backend: it preserves 100% of the current `Store`
//! behaviour and serves as the reference against which `RedbBackend` is
//! verified during equivalence testing (Phase 2).
//!
//! RFC-0100 / P1-T07.

use hashbrown::HashMap;

use crate::store::backend::{StorageBackend, StorageError};
use crate::synapse::Synapse;
use crate::trunk::{Trunk, TrunkPath};
use crate::types::{EdgeKind, NodeId, NodeKind, SourceSpan};

/// In-memory storage backend wrapping Trunk + Synapse + metadata maps.
///
/// All operations are O(1) amortised (hash-table + trie lookup).
/// No persistence: data is lost when the backend is dropped.
pub struct InMemoryBackend {
    trunk: Trunk,
    synapse: Synapse,
    kind_map: HashMap<NodeId, NodeKind>,
    span_map: HashMap<NodeId, SourceSpan>,
}

impl InMemoryBackend {
    /// Create an empty backend.
    #[must_use]
    pub fn new() -> Self {
        Self {
            trunk: Trunk::new(),
            synapse: Synapse::new(),
            kind_map: HashMap::new(),
            span_map: HashMap::new(),
        }
    }
}

impl Default for InMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageBackend for InMemoryBackend {
    // ── node primitives ───────────────────────────────────────────────────

    fn upsert_node(&mut self, path: &str) -> NodeId {
        match TrunkPath::parse(path) {
            Ok(tp) => self.trunk.upsert(tp),
            Err(_) => NodeId::NULL,
        }
    }

    fn remove_node(&mut self, id: NodeId) {
        self.trunk.remove(id);
        self.synapse.remove_node(id);
        self.kind_map.remove(&id);
        self.span_map.remove(&id);
    }

    fn path_of(&self, id: NodeId) -> Option<String> {
        self.trunk.path_of(id).map(str::to_owned)
    }

    fn lookup_path(&self, path: &str) -> Option<NodeId> {
        self.trunk.lookup_path(path)
    }

    fn node_count(&self) -> usize {
        self.trunk.len()
    }

    fn all_paths(&self) -> Vec<String> {
        self.trunk.all_paths().map(str::to_owned).collect()
    }

    // ── kind / span ────────────────────────────────────────────────────────

    fn set_kind(&mut self, id: NodeId, kind: NodeKind) {
        self.kind_map.insert(id, kind);
    }

    fn kind_of(&self, id: NodeId) -> Option<NodeKind> {
        self.kind_map.get(&id).copied()
    }

    fn set_span(&mut self, id: NodeId, span: SourceSpan) {
        self.span_map.insert(id, span);
    }

    fn span_of(&self, id: NodeId) -> Option<SourceSpan> {
        self.span_map.get(&id).copied()
    }

    // ── edge primitives ────────────────────────────────────────────────────

    fn upsert_edge(&mut self, kind: EdgeKind, src: NodeId, dst: NodeId) {
        self.synapse.add(kind, src, dst);
    }

    fn remove_node_edges(&mut self, id: NodeId) {
        self.synapse.remove_node(id);
    }

    fn outgoing(&self, src: NodeId, kind: EdgeKind) -> Vec<NodeId> {
        let mut v = self.synapse.outgoing(src, kind).to_vec();
        v.sort_unstable();
        v
    }

    fn incoming(&self, dst: NodeId, kind: EdgeKind) -> Vec<NodeId> {
        let mut v = self.synapse.incoming(dst, kind).to_vec();
        v.sort_unstable();
        v
    }

    fn edge_count(&self) -> usize {
        self.synapse.edge_count()
    }

    fn all_edges(&self) -> Vec<(EdgeKind, NodeId, NodeId)> {
        self.synapse.all_edges().collect()
    }

    // ── metrics ────────────────────────────────────────────────────────────

    fn heap_size_estimate(&self) -> usize {
        let nodes = self.trunk.len();
        let edges = self.synapse.edge_count();
        nodes * 256 + edges * 24
    }

    // ── persistence ────────────────────────────────────────────────────────

    fn flush(&mut self) -> Result<(), StorageError> {
        Ok(())
    }
}
