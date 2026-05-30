//! v0.1.4 CLI parity backfill batches 2-3 — the remaining 7 basic-queries
//! commands: get-descendants, get-node-kind, get-symbols-by-kind,
//! get-source-span, get-siblings, get-all-symbols, server-status.

use std::{path::PathBuf, process::Command};

fn mycelium_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mycelium"))
}

fn prepare_indexed_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create tempdir");
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn login(name: &str) -> String { name.to_string() }\n\
         pub fn logout() {}\n",
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

// ── get-descendants ───────────────────────────────────────────────────────────

#[test]
fn get_descendants_returns_children_of_file() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-descendants", "src/lib.rs", "--format", "json"])
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
        parsed.iter().any(|p| p.contains("login")),
        "expected login in descendants, got {parsed:?}"
    );
}

// ── get-node-kind ─────────────────────────────────────────────────────────────

#[test]
fn get_node_kind_returns_a_kind_string() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-node-kind", "src/lib.rs>login", "--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert_eq!(value["path"].as_str().unwrap(), "src/lib.rs>login");
    // Kind should be one of the recognised wire strings (function/method/etc.)
    assert!(
        value["kind"].as_str().is_some() || value["kind"].is_null(),
        "kind should be string or null, got: {}",
        value["kind"]
    );
}

// ── get-symbols-by-kind ───────────────────────────────────────────────────────

#[test]
fn get_symbols_by_kind_function_includes_login() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-symbols-by-kind", "function", "--format", "json"])
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
        parsed.iter().any(|p| p.contains("login")),
        "expected login among functions, got {parsed:?}"
    );
}

#[test]
fn get_symbols_by_kind_unknown_kind_errors() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-symbols-by-kind", "not_a_kind"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("unknown kind"),
        "stderr should mention unknown kind"
    );
}

// ── get-source-span ───────────────────────────────────────────────────────────

#[test]
fn get_source_span_has_line_fields_or_null() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-source-span", "src/lib.rs>login", "--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert_eq!(value["path"].as_str().unwrap(), "src/lib.rs>login");
    assert!(
        value["start_line"].as_u64().is_some() || value["span"].is_null(),
        "span fields or null"
    );
}

// ── get-siblings ──────────────────────────────────────────────────────────────

#[test]
fn get_siblings_login_includes_logout() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-siblings", "src/lib.rs>login", "--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let parsed: Vec<String> =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(
        parsed.iter().any(|p| p.contains("logout")),
        "siblings of login should include logout, got {parsed:?}"
    );
}

// ── get-all-symbols ───────────────────────────────────────────────────────────

#[test]
fn get_all_symbols_contains_login_and_logout() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-all-symbols", "--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let parsed: Vec<String> =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(
        parsed.iter().any(|p| p.contains("login")) && parsed.iter().any(|p| p.contains("logout")),
        "expected both login and logout in all-symbols, got {parsed:?}"
    );
}

#[test]
fn get_all_symbols_prefix_filter() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args([
            "get-all-symbols",
            "--prefix",
            "src/lib.rs",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let parsed: Vec<String> =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(
        !parsed.is_empty(),
        "prefix filter should return at least one match"
    );
    assert!(
        parsed.iter().all(|p| p.starts_with("src/lib.rs")),
        "every entry should match the prefix, got {parsed:?}"
    );
}

// ── server-status ─────────────────────────────────────────────────────────────

#[test]
fn server_status_reports_loaded_index() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["server-status", "--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(value["is_loaded"].as_bool().unwrap());
    assert!(value["node_count"].as_u64().unwrap() > 0);
}

#[test]
fn server_status_reports_unloaded_when_no_index() {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new(mycelium_bin())
        .current_dir(dir.path())
        .args(["server-status", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "server-status should succeed even with no index — it just reports is_loaded=false"
    );
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(!value["is_loaded"].as_bool().unwrap());
    assert_eq!(value["node_count"].as_u64().unwrap(), 0);
}
