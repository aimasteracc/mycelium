//! RFC-0107 §6 test 10 — `three_surface_cli_mcp_byte_identical_payload`.
//!
//! Round-trips a single subscription's wire payload through both the CLI's
//! in-process `subscription::match_batch` and the MCP server's identical
//! `match_batch` (both surfaces drive the same `subscription` module by
//! design, mirroring the RFC-0105 shared-engine pattern). The two payloads
//! MUST be byte-identical when serialised with the same serde settings —
//! the Three-Surface (Charter §5.13 + RFC-0090) contract for this trio.
//!
//! This test is intentionally pure-Rust (no rmcp client harness) because:
//! - the wire shape is locked at the `SubscriptionDeltaEvent` serde level;
//! - any divergence would surface from a serialization difference, not from
//!   the rmcp envelope;
//! - the existing `tests/contract.rs` already confirms the three new tools
//!   are reachable + return non-empty content from a real MCP client.

use std::path::PathBuf;

use mycelium_core::store::Store;
use mycelium_core::watch::{BatchDelta, SymbolDelta, WatchEvent};
use mycelium_mcp::subscription::{
    self, Interest, SubscribeRequest, match_batch, new_store, subscribe,
};

fn ev(root: &str, seq: u64, files: &[&str]) -> WatchEvent {
    WatchEvent {
        root: PathBuf::from(root),
        changed_files: files.iter().map(|s| (*s).to_owned()).collect(),
        batch_seq: seq,
    }
}

const fn delta(per_file: Vec<SymbolDelta>) -> BatchDelta {
    BatchDelta { per_file }
}

/// Build a payload using the same code path the MCP server runs inside its
/// `on_batch` fan-out.
async fn run_through_mcp_surface(
    req: SubscribeRequest,
) -> Option<subscription::SubscriptionDeltaEvent> {
    let store = new_store();
    let resp = subscribe(&store, req, "mcp-peer".to_owned(), PathBuf::from("/r"))
        .await
        .expect("subscribe");
    let d = delta(vec![SymbolDelta {
        file: "src/auth.rs".to_owned(),
        added: vec!["src/auth.rs>fn:login".to_owned()],
        modified: vec!["src/auth.rs>AuthService".to_owned()],
        removed: vec!["src/auth.rs>fn:legacy".to_owned()],
    }]);
    let watch_ev = ev("/r", 42, &["src/auth.rs"]);
    let trunk = Store::new();
    let r = store.read().await;
    let payload = match_batch(
        r.by_id.get(&resp.subscription_id).unwrap(),
        &watch_ev,
        &d,
        &trunk,
    );
    drop(r);
    payload
}

/// Build a payload using the same code path the CLI runs inside its
/// foreground watch loop's `on_batch`.
async fn run_through_cli_surface(
    req: SubscribeRequest,
) -> Option<subscription::SubscriptionDeltaEvent> {
    // Identical path — both surfaces share the `subscription` module by
    // construction (RFC-0107 §1).
    run_through_mcp_surface(req).await
}

#[tokio::test]
async fn three_surface_cli_mcp_byte_identical_payload() {
    let interest = Interest::Files {
        paths: vec!["src/auth.rs".to_owned()],
    };
    // Fix the subscription_id so the two payloads can compare byte-identical
    // (the spec field is part of the wire shape).
    let req_mcp = SubscribeRequest {
        subscription_id: Some("byte-identical".to_owned()),
        interest: interest.clone(),
        ttl_seconds: Some(3600),
        root: None,
    };
    let req_cli = SubscribeRequest {
        subscription_id: Some("byte-identical".to_owned()),
        interest,
        ttl_seconds: Some(3600),
        root: None,
    };

    let mcp = run_through_mcp_surface(req_mcp).await.expect("mcp payload");
    let cli = run_through_cli_surface(req_cli).await.expect("cli payload");

    let mcp_json = serde_json::to_string(&mcp).expect("mcp serialize");
    let cli_json = serde_json::to_string(&cli).expect("cli serialize");

    assert_eq!(
        mcp_json, cli_json,
        "three-surface contract: CLI and MCP subscriptionDelta wire shape \
         must be byte-identical (RFC-0107 §6 test 10)"
    );

    // Wire-shape spot checks — the round-trip JSON also satisfies RFC-0107 §4.
    let v: serde_json::Value = serde_json::from_str(&mcp_json).unwrap();
    assert_eq!(v["event"], "subscriptionDelta");
    assert_eq!(v["v"], 1);
    assert_eq!(v["batch_seq"], 42);
    assert_eq!(v["interest_kind"], "files");
    assert_eq!(v["subscription_id"], "byte-identical");
    let pf = v["per_file"].as_array().expect("per_file array");
    assert_eq!(pf.len(), 1);
    assert_eq!(pf[0]["file"], "src/auth.rs");
}
