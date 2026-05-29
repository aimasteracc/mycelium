//! v0.1.5 CLI parity backfill batch 5 — the 8 inheritance commands.
//! Closes the `inheritance` category Three-Surface.

use std::{path::PathBuf, process::Command};

fn mycelium_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mycelium"))
}

/// Python 3-class extends chain: Grandparent <- Parent <- Child.
fn prepare_python_chain() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.py"),
        "class Grandparent:\n    pass\n\nclass Parent(Grandparent):\n    pass\n\nclass Child(Parent):\n    pass\n",
    )
    .unwrap();
    let status = Command::new(mycelium_bin())
        .args(["index", root.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());
    dir
}

/// Rust: one trait + two impls (Implements edges).
fn prepare_rust_traits() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub trait Repository { fn name(&self) -> &'static str; }\n\
         pub struct PostgresRepo;\n\
         impl Repository for PostgresRepo { fn name(&self) -> &'static str { \"postgres\" } }\n\
         pub struct SqliteRepo;\n\
         impl Repository for SqliteRepo { fn name(&self) -> &'static str { \"sqlite\" } }\n",
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
    assert!(status.success());
    dir
}

fn json_out(args: &[&str], cwd: &std::path::Path) -> serde_json::Value {
    let out = Command::new(mycelium_bin())
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "args={args:?} stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap()
}

#[test]
fn get_extends_envelope() {
    let p = prepare_python_chain();
    let v = json_out(
        &["get-extends", "src/lib.py>Child", "--format", "json"],
        p.path(),
    );
    assert!(v["extends"].is_array());
    assert!(v["extended_by"].is_array());
}

#[test]
fn extends_tree_envelope() {
    let p = prepare_python_chain();
    let v = json_out(
        &[
            "extends-tree",
            "src/lib.py>Child",
            "--max-depth",
            "5",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["root"]["path"].as_str().unwrap().contains("Child"));
    assert!(v["root"]["parents"].is_array());
}

#[test]
fn subclasses_tree_envelope() {
    let p = prepare_python_chain();
    let v = json_out(
        &[
            "subclasses-tree",
            "src/lib.py>Grandparent",
            "--max-depth",
            "5",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["root"]["path"].as_str().unwrap().contains("Grandparent"));
    assert!(v["root"]["subclasses"].is_array());
}

#[test]
fn find_extends_path_envelope() {
    let p = prepare_python_chain();
    let v = json_out(
        &[
            "find-extends-path",
            "--from",
            "src/lib.py>Child",
            "--to",
            "src/lib.py>Grandparent",
            "--format",
            "json",
        ],
        p.path(),
    );
    // Either succeeded (hops > 0) or returned the no-path message envelope —
    // both are valid shapes the MCP tool also returns.
    assert!(v["path"].is_array() || v.get("message").is_some());
}

#[test]
fn get_implements_envelope() {
    let p = prepare_rust_traits();
    let v = json_out(
        &[
            "get-implements",
            "src/lib.rs>PostgresRepo",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["implements"].is_array());
    assert!(v["implemented_by"].is_array());
}

#[test]
fn implements_tree_envelope() {
    let p = prepare_rust_traits();
    let v = json_out(
        &[
            "implements-tree",
            "src/lib.rs>PostgresRepo",
            "--max-depth",
            "3",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["root"]["path"].as_str().unwrap().contains("PostgresRepo"));
    assert!(v["root"]["interfaces"].is_array());
}

#[test]
fn implementors_tree_envelope() {
    let p = prepare_rust_traits();
    let v = json_out(
        &[
            "implementors-tree",
            "src/lib.rs>Repository",
            "--max-depth",
            "3",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["root"]["path"].as_str().unwrap().contains("Repository"));
    assert!(v["root"]["implementors"].is_array());
}

#[test]
fn find_implements_path_envelope() {
    let p = prepare_rust_traits();
    let v = json_out(
        &[
            "find-implements-path",
            "--from",
            "src/lib.rs>PostgresRepo",
            "--to",
            "src/lib.rs>Repository",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["path"].is_array() || v.get("message").is_some());
}
