//! RFC-0120 Phase 3B — byte-identity harness for `mycelium get-token-stats`.
//!
//! Spawns the compiled `mycelium get-token-stats` binary, captures stdout,
//! calls `mycelium_mcp::token_bench::token_stats_payload()` in-process, and
//! asserts the two JSON values are structurally identical.
//!
//! This is the concrete harness RFC-0120 §Design specified: spawns the compiled
//! `mycelium token-stats` binary, captures stdout, calls the MCP tool body in-process
//! over the same core, and `assert_eq!`s the two JSON values.

use std::{path::PathBuf, process::Command};

fn mycelium_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mycelium"))
}

#[test]
fn get_token_stats_exits_zero() {
    let out = Command::new(mycelium_bin())
        .args(["get-token-stats"])
        .output()
        .expect("failed to spawn mycelium");
    assert!(
        out.status.success(),
        "exit {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn get_token_stats_byte_identity_with_mcp_body() {
    let out = Command::new(mycelium_bin())
        .args(["get-token-stats"])
        .output()
        .expect("failed to spawn mycelium");
    assert!(
        out.status.success(),
        "exit {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    let cli_stdout = String::from_utf8(out.stdout).expect("stdout is valid UTF-8");
    let cli_json: serde_json::Value =
        serde_json::from_str(cli_stdout.trim()).expect("CLI output is valid JSON");

    let mcp_json = mycelium_mcp::token_bench::token_stats_payload();

    assert_eq!(
        cli_json, mcp_json,
        "CLI and MCP token-stats payloads must be byte-identical"
    );
}

#[test]
fn get_token_stats_output_has_required_keys() {
    let out = Command::new(mycelium_bin())
        .args(["get-token-stats"])
        .output()
        .expect("failed to spawn mycelium");
    assert!(out.status.success());

    let json: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();

    assert!(json["tokenizer"].is_string(), "missing tokenizer");
    assert!(json["corpus_version"].is_string(), "missing corpus_version");
    assert!(json["fixtures"].is_array(), "missing fixtures");
    assert!(
        json["aggregate_json_tokens"].is_number(),
        "missing aggregate_json_tokens"
    );
    assert!(
        json["aggregate_text_tokens"].is_number(),
        "missing aggregate_text_tokens"
    );
    assert!(
        json["text_to_json_token_ratio"].is_number(),
        "missing text_to_json_token_ratio"
    );
    assert!(
        json["token_reduction_pct"].is_number(),
        "missing token_reduction_pct"
    );
    assert!(
        json["wire_format_byte_ratio"].is_number(),
        "missing wire_format_byte_ratio"
    );
}

#[test]
fn get_token_stats_two_axes_are_distinct() {
    let out = Command::new(mycelium_bin())
        .args(["get-token-stats"])
        .output()
        .expect("failed to spawn mycelium");
    assert!(out.status.success());

    let json: serde_json::Value =
        serde_json::from_str(String::from_utf8(out.stdout).unwrap().trim()).unwrap();

    let token_ratio = json["text_to_json_token_ratio"].as_f64().unwrap();
    let byte_ratio = json["wire_format_byte_ratio"].as_f64().unwrap();

    assert!(
        (token_ratio - byte_ratio).abs() > 0.001,
        "token ratio ({token_ratio}) and byte ratio ({byte_ratio}) must be on distinct axes"
    );
}
