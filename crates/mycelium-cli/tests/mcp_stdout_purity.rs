//! Regression test for issue #150: `mycelium serve --mcp` must emit only
//! JSON-RPC frames on stdout. All tracing/log output goes to stderr.
//!
//! Without this guard, a strict MCP client cannot consume `mycelium serve`
//! because the stream is malformed by interleaved INFO log records.

use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

/// Path to the compiled `mycelium` binary set by Cargo for integration tests.
fn mycelium_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mycelium"))
}

/// Build a minimal indexed project under a temp root so the server has
/// something to load. Returns the temp dir handle (drop deletes the dir).
fn prepare_indexed_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create tempdir");
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn hello() -> &'static str { \"hi\" }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"x\"\nversion=\"0.0.0\"\nedition=\"2021\"\n",
    )
    .unwrap();

    // Build the index so `serve --mcp` has a snapshot to load.
    let status = Command::new(mycelium_bin())
        .args(["index", root.to_str().unwrap()])
        .status()
        .expect("spawn mycelium index");
    assert!(status.success(), "mycelium index failed");

    dir
}

#[test]
fn serve_mcp_stdout_is_pure_jsonrpc() {
    let project = prepare_indexed_project();

    // Spawn `mycelium serve --mcp --root <tmp>`.
    let mut child = Command::new(mycelium_bin())
        .args(["serve", "--mcp", "--root", project.path().to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("RUST_LOG")
        .spawn()
        .expect("spawn mycelium serve --mcp");

    // Send a single initialize request so the server emits at least one
    // JSON-RPC response on stdout.
    {
        let stdin = child.stdin.as_mut().expect("stdin pipe");
        let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"stdout-purity-test","version":"0.1"}}}"#;
        stdin.write_all(init.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.flush().unwrap();
    }

    // Give the server up to 5 s to respond.
    let stdout = child.stdout.take().expect("stdout pipe");
    let reader = BufReader::new(stdout);

    // Read stdout lines from a thread so we can time-box the wait.
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        for line in reader.lines() {
            if let Ok(line) = line {
                if tx.send(line).is_err() {
                    break;
                }
            } else {
                break;
            }
        }
    });

    // Collect every line that appears within 5 s.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut lines: Vec<String> = Vec::new();
    while std::time::Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => {
                lines.push(line);
                // After the first response, give the server 200 ms grace to
                // flush stragglers, drain whatever else is buffered, and stop
                // waiting. We have enough data to assert.
                thread::sleep(Duration::from_millis(200));
                while let Ok(l) = rx.try_recv() {
                    lines.push(l);
                }
                break;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    assert!(
        !lines.is_empty(),
        "serve --mcp produced no stdout in 5 s — expected at least one JSON-RPC initialize response"
    );

    // Every non-empty stdout line MUST parse as a JSON object.
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(trimmed);
        assert!(
            parsed.is_ok(),
            "stdout line {i} is not valid JSON (issue #150 regression):\n  {trimmed:?}\n\
             All stdout lines from `mycelium serve --mcp` must be JSON-RPC frames; \
             tracing/log output must go to stderr."
        );
    }
}
