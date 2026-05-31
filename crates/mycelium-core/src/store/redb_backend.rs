//! `RedbBackend` — `StorageBackend` implementation backed by a redb ACID B-tree.
//!
//! Only compiled when the `redb-backend` cargo feature is enabled.
//!
//! ## Schema (ADR-0007 §8-table layout — APPEND-ONLY)
//!
//! | Table | Key | Value | Purpose |
//! |---|---|---|---|
//! | `trunk_by_id` | `u64` (`NodeId`) | `&str` (path) | id → path lookup |
//! | `trunk_by_path` | `&[u8]` (NUL-encoded path) | `u64` | path → id lookup |
//! | `kind_map` | `u64` (`NodeId`) | `u8` (`NodeKind` tag) | per-node kind |
//! | `span_map` | `u64` (`NodeId`) | `&[u8]` (24-byte span) | per-node span |
//! | `synapse_fwd` | `&[u8]` (kind `u16` ++ src `u64`) | `&[u8]` (packed dst list) | forward adjacency |
//! | `synapse_rev` | `&[u8]` (kind `u16` ++ dst `u64`) | `&[u8]` (packed src list) | reverse adjacency |
//! | `meta` | `&str` | `u64` | schema version, stats |
//!
//! RFC-0100 / P1-T09. Key encoding from P1-T05 (`redb_keys.rs`).

use std::path::Path;

use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition};

use crate::store::backend::{StorageBackend, StorageError as BackendError};
use crate::store::redb_keys::{decode_adj_key, encode_adj_key, encode_path_key};
use crate::store::redb_tags::{edge_kind_tag, tag_to_edge_kind};
use crate::types::{EdgeKind, NodeId, NodeKind, SourceSpan};

// ── table definitions ────────────────────────────────────────────────────────

#[allow(elided_lifetimes_in_paths)]
const TRUNK_BY_ID: TableDefinition<'_, u64, &str> = TableDefinition::new("trunk_by_id");
#[allow(elided_lifetimes_in_paths)]
const TRUNK_BY_PATH: TableDefinition<'_, &[u8], u64> = TableDefinition::new("trunk_by_path");
#[allow(elided_lifetimes_in_paths)]
const KIND_MAP: TableDefinition<'_, u64, u8> = TableDefinition::new("kind_map");
#[allow(elided_lifetimes_in_paths)]
const SPAN_MAP: TableDefinition<'_, u64, &[u8]> = TableDefinition::new("span_map");
#[allow(elided_lifetimes_in_paths)]
const SYNAPSE_FWD: TableDefinition<'_, &[u8], &[u8]> = TableDefinition::new("synapse_fwd");
#[allow(elided_lifetimes_in_paths)]
const SYNAPSE_REV: TableDefinition<'_, &[u8], &[u8]> = TableDefinition::new("synapse_rev");
#[allow(elided_lifetimes_in_paths)]
const META: TableDefinition<'_, &str, u64> = TableDefinition::new("meta");

/// Current schema version written into the `meta` table on first open.
const SCHEMA_VERSION: u64 = 1;

// ── helper: encode/decode span ───────────────────────────────────────────────

fn encode_span(span: SourceSpan) -> [u8; 24] {
    let mut out = [0u8; 24];
    out[0..4].copy_from_slice(&span.start_line.to_le_bytes());
    out[4..8].copy_from_slice(&span.start_col.to_le_bytes());
    out[8..12].copy_from_slice(&span.end_line.to_le_bytes());
    out[12..16].copy_from_slice(&span.end_col.to_le_bytes());
    out[16..20].copy_from_slice(&span.start_byte.to_le_bytes());
    out[20..24].copy_from_slice(&span.end_byte.to_le_bytes());
    out
}

fn decode_span(bytes: &[u8]) -> SourceSpan {
    if bytes.len() < 24 {
        return SourceSpan::default();
    }
    SourceSpan {
        start_line: u32::from_le_bytes(bytes[0..4].try_into().unwrap_or([0; 4])),
        start_col: u32::from_le_bytes(bytes[4..8].try_into().unwrap_or([0; 4])),
        end_line: u32::from_le_bytes(bytes[8..12].try_into().unwrap_or([0; 4])),
        end_col: u32::from_le_bytes(bytes[12..16].try_into().unwrap_or([0; 4])),
        start_byte: u32::from_le_bytes(bytes[16..20].try_into().unwrap_or([0; 4])),
        end_byte: u32::from_le_bytes(bytes[20..24].try_into().unwrap_or([0; 4])),
    }
}

