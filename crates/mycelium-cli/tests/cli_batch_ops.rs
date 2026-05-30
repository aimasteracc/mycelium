//! v0.1.5 CLI batch 9 — 4 batch-ops commands.

use std::{path::PathBuf, process::Command};

fn mycelium_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mycelium"))
}

fn prepare_diamond() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn d() -> i32 { 4 }\n\
         pub fn e() -> i32 { 5 }\n\
         pub fn c() -> i32 { d() + 1 }\n\
         pub fn b() -> i32 { c() + e() }\n\
         pub fn a() -> i32 { b() * 2 }\n",
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
fn batch_symbol_info_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "batch-symbol-info",
            "--paths",
            "src/lib.rs>a,src/lib.rs>b,src/lib.rs>c",
            "--format",
            "json",
        ],
        p.path(),
    );
    let arr = v["symbols"].as_array().unwrap();
    assert_eq!(arr.len(), 3);
    for entry in arr {
        assert!(entry["path"].is_string());
    }
}

#[test]
fn batch_node_degree_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "batch-node-degree",
            "--paths",
            "src/lib.rs>a,src/lib.rs>b",
            "--format",
            "json",
        ],
        p.path(),
    );
    let arr = v["degrees"].as_array().unwrap();
    assert_eq!(arr.len(), 2);
}

#[test]
fn batch_reachable_from_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "batch-reachable-from",
            "--paths",
            "src/lib.rs>a,src/lib.rs>b",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["reachable"].is_array());
}

#[test]
fn batch_reachable_to_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "batch-reachable-to",
            "--paths",
            "src/lib.rs>d,src/lib.rs>e",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["reachable"].is_array());
}

// Issue #298: batch commands should accept repeated --paths flags (not just comma-separated string).
#[test]
fn batch_symbol_info_repeated_paths_flag() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "batch-symbol-info",
            "--paths",
            "src/lib.rs>a",
            "--paths",
            "src/lib.rs>b",
            "--format",
            "json",
        ],
        p.path(),
    );
    let arr = v["symbols"].as_array().unwrap();
    assert_eq!(arr.len(), 2);
}

#[test]
fn batch_node_degree_repeated_paths_flag() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "batch-node-degree",
            "--paths",
            "src/lib.rs>a",
            "--paths",
            "src/lib.rs>b",
            "--format",
            "json",
        ],
        p.path(),
    );
    let arr = v["degrees"].as_array().unwrap();
    assert_eq!(arr.len(), 2);
}
