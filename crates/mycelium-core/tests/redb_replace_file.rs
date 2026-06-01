//! RFC-0100 / #343 — RED-first tests for redb file-scoped replacement.
//!
//! These tests define the incremental persistence unit: replacing one source
//! file must use the persisted `file_index` to remove only that file's old
//! nodes and owned edges, while preserving unrelated files and their edges.

#![cfg(feature = "redb-backend")]

use mycelium_core::store::backend::StorageBackend;
use mycelium_core::store::redb_backend::{FileEdge, FileNode, RedbBackend};
use mycelium_core::trunk::path_to_node_id;
use mycelium_core::types::{EdgeKind, NodeId, NodeKind, SourceSpan};

fn open_at(path: &std::path::Path) -> RedbBackend {
    RedbBackend::open(path).expect("open redb")
}

fn node(path: &str, kind: NodeKind) -> FileNode {
    FileNode {
        path: path.to_owned(),
        kind: Some(kind),
        span: None,
    }
}

fn node_with_span(path: &str, kind: NodeKind, span: SourceSpan) -> FileNode {
    FileNode {
        path: path.to_owned(),
        kind: Some(kind),
        span: Some(span),
    }
}

fn edge(kind: EdgeKind, src_path: &str, dst_path: &str) -> FileEdge {
    FileEdge {
        kind,
        src: id(src_path),
        dst: id(dst_path),
    }
}

fn id(path: &str) -> NodeId {
    path_to_node_id(path)
}

fn sorted(mut ids: Vec<NodeId>) -> Vec<NodeId> {
    ids.sort_unstable();
    ids
}

const fn span() -> SourceSpan {
    SourceSpan {
        start_line: 3,
        start_col: 1,
        end_line: 8,
        end_col: 2,
        start_byte: 40,
        end_byte: 180,
    }
}

#[test]
fn replace_file_uses_persisted_file_index_across_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("replace_file_persisted_index.redb");

    {
        let mut backend = open_at(&path);
        backend
            .replace_file("src/a.rs", &[node("src/a.rs>Old", NodeKind::Struct)], &[])
            .expect("initial replace_file");
    }

    {
        let mut backend = open_at(&path);
        backend
            .replace_file("src/a.rs", &[node("src/a.rs>New", NodeKind::Struct)], &[])
            .expect("second replace_file after reopen");

        assert_eq!(backend.lookup_path("src/a.rs>Old"), None);
        assert_eq!(
            backend.lookup_path("src/a.rs>New"),
            Some(id("src/a.rs>New"))
        );
    }
}

#[test]
fn replace_file_removes_old_nodes_metadata_and_owned_edges_only() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("replace_file_owned_edges.redb");
    let mut backend = open_at(&path);

    backend
        .replace_file("src/b.rs", &[node("src/b.rs>B", NodeKind::Function)], &[])
        .expect("insert b");
    backend
        .replace_file(
            "src/c.rs",
            &[node("src/c.rs>C", NodeKind::Function)],
            &[edge(EdgeKind::Calls, "src/c.rs>C", "src/b.rs>B")],
        )
        .expect("insert c");
    backend
        .replace_file(
            "src/a.rs",
            &[
                node("src/a.rs>A", NodeKind::Struct),
                node_with_span("src/a.rs>A>old", NodeKind::Method, span()),
            ],
            &[edge(EdgeKind::Calls, "src/a.rs>A>old", "src/b.rs>B")],
        )
        .expect("insert a v1");

    assert_eq!(
        backend.incoming(id("src/b.rs>B"), EdgeKind::Calls),
        sorted(vec![id("src/a.rs>A>old"), id("src/c.rs>C")])
    );

    backend
        .replace_file("src/a.rs", &[node("src/a.rs>A>new", NodeKind::Method)], &[])
        .expect("replace a");

    assert_eq!(backend.lookup_path("src/a.rs>A"), None);
    assert_eq!(backend.lookup_path("src/a.rs>A>old"), None);
    assert_eq!(backend.kind_of(id("src/a.rs>A>old")), None);
    assert_eq!(backend.span_of(id("src/a.rs>A>old")), None);
    assert_eq!(
        backend.lookup_path("src/a.rs>A>new"),
        Some(id("src/a.rs>A>new"))
    );

    assert_eq!(backend.lookup_path("src/b.rs>B"), Some(id("src/b.rs>B")));
    assert_eq!(backend.lookup_path("src/c.rs>C"), Some(id("src/c.rs>C")));
    assert_eq!(
        backend.incoming(id("src/b.rs>B"), EdgeKind::Calls),
        vec![id("src/c.rs>C")],
        "replacing src/a.rs must remove only src/a.rs owned edge to B"
    );
}

#[test]
fn replace_file_removes_external_edges_pointing_into_old_nodes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("replace_file_external_edges.redb");
    let mut backend = open_at(&path);

    backend
        .replace_file("src/a.rs", &[node("src/a.rs>A", NodeKind::Struct)], &[])
        .expect("insert a");
    backend
        .replace_file(
            "src/b.rs",
            &[node("src/b.rs>B", NodeKind::Function)],
            &[edge(EdgeKind::References, "src/b.rs>B", "src/a.rs>A")],
        )
        .expect("insert b");

    assert_eq!(
        backend.outgoing(id("src/b.rs>B"), EdgeKind::References),
        vec![id("src/a.rs>A")]
    );

    backend
        .replace_file("src/a.rs", &[node("src/a.rs>A2", NodeKind::Struct)], &[])
        .expect("replace a");

    assert_eq!(backend.lookup_path("src/a.rs>A"), None);
    assert_eq!(
        backend.outgoing(id("src/b.rs>B"), EdgeKind::References),
        Vec::<NodeId>::new(),
        "external references to removed src/a.rs nodes must be stripped"
    );
    assert_eq!(backend.lookup_path("src/b.rs>B"), Some(id("src/b.rs>B")));
}
