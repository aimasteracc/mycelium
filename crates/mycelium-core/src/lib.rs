//! # mycelium-core
//!
//! The reactive code intelligence graph engine.
//!
//! See [RFC-0001](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0001-trunk-and-synapse.md)
//! for the storage layer design, and the
//! [project Charter](https://github.com/aimasteracc/mycelium/blob/develop/CHARTER.md)
//! for the performance SLAs that this crate must meet.
//!
//! ## Layered architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │ Cortex  (reactive layer — RFC-0003)                     │
//! ├─────────────────────────────────────────────────────────┤
//! │ Store   (unified read/write surface)                    │
//! ├──────────────────────────┬──────────────────────────────┤
//! │ Trunk                    │ Synapse                      │
//! │ (containment tree —      │ (cross-cutting edges —       │
//! │ radix trie, path-encoded │ CSR per edge kind, forward + │
//! │ node IDs)                │ reverse adjacency)           │
//! └──────────────────────────┴──────────────────────────────┘
//! ```
//!
//! ## Current implementation status
//!
//! | Component | Status |
//! |---|---|
//! | [`types`] (NodeId, NodeKind, EdgeKind, Language) | minimal, ready |
//! | [`trunk`] (containment store) | **v0.1 spike**: HashMap-backed (correct semantics, sub-optimal layout). Radix trie optimization tracked under RFC-0001 Open Question #2. |
//! | [`synapse`] (edges) | scaffolded, implementation pending |
//! | [`store`] (unified API) | **v0.1**: in-memory Trunk + Synapse wiring. Persistence deferred to P4. |
//! | [`extractor`] (tree-sitter → Store bridge) | **RFC-0002**: parses source files with tree-sitter queries, populates Store. |
//! | [`cortex`] (Salsa reactive layer) | **RFC-0003 Phase 1**: `InputFile` input + `index_file` tracked query wired via `Cortex` db. |

#![doc(html_root_url = "https://docs.rs/mycelium-core")]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod budget;
pub mod context;
pub mod cortex;
pub mod error;
pub mod extractor;
pub mod queries;
pub mod store;
pub mod synapse;
pub mod trunk;
pub mod types;
pub mod verdict;
pub mod watch;

pub use error::{Error, Result};
pub use store::{
    CalleeNode, CallerNode, ClosenessCentralityEntry, CrossRefs, DegreeCentralityEntry,
    DegreeHistogram, EdgeKindMetrics, ExtendsNode, GraphStats, ImplementorNode, ImplementsNode,
    ImportNode, ImporterNode, NodeDegree, OutgoingRefs, SccEntry, Store, SubclassNode,
    SymbolNeighborhood, TopologicalOrder,
};
pub use types::{EdgeKind, Language, NodeId, NodeKind, SourceSpan};
