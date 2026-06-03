//! Server-initiated `mycelium/queryResultChanged` notification (RFC-0108).
//!
//! Salsa Phase 2 — step 4/4 of the reactive-completion roadmap. Whereas
//! RFC-0107 emits a scoped per-file/symbol/selector delta, RFC-0108 emits
//! one notification per Query subscription per batch — and **only when the
//! query result actually changed** (Salsa-style backdated equality via a
//! BLAKE3-128 hash of the canonical-JSON result).
//!
//! Module shape mirrors `push.rs` / `subscription.rs`: frozen-at-v1
//! constants, `pub(super)` items, `into_custom_notification` builder. The
//! capture of `Peer`, the actual send, and the lib.rs wiring live next to
//! the rest of the watch plumbing.

// `pub(super)` items inside a private module are flagged as redundant by
// `clippy::redundant_pub_crate`. They're not redundant *here* — the module is
// intentionally private and the items are intentionally callable from `lib.rs`.
#![allow(clippy::redundant_pub_crate)]

use serde::{Deserialize, Serialize};

// ── frozen-at-v1 constants ───────────────────────────────────────────────────

/// Method name of the per-Query-subscription result-changed notification.
///
/// **Frozen at v1.** Changing this is a wire-contract break.
pub(super) const METHOD: &str = "mycelium/queryResultChanged";

/// Per-array cap on `summary.added` / `summary.removed`. Matches the
/// RFC-0107 [`super::subscription::MAX_PER_ARRAY`] discipline.
pub(super) const MAX_PER_ARRAY: usize = 50;

/// Prefix for the BLAKE3-128 hex string in `result_hash_*` fields.
///
/// **Frozen at v1** so clients can route on prefix alone.
pub(super) const HASH_PREFIX: &str = "b3:";

/// Default agent-facing hint string. Agents may ignore.
pub(super) const DEFAULT_HINT: &str =
    "Query result changed; re-fetch the affected slice if needed.";

// ── public-shape (wire) types ────────────────────────────────────────────────

/// Set-shaped result summary (D2 (ii) hybrid). Present iff the query is
/// naturally a set of trunk paths (Selector / Callers / Callees / Impact).
/// Omitted for tree-shaped results (Context).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetSummary {
    /// Trunk paths that appeared since the last delivery, capped at
    /// [`MAX_PER_ARRAY`].
    pub added: Vec<String>,
    /// Pre-cap count of `added`.
    pub added_count: u64,
    /// `true` when `added` was truncated to [`MAX_PER_ARRAY`].
    pub added_truncated: bool,
    /// Trunk paths that disappeared since the last delivery.
    pub removed: Vec<String>,
    /// Pre-cap count of `removed`.
    pub removed_count: u64,
    /// `true` when `removed` was truncated.
    pub removed_truncated: bool,
}

/// Wire-shape of the `mycelium/queryResultChanged` notification payload.
///
/// **Frozen at v1.** Any breaking change increments `v` and gets a new RFC.
/// Field order matches RFC-0108 §4 exactly.
///
/// Note: `new_result` is a `serde_json::Value` so this type cannot derive
/// `Eq` (values containing JSON `null` only support `PartialEq`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(clippy::derive_partial_eq_without_eq)]
pub struct QueryResultChangedEvent {
    /// Constant `"queryResultChanged"` — disambiguates from other Mycelium
    /// events.
    pub event: String,
    /// Schema version.
    pub v: u32,
    /// Subscription that produced this delta.
    pub subscription_id: String,
    /// Absolute path of the watched root.
    pub root: String,
    /// Monotonic batch counter from RFC-0105.
    pub batch_seq: u64,
    /// `"selector" | "callers" | "callees" | "impact" | "context"`.
    pub query_kind: String,
    /// BLAKE3-128 hex of the previous result, prefix `"b3:"`. `None` on the
    /// first delivery for this subscription.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_hash_old: Option<String>,
    /// BLAKE3-128 hex of the new result, prefix `"b3:"`.
    pub result_hash_new: String,
    /// The full query result. Shape depends on `query_kind` — for set-shaped
    /// queries this is a `Vec<String>` of trunk paths; for `context` it
    /// is the seven-key payload from `mycelium_context`.
    pub new_result: serde_json::Value,
    /// Present iff `query_kind` is set-shaped (Selector/Callers/Callees/
    /// Impact). Omitted entirely for tree-shaped results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<SetSummary>,
    /// Wall-clock evaluation cost in milliseconds (RFC-0108 §2 budget gate).
    pub evaluation_ms: u64,
    /// Free-text human-friendly suggestion.
    pub hint: String,
}

impl QueryResultChangedEvent {
    /// Build the rmcp `CustomNotification` envelope. Best-effort — returns
    /// `None` on serialization failure (effectively OOM only).
    #[must_use]
    pub(super) fn into_custom_notification(self) -> Option<rmcp::model::CustomNotification> {
        let params = serde_json::to_value(&self).ok()?;
        Some(rmcp::model::CustomNotification::new(METHOD, Some(params)))
    }
}

// ── canonical JSON hashing ──────────────────────────────────────────────────

