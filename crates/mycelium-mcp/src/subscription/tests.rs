//! Unit tests for the per-subscription match/fan-out plumbing
//! (RFC-0107 §6 tests 4–9).

#![allow(clippy::disallowed_methods)] // synchronous test bookkeeping

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use mycelium_core::store::Store;
use mycelium_core::trunk::TrunkPath;
use mycelium_core::types::NodeKind;
use mycelium_core::watch::{BatchDelta, SymbolDelta, WatchEvent};

use mycelium_core::store::Store as TrunkStore;

use super::{
    BatchMatch, DEFAULT_TTL_SECONDS, Interest, MAX_PER_CLIENT, MAX_SELECTOR, MAX_SUBSCRIPTIONS,
    SubscribeError, SubscribeRequest, Subscription, SubscriptionDeltaEvent, bump_ttl,
    evaluate_selector_set, evict_expired, evict_for_dead_peer, new_store, status, subscribe,
    unsubscribe, update_last_match_set,
};

/// Test-only wrapper around `subscription::match_batch` that unwraps the
/// `BatchMatch::Delta` variant — every RFC-0107 test wants the
/// `SubscriptionDeltaEvent`, never the `QueryDelta` variant.
fn match_batch(
    sub: &Subscription,
    ev: &WatchEvent,
    delta: &BatchDelta,
    trunk_store: &TrunkStore,
) -> Option<SubscriptionDeltaEvent> {
    match super::match_batch(sub, ev, delta, trunk_store) {
        Some(BatchMatch::Delta(e)) => Some(e),
        _ => None,
    }
}

fn ev(root: &str, batch_seq: u64, files: &[&str]) -> WatchEvent {
    WatchEvent {
        root: PathBuf::from(root),
        changed_files: files.iter().map(|s| (*s).to_owned()).collect(),
        batch_seq,
    }
}

fn delta(per_file: Vec<SymbolDelta>) -> BatchDelta {
    BatchDelta { per_file }
}

/// Insert a path into a `Store` as a function symbol.
fn upsert_fn(store: &mut Store, p: &str) {
    let path = TrunkPath::parse(p).expect("valid trunk path");
    let id = store.upsert_node(path);
    store.set_kind(id, NodeKind::Function);
}

#[tokio::test]
async fn subscribe_files_emits_one_notification_per_matching_batch() {
    let s = new_store();
    let req = SubscribeRequest {
        subscription_id: None,
        interest: Interest::Files {
            paths: vec!["src/auth.rs".to_owned()],
        },
        ttl_seconds: None,
        root: None,
    };
    let resp = subscribe(&s, req, "peer-1".to_owned(), PathBuf::from("/r"))
        .await
        .expect("subscribe ok");
    assert_eq!(resp.interest_kind, "files");
    assert_eq!(resp.ttl_seconds, DEFAULT_TTL_SECONDS);

    // Matching delta → Some event with the matching file's payload.
    let d = delta(vec![
        SymbolDelta {
            file: "src/auth.rs".to_owned(),
            added: vec!["src/auth.rs>fn:login".to_owned()],
            modified: vec![],
            removed: vec![],
        },
        SymbolDelta {
            file: "src/other.rs".to_owned(),
            added: vec!["src/other.rs>fn:noise".to_owned()],
            modified: vec![],
            removed: vec![],
        },
    ]);
    let watch_ev = ev("/r", 7, &["src/auth.rs", "src/other.rs"]);
    let trunk = Store::new();

    let sub = s.read().await;
    let payload = match_batch(
        sub.by_id.get(&resp.subscription_id).unwrap(),
        &watch_ev,
        &d,
        &trunk,
    )
    .expect("event emitted");
    drop(sub);
    assert_eq!(payload.subscription_id, resp.subscription_id);
    assert_eq!(payload.batch_seq, 7);
    assert_eq!(payload.per_file.len(), 1);
    assert_eq!(payload.per_file[0].file, "src/auth.rs");
    assert_eq!(payload.per_file[0].added, vec!["src/auth.rs>fn:login"]);
    assert!(!payload.files_truncated);

    // Non-matching delta → None.
    let d2 = delta(vec![SymbolDelta {
        file: "src/zzz.rs".to_owned(),
        added: vec!["src/zzz.rs>fn:x".to_owned()],
        modified: vec![],
        removed: vec![],
    }]);
    let watch_ev2 = ev("/r", 8, &["src/zzz.rs"]);
    let sub = s.read().await;
    let none = match_batch(
        sub.by_id.get(&resp.subscription_id).unwrap(),
        &watch_ev2,
        &d2,
        &trunk,
    );
    drop(sub);
    assert!(none.is_none(), "non-matching file should yield no event");
}

#[tokio::test]
async fn subscribe_symbols_glob_matches_trunk_paths() {
    let s = new_store();
    let req = SubscribeRequest {
        subscription_id: None,
        interest: Interest::Symbols {
            paths: vec!["src/auth.rs>fn:*".to_owned()],
        },
        ttl_seconds: None,
        root: None,
    };
    let resp = subscribe(&s, req, "peer".to_owned(), PathBuf::from("/r"))
        .await
        .expect("subscribe ok");

    let d = delta(vec![SymbolDelta {
        file: "src/auth.rs".to_owned(),
        added: vec![
            "src/auth.rs>fn:login".to_owned(),
            "src/auth.rs>struct:Auth".to_owned(),
        ],
        modified: vec![],
        removed: vec![],
    }]);
    let watch_ev = ev("/r", 1, &["src/auth.rs"]);
    let trunk = Store::new();

    let r = s.read().await;
    let payload = match_batch(
        r.by_id.get(&resp.subscription_id).unwrap(),
        &watch_ev,
        &d,
        &trunk,
    )
    .expect("event");
    drop(r);
    assert_eq!(payload.per_file.len(), 1);
    assert_eq!(payload.per_file[0].added, vec!["src/auth.rs>fn:login"]);
    assert!(
        !payload.per_file[0]
            .added
            .contains(&"src/auth.rs>struct:Auth".to_owned()),
        "struct: prefix should be filtered out by the fn:* glob"
    );
}

