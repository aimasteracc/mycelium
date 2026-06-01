//! RED-first TDD tests for `RedbBackend`.
//!
//! All tests behind `#[cfg(feature = "redb-backend")]`.
//! Must FAIL before the implementation exists, then pass GREEN after P1-T09.
//!
//! Two test categories:
//!  1. Schema/open: opening a fresh DB creates the 8 tables; schema version guard.
//!  2. Parity with `InMemoryBackend`: same CRUD ops yield identical observations.
//!
//! Charter §5.1 / Dev≠QA rule: tests authored before implementation.

#![cfg(feature = "redb-backend")]

use mycelium_core::store::backend::{StorageBackend, StorageError};
use mycelium_core::store::in_memory::InMemoryBackend;
use mycelium_core::store::redb_backend::RedbBackend;
use mycelium_core::types::{EdgeKind, NodeKind, SourceSpan};
use redb::{Database, TableDefinition};

// ── helpers ──────────────────────────────────────────────────────────────────

fn fresh_redb() -> RedbBackend {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("test.redb");
    // `dir` must outlive the backend so the file is not deleted while open.
    // We accept the leak here because tests are short-lived.
    std::mem::forget(dir);
    RedbBackend::open(&path).expect("open fresh redb")
}

fn fresh_mem() -> InMemoryBackend {
    InMemoryBackend::new()
}

// ── open / schema ─────────────────────────────────────────────────────────────

#[test]
fn redb_opens_fresh_db_without_error() {
    let _b = fresh_redb();
}

#[test]
fn redb_is_object_safe() {
    let b: Box<dyn StorageBackend> = Box::new(fresh_redb());
    drop(b);
}

#[test]
fn redb_open_existing_rejects_newer_schema_version() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("future.redb");
    let db = Database::create(&path).expect("create future db");
    let txn = db.begin_write().expect("begin write");
    {
        let mut meta = txn
            .open_table(TableDefinition::<&str, u64>::new("meta"))
            .expect("open meta");
        meta.insert("schema_version", 999)
            .expect("insert future schema");
    }
    txn.commit().expect("commit future db");
    drop(db);

    let Err(err) = RedbBackend::open_existing(&path) else {
        panic!("future schema must be rejected");
    };
    match err {
        StorageError::SchemaVersion { file, supported } => {
            assert_eq!(file, 999);
            assert_eq!(supported, 2);
        }
        other => panic!("expected schema version error, got {other:?}"),
    }
}

// ── node parity ───────────────────────────────────────────────────────────────

#[test]
fn redb_upsert_and_lookup_parity() {
    let mut r = fresh_redb();
    let mut m = fresh_mem();

    let path = "src/lib.rs>main";
    let rid = r.upsert_node(path);
    let mid = m.upsert_node(path);

    assert_eq!(rid, mid, "NodeId must match InMemory (content hash)");
    assert_eq!(r.lookup_path(path), m.lookup_path(path));
    assert_eq!(r.path_of(rid), m.path_of(mid));
}

#[test]
fn redb_upsert_idempotent() {
    let mut r = fresh_redb();
    let id1 = r.upsert_node("src/a.rs>fn_a");
    let id2 = r.upsert_node("src/a.rs>fn_a");
    assert_eq!(id1, id2);
}

#[test]
fn redb_node_count_parity() {
    let mut r = fresh_redb();
    let mut m = fresh_mem();

    assert_eq!(r.node_count(), 0);
    for p in &["src/a.rs>x", "src/b.rs>y", "src/c.rs>z"] {
        r.upsert_node(p);
        m.upsert_node(p);
    }
    assert_eq!(r.node_count(), m.node_count());
}

#[test]
fn redb_remove_node_parity() {
    let mut r = fresh_redb();
    let mut m = fresh_mem();

    let path = "src/lib.rs>target";
    let rid = r.upsert_node(path);
    let mid = m.upsert_node(path);
    r.remove_node(rid);
    m.remove_node(mid);

    assert_eq!(r.lookup_path(path), m.lookup_path(path));
    assert_eq!(r.path_of(rid), m.path_of(mid));
}

// ── kind / span parity ────────────────────────────────────────────────────────