// ── helper: encode/decode NodeKind ───────────────────────────────────────────

#[must_use]
const fn node_kind_tag(kind: NodeKind) -> u8 {
    match kind {
        NodeKind::File => 0,
        NodeKind::Module => 1,
        NodeKind::Class => 2,
        NodeKind::Struct => 3,
        NodeKind::Interface => 4,
        NodeKind::Function => 5,
        NodeKind::Method => 6,
        NodeKind::Property => 7,
        NodeKind::Field => 8,
        NodeKind::Variable => 9,
        NodeKind::Constant => 10,
        NodeKind::Enum => 11,
        NodeKind::EnumMember => 12,
        NodeKind::TypeAlias => 13,
        NodeKind::Parameter => 14,
        NodeKind::Import => 15,
        NodeKind::Export => 16,
        NodeKind::Route => 17,
        NodeKind::Component => 18,
        #[allow(unreachable_patterns)]
        _ => 255,
    }
}

#[must_use]
const fn tag_to_node_kind(tag: u8) -> Option<NodeKind> {
    Some(match tag {
        0 => NodeKind::File,
        1 => NodeKind::Module,
        2 => NodeKind::Class,
        3 => NodeKind::Struct,
        4 => NodeKind::Interface,
        5 => NodeKind::Function,
        6 => NodeKind::Method,
        7 => NodeKind::Property,
        8 => NodeKind::Field,
        9 => NodeKind::Variable,
        10 => NodeKind::Constant,
        11 => NodeKind::Enum,
        12 => NodeKind::EnumMember,
        13 => NodeKind::TypeAlias,
        14 => NodeKind::Parameter,
        15 => NodeKind::Import,
        16 => NodeKind::Export,
        17 => NodeKind::Route,
        18 => NodeKind::Component,
        _ => return None,
    })
}

// ── helper: pack/unpack adjacency lists ──────────────────────────────────────

fn pack_ids(ids: &[u64]) -> Vec<u8> {
    ids.iter().flat_map(|id| id.to_be_bytes()).collect()
}

fn unpack_ids(bytes: &[u8]) -> Vec<u64> {
    bytes
        .chunks_exact(8)
        .map(|c| u64::from_be_bytes(c.try_into().unwrap_or([0; 8])))
        .collect()
}

// ── error bridge ─────────────────────────────────────────────────────────────

fn db_err(e: impl std::fmt::Display) -> BackendError {
    BackendError::Backend(e.to_string())
}

// ── backend struct ────────────────────────────────────────────────────────────

/// Storage backend backed by a redb ACID B-tree database.
///
/// Writes auto-commit after each operation in Phase 1.
/// Reads use short-lived read transactions that close immediately.
pub struct RedbBackend {
    db: Database,
}

impl RedbBackend {
    /// Open or create a redb database at `path`.
    ///
    /// On first open, creates all 7 tables and writes `SCHEMA_VERSION` to meta.
    /// On subsequent opens, verifies that the stored schema version matches
    /// `SCHEMA_VERSION`.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened, the schema is
    /// incompatible, or table creation fails.
    pub fn open(path: &Path) -> Result<Self, BackendError> {
        let db = if path.exists() {
            Database::open(path).map_err(db_err)?
        } else {
            Database::create(path).map_err(db_err)?
        };
        let this = Self { db };
        this.init_schema()?;
        Ok(this)
    }