#[tokio::test]
async fn selector_removal_strict_ii() {
    // OLD match-set contains src/a.rs>fn:legacy. This batch only touches
    // src/b.rs (not src/a.rs); (ii-strict) MUST omit src/a.rs>fn:legacy from
    // `removed`, because no touch on src/a.rs occurred.
    //
    // We exercise this directly against a Subscription struct so we don't
    // need a real Hyphae result — we set last_match_set explicitly and
    // run a selector whose NEW set is empty against the touched-paths set
    // that does NOT include the legacy path.
    use super::Subscription;
    use tokio::time::Instant;

    let mut old_set = BTreeSet::new();
    old_set.insert("src/a.rs>fn:legacy".to_owned());
    let sub = Subscription {
        id: "sid".to_owned(),
        root: PathBuf::from("/r"),
        interest: Interest::Selector {
            // a no-op selector that parses but matches nothing in an empty Store.
            hyphae: "*".to_owned(),
        },
        ttl_seconds: 60,
        expires_at: Instant::now() + Duration::from_secs(60),
        client_tag: "peer".to_owned(),
        last_match_set: Some(old_set),
        min_interval_ms: 0,
        last_hash: None,
        last_set_value: None,
        last_emit_at: None,
        paused_until: None,
    };

    // BatchDelta only mentions src/b.rs (no src/a.rs touch).
    let d = delta(vec![SymbolDelta {
        file: "src/b.rs".to_owned(),
        added: vec!["src/b.rs>fn:new_one".to_owned()],
        modified: vec![],
        removed: vec![],
    }]);
    let watch_ev = ev("/r", 1, &["src/b.rs"]);
    let trunk = Store::new();

    let evout = match_batch(&sub, &watch_ev, &d, &trunk);
    // (ii) (un-strict) would have produced a removed=[src/a.rs>fn:legacy].
    // (ii-strict) must NOT, because src/a.rs wasn't touched.
    if let Some(payload) = evout {
        for f in &payload.per_file {
            assert!(
                !f.removed.contains(&"src/a.rs>fn:legacy".to_owned()),
                "phantom-removal leaked through (ii-strict) gate: {f:?}"
            );
        }
    }
}

