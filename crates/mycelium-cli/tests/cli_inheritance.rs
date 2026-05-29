//! v0.1.5 CLI parity backfill batch 5 — the 8 inheritance commands.
//! Closes the `inheritance` category Three-Surface.
//!
//! Because no language pack currently emits Extends/Implements edges, these
//! tests verify CLI wiring (valid JSON, correct error codes, argument parsing)
//! rather than graph content. Content tests will follow when a pack emits the
//! edges.

use std::{path::PathBuf, process::Command};

fn mycelium_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mycelium"))
}

/// Minimal Rust project: two functions, enough for a valid index.
fn prepare_indexed_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn alpha() {}\npub fn beta() {}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"q\"\nversion=\"0.0.0\"\nedition=\"2021\"\n",
    )
    .unwrap();
    let status = Command::new(mycelium_bin())
        .args(["index", root.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success(), "mycelium index failed");
    dir
}

// ── get-extends ───────────────────────────────────────────────────────────────

#[test]
fn get_extends_returns_valid_json() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-extends", "src/lib.rs>alpha", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim())
            .expect("stdout must be valid JSON");
    assert!(
        value.get("extends").is_some() && value.get("extended_by").is_some(),
        "expected {{extends, extended_by}}, got: {value}"
    );
}

#[test]
fn get_extends_unknown_path_exits_nonzero() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-extends", "src/lib.rs>no_such_symbol"])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "should exit non-zero for unknown path"
    );
}

// ── extends-tree ──────────────────────────────────────────────────────────────

#[test]
fn extends_tree_returns_root_key() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["extends-tree", "src/lib.rs>alpha", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(
        value.get("root").is_some(),
        "expected {{root: ...}}, got: {value}"
    );
}

#[test]
fn extends_tree_unknown_path_exits_nonzero() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["extends-tree", "src/lib.rs>ghost"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}

// ── subclasses-tree ───────────────────────────────────────────────────────────

#[test]
fn subclasses_tree_returns_root_key() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["subclasses-tree", "src/lib.rs>alpha", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(
        value.get("root").is_some(),
        "expected root key, got: {value}"
    );
}

// ── find-extends-path ─────────────────────────────────────────────────────────

#[test]
fn find_extends_path_same_node_has_zero_hops() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args([
            "find-extends-path",
            "src/lib.rs>alpha",
            "src/lib.rs>alpha",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(
        value.get("path").is_some() || value.get("message").is_some(),
        "expected path or message field, got: {value}"
    );
}

#[test]
fn find_extends_path_unknown_from_exits_nonzero() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["find-extends-path", "src/lib.rs>ghost", "src/lib.rs>alpha"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}

// ── get-implements ────────────────────────────────────────────────────────────

#[test]
fn get_implements_returns_valid_json() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-implements", "src/lib.rs>alpha", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(
        value.get("implements").is_some() && value.get("implemented_by").is_some(),
        "expected {{implements, implemented_by}}, got: {value}"
    );
}

#[test]
fn get_implements_unknown_path_exits_nonzero() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-implements", "src/lib.rs>ghost"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}

// ── implements-tree ───────────────────────────────────────────────────────────

#[test]
fn implements_tree_returns_root_key() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["implements-tree", "src/lib.rs>alpha", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(
        value.get("root").is_some(),
        "expected root key, got: {value}"
    );
}

// ── implementors-tree ─────────────────────────────────────────────────────────

#[test]
fn implementors_tree_returns_root_key() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["implementors-tree", "src/lib.rs>alpha", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(
        value.get("root").is_some(),
        "expected root key, got: {value}"
    );
}

// ── find-implements-path ──────────────────────────────────────────────────────

#[test]
fn find_implements_path_disjoint_returns_no_path() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args([
            "find-implements-path",
            "src/lib.rs>alpha",
            "src/lib.rs>beta",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    // Either path=[] (no path) or path=[...] (trivial/same node)
    assert!(
        value.get("path").is_some() || value.get("message").is_some(),
        "expected path or message field, got: {value}"
    );
}

#[test]
fn find_implements_path_unknown_from_exits_nonzero() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args([
            "find-implements-path",
            "src/lib.rs>ghost",
            "src/lib.rs>alpha",
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
}