    fn init_schema(&self) -> Result<(), BackendError> {
        let txn = self.db.begin_write().map_err(db_err)?;
        {
            txn.open_table(TRUNK_BY_ID).map_err(db_err)?;
            txn.open_table(TRUNK_BY_PATH).map_err(db_err)?;
            txn.open_table(KIND_MAP).map_err(db_err)?;
            txn.open_table(SPAN_MAP).map_err(db_err)?;
            txn.open_table(SYNAPSE_FWD).map_err(db_err)?;
            txn.open_table(SYNAPSE_REV).map_err(db_err)?;
            let mut meta = txn.open_table(META).map_err(db_err)?;
            if meta.get("schema_version").map_err(db_err)?.is_none() {
                meta.insert("schema_version", SCHEMA_VERSION)
                    .map_err(db_err)?;
            } else {
                let stored = meta
                    .get("schema_version")
                    .map_err(db_err)?
                    .map_or(0, |g| g.value());
                if stored != SCHEMA_VERSION {
                    return Err(BackendError::SchemaVersion {
                        file: u32::try_from(stored).unwrap_or(u32::MAX),
                        supported: u32::try_from(SCHEMA_VERSION).unwrap_or(u32::MAX),
                    });
                }
            }
        }
        txn.commit().map_err(db_err)?;
        Ok(())
    }

    // ── fwd adjacency helpers ─────────────────────────────────────────────────

    fn read_fwd(&self, kind: EdgeKind, src: u64) -> Vec<u64> {
        let key = encode_adj_key(edge_kind_tag(kind), src);
        let Ok(txn) = self.db.begin_read() else {
            return Vec::new();
        };
        let Ok(table) = txn.open_table(SYNAPSE_FWD) else {
            return Vec::new();
        };
        table
            .get(key.as_slice())
            .ok()
            .flatten()
            .map_or_else(Vec::new, |g| unpack_ids(g.value()))
    }

    fn write_fwd(&self, kind: EdgeKind, src: u64, dsts: &[u64]) -> Result<(), BackendError> {
        let key = encode_adj_key(edge_kind_tag(kind), src);
        let txn = self.db.begin_write().map_err(db_err)?;
        {
            let mut table = txn.open_table(SYNAPSE_FWD).map_err(db_err)?;
            if dsts.is_empty() {
                table.remove(key.as_slice()).map_err(db_err)?;
            } else {
                let packed = pack_ids(dsts);
                table
                    .insert(key.as_slice(), packed.as_slice())
                    .map_err(db_err)?;
            }
        }
        txn.commit().map_err(db_err)?;
        Ok(())
    }

    fn read_rev(&self, kind: EdgeKind, dst: u64) -> Vec<u64> {
        let key = encode_adj_key(edge_kind_tag(kind), dst);
        let Ok(txn) = self.db.begin_read() else {
            return Vec::new();
        };
        let Ok(table) = txn.open_table(SYNAPSE_REV) else {
            return Vec::new();
        };
        table
            .get(key.as_slice())
            .ok()
            .flatten()
            .map_or_else(Vec::new, |g| unpack_ids(g.value()))
    }

    fn write_rev(&self, kind: EdgeKind, dst: u64, srcs: &[u64]) -> Result<(), BackendError> {
        let key = encode_adj_key(edge_kind_tag(kind), dst);
        let txn = self.db.begin_write().map_err(db_err)?;
        {
            let mut table = txn.open_table(SYNAPSE_REV).map_err(db_err)?;
            if srcs.is_empty() {
                table.remove(key.as_slice()).map_err(db_err)?;
            } else {
                let packed = pack_ids(srcs);
                table
                    .insert(key.as_slice(), packed.as_slice())
                    .map_err(db_err)?;
            }
        }
        txn.commit().map_err(db_err)?;
        Ok(())
    }
}

// ── all edge kinds we must scan when removing a node ─────────────────────────

const ALL_EDGE_KINDS: &[EdgeKind] = &[
    EdgeKind::Contains,
    EdgeKind::Calls,
    EdgeKind::Imports,
    EdgeKind::TypeImports,
    EdgeKind::Exports,
    EdgeKind::Extends,
    EdgeKind::Implements,
    EdgeKind::References,
    EdgeKind::TypeOf,
    EdgeKind::Returns,
    EdgeKind::Instantiates,
    EdgeKind::Overrides,
    EdgeKind::Decorates,
    EdgeKind::Aggregates,
    EdgeKind::Composes,
    EdgeKind::Uses,
];

