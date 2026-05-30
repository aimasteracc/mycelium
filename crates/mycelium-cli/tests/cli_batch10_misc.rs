//! v0.1.5 CLI batch 10 — 11 final cross-category commands.

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
fn get_node_degree_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &["get-node-degree", "src/lib.rs>a", "--format", "json"],
        p.path(),
    );
    assert!(v["in_calls"].is_number());
    assert!(v["out_calls"].is_number());
}

#[test]
fn get_files_smoke() {
    let p = prepare_diamond();
    let v = json_out(&["get-files", "--format", "json"], p.path());
    assert!(v["files"].is_array());
}

#[test]
fn get_symbol_count_by_kind_smoke() {
    let p = prepare_diamond();
    let v = json_out(&["get-symbol-count-by-kind", "--format", "json"], p.path());
    assert!(v["kinds"].is_array());
    assert!(v["total"].is_number());
}

#[test]
fn get_leaf_symbols_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-leaf-symbols",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["symbols"].is_array());
}

#[test]
fn get_common_callers_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-common-callers",
            "--paths",
            "src/lib.rs>d,src/lib.rs>e",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["callers"].is_array());
}

#[test]
fn get_common_callees_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-common-callees",
            "--paths",
            "src/lib.rs>a,src/lib.rs>b",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["callees"].is_array());
}

#[test]
fn get_common_reachable_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-common-reachable",
            "--path1",
            "src/lib.rs>a",
            "--path2",
            "src/lib.rs>b",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["common"].is_array());
}

#[test]
fn get_mutual_reachability_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-mutual-reachability",
            "--path1",
            "src/lib.rs>a",
            "--path2",
            "src/lib.rs>d",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["forward"].is_boolean());
    assert!(v["backward"].is_boolean());
    assert!(v["mutual"].is_boolean());
}

#[test]
fn find_call_path_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "find-call-path",
            "--from",
            "src/lib.rs>a",
            "--to",
            "src/lib.rs>d",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["path"].is_array());
}

#[test]
fn find_import_path_smoke() {
    let p = prepare_diamond();
    // No imports in a single-file Rust project; should report unreachable
    // with message envelope.
    let v = json_out(
        &[
            "find-import-path",
            "--from",
            "src/lib.rs",
            "--to",
            "src/lib.rs",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["path"].is_array() || v.get("message").is_some());
}
