//! RED-first TDD tests for `Store::load` format detection (P1-T10).
//!
//! Three dispatch paths:
//!   1. redb file → loaded via `RedbBackend` (feature-gated)
//!   2. legacy rmp file → loaded via `rmp_serde` (always available)
//!   3. garbage file → `anyhow::Error` returned, no panic
//!
//! Tests 1 must fail (compile error) before `Store::load` gains format detection.
//! Tests 2 and 3 may already pass (rmp path unchanged) — that's expected for GREEN.
//!
//! Charter §5.1 / Dev≠QA.

use std::io::Write as _;

use mycelium_core::store::Store;
use mycelium_core::trunk::TrunkPath;
use mycelium_core::types::EdgeKind;

// ── helpers ──────────────────────────────────────────────────────────────────

fn store_with_data() -> Store {
    let mut s = Store::new();
    let a = s.upsert_node(TrunkPath::parse("src/a.rs>fn_a").unwrap());
    let b = s.upsert_node(TrunkPath::parse("src/b.rs>fn_b").unwrap());
    s.upsert_edge(EdgeKind::Calls, a, b);
    s
}

// ── legacy rmp path (always compiled) ────────────────────────────────────────

#[test]
fn load_rmp_file_round_trips() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("snap.myc");

    let orig = store_with_data();
    orig.save(&path).expect("save");

    let loaded = Store::load(&path).expect("load");
    assert_eq!(loaded.node_count(), orig.node_count());
    assert_eq!(loaded.edge_count(), orig.edge_count());
    assert_eq!(loaded.lookup("src/a.rs>fn_a"), orig.lookup("src/a.rs>fn_a"));
}

#[test]
fn load_nonexistent_file_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("does_not_exist.myc");
    assert!(Store::load(&path).is_err());
}

#[test]
fn load_garbage_file_returns_error_not_panic() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("garbage.myc");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(b"this is not a valid snapshot format").unwrap();
    drop(f);
    assert!(Store::load(&path).is_err(), "garbage should yield error");
}

// ── redb dispatch path (feature-gated) ───────────────────────────────────────

#[cfg(feature = "redb-backend")]
mod redb_dispatch {
    use super::*;
    use mycelium_core::store::backend::StorageBackend as _;
    use mycelium_core::store::redb_backend::RedbBackend;

    #[test]
    fn load_detects_redb_format_and_returns_correct_data() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("graph.redb");

        // Write data via RedbBackend directly.
        let mut backend = RedbBackend::open(&path).expect("create redb");
        let a = backend.upsert_node("src/a.rs>fn_a");
        let b = backend.upsert_node("src/b.rs>fn_b");
        backend.upsert_edge(EdgeKind::Calls, a, b);
        backend.flush().expect("flush");
        drop(backend);

        // Store::load should detect the redb format and read the data.
        let store = Store::load(&path).expect("load redb");
        assert_eq!(store.node_count(), 2, "node count must match");
        assert_eq!(store.edge_count(), 1, "edge count must match");
        assert_eq!(store.lookup("src/a.rs>fn_a"), Some(a), "node id must match");
    }

    #[test]
    fn load_rmp_still_works_when_redb_feature_enabled() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("legacy.myc");

        let orig = store_with_data();
        orig.save(&path).expect("save rmp");

        // With redb-backend enabled, rmp files must still load correctly.
        let loaded = Store::load(&path).expect("load rmp with feature on");
        assert_eq!(loaded.node_count(), orig.node_count());
    }
}
