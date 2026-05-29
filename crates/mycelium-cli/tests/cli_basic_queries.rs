//! v0.1.4 CLI parity backfill batch 1 — `mycelium search-symbol`,
//! `mycelium get-symbol-info`, `mycelium get-ancestors`.
//!
//! These three CLI subcommands are the human-facing twins of MCP tools
//! `mycelium_search_symbol`, `mycelium_get_symbol_info`,
//! `mycelium_get_ancestors`. The tests assert that:
//!
//! 1. The CLI prints something readable in text mode.
//! 2. `--format=json` produces a structurally equivalent value to the
//!    MCP response.
//! 3. Errors (missing index, unknown path) surface with non-zero exit.

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

// ── mycelium search-symbol ────────────────────────────────────────────────────

#[test]
fn search_symbol_text_includes_match() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["search-symbol", "login"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("login"),
        "expected 'login' in output, got:\n{stdout}"
    );
}

#[test]
fn search_symbol_json_is_array_of_strings() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["search-symbol", "login", "--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let parsed: Vec<String> = serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim())
        .expect("stdout must be JSON array of strings");
    assert!(parsed.iter().any(|s| s.contains("login")));
}

#[test]
fn search_symbol_respects_limit() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["search-symbol", "o", "--limit", "1", "--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let parsed: Vec<String> =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(
        parsed.len() <= 1,
        "limit 1 should return at most 1 match, got {}",
        parsed.len()
    );
}

// ── mycelium get-symbol-info ──────────────────────────────────────────────────

#[test]
fn get_symbol_info_json_has_required_fields() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-symbol-info", "src/lib.rs>login", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert_eq!(value["path"].as_str().unwrap(), "src/lib.rs>login");
    assert!(value["ancestors"].is_array());
    assert!(value["descendants"].is_array());
    assert!(value["callers"].is_array());
    assert!(value["callees"].is_array());
}

#[test]
fn get_symbol_info_unknown_path_exits_nonzero() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-symbol-info", "src/nowhere.rs>missing"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("path not found") || stderr.contains("not in the index"),
        "stderr should explain missing path. got:\n{stderr}"
    );
}

// ── mycelium get-ancestors ────────────────────────────────────────────────────

#[test]
fn get_ancestors_json_returns_chain() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-ancestors", "src/lib.rs>login", "--format", "json"])
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
        parsed.iter().any(|p| p == "src/lib.rs"),
        "expected 'src/lib.rs' in ancestors, got {parsed:?}"
    );
}

#[test]
fn get_ancestors_unknown_path_exits_nonzero() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-ancestors", "src/nowhere.rs>missing"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}

// ── shared error: missing index ───────────────────────────────────────────────

#[test]
fn search_with_no_index_exits_nonzero() {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new(mycelium_bin())
        .current_dir(dir.path())
        .args(["search-symbol", "anything"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.to_lowercase().contains("no index")
            || stderr.contains("mycelium index")
            || stderr.contains(".mycelium/index.rmp"),
        "stderr should explain the missing index. got:\n{stderr}"
    );
}
