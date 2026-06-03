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

use super::{
    DEFAULT_TTL_SECONDS, Interest, MAX_PER_CLIENT, MAX_SUBSCRIPTIONS, SubscribeError,
    SubscribeRequest, evict_expired, evict_for_dead_peer, match_batch, new_store, subscribe,
    unsubscribe,
};

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
