//! v0.1.5 CLI batch 8 — 14 graph-structure commands.

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
fn get_stats_smoke() {
    let p = prepare_diamond();
    let v = json_out(&["get-stats", "--format", "json"], p.path());
    assert!(v["total_nodes"].is_number());
    assert!(v["total_edges"].is_number());
}

#[test]
fn get_graph_metrics_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-graph-metrics",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["symbol_count"].is_number());
    assert!(v["density"].is_number());
}

#[test]
fn detect_cycles_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &["detect-cycles", "--edge-kind", "calls", "--format", "json"],
        p.path(),
    );
    assert!(v["cycle_nodes"].is_array());
    assert!(v["count"].is_number());
}

#[test]
fn get_scc_groups_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &["get-scc-groups", "--edge-kind", "calls", "--format", "json"],
        p.path(),
    );
    assert!(v["groups"].is_array());
}

#[test]
fn topological_sort_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "topological-sort",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["order"].is_array());
}

#[test]
fn find_articulation_points_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "find-articulation-points",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["points"].is_array());
}

#[test]
fn find_bridge_edges_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "find-bridge-edges",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["bridges"].is_array());
}

#[test]
fn get_biconnected_components_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-biconnected-components",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["components"].is_array());
}

#[test]
fn get_k_core_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-k-core",
            "--edge-kind",
            "calls",
            "--k",
            "1",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["core"].is_array());
}

#[test]
fn get_dependency_layers_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-dependency-layers",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["layers"].is_array());
}

#[test]
fn get_scc_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &["get-scc", "--edge-kind", "calls", "--format", "json"],
        p.path(),
    );
    assert!(v["components"].is_array());
    assert!(v["total_components"].is_number());
}

#[test]
fn get_wcc_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &["get-wcc", "--edge-kind", "calls", "--format", "json"],
        p.path(),
    );
    assert!(v["components"].is_array());
}

#[test]
fn get_degree_histogram_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-degree-histogram",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["in_degrees"].is_array());
    assert!(v["out_degrees"].is_array());
}

#[test]
fn find_cycle_members_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "find-cycle-members",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["members"].is_array());
}
