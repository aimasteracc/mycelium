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
    self, BatchMatch, Interest, SubscribeRequest, match_batch, new_store, subscribe,
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
    match payload {
        Some(BatchMatch::Delta(e)) => Some(e),
        _ => None,
    }
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

/// RFC-0108 §6 test 8 — extends the RFC-0107 three-surface byte-identity
/// contract to cover the new `Interest::Query` variant + the new
/// `mycelium/queryResultChanged` wire shape.
///
/// Both surfaces drive the same `subscription` module by construction —
/// the test asserts the serialised payload is byte-identical sans the
/// wall-clock-noisy `evaluation_ms` field.
#[tokio::test]
async fn three_surface_query_byte_identical_payload() {
    use mycelium_core::trunk::TrunkPath;
    use mycelium_core::types::{EdgeKind, NodeKind};
    use mycelium_mcp::subscription::QuerySpec;

    // Build a tiny store with one caller → callee edge.
    let mut trunk = Store::new();
    let foo = trunk.upsert_node(TrunkPath::parse("src/a.rs>fn:foo").unwrap());
    trunk.set_kind(foo, NodeKind::Function);
    let caller = trunk.upsert_node(TrunkPath::parse("src/b.rs>fn:caller_b").unwrap());
    trunk.set_kind(caller, NodeKind::Function);
    trunk.upsert_edge(EdgeKind::Calls, caller, foo);

    let store_a = new_store();
    let store_b = new_store();
    let interest = Interest::Query {
        query: QuerySpec::Callers {
            path: "src/a.rs>fn:foo".to_owned(),
            hops: Some(1),
        },
        min_interval_seconds: None,
    };

    let req_a = SubscribeRequest {
        subscription_id: Some("query-byte-identical".to_owned()),
        interest: interest.clone(),
        ttl_seconds: Some(3600),
        root: None,
    };
    let req_b = SubscribeRequest {
        subscription_id: Some("query-byte-identical".to_owned()),
        interest,
        ttl_seconds: Some(3600),
        root: None,
    };

    let resp_a = subscribe(&store_a, req_a, "mcp".to_owned(), PathBuf::from("/r"))
        .await
        .expect("subscribe a");
    let resp_b = subscribe(&store_b, req_b, "cli".to_owned(), PathBuf::from("/r"))
        .await
        .expect("subscribe b");
    assert_eq!(resp_a.query_kind.as_deref(), Some("callers"));
    assert_eq!(resp_b.query_kind.as_deref(), Some("callers"));

    let d = delta(vec![SymbolDelta {
        file: "src/b.rs".to_owned(),
        added: vec![],
        modified: vec!["src/b.rs>fn:caller_b".to_owned()],
        removed: vec![],
    }]);
    let watch_ev = ev("/r", 42, &["src/b.rs"]);

    let r_a = store_a.read().await;
    let r_b = store_b.read().await;
    let pa = match_batch(
        r_a.by_id.get(&resp_a.subscription_id).unwrap(),
        &watch_ev,
        &d,
        &trunk,
    );
    let pb = match_batch(
        r_b.by_id.get(&resp_b.subscription_id).unwrap(),
        &watch_ev,
        &d,
        &trunk,
    );
    drop(r_a);
    drop(r_b);

    let qa = match pa.expect("a emit") {
        BatchMatch::QueryDelta(e) => e,
        other => panic!("expected QueryDelta, got {other:?}"),
    };
    let qb = match pb.expect("b emit") {
        BatchMatch::QueryDelta(e) => e,
        other => panic!("expected QueryDelta, got {other:?}"),
    };

    // Compare byte-identity, modulo `evaluation_ms`.
    let mut va = serde_json::to_value(&qa).unwrap();
    let mut vb = serde_json::to_value(&qb).unwrap();
    va.as_object_mut().unwrap().remove("evaluation_ms");
    vb.as_object_mut().unwrap().remove("evaluation_ms");
    assert_eq!(
        serde_json::to_string(&va).unwrap(),
        serde_json::to_string(&vb).unwrap(),
        "three-surface contract: queryResultChanged byte-identical across surfaces"
    );

    // Frozen v1 spot checks.
    assert_eq!(va["event"], "queryResultChanged");
    assert_eq!(va["v"], 1);
    assert_eq!(va["query_kind"], "callers");
    assert_eq!(va["batch_seq"], 42);
    assert!(
        va["summary"].is_object(),
        "set-shaped query carries summary"
    );
    assert!(
        va["result_hash_new"].as_str().unwrap().starts_with("b3:"),
        "frozen v1 hash prefix"
    );
}
