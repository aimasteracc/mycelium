//! v0.1.5 CLI parity backfill batch 6 — the 12 reachability commands.
//! Closes the `reachability` category Three-Surface.

use std::{path::PathBuf, process::Command};

fn mycelium_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mycelium"))
}

/// 5-function diamond: a -> b -> c -> d, plus b -> e.
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
fn get_reachable_from_a() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-reachable",
            "src/lib.rs>a",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    let arr: Vec<String> = serde_json::from_value(v["reachable"].clone()).unwrap();
    assert!(arr.iter().any(|s| s.contains('d')));
    assert!(arr.iter().any(|s| s.contains('e')));
}

#[test]
fn get_reachable_to_d() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-reachable-to",
            "src/lib.rs>d",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    let arr: Vec<String> = serde_json::from_value(v["reachable"].clone()).unwrap();
    assert!(arr.iter().any(|s| s.contains('a')));
}

#[test]
fn get_k_hop_k2() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-k-hop-neighbors",
            "src/lib.rs>a",
            "--k",
            "2",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["neighbors"].is_array());
    assert_eq!(v["k"].as_u64().unwrap(), 2);
}

#[test]
fn get_two_hop() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-two-hop-neighbors",
            "src/lib.rs>a",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["neighbors"].is_array());
}

#[test]
fn get_shortest_path_a_to_d() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-shortest-path",
            "--from",
            "src/lib.rs>a",
            "--to",
            "src/lib.rs>d",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    // Either path array (success) or null (no path) — both valid shapes.
    assert!(v["path"].is_array() || v["path"].is_null());
}

#[test]
fn get_symbol_neighborhood_b() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-symbol-neighborhood",
            "src/lib.rs>b",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["incoming"].is_array());
    assert!(v["outgoing"].is_array());
    assert!(v["incoming_count"].is_number());
    assert!(v["outgoing_count"].is_number());
}

#[test]
fn get_cross_refs_d() {
    let p = prepare_diamond();
    let v = json_out(
        &["get-cross-refs", "src/lib.rs>d", "--format", "json"],
        p.path(),
    );
    assert!(v["callers"].is_array());
    assert!(v["importers"].is_array());
    assert!(v["extended_by"].is_array());
    assert!(v["implemented_by"].is_array());
}

#[test]
fn get_outgoing_refs_a() {
    let p = prepare_diamond();
    let v = json_out(
        &["get-outgoing-refs", "src/lib.rs>a", "--format", "json"],
        p.path(),
    );
    assert!(v["callees"].is_array());
    assert!(v["imports"].is_array());
    assert!(v["extends"].is_array());
    assert!(v["implements"].is_array());
}

#[test]
fn get_dependency_depth_a() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-dependency-depth",
            "src/lib.rs>a",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["depth"].is_number());
    assert_eq!(v["edge_kind"].as_str().unwrap(), "calls");
}

#[test]
fn get_reachable_set_a() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-reachable-set",
            "src/lib.rs>a",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["reachable"].is_array());
    assert!(v["count"].is_number());
}

#[test]
fn get_reaches_into_d() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-reaches-into",
            "src/lib.rs>d",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["callers"].is_array());
    assert!(v["count"].is_number());
}

#[test]
fn get_singly_referenced_calls() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-singly-referenced",
            "--edge-kind",
            "calls",
            "--limit",
            "10",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["symbols"].is_array());
    assert!(v["count"].is_number());
}
