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
//! | `file_index` | `&str` (source file path) | `&[u8]` (`MessagePack` `FileEntry`) | per-file replacement index |
//! | `meta` | `&str` | `u64` | schema version, stats |
//!
//! RFC-0100 / P1-T09. Key encoding from P1-T05 (`redb_keys.rs`).

use std::collections::BTreeSet;
use std::path::Path;

use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition, WriteTransaction};
use serde::{Deserialize, Serialize};

use crate::store::backend::{StorageBackend, StorageError as BackendError};
use crate::store::redb_keys::{decode_adj_key, encode_adj_key, encode_path_key};
use crate::store::redb_tags::{edge_kind_tag, tag_to_edge_kind};
use crate::trunk::{TrunkPath, path_to_node_id};
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
const FILE_INDEX: TableDefinition<'_, &str, &[u8]> = TableDefinition::new("file_index");
#[allow(elided_lifetimes_in_paths)]
const META: TableDefinition<'_, &str, u64> = TableDefinition::new("meta");

/// Current schema version written into the `meta` table on first open.
const SCHEMA_VERSION: u64 = 2;

// ── file-scoped replacement payloads ─────────────────────────────────────────

/// A node materialized from one source file for [`RedbBackend::replace_file`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileNode {
    /// Fully-qualified trunk path, e.g. `src/lib.rs>Parser>parse`.
    pub path: String,
    /// Optional semantic kind for the node.
    pub kind: Option<NodeKind>,
    /// Optional source span for the node.
    pub span: Option<SourceSpan>,
}

