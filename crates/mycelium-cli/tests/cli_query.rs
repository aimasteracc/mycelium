//! Issue #151 — `mycelium query <hyphae>` must wire the Hyphae DSL all the
//! way through: parse, load `.mycelium/index.rmp`, evaluate, print matches.
//!
//! The previous behaviour was a `tracing::warn!` stub. These tests are the
//! contract for the real implementation that lands in v0.1.3.

use std::{path::PathBuf, process::Command};

fn mycelium_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mycelium"))
}

/// Build a tiny indexed project under a temp root and return its tempdir.
fn prepare_indexed_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create tempdir");
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    // Two top-level functions: `login` and `logout`. Enough for #name and
    // .function selectors to discriminate.
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

    let out = Command::new(mycelium_bin())
        .args(["index", root.to_str().unwrap()])
        .output()
        .expect("spawn mycelium index");
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
fn query_by_name_selector_text_output() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["query", "#login"])
        .output()
        .expect("spawn mycelium query");

    assert!(
        out.status.success(),
        "mycelium query failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8 stdout");
    assert!(
        stdout.contains("login"),
        "expected 'login' in stdout for query '#login', got:\n{stdout}"
    );
    assert!(
        !stdout.contains("logout"),
        "name selector '#login' must NOT match 'logout'; got:\n{stdout}"
    );
}

#[test]
fn query_by_kind_selector_text_output() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["query", ".function"])
        .output()
        .expect("spawn mycelium query");

    assert!(out.status.success(), "mycelium query failed");
    let stdout = String::from_utf8(out.stdout).expect("utf8 stdout");
    // Both functions match the kind selector.
    assert!(
        stdout.contains("login") && stdout.contains("logout"),
        ".function selector must return both functions, got:\n{stdout}"
    );
}

#[test]
fn query_json_format_is_valid_json_array_of_strings() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["query", ".function", "--format", "json"])
        .output()
        .expect("spawn mycelium query");

    assert!(out.status.success(), "mycelium query failed");
    let stdout = String::from_utf8(out.stdout).expect("utf8 stdout");
    let parsed: Vec<String> =
        serde_json::from_str(stdout.trim()).expect("stdout must be a JSON array of strings");
    assert!(
        parsed.iter().any(|s| s.contains("login")),
        "JSON output should contain 'login', got: {parsed:?}"
    );
    assert!(
        parsed.iter().any(|s| s.contains("logout")),
        "JSON output should contain 'logout', got: {parsed:?}"
    );
}

#[test]
fn query_with_no_index_reports_a_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new(mycelium_bin())
        .current_dir(dir.path())
        .args(["query", "#anything"])
        .output()
        .expect("spawn mycelium query");

    assert!(
        !out.status.success(),
        "querying without an index should exit non-zero"
    );
    let stderr = String::from_utf8(out.stderr).expect("utf8 stderr");
    // The error message must point at the missing snapshot, not panic at the
    // user. Either the literal phrase "no index" or "run `mycelium index`" is
    // acceptable; both are user-recovery hints.
    assert!(
        stderr.to_lowercase().contains("no index")
            || stderr.contains("mycelium index")
            || stderr.contains(".mycelium/index.rmp"),
        "stderr should explain the missing index. got:\n{stderr}"
    );
}

#[test]
fn query_with_invalid_selector_exits_non_zero_with_parse_error() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["query", "this is not a selector >>"])
        .output()
        .expect("spawn mycelium query");

    assert!(
        !out.status.success(),
        "malformed selector should exit non-zero"
    );
    let stderr = String::from_utf8(out.stderr).expect("utf8 stderr");
    assert!(
        stderr.to_lowercase().contains("parse") || stderr.to_lowercase().contains("hyphae"),
        "stderr should mention parse/hyphae for a malformed selector. got:\n{stderr}"
    );
}
