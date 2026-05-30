//! v0.1.5 CLI parity backfill batch 3 — the 7 call-graph commands.
//! Closes the `call-graph` category Three-Surface.

use std::{path::PathBuf, process::Command};

fn mycelium_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mycelium"))
}

/// 3-function chain: `entry()` -> `middle()` -> `leaf()`.
/// Smallest fixture exercising Calls edges + entry/dead/isolated diagnostics.
fn prepare_chain_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn leaf() -> i32 { 42 }\n\
         pub fn middle() -> i32 { leaf() + 1 }\n\
         pub fn entry() -> i32 { middle() * 2 }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"q\"\nversion=\"0.0.0\"\nedition=\"2021\"\n",
    )
    .unwrap();
    let out = Command::new(mycelium_bin())
        .args(["index", root.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "mycelium index failed (exit={:?})\nroot: {}\nstdout: {}\nstderr: {}",
        out.status.code(),
        root.display(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    dir
}

#[test]
fn get_callees_of_entry_includes_middle() {
    let project = prepare_chain_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-callees", "src/lib.rs>entry", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let parsed: Vec<String> =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(
        parsed.iter().any(|p| p.contains("middle")),
        "got {parsed:?}"
    );
}

#[test]
fn get_callers_of_leaf_includes_middle() {
    let project = prepare_chain_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-callers", "src/lib.rs>leaf", "--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let parsed: Vec<String> =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(
        parsed.iter().any(|p| p.contains("middle")),
        "got {parsed:?}"
    );
}

#[test]
fn get_callee_tree_root_has_children() {
    let project = prepare_chain_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args([
            "get-callee-tree",
            "src/lib.rs>entry",
            "--max-depth",
            "3",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(value["path"].as_str().unwrap().contains("entry"));
    assert!(
        !value["children"].as_array().unwrap().is_empty(),
        "expected at least one child, got: {value}"
    );
}

#[test]
fn get_caller_tree_root_has_callers() {
    let project = prepare_chain_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args([
            "get-caller-tree",
            "src/lib.rs>leaf",
            "--max-depth",
            "3",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(value["path"].as_str().unwrap().contains("leaf"));
    assert!(
        !value["callers"].as_array().unwrap().is_empty(),
        "expected at least one caller"
    );
}

#[test]
fn get_entry_points_includes_chain_root() {
    let project = prepare_chain_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-entry-points", "--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let parsed: Vec<String> =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(
        parsed.iter().any(|p| p.contains("entry")),
        "expected 'entry' in entry-points, got {parsed:?}"
    );
}

#[test]
fn get_dead_symbols_runs_smoke() {
    let project = prepare_chain_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-dead-symbols", "--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let _parsed: Vec<String> =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
}

#[test]
fn get_isolated_symbols_runs_smoke() {
    let project = prepare_chain_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-isolated-symbols", "--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let _parsed: Vec<String> =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
}
