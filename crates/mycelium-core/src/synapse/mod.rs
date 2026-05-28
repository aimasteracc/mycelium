//! Synapse — cross-cutting edge storage.
//!
//! Each [`EdgeKind`](crate::types::EdgeKind) is stored independently with
//! forward and reverse adjacency materialized, so "who calls X?" and
//! "what does X call?" are both O(degree) lookups.
//!
//! ## Current status
//!
//! Scaffolded for RFC-0001 §3.2. The data structure shape is fixed; the
//! efficient CSR encoding lands as an optimization PR after the
//! HashMap-backed v0.1 spike validates the API.

#[cfg(test)]
mod tests;

use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

use crate::types::{EdgeKind, NodeId};

/// Per-kind directed adjacency.
///
/// `forward[u]` = list of nodes that `u` points to with this edge kind.
/// `reverse[v]` = list of nodes that point to `v` with this edge kind.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AdjacencyList {
    forward: HashMap<NodeId, Vec<NodeId>>,
    reverse: HashMap<NodeId, Vec<NodeId>>,
}

impl AdjacencyList {
    /// Create an empty adjacency list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a directed edge `src → dst`. Idempotent: adding the same edge
    /// twice does not duplicate it.
    pub fn add(&mut self, src: NodeId, dst: NodeId) {
        let fwd = self.forward.entry(src).or_default();
        if !fwd.contains(&dst) {
            fwd.push(dst);
        }
        let rev = self.reverse.entry(dst).or_default();
        if !rev.contains(&src) {
            rev.push(src);
        }
    }

    /// Targets of `src` under this edge kind.
    #[must_use]
    pub fn outgoing(&self, src: NodeId) -> &[NodeId] {
        self.forward.get(&src).map_or(&[], Vec::as_slice)
    }

    /// Sources that point to `dst` under this edge kind.
    #[must_use]
    pub fn incoming(&self, dst: NodeId) -> &[NodeId] {
        self.reverse.get(&dst).map_or(&[], Vec::as_slice)
    }

    /// Rewire all edges touching `from` so they touch `to` instead, then
    /// remove `from` from both adjacency maps.
    ///
    /// After this call every `src → from` becomes `src → to` and every
    /// `from → dst` becomes `to → dst`. Self-edges (`to → to`) are
    /// silently dropped to avoid degenerate cycles.
    pub fn redirect_node(&mut self, from: NodeId, to: NodeId) {
        // Redirect incoming: src → from  ⟹  src → to
        if let Some(sources) = self.reverse.remove(&from) {
            for &src in &sources {
                if let Some(dsts) = self.forward.get_mut(&src) {
                    dsts.retain(|&d| d != from);
                    if src != to && !dsts.contains(&to) {
                        dsts.push(to);
                    }
                }
            }
            let rev_to = self.reverse.entry(to).or_default();
            for src in sources {
                if src != to && !rev_to.contains(&src) {
                    rev_to.push(src);
                }
            }
        }
        // Redirect outgoing: from → dst  ⟹  to → dst
        if let Some(targets) = self.forward.remove(&from) {
            for &dst in &targets {
                if let Some(srcs) = self.reverse.get_mut(&dst) {
                    srcs.retain(|&s| s != from);
                    if dst != to && !srcs.contains(&to) {
                        srcs.push(to);
                    }
                }
            }
            let fwd_to = self.forward.entry(to).or_default();
            for dst in targets {
                if dst != to && !fwd_to.contains(&dst) {
                    fwd_to.push(dst);
                }
            }
        }
    }

    /// Remove all edges involving `id` (both as source and target).
    pub fn remove_node(&mut self, id: NodeId) {
        if let Some(targets) = self.forward.remove(&id) {
            for t in targets {
                if let Some(srcs) = self.reverse.get_mut(&t) {
                    srcs.retain(|&s| s != id);
                }
            }
        }
        if let Some(sources) = self.reverse.remove(&id) {
            for s in sources {
                if let Some(dsts) = self.forward.get_mut(&s) {
                    dsts.retain(|&d| d != id);
                }
            }
        }
    }

    /// Total number of directed edges stored.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.forward.values().map(Vec::len).sum()
    }
}

/// A multi-kind synapse store.
///
/// One [`AdjacencyList`] per [`EdgeKind`].
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Synapse {
    by_kind: HashMap<EdgeKind, AdjacencyList>,
}

impl Synapse {
    /// Create an empty synapse store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert an edge of `kind` from `src` to `dst`.
    pub fn add(&mut self, kind: EdgeKind, src: NodeId, dst: NodeId) {
        self.by_kind.entry(kind).or_default().add(src, dst);
    }

    /// Outgoing edges of `kind` from `src`.
    #[must_use]
    pub fn outgoing(&self, src: NodeId, kind: EdgeKind) -> &[NodeId] {
        self.by_kind.get(&kind).map_or(&[], |a| a.outgoing(src))
    }

    /// Incoming edges of `kind` to `dst`.
    #[must_use]
    pub fn incoming(&self, dst: NodeId, kind: EdgeKind) -> &[NodeId] {
        self.by_kind.get(&kind).map_or(&[], |a| a.incoming(dst))
    }

    /// Total number of directed edges across all edge kinds.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.by_kind.values().map(AdjacencyList::edge_count).sum()
    }

    /// Rewire all edges touching `from` to touch `to` instead, across all
    /// edge kinds. Delegates to [`AdjacencyList::redirect_node`].
    pub fn redirect_node(&mut self, from: NodeId, to: NodeId) {
        for adj in self.by_kind.values_mut() {
            adj.redirect_node(from, to);
        }
    }

    /// Remove all edges involving `id`, across all edge kinds.
    pub fn remove_node(&mut self, id: NodeId) {
        for adj in self.by_kind.values_mut() {
            adj.remove_node(id);
        }
    }
}