/// Convert a `serde_json::Value` into its canonical-JSON byte form: object
/// keys sorted ascending; arrays preserved as-is; no whitespace.
///
/// Used to make the BLAKE3-128 hash stable across serde key-ordering quirks
/// (RFC-0108 §6 test 2 canary). Pure, recursive.
fn canonicalize(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(m) => {
            let mut sorted: std::collections::BTreeMap<String, serde_json::Value> =
                std::collections::BTreeMap::new();
            for (k, val) in m {
                sorted.insert(k.clone(), canonicalize(val));
            }
            let mut out = serde_json::Map::with_capacity(sorted.len());
            for (k, val) in sorted {
                out.insert(k, val);
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(a) => {
            serde_json::Value::Array(a.iter().map(canonicalize).collect())
        }
        other => other.clone(),
    }
}

/// Hash a `serde_json::Value` into a stable BLAKE3-128 (16 bytes).
///
/// Canonical-JSON discipline: object keys are sorted ascending before
/// serialization. Arrays are NOT sorted (semantics-preserving).
#[must_use]
pub(super) fn canonical_json_hash(v: &serde_json::Value) -> [u8; 16] {
    let canon = canonicalize(v);
    let bytes = serde_json::to_vec(&canon).unwrap_or_default();
    let full = blake3::hash(&bytes);
    let mut out = [0_u8; 16];
    out.copy_from_slice(&full.as_bytes()[..16]);
    out
}

/// Render a 16-byte hash as `"b3:<lowercase-hex>"`. **Frozen v1 prefix.**
#[must_use]
pub(super) fn hash_hex(bytes: &[u8; 16]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(HASH_PREFIX.len() + 32);
    s.push_str(HASH_PREFIX);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Cap-and-sort one set-summary array; returns `(vec, pre_cap, truncated)`.
#[must_use]
pub(super) fn cap_set(mut v: Vec<String>) -> (Vec<String>, u64, bool) {
    v.sort();
    v.dedup();
    let pre = v.len() as u64;
    let truncated = v.len() > MAX_PER_ARRAY;
    if truncated {
        v.truncate(MAX_PER_ARRAY);
    }
    (v, pre, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hash_prefix_is_frozen_v1() {
        let h = canonical_json_hash(&json!({"a": 1}));
        let s = hash_hex(&h);
        assert!(s.starts_with("b3:"), "wire prefix is `b3:` (v1 contract)");
        assert_eq!(s.len(), 3 + 32, "BLAKE3-128 hex = 32 chars + prefix");
    }

    #[test]
    fn canonical_hash_invariant_to_key_order() {
        // Two logically identical objects with keys in different insertion
        // orders MUST hash to the same bytes.
        let mut a = serde_json::Map::new();
        a.insert("z".into(), json!(1));
        a.insert("a".into(), json!(2));
        let mut b = serde_json::Map::new();
        b.insert("a".into(), json!(2));
        b.insert("z".into(), json!(1));
        let ha = canonical_json_hash(&serde_json::Value::Object(a));
        let hb = canonical_json_hash(&serde_json::Value::Object(b));
        assert_eq!(ha, hb, "canonical-JSON hash is key-order-invariant");
    }

    #[test]
    fn nested_canonical_hash_invariant() {
        let v1 = json!({ "outer": { "z": 1, "a": [3, 2, 1] }, "x": "y" });
        let v2 = json!({ "x": "y", "outer": { "a": [3, 2, 1], "z": 1 } });
        assert_eq!(canonical_json_hash(&v1), canonical_json_hash(&v2));
    }

    #[test]
    fn array_order_is_preserved_in_hash() {
        // Arrays are semantically ordered — same values in a different order
        // should produce a DIFFERENT hash.
        let v1 = json!({ "a": [1, 2, 3] });
        let v2 = json!({ "a": [3, 2, 1] });
        assert_ne!(canonical_json_hash(&v1), canonical_json_hash(&v2));
    }

    #[test]
    fn into_custom_notification_uses_frozen_method() {
        let ev = QueryResultChangedEvent {
            event: "queryResultChanged".to_owned(),
            v: 1,
            subscription_id: "s1".to_owned(),
            root: "/r".to_owned(),
            batch_seq: 1,
            query_kind: "callers".to_owned(),
            result_hash_old: None,
            result_hash_new: hash_hex(&canonical_json_hash(&json!([]))),
            new_result: json!([]),
            summary: None,
            evaluation_ms: 0,
            hint: DEFAULT_HINT.to_owned(),
        };
        let n = ev.into_custom_notification().expect("serializable");
        assert_eq!(n.method.as_str(), METHOD);
        assert_eq!(n.method.as_str(), "mycelium/queryResultChanged");
    }

    #[test]
    fn summary_omitted_when_none() {
        let ev = QueryResultChangedEvent {
            event: "queryResultChanged".to_owned(),
            v: 1,
            subscription_id: "s1".to_owned(),
            root: "/r".to_owned(),
            batch_seq: 1,
            query_kind: "context".to_owned(),
            result_hash_old: None,
            result_hash_new: hash_hex(&canonical_json_hash(&json!({}))),
            new_result: json!({}),
            summary: None,
            evaluation_ms: 0,
            hint: DEFAULT_HINT.to_owned(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        let obj = v.as_object().unwrap();
        assert!(
            !obj.contains_key("summary"),
            "tree-shaped query MUST omit `summary` field (RFC-0108 §4)"
        );
        // result_hash_old also omitted on first delivery.
        assert!(!obj.contains_key("result_hash_old"));
    }

    #[test]
    fn cap_set_truncates_at_50() {
        let many: Vec<String> = (0..60).map(|i| format!("p{i:02}")).collect();
        let (v, pre, trunc) = cap_set(many);
        assert_eq!(v.len(), MAX_PER_ARRAY);
        assert_eq!(pre, 60);
        assert!(trunc);
    }
}