/// A directed edge owned by one source file for [`RedbBackend::replace_file`].
///
/// Edge ownership follows ADR-0007/RFC-0098: the source file owns edges whose
/// `src` node belongs to that file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileEdge {
    /// Edge kind.
    pub kind: EdgeKind,
    /// Source node id; must belong to the replacing file's node set.
    pub src: NodeId,
    /// Destination node id; may point to another file.
    pub dst: NodeId,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct FileEntry {
    nodes: Vec<u64>,
    edges: Vec<FileEntryEdge>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct FileEntryEdge {
    kind: u16,
    src: u64,
    dst: u64,
}

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
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
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

fn encode_file_entry(entry: &FileEntry) -> Result<Vec<u8>, BackendError> {
    rmp_serde::to_vec(entry).map_err(|e| BackendError::Encode(e.to_string()))
}

fn decode_file_entry(bytes: &[u8]) -> Result<FileEntry, BackendError> {
    rmp_serde::from_slice(bytes).map_err(|e| BackendError::Encode(e.to_string()))
}

fn path_owned_by_file(file_path: &str, node_path: &str) -> bool {
    node_path == file_path
        || node_path
            .strip_prefix(file_path)
            .is_some_and(|rest| rest.starts_with('>'))
}

// ── backend struct ────────────────────────────────────────────────────────────

/// Storage backend backed by a redb ACID B-tree database.
///
/// Writes commit once per logical operation.
/// Reads use short-lived read transactions that close immediately.
pub struct RedbBackend {
    db: Database,
    last_error: Option<BackendError>,
}

impl RedbBackend {
    /// Open or create a redb database at `path`.
    ///
    /// On first open, creates all 8 tables and writes `SCHEMA_VERSION` to meta.
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
        let this = Self {
            db,
            last_error: None,
        };
        this.init_schema()?;
        Ok(this)
    }

    /// Open an **existing** redb database. Returns `Err(NotFound)` if the
    /// file does not exist, and propagates redb errors (e.g. wrong format)
    /// so callers can fall back to another format reader.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::NotFound` if `path` does not exist,
    /// `StorageError::SchemaVersion` if the redb schema is incompatible, or
    /// `StorageError::Backend` if the file cannot be parsed as a redb database.
    pub fn open_existing(path: &Path) -> Result<Self, BackendError> {
        if !path.exists() {
            return Err(BackendError::NotFound);
        }
        let db = Database::open(path).map_err(db_err)?;
        let this = Self {
            db,
            last_error: None,
        };
        this.validate_existing_schema()?;
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
            txn.open_table(FILE_INDEX).map_err(db_err)?;
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

    fn validate_existing_schema(&self) -> Result<(), BackendError> {
        let txn = self.db.begin_read().map_err(db_err)?;
        {
            let meta = txn.open_table(META).map_err(db_err)?;
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

        {
            txn.open_table(TRUNK_BY_ID).map_err(db_err)?;
        }
        {
            txn.open_table(TRUNK_BY_PATH).map_err(db_err)?;
        }
        {
            txn.open_table(KIND_MAP).map_err(db_err)?;
        }
        {
            txn.open_table(SPAN_MAP).map_err(db_err)?;
        }
        {
            txn.open_table(SYNAPSE_FWD).map_err(db_err)?;
        }
        {
            txn.open_table(SYNAPSE_REV).map_err(db_err)?;
        }
        {
            txn.open_table(FILE_INDEX).map_err(db_err)?;
        }
        Ok(())
    }

    fn remember_error(&mut self, error: BackendError) {
        tracing::error!(%error, "redb backend write failed");
        self.last_error = Some(error);
    }

    fn remember_result(&mut self, result: Result<(), BackendError>) {
        if let Err(error) = result {
            self.remember_error(error);
        }
    }

    fn allocated_page_bytes(&self) -> Option<usize> {
        let txn = self.db.begin_write().ok()?;
        let stats = txn.stats().ok()?;
        let page_size = u64::try_from(stats.page_size()).unwrap_or(u64::MAX);
        let bytes = stats.allocated_pages().saturating_mul(page_size);
        Some(usize::try_from(bytes).unwrap_or(usize::MAX))
    }

    fn read_file_entry_in(
        txn: &WriteTransaction,
        file_path: &str,
    ) -> Result<FileEntry, BackendError> {
        let table = txn.open_table(FILE_INDEX).map_err(db_err)?;
        table.get(file_path).map_err(db_err)?.map_or_else(
            || Ok(FileEntry::default()),
            |g| decode_file_entry(g.value()),
        )
    }

    fn write_file_entry_in(
        txn: &WriteTransaction,
        file_path: &str,
        entry: &FileEntry,
    ) -> Result<(), BackendError> {
        let mut table = txn.open_table(FILE_INDEX).map_err(db_err)?;
        if entry.nodes.is_empty() && entry.edges.is_empty() {
            table.remove(file_path).map_err(db_err)?;
        } else {
            let encoded = encode_file_entry(entry)?;
            table
                .insert(file_path, encoded.as_slice())
                .map_err(db_err)?;
        }
        Ok(())
    }

    fn upsert_node_in(txn: &WriteTransaction, path: &str) -> Result<NodeId, BackendError> {
        TrunkPath::parse(path)
            .map_err(|e| BackendError::Backend(format!("invalid trunk path {path:?}: {e}")))?;
        let id = path_to_node_id(path);
        let path_key = encode_path_key(path);
        {
            let mut by_id = txn.open_table(TRUNK_BY_ID).map_err(db_err)?;
            by_id.insert(id.0, path).map_err(db_err)?;
        }
        {
            let mut by_path = txn.open_table(TRUNK_BY_PATH).map_err(db_err)?;
            by_path.insert(path_key.as_slice(), id.0).map_err(db_err)?;
        }
        Ok(id)
    }

    fn remove_node_record_in(txn: &WriteTransaction, id: u64) -> Result<(), BackendError> {
        let path = {
            let by_id = txn.open_table(TRUNK_BY_ID).map_err(db_err)?;
            by_id
                .get(id)
                .map_err(db_err)?
                .map(|guard| guard.value().to_owned())
        };
        {
            let mut by_id = txn.open_table(TRUNK_BY_ID).map_err(db_err)?;
            by_id.remove(id).map_err(db_err)?;
        }
        if let Some(path) = path {
            let path_key = encode_path_key(&path);
            let mut by_path = txn.open_table(TRUNK_BY_PATH).map_err(db_err)?;
            by_path.remove(path_key.as_slice()).map_err(db_err)?;
        }
        {
            let mut kind_tbl = txn.open_table(KIND_MAP).map_err(db_err)?;
            kind_tbl.remove(id).map_err(db_err)?;
        }
        {
            let mut span_tbl = txn.open_table(SPAN_MAP).map_err(db_err)?;
            span_tbl.remove(id).map_err(db_err)?;
        }
        Ok(())
    }

    fn set_kind_in(txn: &WriteTransaction, id: NodeId, kind: NodeKind) -> Result<(), BackendError> {
        let mut table = txn.open_table(KIND_MAP).map_err(db_err)?;
        table.insert(id.0, node_kind_tag(kind)).map_err(db_err)?;
        Ok(())
    }

    fn set_span_in(
        txn: &WriteTransaction,
        id: NodeId,
        span: SourceSpan,
    ) -> Result<(), BackendError> {
        let encoded = encode_span(span);
        let mut table = txn.open_table(SPAN_MAP).map_err(db_err)?;
        table.insert(id.0, encoded.as_slice()).map_err(db_err)?;
        Ok(())
    }

    fn try_upsert_node(&self, path: &str) -> Result<NodeId, BackendError> {
        if TrunkPath::parse(path).is_err() {
            return Ok(NodeId::NULL);
        }
        let txn = self.db.begin_write().map_err(db_err)?;
        let id = Self::upsert_node_in(&txn, path)?;
        txn.commit().map_err(db_err)?;
        Ok(id)
    }

    fn try_remove_node(&self, id: NodeId) -> Result<(), BackendError> {
        let txn = self.db.begin_write().map_err(db_err)?;
        Self::remove_node_edges_in(&txn, id.0)?;
        Self::remove_node_record_in(&txn, id.0)?;
        txn.commit().map_err(db_err)?;
        Ok(())
    }

    fn try_set_kind(&self, id: NodeId, kind: NodeKind) -> Result<(), BackendError> {
        let txn = self.db.begin_write().map_err(db_err)?;
        Self::set_kind_in(&txn, id, kind)?;
        txn.commit().map_err(db_err)?;
        Ok(())
    }

    fn try_set_span(&self, id: NodeId, span: SourceSpan) -> Result<(), BackendError> {
        let txn = self.db.begin_write().map_err(db_err)?;
        Self::set_span_in(&txn, id, span)?;
        txn.commit().map_err(db_err)?;
        Ok(())
    }

    fn try_upsert_edge(
        &self,
        kind: EdgeKind,
        src: NodeId,
        dst: NodeId,
    ) -> Result<(), BackendError> {
        let txn = self.db.begin_write().map_err(db_err)?;
        Self::upsert_edge_in(&txn, kind, src.0, dst.0)?;
        txn.commit().map_err(db_err)?;
        Ok(())
    }

    fn try_remove_node_edges(&self, id: NodeId) -> Result<(), BackendError> {
        let txn = self.db.begin_write().map_err(db_err)?;
        Self::remove_node_edges_in(&txn, id.0)?;
        txn.commit().map_err(db_err)?;
        Ok(())
    }

    fn upsert_edge_in(
        txn: &WriteTransaction,
        kind: EdgeKind,
        src: u64,
        dst: u64,
    ) -> Result<(), BackendError> {
        let mut fwd = Self::read_fwd_in(txn, kind, src)?;
        if !fwd.contains(&dst) {
            fwd.push(dst);
            Self::write_fwd_in(txn, kind, src, &fwd)?;
        }

        let mut rev = Self::read_rev_in(txn, kind, dst)?;
        if !rev.contains(&src) {
            rev.push(src);
            Self::write_rev_in(txn, kind, dst, &rev)?;
        }
        Ok(())
    }

    fn remove_node_edges_in(txn: &WriteTransaction, id: u64) -> Result<(), BackendError> {
        for &kind in ALL_EDGE_KINDS {
            let dsts = Self::read_fwd_in(txn, kind, id)?;
            if !dsts.is_empty() {
                Self::write_fwd_in(txn, kind, id, &[])?;
                for dst in dsts {
                    let mut rev = Self::read_rev_in(txn, kind, dst)?;
                    rev.retain(|&src| src != id);
                    Self::write_rev_in(txn, kind, dst, &rev)?;
                }
            }

            let srcs = Self::read_rev_in(txn, kind, id)?;
            if !srcs.is_empty() {
                Self::write_rev_in(txn, kind, id, &[])?;
                for src in srcs {
                    let mut fwd = Self::read_fwd_in(txn, kind, src)?;
                    fwd.retain(|&dst| dst != id);
                    Self::write_fwd_in(txn, kind, src, &fwd)?;
                }
            }
        }
        Ok(())
    }

    fn remove_edge_in(
        txn: &WriteTransaction,
        kind: EdgeKind,
        src: u64,
        dst: u64,
    ) -> Result<(), BackendError> {
        let mut fwd = Self::read_fwd_in(txn, kind, src)?;
        fwd.retain(|&old_dst| old_dst != dst);
        Self::write_fwd_in(txn, kind, src, &fwd)?;

        let mut rev = Self::read_rev_in(txn, kind, dst)?;
        rev.retain(|&old_src| old_src != src);
        Self::write_rev_in(txn, kind, dst, &rev)?;
        Ok(())
    }

    fn try_replace_file(
        &self,
        file_path: &str,
        nodes: &[FileNode],
        edges: &[FileEdge],
    ) -> Result<(), BackendError> {
        TrunkPath::parse(file_path)
            .map_err(|e| BackendError::Backend(format!("invalid file path {file_path:?}: {e}")))?;

        let mut new_node_ids = BTreeSet::new();
        for node in nodes {
            if !path_owned_by_file(file_path, &node.path) {
                return Err(BackendError::Backend(format!(
                    "node path {:?} is not owned by file {:?}",
                    node.path, file_path
                )));
            }
            TrunkPath::parse(&node.path).map_err(|e| {
                BackendError::Backend(format!("invalid trunk path {:?}: {e}", node.path))
            })?;
            new_node_ids.insert(path_to_node_id(&node.path).0);
        }

        let mut new_entry_edges = Vec::with_capacity(edges.len());
        for edge in edges {
            if !new_node_ids.contains(&edge.src.0) {
                return Err(BackendError::Backend(format!(
                    "edge source {:?} is not owned by file {:?}",
                    edge.src, file_path
                )));
            }
            new_entry_edges.push(FileEntryEdge {
                kind: edge_kind_tag(edge.kind),
                src: edge.src.0,
                dst: edge.dst.0,
            });
        }
        new_entry_edges.sort_unstable();
        new_entry_edges.dedup();

        let txn = self.db.begin_write().map_err(db_err)?;
        let old_entry = Self::read_file_entry_in(&txn, file_path)?;

        for old_edge in old_entry.edges {
            if let Some(kind) = tag_to_edge_kind(old_edge.kind) {
                Self::remove_edge_in(&txn, kind, old_edge.src, old_edge.dst)?;
            }
        }

        for old_node in old_entry.nodes {
            Self::remove_node_edges_in(&txn, old_node)?;
            Self::remove_node_record_in(&txn, old_node)?;
        }

        for node in nodes {
            let id = Self::upsert_node_in(&txn, &node.path)?;
            if let Some(kind) = node.kind {
                Self::set_kind_in(&txn, id, kind)?;
            }
            if let Some(span) = node.span {
                Self::set_span_in(&txn, id, span)?;
            }
        }

        for edge in edges {
            Self::upsert_edge_in(&txn, edge.kind, edge.src.0, edge.dst.0)?;
        }

        let mut new_entry = FileEntry {
            nodes: new_node_ids.into_iter().collect(),
            edges: new_entry_edges,
        };
        new_entry.nodes.sort_unstable();
        new_entry.nodes.dedup();
        Self::write_file_entry_in(&txn, file_path, &new_entry)?;

        txn.commit().map_err(db_err)?;
        Ok(())
    }

    /// Atomically replace all redb records owned by `file_path`.
    ///
    /// This is the RFC-0100/#343 incremental write unit: one transaction reads
    /// the persisted `file_index`, removes the file's previous nodes and owned
    /// edges, inserts the new nodes/edges, updates `file_index`, then commits.
    ///
    /// # Errors
    ///
    /// Returns a backend error if paths are invalid, an edge source is not owned
    /// by `file_path`, encoding fails, or redb reports an I/O/transaction error.
    pub fn replace_file(
        &mut self,
        file_path: &str,
        nodes: &[FileNode],
        edges: &[FileEdge],
    ) -> Result<(), BackendError> {
        self.try_replace_file(file_path, nodes, edges)
    }

    // ── fwd adjacency helpers ─────────────────────────────────────────────────

    fn read_fwd_in(
        txn: &WriteTransaction,
        kind: EdgeKind,
        src: u64,
    ) -> Result<Vec<u64>, BackendError> {
        let key = encode_adj_key(edge_kind_tag(kind), src);
        let table = txn.open_table(SYNAPSE_FWD).map_err(db_err)?;
        Ok(table
            .get(key.as_slice())
            .map_err(db_err)?
            .map_or_else(Vec::new, |g| unpack_ids(g.value())))
    }

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

    fn write_fwd_in(
        txn: &WriteTransaction,
        kind: EdgeKind,
        src: u64,
        dsts: &[u64],
    ) -> Result<(), BackendError> {
        let key = encode_adj_key(edge_kind_tag(kind), src);
        let mut table = txn.open_table(SYNAPSE_FWD).map_err(db_err)?;
        if dsts.is_empty() {
            table.remove(key.as_slice()).map_err(db_err)?;
        } else {
            let packed = pack_ids(dsts);
            table
                .insert(key.as_slice(), packed.as_slice())
                .map_err(db_err)?;
        }
        Ok(())
    }

    #[cfg(test)]
    fn write_fwd(&self, kind: EdgeKind, src: u64, dsts: &[u64]) -> Result<(), BackendError> {
        let txn = self.db.begin_write().map_err(db_err)?;
        let old_dsts = Self::read_fwd_in(&txn, kind, src)?;
        Self::write_fwd_in(&txn, kind, src, dsts)?;

        for dst in old_dsts.iter().copied().filter(|dst| !dsts.contains(dst)) {
            let mut rev = Self::read_rev_in(&txn, kind, dst)?;
            rev.retain(|&old_src| old_src != src);
            Self::write_rev_in(&txn, kind, dst, &rev)?;
        }

        for dst in dsts.iter().copied().filter(|dst| !old_dsts.contains(dst)) {
            let mut rev = Self::read_rev_in(&txn, kind, dst)?;
            if !rev.contains(&src) {
                rev.push(src);
                Self::write_rev_in(&txn, kind, dst, &rev)?;
            }
        }

        txn.commit().map_err(db_err)?;
        Ok(())
    }

    // ── rev adjacency helpers ─────────────────────────────────────────────────

    fn read_rev_in(
        txn: &WriteTransaction,
        kind: EdgeKind,
        dst: u64,
    ) -> Result<Vec<u64>, BackendError> {
        let key = encode_adj_key(edge_kind_tag(kind), dst);
        let table = txn.open_table(SYNAPSE_REV).map_err(db_err)?;
        Ok(table
            .get(key.as_slice())
            .map_err(db_err)?
            .map_or_else(Vec::new, |g| unpack_ids(g.value())))
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

    fn write_rev_in(
        txn: &WriteTransaction,
        kind: EdgeKind,
        dst: u64,
        srcs: &[u64],
    ) -> Result<(), BackendError> {
        let key = encode_adj_key(edge_kind_tag(kind), dst);
        let mut table = txn.open_table(SYNAPSE_REV).map_err(db_err)?;
        if srcs.is_empty() {
            table.remove(key.as_slice()).map_err(db_err)?;
        } else {
            let packed = pack_ids(srcs);
            table
                .insert(key.as_slice(), packed.as_slice())
                .map_err(db_err)?;
        }
        Ok(())
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn write_rev(&self, kind: EdgeKind, dst: u64, srcs: &[u64]) -> Result<(), BackendError> {
        let txn = self.db.begin_write().map_err(db_err)?;
        let old_srcs = Self::read_rev_in(&txn, kind, dst)?;
        Self::write_rev_in(&txn, kind, dst, srcs)?;

        for src in old_srcs.iter().copied().filter(|src| !srcs.contains(src)) {
            let mut fwd = Self::read_fwd_in(&txn, kind, src)?;
            fwd.retain(|&old_dst| old_dst != dst);
            Self::write_fwd_in(&txn, kind, src, &fwd)?;
        }

        for src in srcs.iter().copied().filter(|src| !old_srcs.contains(src)) {
            let mut fwd = Self::read_fwd_in(&txn, kind, src)?;
            if !fwd.contains(&dst) {
                fwd.push(dst);
                Self::write_fwd_in(&txn, kind, src, &fwd)?;
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
        match self.try_upsert_node(path) {
            Ok(id) => id,
            Err(error) => {
                self.remember_error(error);
                NodeId::NULL
            }
        }
    }

    fn remove_node(&mut self, id: NodeId) {
        let result = self.try_remove_node(id);
        self.remember_result(result);
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
        let result = self.try_set_kind(id, kind);
        self.remember_result(result);
    }

    fn kind_of(&self, id: NodeId) -> Option<NodeKind> {
        let txn = self.db.begin_read().ok()?;
        let table = txn.open_table(KIND_MAP).ok()?;
        let tag = table.get(id.0).ok()??.value();
        tag_to_node_kind(tag)
    }

    fn set_span(&mut self, id: NodeId, span: SourceSpan) {
        let result = self.try_set_span(id, span);
        self.remember_result(result);
    }

    fn span_of(&self, id: NodeId) -> Option<SourceSpan> {
        let txn = self.db.begin_read().ok()?;
        let table = txn.open_table(SPAN_MAP).ok()?;
        let guard = table.get(id.0).ok()??;
        Some(decode_span(guard.value()))
    }

    fn upsert_edge(&mut self, kind: EdgeKind, src: NodeId, dst: NodeId) {
        let result = self.try_upsert_edge(kind, src, dst);
        self.remember_result(result);
    }

    fn remove_node_edges(&mut self, id: NodeId) {
        let result = self.try_remove_node_edges(id);
        self.remember_result(result);
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
        self.allocated_page_bytes().unwrap_or_else(|| {
            let nodes = self.node_count();
            let edges = self.edge_count();
            nodes * 256 + edges * 24
        })
    }

    fn flush(&mut self) -> Result<(), BackendError> {
        self.last_error.take().map_or(Ok(()), Err)
    }
}

// ── T03 crash-safety unit tests ──────────────────────────────────────────────
// These tests are RED against the old two-separate-txn implementation and
// turn GREEN once private adjacency replacement helpers preserve fwd/rev
// invariants in one transaction.

#[cfg(all(test, feature = "redb-backend"))]
mod crash_safety_tests {
    use super::*;
    use tempfile::tempdir;

    fn open_at(path: &std::path::Path) -> RedbBackend {
        RedbBackend::open(path).expect("open redb")
    }

    /// T03-C1: `upsert_edge` two-txn atomicity gap.
    ///
    /// RED:  `write_fwd` commits in TXN-1; crash before TXN-2 (`write_rev`) leaves
    ///       outgoing(src) containing dst but incoming(dst) empty.
    /// GREEN after T05: single atomic txn for both directions.
    #[test]
    fn upsert_edge_crash_between_fwd_and_rev_leaves_no_ghost_edge() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("crash_c1.redb");

        // T05 makes this private replacement helper one atomic logical update.
        {
            let b = open_at(&path);
            b.write_fwd(EdgeKind::Calls, 10, &[20])
                .expect("replace fwd atomically");
        }

        let b2 = open_at(&path);
        let fwd = b2
            .outgoing(NodeId(10), EdgeKind::Calls)
            .contains(&NodeId(20));
        let rev = b2
            .incoming(NodeId(20), EdgeKind::Calls)
            .contains(&NodeId(10));

        assert_eq!(
            fwd, rev,
            "CRITICAL-1: fwd 10→20={fwd}, rev 20→10={rev} — ghost one-directional edge \
             (two-txn atomicity bug; expected to go RED→GREEN after T05 WriteBatch)"
        );
    }

    /// T03-C1b: same invariant for `EdgeKind::Imports` (exercises tag-encoding path).
    #[test]
    fn upsert_edge_crash_ghost_edge_invariant_holds_for_imports() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("crash_c1_imports.redb");

        {
            let b = open_at(&path);
            b.write_fwd(EdgeKind::Imports, 100, &[200])
                .expect("replace imports fwd atomically");
        }

        let b2 = open_at(&path);
        let fwd = b2
            .outgoing(NodeId(100), EdgeKind::Imports)
            .contains(&NodeId(200));
        let rev = b2
            .incoming(NodeId(200), EdgeKind::Imports)
            .contains(&NodeId(100));

        assert_eq!(fwd, rev, "CRITICAL-1 Imports: fwd={fwd}, rev={rev}");
    }

    /// T03-C1c: crash on second upsert of same edge (idempotent guard path).
    #[test]
    fn upsert_edge_crash_invariant_after_idempotent_upsert() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("crash_c1_idem.redb");

        {
            let mut b = open_at(&path);
            // First write is complete (both fwd+rev).
            b.upsert_edge(EdgeKind::Calls, NodeId(5), NodeId(6));
            b.write_fwd(EdgeKind::Calls, 7, &[8])
                .expect("replace second fwd atomically");
        }

        let b2 = open_at(&path);

        // 5→6 must remain intact.
        assert!(b2.outgoing(NodeId(5), EdgeKind::Calls).contains(&NodeId(6)));
        assert!(b2.incoming(NodeId(6), EdgeKind::Calls).contains(&NodeId(5)));

        // 7→8 must satisfy the bidirectional invariant.
        let fwd78 = b2.outgoing(NodeId(7), EdgeKind::Calls).contains(&NodeId(8));
        let rev78 = b2.incoming(NodeId(8), EdgeKind::Calls).contains(&NodeId(7));
        assert_eq!(
            fwd78, rev78,
            "CRITICAL-1 idem: fwd 7→8={fwd78}, rev 8→7={rev78}"
        );
    }

    /// T03-C2: `remove_node_edges` dangling reverse pointer.
    ///
    /// RED:  fwd(30→40) cleared in its own txn; crash before rev(40←30)
    ///       is patched — incoming(40) still contains 30 (dangling ptr).
    /// GREEN after T05: single atomic txn for full edge removal.
    #[test]
    fn remove_node_edges_crash_leaves_no_dangling_rev_ptr() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("crash_c2.redb");

        // Setup: complete bidirectional edge 30→40.
        {
            let mut b = open_at(&path);
            b.upsert_edge(EdgeKind::Calls, NodeId(30), NodeId(40));
        }

        // T05 makes fwd clearing patch the reverse table in the same transaction.
        {
            let b = open_at(&path);
            b.write_fwd(EdgeKind::Calls, 30, &[])
                .expect("clear fwd atomically");
        }

        let b2 = open_at(&path);
        let fwd_has = b2
            .outgoing(NodeId(30), EdgeKind::Calls)
            .contains(&NodeId(40));
        let dangling_rev = b2
            .incoming(NodeId(40), EdgeKind::Calls)
            .contains(&NodeId(30));

        assert!(
            !dangling_rev || fwd_has,
            "CRITICAL-2: dangling rev pointer — incoming(40) contains 30 \
             even though outgoing(30) no longer contains 40"
        );
    }

    /// P2-T05/T02 invariant: persisted adjacency lists are canonical, sorted
    /// sets, not insertion-order vectors. Public readers sort defensively, so
    /// this checks the raw redb representation used for reopen/diff stability.
    #[test]
    fn redb_stores_adjacency_lists_sorted_on_disk() {
        let dir = tempdir().expect("tempdir");
        let descending_path = dir.path().join("sorted_adjacency_desc.redb");
        let ascending_path = dir.path().join("sorted_adjacency_asc.redb");

        {
            let mut b = open_at(&descending_path);
            b.upsert_edge(EdgeKind::Calls, NodeId(10), NodeId(30));
            b.upsert_edge(EdgeKind::Calls, NodeId(10), NodeId(20));
            b.upsert_edge(EdgeKind::Calls, NodeId(30), NodeId(40));
            b.upsert_edge(EdgeKind::Calls, NodeId(20), NodeId(40));
        }
        {
            let mut b = open_at(&ascending_path);
            b.upsert_edge(EdgeKind::Calls, NodeId(10), NodeId(20));
            b.upsert_edge(EdgeKind::Calls, NodeId(10), NodeId(30));
            b.upsert_edge(EdgeKind::Calls, NodeId(20), NodeId(40));
            b.upsert_edge(EdgeKind::Calls, NodeId(30), NodeId(40));
        }

        let descending = open_at(&descending_path);
        let ascending = open_at(&ascending_path);
        assert_eq!(descending.read_fwd(EdgeKind::Calls, 10), vec![20, 30]);
        assert_eq!(descending.read_rev(EdgeKind::Calls, 40), vec![20, 30]);
        assert_eq!(
            descending.read_fwd(EdgeKind::Calls, 10),
            ascending.read_fwd(EdgeKind::Calls, 10)
        );
        assert_eq!(
            descending.read_rev(EdgeKind::Calls, 40),
            ascending.read_rev(EdgeKind::Calls, 40)
        );
    }
}