#[test]
fn redb_kind_roundtrips_parity() {
    let mut r = fresh_redb();
    let mut m = fresh_mem();

    let path = "src/lib.rs>MyClass";
    let rid = r.upsert_node(path);
    let mid = m.upsert_node(path);

    r.set_kind(rid, NodeKind::Class);
    m.set_kind(mid, NodeKind::Class);
    assert_eq!(r.kind_of(rid), m.kind_of(mid));
}

#[test]
fn redb_span_roundtrips_parity() {
    let mut r = fresh_redb();
    let mut m = fresh_mem();

    let path = "src/lib.rs>fn_a";
    let rid = r.upsert_node(path);
    let mid = m.upsert_node(path);

    let span = SourceSpan {
        start_line: 10,
        start_col: 0,
        end_line: 20,
        end_col: 1,
        start_byte: 200,
        end_byte: 400,
    };
    r.set_span(rid, span);
    m.set_span(mid, span);
    assert_eq!(r.span_of(rid), m.span_of(mid));
}

// ── edge parity ───────────────────────────────────────────────────────────────

#[test]
fn redb_edge_upsert_parity() {
    let mut r = fresh_redb();
    let mut m = fresh_mem();

    let ra = r.upsert_node("src/a.rs>A");
    let rb = r.upsert_node("src/b.rs>B");
    let ma = m.upsert_node("src/a.rs>A");
    let mb = m.upsert_node("src/b.rs>B");

    r.upsert_edge(EdgeKind::Calls, ra, rb);
    m.upsert_edge(EdgeKind::Calls, ma, mb);

    assert_eq!(
        r.outgoing(ra, EdgeKind::Calls),
        m.outgoing(ma, EdgeKind::Calls)
    );
    assert_eq!(
        r.incoming(rb, EdgeKind::Calls),
        m.incoming(mb, EdgeKind::Calls)
    );
}

#[test]
fn redb_edge_count_parity() {
    let mut r = fresh_redb();
    let mut m = fresh_mem();

    let ra = r.upsert_node("x>a");
    let rb = r.upsert_node("x>b");
    let rc = r.upsert_node("x>c");
    let ma = m.upsert_node("x>a");
    let mb = m.upsert_node("x>b");
    let mc = m.upsert_node("x>c");

    r.upsert_edge(EdgeKind::Calls, ra, rb);
    r.upsert_edge(EdgeKind::Imports, ra, rc);
    m.upsert_edge(EdgeKind::Calls, ma, mb);
    m.upsert_edge(EdgeKind::Imports, ma, mc);

    assert_eq!(r.edge_count(), m.edge_count());
}

// ── flush ─────────────────────────────────────────────────────────────────────

#[test]
fn redb_flush_is_ok() {
    let mut r = fresh_redb();
    r.upsert_node("src/lib.rs>main");
    assert!(r.flush().is_ok());
}

// ── mmap footprint estimate ──────────────────────────────────────────────────

#[test]
fn redb_heap_estimate_includes_empty_database_pages() {
    let r = fresh_redb();

    assert!(
        r.heap_size_estimate() >= 4096,
        "fresh redb schema must report real allocated mmap/storage pages, not a zero node formula"
    );
}

#[test]
fn redb_heap_estimate_is_not_inmemory_node_formula() {
    let mut r = fresh_redb();
    r.upsert_node("src/lib.rs>main");

    assert!(
        r.heap_size_estimate() > 256,
        "redb estimate must come from redb page stats, not nodes*256 + edges*24"
    );
}

// ── persistence across open/close ────────────────────────────────────────────

#[test]
fn redb_persists_across_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("persist.redb");

    let node_path = "src/lib.rs>persisted_fn";
    let id = {
        let mut r = RedbBackend::open(&path).expect("open for write");
        let id = r.upsert_node(node_path);
        r.flush().expect("flush");
        id
    };

    {
        let r = RedbBackend::open(&path).expect("reopen");
        assert_eq!(
            r.lookup_path(node_path),
            Some(id),
            "node must survive reopen"
        );
        assert_eq!(r.path_of(id), Some(node_path.to_owned()));
    }
}