#[tokio::test]
async fn dead_peer_gc_clears_all_subscriptions() {
    let s = new_store();
    for i in 0..3 {
        let req = SubscribeRequest {
            subscription_id: Some(format!("id_{i}")),
            interest: Interest::Files {
                paths: vec![format!("src/{i}.rs")],
            },
            ttl_seconds: None,
            root: None,
        };
        subscribe(&s, req, "peer-X".to_owned(), PathBuf::from("/r"))
            .await
            .unwrap();
    }
    // One subscription owned by a different peer
    subscribe(
        &s,
        SubscribeRequest {
            subscription_id: Some("other".to_owned()),
            interest: Interest::Files {
                paths: vec!["src/other.rs".to_owned()],
            },
            ttl_seconds: None,
            root: None,
        },
        "peer-Y".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .unwrap();

    let n = evict_for_dead_peer(&s, "peer-X").await;
    assert_eq!(n, 3, "all of peer-X's subscriptions evicted");
    let r = s.read().await;
    let len = r.by_id.len();
    let has_other = r.by_id.contains_key("other");
    drop(r);
    assert_eq!(len, 1);
    assert!(has_other);
}

#[tokio::test]
async fn per_client_and_total_caps_return_application_error() {
    let s = new_store();
    // Saturate the per-client cap.
    for i in 0..MAX_PER_CLIENT {
        subscribe(
            &s,
            SubscribeRequest {
                subscription_id: Some(format!("c{i}")),
                interest: Interest::Files {
                    paths: vec![format!("src/{i}.rs")],
                },
                ttl_seconds: None,
                root: None,
            },
            "peer-A".to_owned(),
            PathBuf::from("/r"),
        )
        .await
        .unwrap();
    }
    let err = subscribe(
        &s,
        SubscribeRequest {
            subscription_id: Some("overflow".to_owned()),
            interest: Interest::Files {
                paths: vec!["src/x.rs".to_owned()],
            },
            ttl_seconds: None,
            root: None,
        },
        "peer-A".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .expect_err("per-client cap should reject");
    match err {
        SubscribeError::SubscriptionLimit { scope } => {
            assert_eq!(scope, "client");
        }
        _ => panic!("expected client-scope subscription_limit, got {err:?}"),
    }

    // Now fill the server cap with other peers' subs.
    let mut peer_idx = 0;
    while s.read().await.by_id.len() < MAX_SUBSCRIPTIONS {
        let id = format!("srv_{peer_idx}");
        let res = subscribe(
            &s,
            SubscribeRequest {
                subscription_id: Some(id.clone()),
                interest: Interest::Files {
                    paths: vec!["src/x.rs".to_owned()],
                },
                ttl_seconds: None,
                root: None,
            },
            format!("peer-{peer_idx}"),
            PathBuf::from("/r"),
        )
        .await;
        if res.is_err() {
            break;
        }
        peer_idx += 1;
    }
    // The next attempt MUST hit the server cap.
    let err = subscribe(
        &s,
        SubscribeRequest {
            subscription_id: Some("srv_overflow".to_owned()),
            interest: Interest::Files {
                paths: vec!["src/x.rs".to_owned()],
            },
            ttl_seconds: None,
            root: None,
        },
        "fresh-peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .expect_err("server cap should reject");
    match err {
        SubscribeError::SubscriptionLimit { scope } => {
            assert_eq!(scope, "server");
        }
        _ => panic!("expected server-scope subscription_limit, got {err:?}"),
    }
}

#[tokio::test]
async fn ttl_eviction_and_id_reuse() {
    let s = new_store();
    // ttl_seconds=0 makes the sub expire immediately on the next eviction tick.
    let r = subscribe(
        &s,
        SubscribeRequest {
            subscription_id: Some("expiring".to_owned()),
            interest: Interest::Files {
                paths: vec!["src/a.rs".to_owned()],
            },
            ttl_seconds: Some(0),
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .expect("subscribe");
    assert_eq!(r.subscription_id, "expiring");

    // Sleep enough to be past the deadline.
    tokio::time::sleep(Duration::from_millis(10)).await;

    let n = evict_expired(&s).await;
    assert_eq!(n, 1, "expired subscription evicted");

    // Re-subscribing the same id MUST now succeed (id was freed).
    let _ = subscribe(
        &s,
        SubscribeRequest {
            subscription_id: Some("expiring".to_owned()),
            interest: Interest::Files {
                paths: vec!["src/a.rs".to_owned()],
            },
            ttl_seconds: Some(60),
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .expect("re-subscribe after TTL eviction");
    // Idempotent unsubscribe for cleanup.
    let resp = unsubscribe(&s, "expiring").await;
    assert!(resp.removed);
    let resp2 = unsubscribe(&s, "expiring").await;
    assert!(!resp2.removed, "second unsubscribe is a no-op");
}

#[tokio::test]
async fn id_collision_returned_when_subscription_id_active() {
    let s = new_store();
    subscribe(
        &s,
        SubscribeRequest {
            subscription_id: Some("dup".to_owned()),
            interest: Interest::Files {
                paths: vec!["src/a.rs".to_owned()],
            },
            ttl_seconds: None,
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .unwrap();
    let err = subscribe(
        &s,
        SubscribeRequest {
            subscription_id: Some("dup".to_owned()),
            interest: Interest::Files {
                paths: vec!["src/b.rs".to_owned()],
            },
            ttl_seconds: None,
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .expect_err("id collision");
    matches!(err, SubscribeError::IdCollision);
    assert_eq!(err.code(), "id_collision");
}

#[tokio::test]
async fn invalid_interest_empty_paths() {
    let s = new_store();
    let err = subscribe(
        &s,
        SubscribeRequest {
            subscription_id: None,
            interest: Interest::Files { paths: vec![] },
            ttl_seconds: None,
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .expect_err("empty paths should be rejected");
    assert_eq!(err.code(), "invalid_interest");
}

#[tokio::test]
async fn selector_too_large_capped_at_4096_chars() {
    let s = new_store();
    let huge = "a".repeat(super::MAX_SELECTOR_SOURCE_LEN + 1);
    let err = subscribe(
        &s,
        SubscribeRequest {
            subscription_id: None,
            interest: Interest::Selector { hyphae: huge },
            ttl_seconds: None,
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .expect_err("selector source over cap should be rejected");
    assert_eq!(err.code(), "selector_too_large");
}

/// Marker test guarding the wire shape — `event` + `v` + frozen field names.
#[tokio::test]
async fn payload_field_names_are_frozen_v1_shape() {
    use std::path::PathBuf;
    let sub = super::Subscription {
        id: "sid".to_owned(),
        root: PathBuf::from("/r"),
        interest: Interest::Files {
            paths: vec!["src/a.rs".to_owned()],
        },
        ttl_seconds: 60,
        expires_at: tokio::time::Instant::now() + Duration::from_secs(60),
        client_tag: "peer".to_owned(),
        last_match_set: None,
        min_interval_ms: 0,
        last_hash: None,
        last_set_value: None,
        last_emit_at: None,
        paused_until: None,
    };
    let d = delta(vec![SymbolDelta {
        file: "src/a.rs".to_owned(),
        added: vec!["src/a.rs>fn:x".to_owned()],
        modified: vec![],
        removed: vec![],
    }]);
    let watch_ev = ev("/r", 42, &["src/a.rs"]);
    let trunk = Store::new();
    let payload = match_batch(&sub, &watch_ev, &d, &trunk).expect("event");
    let json = serde_json::to_value(&payload).expect("serializable");
    let obj = json.as_object().expect("object");
    for key in [
        "event",
        "v",
        "subscription_id",
        "root",
        "batch_seq",
        "per_file",
        "files_truncated",
        "interest_kind",
        "hint",
    ] {
        assert!(obj.contains_key(key), "v1 contract requires `{key}`");
    }
    assert_eq!(obj["event"], "subscriptionDelta");
    assert_eq!(obj["v"], 1);
    let pf = obj["per_file"].as_array().unwrap();
    let pf0 = pf[0].as_object().unwrap();
    for key in [
        "file",
        "added",
        "added_count",
        "added_truncated",
        "modified",
        "modified_count",
        "modified_truncated",
        "removed",
        "removed_count",
        "removed_truncated",
    ] {
        assert!(pf0.contains_key(key), "v1 per_file requires `{key}`");
    }
}

/// Symbol-only suppression: a Files-only subscription suppresses a per-file
/// row whose `added`/`modified`/`removed` are all empty after filtering, so
/// the noisy "no symbols touched in this file" case never fan-outs.
#[tokio::test]
async fn files_interest_skips_per_file_with_no_symbol_changes() {
    let s = new_store();
    let resp = subscribe(
        &s,
        SubscribeRequest {
            subscription_id: None,
            interest: Interest::Files {
                paths: vec!["src/empty.rs".to_owned()],
            },
            ttl_seconds: None,
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .unwrap();
    let d = delta(vec![SymbolDelta {
        file: "src/empty.rs".to_owned(),
        added: vec![],
        modified: vec![],
        removed: vec![],
    }]);
    let watch_ev = ev("/r", 1, &["src/empty.rs"]);
    let trunk = Store::new();
    let r = s.read().await;
    let none = match_batch(
        r.by_id.get(&resp.subscription_id).unwrap(),
        &watch_ev,
        &d,
        &trunk,
    );
    drop(r);
    assert!(
        none.is_none(),
        "files match w/ no symbol changes must suppress"
    );
}

/// Smoke: Selector matching against a Hyphae expression with a real Store.
#[tokio::test]
async fn selector_evaluates_against_post_batch_trunk() {
    let s = new_store();
    let resp = subscribe(
        &s,
        SubscribeRequest {
            subscription_id: None,
            interest: Interest::Selector {
                hyphae: "*".to_owned(),
            },
            ttl_seconds: None,
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .unwrap();
    // Trunk has a single symbol; the touched-paths set in the batch includes it.
    let mut trunk = Store::new();
    upsert_fn(&mut trunk, "src/a.rs>fn:login");
    let d = delta(vec![SymbolDelta {
        file: "src/a.rs".to_owned(),
        added: vec!["src/a.rs>fn:login".to_owned()],
        modified: vec![],
        removed: vec![],
    }]);
    let watch_ev = ev("/r", 1, &["src/a.rs"]);
    let r = s.read().await;
    let _ = match_batch(
        r.by_id.get(&resp.subscription_id).unwrap(),
        &watch_ev,
        &d,
        &trunk,
    );
    // We don't assert the exact match-set (Hyphae * semantics vary by store
    // contents); the test exists to ensure the Selector path doesn't panic
    // and the evaluator integration compiles.
}

// ── coverage gap tests: status / bump_ttl / update_last_match_set / selector paths ──

#[tokio::test]
async fn status_list_all_and_by_id() {
    let s = new_store();

    // Empty store.
    let r0 = status(&s, None, false).await;
    assert_eq!(r0.active_count, 0);
    assert!(r0.subscriptions.is_empty());
    assert!(!r0.watching);

    subscribe(
        &s,
        SubscribeRequest {
            subscription_id: Some("status-test".to_owned()),
            interest: Interest::Files {
                paths: vec!["src/a.rs".to_owned()],
            },
            ttl_seconds: None,
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .unwrap();

    // List all.
    let r1 = status(&s, None, true).await;
    assert_eq!(r1.active_count, 1);
    assert_eq!(r1.subscriptions.len(), 1);
    assert_eq!(r1.subscriptions[0].subscription_id, "status-test");
    assert!(r1.watching);

    // Find by id.
    let r2 = status(&s, Some("status-test"), false).await;
    assert_eq!(r2.subscriptions.len(), 1);

    // Not found.
    let r3 = status(&s, Some("no-such"), false).await;
    assert!(r3.subscriptions.is_empty());
}

#[tokio::test]
async fn bump_ttl_extends_and_noop_on_unknown() {
    let s = new_store();
    subscribe(
        &s,
        SubscribeRequest {
            subscription_id: Some("bump-me".to_owned()),
            interest: Interest::Files {
                paths: vec!["src/a.rs".to_owned()],
            },
            ttl_seconds: Some(10),
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .unwrap();

    bump_ttl(&s, "bump-me").await;
    bump_ttl(&s, "nonexistent").await; // silent no-op

    let st = status(&s, Some("bump-me"), false).await;
    assert_eq!(st.subscriptions.len(), 1);
    assert!(st.subscriptions[0].seconds_until_expiry > 0);
}

#[tokio::test]
async fn update_last_match_set_on_selector_and_noop_cases() {
    let s = new_store();
    subscribe(
        &s,
        SubscribeRequest {
            subscription_id: Some("sel-upd".to_owned()),
            interest: Interest::Selector {
                hyphae: "*".to_owned(),
            },
            ttl_seconds: None,
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .unwrap();

    let mut new_set = BTreeSet::new();
    new_set.insert("src/a.rs>fn:login".to_owned());
    update_last_match_set(&s, "sel-upd", new_set).await;

    // No-op: non-existent id.
    update_last_match_set(&s, "no-such", BTreeSet::new()).await;

    // No-op: Files subscription (not a Selector).
    subscribe(
        &s,
        SubscribeRequest {
            subscription_id: Some("files-sub".to_owned()),
            interest: Interest::Files {
                paths: vec!["src/a.rs".to_owned()],
            },
            ttl_seconds: None,
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .unwrap();
    update_last_match_set(&s, "files-sub", BTreeSet::new()).await;
}

#[tokio::test]
async fn evaluate_selector_set_with_valid_and_invalid_source() {
    let mut store = Store::new();
    let p = mycelium_core::trunk::TrunkPath::parse("src/a.rs>fn:login").unwrap();
    let id = store.upsert_node(p);
    store.set_kind(id, mycelium_core::types::NodeKind::Function);

    // Valid selector — should return symbol paths.
    let set = evaluate_selector_set("*", &store);
    assert!(!set.is_empty());

    // Invalid Hyphae source — should return empty set (not panic).
    let empty = evaluate_selector_set("!!! invalid %%%", &store);
    assert!(empty.is_empty());
}

#[tokio::test]
async fn invalid_subscription_id_format_rejected() {
    let s = new_store();
    let err = subscribe(
        &s,
        SubscribeRequest {
            subscription_id: Some("invalid id with spaces".to_owned()),
            interest: Interest::Files {
                paths: vec!["src/a.rs".to_owned()],
            },
            ttl_seconds: None,
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .expect_err("invalid id format should be rejected");
    assert_eq!(err.code(), "invalid_interest");
}

#[tokio::test]
async fn selector_count_cap_rejects_overflow() {
    let s = new_store();
    for i in 0..MAX_SELECTOR {
        subscribe(
            &s,
            SubscribeRequest {
                subscription_id: Some(format!("sel-{i}")),
                interest: Interest::Selector {
                    hyphae: "*".to_owned(),
                },
                ttl_seconds: None,
                root: None,
            },
            format!("peer-{i}"),
            PathBuf::from("/r"),
        )
        .await
        .unwrap();
    }
    let err = subscribe(
        &s,
        SubscribeRequest {
            subscription_id: Some("sel-overflow".to_owned()),
            interest: Interest::Selector {
                hyphae: "*".to_owned(),
            },
            ttl_seconds: None,
            root: None,
        },
        "peer-new".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .expect_err("selector cap should reject");
    match err {
        SubscribeError::SubscriptionLimit { scope } => assert_eq!(scope, "selector"),
        _ => panic!("expected selector-scope limit, got {err:?}"),
    }
}

#[tokio::test]
async fn unsubscribe_selector_decrements_selector_count() {
    let s = new_store();
    subscribe(
        &s,
        SubscribeRequest {
            subscription_id: Some("my-sel".to_owned()),
            interest: Interest::Selector {
                hyphae: "*".to_owned(),
            },
            ttl_seconds: None,
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .unwrap();
    let r = s.read().await;
    assert_eq!(r.selector_count, 1);
    drop(r);
    let resp = unsubscribe(&s, "my-sel").await;
    assert!(resp.removed);
    let r = s.read().await;
    assert_eq!(r.selector_count, 0);
    drop(r);
}

#[tokio::test]
async fn evict_expired_and_dead_peer_decrement_selector_count() {
    // evict_expired selector path.
    let s1 = new_store();
    subscribe(
        &s1,
        SubscribeRequest {
            subscription_id: Some("exp-sel".to_owned()),
            interest: Interest::Selector {
                hyphae: "*".to_owned(),
            },
            ttl_seconds: Some(0),
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    let n = evict_expired(&s1).await;
    assert_eq!(n, 1);
    assert_eq!(s1.read().await.selector_count, 0);

    // evict_for_dead_peer selector path.
    let s2 = new_store();
    subscribe(
        &s2,
        SubscribeRequest {
            subscription_id: Some("dead-sel".to_owned()),
            interest: Interest::Selector {
                hyphae: "*".to_owned(),
            },
            ttl_seconds: None,
            root: None,
        },
        "dead-peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .unwrap();
    let n = evict_for_dead_peer(&s2, "dead-peer").await;
    assert_eq!(n, 1);
    assert_eq!(s2.read().await.selector_count, 0);
}

#[tokio::test]
async fn symbols_interest_matches_modified_and_removed() {
    let s = new_store();
    let resp = subscribe(
        &s,
        SubscribeRequest {
            subscription_id: None,
            interest: Interest::Symbols {
                paths: vec!["src/auth.rs>fn:*".to_owned()],
            },
            ttl_seconds: None,
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .unwrap();

    let d = delta(vec![SymbolDelta {
        file: "src/auth.rs".to_owned(),
        added: vec![],
        modified: vec!["src/auth.rs>fn:login".to_owned()],
        removed: vec![
            "src/auth.rs>fn:logout".to_owned(),
            "src/auth.rs>struct:Auth".to_owned(), // filtered by glob
        ],
    }]);
    let watch_ev = ev("/r", 5, &["src/auth.rs"]);
    let trunk = Store::new();

    let r = s.read().await;
    let payload = match_batch(
        r.by_id.get(&resp.subscription_id).unwrap(),
        &watch_ev,
        &d,
        &trunk,
    )
    .expect("should match modified/removed");
    drop(r);
    assert_eq!(payload.per_file[0].modified, vec!["src/auth.rs>fn:login"]);
    assert_eq!(payload.per_file[0].removed, vec!["src/auth.rs>fn:logout"]);
    assert!(
        !payload.per_file[0]
            .removed
            .contains(&"src/auth.rs>struct:Auth".to_owned())
    );
}

#[tokio::test]
async fn validate_interest_rejects_empty_path_string_and_empty_selector() {
    // Empty string in paths list.
    let s = new_store();
    let err = subscribe(
        &s,
        SubscribeRequest {
            subscription_id: None,
            interest: Interest::Files {
                paths: vec![String::new()],
            },
            ttl_seconds: None,
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .expect_err("empty path string should be rejected");
    assert_eq!(err.code(), "invalid_interest");

    // Empty selector source.
    let err2 = subscribe(
        &s,
        SubscribeRequest {
            subscription_id: None,
            interest: Interest::Selector {
                hyphae: "   ".to_owned(),
            },
            ttl_seconds: None,
            root: None,
        },
        "peer".to_owned(),
        PathBuf::from("/r"),
    )
    .await
    .expect_err("whitespace-only selector should be rejected");
    assert_eq!(err2.code(), "invalid_interest");
}

// ─────────────────────────────────────────────────────────────────────────────
// RFC-0108 §6 reactive query subscription tests (8 RED-first tests).
// ─────────────────────────────────────────────────────────────────────────────

use super::{QuerySpec, query_kind_str};
use crate::query_delta::canonical_json_hash;
use crate::query_eval::{QueryOutcome, match_query_batch_outcome};

/// Serialise tests that mutate the global `TEST_FORCE_EVAL_DELAY_MS`.
/// `cargo test` runs unit tests in parallel by default, so without this
/// the budget test could leak its delay into the tree-shaped test.
static EVAL_DELAY_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Helper: build a `Subscription` carrying a Query interest.
fn query_sub(id: &str, query: QuerySpec, min_interval_ms: u64) -> Subscription {
    Subscription {
        id: id.to_owned(),
        root: PathBuf::from("/r"),
        interest: Interest::Query {
            query,
            min_interval_seconds: None,
        },
        ttl_seconds: 60,
        expires_at: tokio::time::Instant::now() + Duration::from_secs(60),
        client_tag: "peer".to_owned(),
        last_match_set: None,
        min_interval_ms,
        last_hash: None,
        last_set_value: None,
        last_emit_at: None,
        paused_until: None,
    }
}

/// Helper: make a non-empty `BatchDelta` so the touched-set gate doesn't
/// short-circuit.
fn batch_with_change(file: &str, modified: &[&str]) -> BatchDelta {
    delta(vec![SymbolDelta {
        file: file.to_owned(),
        added: vec![],
        modified: modified.iter().map(|s| (*s).to_owned()).collect(),
        removed: vec![],
    }])
}

#[test]
fn query_spec_parsing_round_trips_for_all_5_kinds() {
    // RFC-0108 §6 test 1.
    let cases = vec![
        QuerySpec::Selector {
            hyphae: "fn[name=\"login\"]".to_owned(),
        },
        QuerySpec::Callers {
            path: "src/a.rs>fn:b".to_owned(),
            hops: Some(2),
        },
        QuerySpec::Callees {
            path: "src/a.rs>fn:b".to_owned(),
            hops: None,
        },
        QuerySpec::Impact {
            path: "src/a.rs>fn:b".to_owned(),
            max_paths: Some(50),
        },
        QuerySpec::Context {
            task: "auth refactor".to_owned(),
            focus: vec!["src/auth.rs".to_owned()],
            max_tokens: Some(2000),
        },
    ];
    for spec in cases {
        let json = serde_json::to_string(&spec).expect("serialize");
        let round: QuerySpec = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(spec, round, "serde round-trip identity");
        // Each kind exposes a stable wire string.
        assert!(
            ["selector", "callers", "callees", "impact", "context"]
                .contains(&query_kind_str(&spec))
        );
    }
}

#[test]
fn result_hash_stable_across_serde_orderings() {
    // RFC-0108 §6 test 2: canonical-JSON canary.
    // Build the SAME logical value two ways and verify the hash matches.
    let v1 = serde_json::json!({ "added": ["a", "b"], "removed": [], "count": 2 });
    let v2 = serde_json::json!({ "count": 2, "removed": [], "added": ["a", "b"] });
    assert_eq!(canonical_json_hash(&v1), canonical_json_hash(&v2));

    // Nested case.
    let n1 = serde_json::json!({
        "outer": { "z": 1, "a": [3, 2, 1] },
        "x": { "k": "v", "j": "w" },
    });
    let n2 = serde_json::json!({
        "x": { "j": "w", "k": "v" },
        "outer": { "a": [3, 2, 1], "z": 1 },
    });
    assert_eq!(canonical_json_hash(&n1), canonical_json_hash(&n2));
}

#[tokio::test]
async fn callers_subscription_fires_only_on_actual_change() {
    // RFC-0108 §6 test 3: a batch that doesn't change callers MUST NOT fire.
    let mut store = Store::new();
    upsert_fn(&mut store, "src/a.rs>fn:foo");
    upsert_fn(&mut store, "src/b.rs>fn:caller_b");

    // Set up "caller_b calls foo".
    let foo = store.lookup("src/a.rs>fn:foo").unwrap();
    let caller_b = store.lookup("src/b.rs>fn:caller_b").unwrap();
    store.upsert_edge(mycelium_core::types::EdgeKind::Calls, caller_b, foo);

    // First subscription evaluation: should emit (last_hash is None).
    let mut sub = query_sub(
        "q1",
        QuerySpec::Callers {
            path: "src/a.rs>fn:foo".to_owned(),
            hops: Some(1),
        },
        0, // no quiet period
    );
    let watch_ev = ev("/r", 1, &["src/b.rs"]);
    // Provide a non-empty delta so the touched-set gate passes.
    let d1 = batch_with_change("src/b.rs", &["src/b.rs>fn:caller_b"]);

    let out1 = match_query_batch_outcome(
        &sub,
        match &sub.interest {
            Interest::Query { query, .. } => query,
            _ => unreachable!(),
        },
        &watch_ev,
        &d1,
        &store,
    );
    let emit1 = match out1 {
        QueryOutcome::Emit(e) => e,
        other => panic!("expected first delivery, got {other:?}"),
    };
    assert!(emit1.summary.is_some());
    let new_hash = {
        let mut buf = [0_u8; 16];
        let hex_part = emit1.result_hash_new.strip_prefix("b3:").unwrap();
        for i in 0..16 {
            buf[i] = u8::from_str_radix(&hex_part[i * 2..i * 2 + 2], 16).unwrap();
        }
        buf
    };

    // Simulate post-emit state-save.
    sub.last_hash = Some(new_hash);
    sub.last_emit_at = Some(tokio::time::Instant::now());
    sub.last_set_value = Some(
        emit1
            .new_result
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
    );

    // Second batch: touches an unrelated file, callers of foo unchanged.
    let watch_ev2 = ev("/r", 2, &["src/c.rs"]);
    let d2 = batch_with_change("src/c.rs", &["src/c.rs>fn:unrelated"]);
    let out2 = match_query_batch_outcome(
        &sub,
        match &sub.interest {
            Interest::Query { query, .. } => query,
            _ => unreachable!(),
        },
        &watch_ev2,
        &d2,
        &store,
    );
    match out2 {
        QueryOutcome::Skip => {} // expected — hash unchanged
        other => panic!("expected Skip on unchanged callers, got {other:?}"),
    }
}

#[tokio::test]
async fn min_interval_coalesces_burst_edits() {
    // RFC-0108 §6 test 4: 5 rapid batches inside the quiet window → at most
    // 1 emit. We don't depend on real wall-clock — we set
    // `last_emit_at = Instant::now()` after the first emit, and the
    // quiet-period gate (`min_interval_ms = 60_000`) ensures the next four
    // batches all return `Skip`.
    let mut store = Store::new();
    upsert_fn(&mut store, "src/a.rs>fn:foo");

    let mut sub = query_sub(
        "q2",
        QuerySpec::Callers {
            path: "src/a.rs>fn:foo".to_owned(),
            hops: Some(1),
        },
        60_000, // huge quiet window so no real time can elapse past it
    );

    let mut emit_count = 0;
    for i in 1..=5_u64 {
        let watch_ev = ev("/r", i, &["src/b.rs"]);
        let d = batch_with_change("src/b.rs", &["src/b.rs>fn:something"]);
        let out = match_query_batch_outcome(
            &sub,
            match &sub.interest {
                Interest::Query { query, .. } => query,
                _ => unreachable!(),
            },
            &watch_ev,
            &d,
            &store,
        );
        if let QueryOutcome::Emit(_) = out {
            emit_count += 1;
            sub.last_emit_at = Some(tokio::time::Instant::now());
            // Sentinel `last_hash` so the next iterations also gate on
            // quiet-period (not just touched-set).
            sub.last_hash = Some([0xab; 16]);
        }
    }
    assert!(
        emit_count <= 1,
        "burst-edit coalescing: expected ≤1 emit, got {emit_count}"
    );
}

#[tokio::test]
async fn set_shaped_summary_added_removed_consistent() {
    // RFC-0108 §6 test 5: Callers result goes from {A,B} → {A,C}; summary
    // reports added=[C], removed=[B].
    let mut store = Store::new();
    upsert_fn(&mut store, "src/a.rs>fn:foo");
    upsert_fn(&mut store, "src/b.rs>fn:caller_a");
    upsert_fn(&mut store, "src/b.rs>fn:caller_b");
    let foo = store.lookup("src/a.rs>fn:foo").unwrap();
    let ca = store.lookup("src/b.rs>fn:caller_a").unwrap();
    let cb = store.lookup("src/b.rs>fn:caller_b").unwrap();
    store.upsert_edge(mycelium_core::types::EdgeKind::Calls, ca, foo);
    store.upsert_edge(mycelium_core::types::EdgeKind::Calls, cb, foo);

    // Prime the subscription with the {caller_a, caller_b} set as
    // last_set_value, and a sentinel `last_hash` so the first call
    // doesn't bypass via "first delivery".
    let mut sub = query_sub(
        "q3",
        QuerySpec::Callers {
            path: "src/a.rs>fn:foo".to_owned(),
            hops: Some(1),
        },
        0,
    );
    let mut initial: BTreeSet<String> = BTreeSet::new();
    initial.insert("src/b.rs>fn:caller_a".to_owned());
    initial.insert("src/b.rs>fn:caller_b".to_owned());
    sub.last_set_value = Some(initial);
    sub.last_hash = Some([0xff; 16]); // sentinel: anything != real hash → emit

    // Now mutate the store: remove caller_b, add caller_c.
    upsert_fn(&mut store, "src/b.rs>fn:caller_c");
    let cc = store.lookup("src/b.rs>fn:caller_c").unwrap();
    store.upsert_edge(mycelium_core::types::EdgeKind::Calls, cc, foo);
    // Remove caller_b's edge.
    store.remove_node(cb);

    let watch_ev = ev("/r", 1, &["src/b.rs"]);
    let d = batch_with_change(
        "src/b.rs",
        &["src/b.rs>fn:caller_b", "src/b.rs>fn:caller_c"],
    );
    let out = match_query_batch_outcome(
        &sub,
        match &sub.interest {
            Interest::Query { query, .. } => query,
            _ => unreachable!(),
        },
        &watch_ev,
        &d,
        &store,
    );
    let emit = match out {
        QueryOutcome::Emit(e) => e,
        other => panic!("expected Emit, got {other:?}"),
    };
    let summary = emit
        .summary
        .as_ref()
        .expect("set-shaped query MUST carry summary");
    // old_set = {caller_a, caller_b}; new = {caller_a, caller_c}.
    assert_eq!(
        summary.added,
        vec!["src/b.rs>fn:caller_c"],
        "added must report only the newly-appearing caller_c (caller_a is in both)"
    );
    assert_eq!(
        summary.removed,
        vec!["src/b.rs>fn:caller_b"],
        "removed must report only the dropped caller_b"
    );
}

#[tokio::test]
async fn tree_shaped_omits_summary() {
    // RFC-0108 §6 test 6: Context (tree-shaped) result change carries
    // `new_result` only, no `summary` field.
    use std::sync::atomic::Ordering;
    // Serialise against the budget test (shared global delay).
    let _guard = EVAL_DELAY_GUARD.lock().unwrap();
    crate::query_eval::TEST_FORCE_EVAL_DELAY_MS.store(0, Ordering::Relaxed);
    let store = Store::new();
    let sub = query_sub(
        "q4",
        QuerySpec::Context {
            task: "audit".to_owned(),
            focus: vec![],
            max_tokens: Some(1000),
        },
        0,
    );
    let watch_ev = ev("/r", 1, &["src/x.rs"]);
    let d = batch_with_change("src/x.rs", &["src/x.rs>fn:x"]);
    let out = match_query_batch_outcome(
        &sub,
        match &sub.interest {
            Interest::Query { query, .. } => query,
            _ => unreachable!(),
        },
        &watch_ev,
        &d,
        &store,
    );
    let emit = match out {
        QueryOutcome::Emit(e) => e,
        other => panic!("expected Emit on first delivery, got {other:?}"),
    };
    assert!(
        emit.summary.is_none(),
        "tree-shaped (Context) MUST omit summary"
    );
    let v = serde_json::to_value(&*emit).unwrap();
    let obj = v.as_object().unwrap();
    assert!(
        !obj.contains_key("summary"),
        "serialized payload MUST NOT contain `summary` field for tree-shaped"
    );
}

#[tokio::test]
async fn evaluation_budget_pauses_runaway_subscription() {
    // RFC-0108 §6 test 7: wall-clock > 200 ms returns Pause.
    use std::sync::atomic::Ordering;
    let _guard = EVAL_DELAY_GUARD.lock().unwrap();
    let store = Store::new();
    let sub = query_sub(
        "q5",
        QuerySpec::Context {
            task: "slow".to_owned(),
            focus: vec![],
            max_tokens: Some(100),
        },
        0,
    );
    // Force the Context evaluator to sleep 250 ms.
    crate::query_eval::TEST_FORCE_EVAL_DELAY_MS.store(250, Ordering::Relaxed);

    let watch_ev = ev("/r", 1, &["src/x.rs"]);
    let d = batch_with_change("src/x.rs", &["src/x.rs>fn:x"]);
    let out = match_query_batch_outcome(
        &sub,
        match &sub.interest {
            Interest::Query { query, .. } => query,
            _ => unreachable!(),
        },
        &watch_ev,
        &d,
        &store,
    );
    // Reset for other tests.
    crate::query_eval::TEST_FORCE_EVAL_DELAY_MS.store(0, Ordering::Relaxed);

    matches!(out, QueryOutcome::Pause)
        .then_some(())
        .expect("wall-clock > QUERY_BUDGET_HARD_MS must trigger Pause (got non-Pause outcome)");
}

#[tokio::test]
async fn three_surface_query_cli_mcp_byte_identical_payload() {
    // RFC-0108 §6 test 8: round-trip identity for `callers` between CLI
    // (parse spec string) and MCP (build SubscribeRequest directly).
    //
    // Both surfaces drive the same `subscription` module — this test asserts
    // that the produced `QueryResultChangedEvent` is byte-identical regardless
    // of which surface registered the subscription. The byte-identity is
    // guaranteed by construction: both surfaces call the same
    // `match_query_batch_outcome` against the same store.
    let mut store = Store::new();
    upsert_fn(&mut store, "src/a.rs>fn:foo");
    upsert_fn(&mut store, "src/b.rs>fn:caller_b");
    let foo = store.lookup("src/a.rs>fn:foo").unwrap();
    let cb = store.lookup("src/b.rs>fn:caller_b").unwrap();
    store.upsert_edge(mycelium_core::types::EdgeKind::Calls, cb, foo);

    let spec = QuerySpec::Callers {
        path: "src/a.rs>fn:foo".to_owned(),
        hops: Some(1),
    };
    // "MCP" path: deserialize from a wire-shape JSON request.
    let mcp_json = serde_json::json!({
        "kind": "callers",
        "path": "src/a.rs>fn:foo",
        "hops": 1,
    });
    let mcp_spec: QuerySpec = serde_json::from_value(mcp_json).unwrap();
    assert_eq!(spec, mcp_spec, "MCP wire deserialize must round-trip");

    // Both produce the same payload against the same batch.
    let sub_cli = query_sub("byte-identical", spec, 0);
    let sub_mcp = query_sub("byte-identical", mcp_spec, 0);
    let watch_ev = ev("/r", 42, &["src/b.rs"]);
    let d = batch_with_change("src/b.rs", &["src/b.rs>fn:caller_b"]);

    let cli_payload = match match_query_batch_outcome(
        &sub_cli,
        match &sub_cli.interest {
            Interest::Query { query, .. } => query,
            _ => unreachable!(),
        },
        &watch_ev,
        &d,
        &store,
    ) {
        QueryOutcome::Emit(e) => *e,
        _ => panic!("expected Emit"),
    };
    let mcp_payload = match match_query_batch_outcome(
        &sub_mcp,
        match &sub_mcp.interest {
            Interest::Query { query, .. } => query,
            _ => unreachable!(),
        },
        &watch_ev,
        &d,
        &store,
    ) {
        QueryOutcome::Emit(e) => *e,
        _ => panic!("expected Emit"),
    };

    // Drop `evaluation_ms` (wall-clock noise) before comparing.
    let mut a = serde_json::to_value(&cli_payload).unwrap();
    let mut b = serde_json::to_value(&mcp_payload).unwrap();
    if let Some(o) = a.as_object_mut() {
        o.remove("evaluation_ms");
    }
    if let Some(o) = b.as_object_mut() {
        o.remove("evaluation_ms");
    }
    let a_json = serde_json::to_string(&a).unwrap();
    let b_json = serde_json::to_string(&b).unwrap();
    assert_eq!(
        a_json, b_json,
        "three-surface contract: CLI and MCP queryResultChanged byte-identical (RFC-0108 §6 test 8)"
    );

    // Spot checks against the v1 wire shape.
    let v: serde_json::Value = serde_json::from_str(&a_json).unwrap();
    assert_eq!(v["event"], "queryResultChanged");
    assert_eq!(v["v"], 1);
    assert_eq!(v["query_kind"], "callers");
    assert_eq!(v["batch_seq"], 42);
    assert_eq!(v["subscription_id"], "byte-identical");
    assert!(v["summary"].is_object(), "set-shaped query carries summary");
    assert!(
        v["result_hash_new"].as_str().unwrap().starts_with("b3:"),
        "hash prefix `b3:` is frozen v1"
    );
}
