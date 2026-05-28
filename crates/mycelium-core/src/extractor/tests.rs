//! Extractor TDD tests — written before implementation per Charter §5.1.
//!
//! Each test maps to a row in RFC-0002 §Testing strategy or a capture kind
//! from the Python queries.scm.

use super::Extractor;
use crate::{store::Store, types::EdgeKind};

// ── helpers ──────────────────────────────────────────────────────────────────

fn python_extractor() -> Extractor {
    let language: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
    let query_src = include_str!("../../../../packs/python/queries.scm");
    Extractor::new(language, query_src).expect("python extractor should build")
}

fn extract(source: &str) -> Store {
    let ext = python_extractor();
    let mut store = Store::new();
    ext.extract("test.py", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store
}

// ── definition.module ────────────────────────────────────────────────────────

#[test]
fn extractor_creates_module_node() {
    let store = extract("");
    assert!(
        store.lookup("test.py").is_some(),
        "empty file should still create a module node"
    );
}

// ── definition.function ──────────────────────────────────────────────────────

#[test]
fn extractor_creates_top_level_function_node() {
    let store = extract("def foo(): pass");
    assert!(
        store.lookup("test.py>foo").is_some(),
        "top-level function should be a node"
    );
}

#[test]
fn extractor_creates_contains_edge_for_function() {
    let store = extract("def foo(): pass");
    let file_id = store.lookup("test.py").unwrap();
    let fn_id = store.lookup("test.py>foo").unwrap();
    assert!(
        store.outgoing(file_id, EdgeKind::Contains).contains(&fn_id),
        "file should contain function via Contains edge"
    );
}

// ── definition.class ─────────────────────────────────────────────────────────

#[test]
fn extractor_creates_class_node() {
    let store = extract("class Bar: pass");
    assert!(
        store.lookup("test.py>Bar").is_some(),
        "top-level class should be a node"
    );
}

#[test]
fn extractor_creates_contains_edge_for_class() {
    let store = extract("class Bar: pass");
    let file_id = store.lookup("test.py").unwrap();
    let cls_id = store.lookup("test.py>Bar").unwrap();
    assert!(
        store
            .outgoing(file_id, EdgeKind::Contains)
            .contains(&cls_id),
        "file should contain class via Contains edge"
    );
}

// ── definition.method ────────────────────────────────────────────────────────

#[test]
fn extractor_creates_method_node_under_class() {
    let source = "class MyClass:\n    def my_method(self): pass";
    let store = extract(source);
    assert!(
        store.lookup("test.py>MyClass>my_method").is_some(),
        "method should be nested under class in path"
    );
}

#[test]
fn extractor_creates_contains_edges_for_method_chain() {
    let source = "class MyClass:\n    def my_method(self): pass";
    let store = extract(source);
    let file_id = store.lookup("test.py").unwrap();
    let cls_id = store.lookup("test.py>MyClass").unwrap();
    let method_id = store.lookup("test.py>MyClass>my_method").unwrap();
    assert!(
        store
            .outgoing(file_id, EdgeKind::Contains)
            .contains(&cls_id),
        "file→class Contains edge"
    );
    assert!(
        store
            .outgoing(cls_id, EdgeKind::Contains)
            .contains(&method_id),
        "class→method Contains edge"
    );
}

// ── decorated definitions ─────────────────────────────────────────────────────

#[test]
fn extractor_handles_decorated_function() {
    let source = "@staticmethod\ndef decorated(): pass";
    let store = extract(source);
    assert!(
        store.lookup("test.py>decorated").is_some(),
        "decorated function should be extracted"
    );
}

#[test]
fn extractor_handles_decorated_class() {
    let source = "@dataclass\nclass Decorated: pass";
    let store = extract(source);
    assert!(
        store.lookup("test.py>Decorated").is_some(),
        "decorated class should be extracted"
    );
}

// ── reference.import ─────────────────────────────────────────────────────────

#[test]
fn extractor_creates_import_edge_for_import_statement() {
    let source = "import os";
    let store = extract(source);
    let file_id = store.lookup("test.py").unwrap();
    let os_id = store.lookup("os");
    assert!(os_id.is_some(), "import creates a node for the module");
    assert!(
        store
            .outgoing(file_id, EdgeKind::Imports)
            .contains(&os_id.unwrap()),
        "file should have Imports edge to os"
    );
}

#[test]
fn extractor_creates_import_edge_for_dotted_import() {
    let source = "import os.path";
    let store = extract(source);
    let file_id = store.lookup("test.py").unwrap();
    let mod_id = store.lookup("os.path");
    assert!(mod_id.is_some(), "dotted import creates a node");
    assert!(
        store
            .outgoing(file_id, EdgeKind::Imports)
            .contains(&mod_id.unwrap()),
        "file should have Imports edge to os.path"
    );
}

// ── reference.import_from ────────────────────────────────────────────────────

#[test]
fn extractor_creates_import_edge_for_from_import() {
    let source = "from pathlib import Path";
    let store = extract(source);
    let file_id = store.lookup("test.py").unwrap();
    let mod_id = store.lookup("pathlib");
    assert!(mod_id.is_some(), "from-import creates module node");
    assert!(
        store
            .outgoing(file_id, EdgeKind::Imports)
            .contains(&mod_id.unwrap()),
        "file should have Imports edge to pathlib"
    );
}

// ── idempotence ──────────────────────────────────────────────────────────────

#[test]
fn extractor_is_idempotent_on_rescan() {
    let source = "class Foo:\n    def bar(self): pass\ndef baz(): pass";
    let ext = python_extractor();
    let mut store = Store::new();
    // Extract twice — same result (upsert semantics).
    ext.extract("test.py", source.as_bytes(), &mut store)
        .unwrap();
    let foo_id_first = store.lookup("test.py>Foo").unwrap();
    ext.extract("test.py", source.as_bytes(), &mut store)
        .unwrap();
    let foo_id_second = store.lookup("test.py>Foo").unwrap();
    assert_eq!(
        foo_id_first, foo_id_second,
        "re-extraction must return the same NodeId (upsert is stable)"
    );
}

// ── multiple symbols ─────────────────────────────────────────────────────────

#[test]
fn extractor_handles_multiple_top_level_definitions() {
    let source = "def alpha(): pass\ndef beta(): pass\nclass Gamma: pass";
    let store = extract(source);
    assert!(store.lookup("test.py>alpha").is_some());
    assert!(store.lookup("test.py>beta").is_some());
    assert!(store.lookup("test.py>Gamma").is_some());
}

#[test]
fn extractor_handles_class_with_multiple_methods() {
    let source = "class Svc:\n    def a(self): pass\n    def b(self): pass";
    let store = extract(source);
    assert!(store.lookup("test.py>Svc>a").is_some());
    assert!(store.lookup("test.py>Svc>b").is_some());
}
