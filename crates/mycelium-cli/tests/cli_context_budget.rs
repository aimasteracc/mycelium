//! RFC-0102 per-call budget knob — CLI surface (`mycelium context --budget`).
//!
//! The byte-identical twin of the MCP `mycelium_context` `budget` field. These
//! tests prove the flag is wired through to the shared
//! `mycelium_core::budget::OutputBudget::resolve`:
//!
//! 1. A valid tier (`disabled`) is accepted and produces an untruncated JSON
//!    payload (no `truncated` / `budget` keys on a tiny project).
//! 2. An unknown value fails fast with a helpful error naming the bad token —
//!    the CLI mirror of the MCP `application_error`.
//! 3. Omitting `--budget` keeps the prior `auto` behavior (back-compat).

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
    let out = Command::new(mycelium_bin())
        .args(["index", root.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success(), "mycelium index failed");
    dir
}

#[test]
fn context_budget_disabled_is_accepted() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args([
            "context", "--task", "login", "--budget", "disabled", "--format", "json",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "context --budget disabled failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Tiny project, disabled budget → nothing truncated.
    assert!(
        !stdout.contains("\"truncated\""),
        "disabled budget must not truncate: {stdout}"
    );
}

#[test]
fn context_budget_unknown_value_fails_fast() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["context", "--task", "login", "--budget", "huge"])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "an unknown --budget value must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("huge"),
        "error should name the bad value, got: {stderr}"
    );
}

#[test]
fn context_without_budget_flag_still_works() {
    let project = prepare_indexed_project();
    let out = Command::new(mycelium_bin())
        .current_dir(project.path())
        .args(["context", "--task", "login", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "context without --budget must keep working: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
