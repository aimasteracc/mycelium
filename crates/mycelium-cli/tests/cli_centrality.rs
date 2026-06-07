//! v0.1.5 CLI batch 7 — 14 centrality commands. Smoke tests on the diamond fixture.

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
fn rank_symbols_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &["rank-symbols", "--limit", "5", "--format", "json"],
        p.path(),
    );
    assert!(v["symbols"].is_array());
}

/// AC-20 (CLI half): rank-symbols --format json output shape must be
/// byte-identical to the MCP contract: {"symbols": [{"path": str, "caller_count": int}, ...]}.
/// Removing or renaming the "caller_count" key in run_rank_symbols makes this fail.
#[test]
fn rank_symbols_json_shape_parity_with_mcp() {
    let p = prepare_diamond();
    let v = json_out(&["rank-symbols", "--format", "json"], p.path());
    let syms = v["symbols"]
        .as_array()
        .expect("'symbols' key must be an array (AC-20 shape parity)");
    assert!(
        !syms.is_empty(),
        "diamond has callee edges; rank-symbols must be non-empty"
    );
    for sym in syms {
        assert!(
            sym["path"].is_string(),
            "each symbol entry must have a string 'path' key (AC-20): {sym}"
        );
        assert!(
            sym["caller_count"].is_number(),
            "each symbol entry must have a numeric 'caller_count' key (AC-20): {sym}"
        );
        let key_count = sym.as_object().map(|o| o.len()).unwrap_or(0);
        assert_eq!(
            key_count, 2,
            "MCP parity: symbol entry must have exactly 2 keys {{path, caller_count}}, got {sym}"
        );
    }
}

#[test]
fn get_top_files_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &["get-top-files", "--limit", "5", "--format", "json"],
        p.path(),
    );
    assert!(v["files"].is_array());
    assert!(v["count"].is_number());
}

#[test]
fn get_most_connected_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-most-connected",
            "--edge-kind",
            "calls",
            "--limit",
            "5",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["symbols"].is_array());
}

#[test]
fn get_hub_symbols_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-hub-symbols",
            "--edge-kind",
            "calls",
            "--limit",
            "5",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["hubs"].is_array());
}

#[test]
fn get_fan_out_rank_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-fan-out-rank",
            "--edge-kind",
            "calls",
            "--limit",
            "5",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["symbols"].is_array());
}

#[test]
fn get_fan_in_rank_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "get-fan-in-rank",
            "--edge-kind",
            "calls",
            "--limit",
            "5",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["symbols"].is_array());
}

#[test]
fn betweenness_centrality_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "betweenness-centrality",
            "--edge-kind",
            "calls",
            "--top-n",
            "5",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["nodes"].is_array());
    assert!(v["symbol_count"].is_number());
}

#[test]
fn closeness_centrality_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "closeness-centrality",
            "--edge-kind",
            "calls",
            "--top-n",
            "5",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["nodes"].is_array());
}

#[test]
fn degree_centrality_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "degree-centrality",
            "--edge-kind",
            "calls",
            "--top-n",
            "5",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["nodes"].is_array());
    assert!(v["sort_by"].is_string());
}

#[test]
fn clustering_coefficient_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "clustering-coefficient",
            "src/lib.rs>b",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["coefficient"].is_number());
}

#[test]
fn eccentricity_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "eccentricity",
            "src/lib.rs>a",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["eccentricity"].is_number());
}

#[test]
fn page_rank_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "page-rank",
            "--edge-kind",
            "calls",
            "--top-n",
            "5",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["nodes"].is_array());
}

#[test]
fn harmonic_centrality_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "harmonic-centrality",
            "src/lib.rs>a",
            "--edge-kind",
            "calls",
            "--format",
            "json",
        ],
        p.path(),
    );
    assert!(v["harmonic_centrality"].is_number());
}

#[test]
fn neighbor_similarity_smoke() {
    let p = prepare_diamond();
    let v = json_out(
        &[
            "neighbor-similarity",
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
    assert!(v["similarity"].is_number());
}
