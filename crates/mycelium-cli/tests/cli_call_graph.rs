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
    let status = Command::new(mycelium_bin())
        .args(["index", root.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());
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
    // RFC-0109 Option A: CLI --format json now emits the same object shape as
    // the MCP tool (`{ "callee_paths": [...] }`), not a bare array.
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    let parsed: Vec<String> = value["callee_paths"]
        .as_array()
        .expect("callee_paths array")
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    assert!(
        parsed.iter().any(|p| p.contains("middle")),
        "got {parsed:?}"
    );
}

// RFC-0109 Option A + RFC-0102 knob on get-callees (CLI surface).

/// `entry()` that calls `f0()..f{n-1}()` — a fan-out wide enough to exceed the
/// small-project edge budget (30) so truncation is observable.
fn prepare_wide_callee_project(n: usize) -> tempfile::TempDir {
    use std::fmt::Write as _;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    let mut src = String::new();
    for i in 0..n {
        let _ = writeln!(src, "pub fn f{i}() {{}}");
    }
    src.push_str("pub fn entry() {\n");
    for i in 0..n {
        let _ = writeln!(src, "    f{i}();");
    }
    src.push_str("}\n");
    std::fs::write(root.join("src/lib.rs"), src).unwrap();
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

#[test]
fn get_callees_text_mode_full_but_json_budgeted() {
    // RFC-0102 / Codex #504 P2: default text mode must NOT silently truncate a
    // human's list, while JSON mode applies the budget (parity with MCP).
    let project = prepare_wide_callee_project(35); // 35 > small edge budget (30)

    // JSON (default auto budget) → truncated to 30 with metadata.
    let json_out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-callees", "src/lib.rs>entry", "--format", "json"])
        .output()
        .unwrap();
    assert!(json_out.status.success());
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(json_out.stdout).unwrap().trim()).unwrap();
    assert_eq!(
        value["callee_paths"].as_array().unwrap().len(),
        30,
        "JSON mode should apply the default budget"
    );
    assert_eq!(value["truncated"], true);

    // Default text mode → full list, no silent truncation.
    let text_out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-callees", "src/lib.rs>entry"])
        .output()
        .unwrap();
    assert!(text_out.status.success());
    let lines = String::from_utf8(text_out.stdout)
        .unwrap()
        .lines()
        .filter(|l| l.contains(">f"))
        .count();
    assert_eq!(lines, 35, "text mode must print the full caller list");
}

#[test]
fn text_mode_explicit_budget_emits_truncation_footer() {
    // RFC-0102 / Codex #513 P2: an explicit --budget in text mode truncates,
    // so it must surface a footer (to stderr) rather than silently drop results.
    let project = prepare_wide_callee_project(35); // 36 symbols > small node budget (15)
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-all-symbols", "--budget", "small"]) // text mode (default)
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout_lines = String::from_utf8(out.stdout).unwrap().lines().count();
    assert_eq!(stdout_lines, 15, "small budget caps the page at 15 symbols");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("shown") && stderr.contains("--budget disabled"),
        "expected a truncation footer on stderr, got: {stderr:?}"
    );
}

#[test]
fn get_callees_json_is_object_with_callee_paths_key() {
    let project = prepare_chain_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-callees", "src/lib.rs>entry", "--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    // Object shape (not a bare array) — the byte-identical twin of the MCP tool.
    assert!(
        value
            .get("callee_paths")
            .and_then(|v| v.as_array())
            .is_some(),
        "expected object with callee_paths array, got: {value}"
    );
}

#[test]
fn get_callees_budget_disabled_accepted_unknown_rejected() {
    let project = prepare_chain_project();
    let ok = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args([
            "get-callees",
            "src/lib.rs>entry",
            "--format",
            "json",
            "--budget",
            "disabled",
        ])
        .output()
        .unwrap();
    assert!(ok.status.success(), "--budget disabled should be accepted");

    let bad = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["get-callees", "src/lib.rs>entry", "--budget", "huge"])
        .output()
        .unwrap();
    assert!(!bad.status.success(), "unknown --budget must fail");
    assert!(
        String::from_utf8_lossy(&bad.stderr).contains("huge"),
        "error should name the bad value"
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
    // RFC-0109 Option A: object shape `{ "caller_paths": [...] }`, byte-identical
    // to the MCP tool.
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    let parsed: Vec<String> = value["caller_paths"]
        .as_array()
        .expect("caller_paths array")
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
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
    // RFC-0109 Option A: object shape `{ "dead_symbols": [...], "count": N }`.
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(
        value
            .get("dead_symbols")
            .and_then(|v| v.as_array())
            .is_some(),
        "expected object with dead_symbols array, got: {value}"
    );
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
    // RFC-0109 Option A: object shape `{ "isolated_symbols": [...], "count": N }`.
    let value: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();
    assert!(
        value
            .get("isolated_symbols")
            .and_then(|v| v.as_array())
            .is_some(),
        "expected object with isolated_symbols array, got: {value}"
    );
}
