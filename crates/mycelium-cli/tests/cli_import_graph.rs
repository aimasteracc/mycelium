//! v0.1.5 CLI parity backfill batch 4 — the 3 import-graph commands.
//! Closes the `import-graph` category Three-Surface.

use std::{path::PathBuf, process::Command};

fn mycelium_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mycelium"))
}

/// Python project: entry.py imports middle, middle.py imports leaf.
/// Python because imports are explicit and the chain is unambiguous.
fn prepare_import_chain() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/leaf.py"), "def leaf_fn():\n    return 42\n").unwrap();
    std::fs::write(
        root.join("src/middle.py"),
        "from .leaf import leaf_fn\n\ndef middle_fn():\n    return leaf_fn() + 1\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/entry.py"),
        "from .middle import middle_fn\n\ndef entry_fn():\n    return middle_fn() * 2\n",
    )
    .unwrap();
    let status = Command::new(mycelium_bin())
        .args(["index", root.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());
    dir
}

#[test]
fn get_imports_envelope_has_imports_and_imported_by() {
    let project = prepare_import_chain();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-imports", "src/middle.py", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(value["imports"].is_array());
    assert!(value["imported_by"].is_array());
}

#[test]
fn get_import_tree_root_envelope() {
    let project = prepare_import_chain();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args([
            "get-import-tree",
            "src/entry.py",
            "--max-depth",
            "3",
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
        value["root"]["path"].as_str().unwrap().contains("entry"),
        "root path should mention entry, got: {value}"
    );
    assert!(value["root"]["imports"].is_array());
}

#[test]
fn get_importers_tree_root_envelope() {
    let project = prepare_import_chain();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args([
            "get-importers-tree",
            "src/leaf.py",
            "--max-depth",
            "3",
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
        value["root"]["path"].as_str().unwrap().contains("leaf"),
        "root path should mention leaf, got: {value}"
    );
    assert!(value["root"]["importers"].is_array());
}

#[test]
fn get_imports_unknown_path_exits_nonzero() {
    let project = prepare_import_chain();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-imports", "src/nowhere.py"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("path not found"),
        "stderr should explain missing path. got:\n{stderr}"
    );
}