// ── StorageBackend impl ───────────────────────────────────────────────────────

impl StorageBackend for RedbBackend {
    fn upsert_node(&mut self, path: &str) -> NodeId {
        if crate::trunk::TrunkPath::parse(path).is_err() {
            return NodeId::NULL;
        }
        let id = crate::trunk::path_to_node_id(path);
        let path_key = encode_path_key(path);
        let result: Result<(), BackendError> = (|| {
            let txn = self.db.begin_write().map_err(db_err)?;
            {
                let mut by_id = txn.open_table(TRUNK_BY_ID).map_err(db_err)?;
                let mut by_path = txn.open_table(TRUNK_BY_PATH).map_err(db_err)?;
                by_id.insert(id.0, path).map_err(db_err)?;
                by_path.insert(path_key.as_slice(), id.0).map_err(db_err)?;
            }
            txn.commit().map_err(db_err)?;
            Ok(())
        })();
        if result.is_err() {
            return NodeId::NULL;
        }
        id
    }

    fn remove_node(&mut self, id: NodeId) {
        self.remove_node_edges(id);
        let path = self.path_of(id);
        let _ = (|| -> Result<(), BackendError> {
            let txn = self.db.begin_write().map_err(db_err)?;
            {
                let mut by_id = txn.open_table(TRUNK_BY_ID).map_err(db_err)?;
                let mut by_path = txn.open_table(TRUNK_BY_PATH).map_err(db_err)?;
                let mut kind_tbl = txn.open_table(KIND_MAP).map_err(db_err)?;
                let mut span_tbl = txn.open_table(SPAN_MAP).map_err(db_err)?;
                by_id.remove(id.0).map_err(db_err)?;
                if let Some(p) = &path {
                    let pk = encode_path_key(p);
                    by_path.remove(pk.as_slice()).map_err(db_err)?;
                }
                kind_tbl.remove(id.0).map_err(db_err)?;
                span_tbl.remove(id.0).map_err(db_err)?;
            }
            txn.commit().map_err(db_err)?;
            Ok(())
        })();
    }

    fn path_of(&self, id: NodeId) -> Option<String> {
        let txn = self.db.begin_read().ok()?;
        let table = txn.open_table(TRUNK_BY_ID).ok()?;
        table.get(id.0).ok()?.map(|g| g.value().to_owned())
    }

    fn lookup_path(&self, path: &str) -> Option<NodeId> {
        let path_key = encode_path_key(path);
        let txn = self.db.begin_read().ok()?;
        let table = txn.open_table(TRUNK_BY_PATH).ok()?;
        table
            .get(path_key.as_slice())
            .ok()?
            .map(|g| NodeId(g.value()))
    }

    fn node_count(&self) -> usize {
        let Ok(txn) = self.db.begin_read() else {
            return 0;
        };
        let Ok(table) = txn.open_table(TRUNK_BY_ID) else {
            return 0;
        };
        usize::try_from(table.len().unwrap_or(0)).unwrap_or(usize::MAX)
    }

    fn all_paths(&self) -> Vec<String> {
        let Ok(txn) = self.db.begin_read() else {
            return Vec::new();
        };
        let Ok(table) = txn.open_table(TRUNK_BY_ID) else {
            return Vec::new();
        };
        let Ok(iter) = table.iter() else {
            return Vec::new();
        };
        iter.filter_map(Result::ok)
            .map(|(_, v)| v.value().to_owned())
            .collect()
    }

    fn set_kind(&mut self, id: NodeId, kind: NodeKind) {
        let _ = (|| -> Result<(), BackendError> {
            let txn = self.db.begin_write().map_err(db_err)?;
            {
                let mut table = txn.open_table(KIND_MAP).map_err(db_err)?;
                table.insert(id.0, node_kind_tag(kind)).map_err(db_err)?;
            }
            txn.commit().map_err(db_err)?;
            Ok(())
        })();
    }

    fn kind_of(&self, id: NodeId) -> Option<NodeKind> {
        let txn = self.db.begin_read().ok()?;
        let table = txn.open_table(KIND_MAP).ok()?;
        let tag = table.get(id.0).ok()??.value();
        tag_to_node_kind(tag)
    }

    fn set_span(&mut self, id: NodeId, span: SourceSpan) {
        let encoded = encode_span(span);
        let _ = (|| -> Result<(), BackendError> {
            let txn = self.db.begin_write().map_err(db_err)?;
            {
                let mut table = txn.open_table(SPAN_MAP).map_err(db_err)?;
                table.insert(id.0, encoded.as_slice()).map_err(db_err)?;
            }
            txn.commit().map_err(db_err)?;
            Ok(())
        })();
    }

    fn span_of(&self, id: NodeId) -> Option<SourceSpan> {
        let txn = self.db.begin_read().ok()?;
        let table = txn.open_table(SPAN_MAP).ok()?;
        let guard = table.get(id.0).ok()??;
        Some(decode_span(guard.value()))
    }

    fn upsert_edge(&mut self, kind: EdgeKind, src: NodeId, dst: NodeId) {
        let mut fwd = self.read_fwd(kind, src.0);
        if !fwd.contains(&dst.0) {
            fwd.push(dst.0);
            let _ = self.write_fwd(kind, src.0, &fwd);
        }
        let mut rev = self.read_rev(kind, dst.0);
        if !rev.contains(&src.0) {
            rev.push(src.0);
            let _ = self.write_rev(kind, dst.0, &rev);
        }
    }

    fn remove_node_edges(&mut self, id: NodeId) {
        for &kind in ALL_EDGE_KINDS {
            let dsts = self.read_fwd(kind, id.0);
            if !dsts.is_empty() {
                let _ = self.write_fwd(kind, id.0, &[]);
                for dst in dsts {
                    let mut rev = self.read_rev(kind, dst);
                    rev.retain(|&s| s != id.0);
                    let _ = self.write_rev(kind, dst, &rev);
                }
            }
            let srcs = self.read_rev(kind, id.0);
            if !srcs.is_empty() {
                let _ = self.write_rev(kind, id.0, &[]);
                for src in srcs {
                    let mut fwd = self.read_fwd(kind, src);
                    fwd.retain(|&d| d != id.0);
                    let _ = self.write_fwd(kind, src, &fwd);
                }
            }
        }
    }

    fn outgoing(&self, src: NodeId, kind: EdgeKind) -> Vec<NodeId> {
        let mut ids: Vec<NodeId> = self.read_fwd(kind, src.0).into_iter().map(NodeId).collect();
        ids.sort_unstable();
        ids
    }

    fn incoming(&self, dst: NodeId, kind: EdgeKind) -> Vec<NodeId> {
        let mut ids: Vec<NodeId> = self.read_rev(kind, dst.0).into_iter().map(NodeId).collect();
        ids.sort_unstable();
        ids
    }

    fn edge_count(&self) -> usize {
        let Ok(txn) = self.db.begin_read() else {
            return 0;
        };
        let Ok(table) = txn.open_table(SYNAPSE_FWD) else {
            return 0;
        };
        let Ok(iter) = table.iter() else {
            return 0;
        };
        iter.filter_map(Result::ok)
            .map(|(_, v)| unpack_ids(v.value()).len())
            .sum()
    }

    fn all_edges(&self) -> Vec<(EdgeKind, NodeId, NodeId)> {
        let Ok(txn) = self.db.begin_read() else {
            return Vec::new();
        };
        let Ok(table) = txn.open_table(SYNAPSE_FWD) else {
            return Vec::new();
        };
        let Ok(iter) = table.iter() else {
            return Vec::new();
        };
        let mut result = Vec::new();
        for entry in iter.filter_map(Result::ok) {
            let (kind_tag, src) = decode_adj_key(entry.0.value());
            if let Some(kind) = tag_to_edge_kind(kind_tag) {
                for dst in unpack_ids(entry.1.value()) {
                    result.push((kind, NodeId(src), NodeId(dst)));
                }
            }
        }
        result
    }

    fn heap_size_estimate(&self) -> usize {
        let nodes = self.node_count();
        let edges = self.edge_count();
        nodes * 256 + edges * 24
    }

    fn flush(&mut self) -> Result<(), BackendError> {
        Ok(())
    }
}
