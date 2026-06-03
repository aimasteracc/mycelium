//! Server-initiated `mycelium/graphChanged` notification (RFC-0106).
//!
//! Founder ratified **Option B** (custom JSON-RPC method, 2026-06-03): when the
//! shared watch engine (RFC-0105) commits a batch, the MCP server emits **one**
//! `CustomNotification` with method `"mycelium/graphChanged"` and a frozen
//! `GraphChangedEvent` v1 JSON payload. Best-effort delivery — a dead client
//! never aborts the watch loop.
//!
//! This module is intentionally tiny: it owns
//! - the wire-shape type (`GraphChangedEvent`, frozen for v1)
//! - the `WatchEvent → GraphChangedEvent` reducer (pure function, cap + truncation)
//! - the helper that builds the rmcp `CustomNotification` envelope.
//!
//! The capture of `Peer<RoleServer>`, the spawn of the actual send, and the
//! wiring into the watch `on_batch` closure live in `lib.rs` next to the rest
//! of the watch plumbing — that's where the rest of the cross-cutting state
//! (notifier `Arc<Mutex<Option<Peer>>>`) lives.

// `pub(super)` items inside a private module are flagged as redundant by
// `clippy::redundant_pub_crate`. They're not redundant *here* — the module is
// intentionally private and the items are intentionally callable from `lib.rs`.
#![allow(clippy::redundant_pub_crate)]

use mycelium_core::watch::WatchEvent;
use serde::{Deserialize, Serialize};

/// Method name of the server-initiated notification.
///
/// **Frozen at v1.** Changing this is a wire-contract break.
pub(super) const METHOD: &str = "mycelium/graphChanged";

/// Maximum number of `changed_files` carried per notification.
///
/// Above this, `changed_files` is truncated and `truncated` is set to `true`;
/// `changed_count` always reports the **pre-cap** total so the agent knows the
/// real magnitude. **Frozen at v1.**
pub(super) const MAX_CHANGED_FILES: usize = 50;

/// Default agent-facing hint string. Agents may ignore.
const DEFAULT_HINT: &str = "Re-query mycelium_context for the area you care about.";

/// Wire-shape of the `mycelium/graphChanged` notification payload.
///
/// **Frozen at v1.** Any breaking change increments the `v` field and gets a
/// new RFC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(super) struct GraphChangedEvent {
    /// Constant `"graphChanged"` — disambiguates this from future Mycelium events.
    pub event: String,
    /// Schema version. Increments on any breaking change to this shape.
    pub v: u32,
    /// Absolute path of the watched root.
    pub root: String,
    /// Monotonic batch counter from RFC-0105 — clients can detect dropped batches.
    pub batch_seq: u64,
    /// Repository-relative changed file paths (`/`-normalized), capped at
    /// [`MAX_CHANGED_FILES`]. Sorted + deduped by the watch engine.
    pub changed_files: Vec<String>,
    /// Total number of changed files **before** truncation. Equals
    /// `changed_files.len()` when not truncated.
    pub changed_count: u64,
    /// `true` when the original batch contained more than [`MAX_CHANGED_FILES`]
    /// files and `changed_files` was truncated.
    pub truncated: bool,
    /// Free-text human-friendly suggestion. Agents may ignore.
    pub hint: String,
}

impl GraphChangedEvent {
    /// Project a [`WatchEvent`] from RFC-0105 into the wire-shape v1 payload,
    /// applying the [`MAX_CHANGED_FILES`] cap.
    ///
    /// Pure function — exhaustively tested.
    #[must_use]
    pub(super) fn from_watch_event(ev: &WatchEvent) -> Self {
        let total = ev.changed_files.len();
        let truncated = total > MAX_CHANGED_FILES;
        let changed_files = if truncated {
            ev.changed_files[..MAX_CHANGED_FILES].to_vec()
        } else {
            ev.changed_files.clone()
        };
        Self {
            event: "graphChanged".to_owned(),
            v: 1,
            root: ev.root.to_string_lossy().into_owned(),
            batch_seq: ev.batch_seq,
            changed_files,
            changed_count: total as u64,
            truncated,
            hint: DEFAULT_HINT.to_owned(),
        }
    }

    /// Build the rmcp `CustomNotification` envelope that gets sent over the wire.
    ///
    /// Best-effort: returns `None` if serialization fails (which would only
    /// happen on an OOM-class condition; the caller logs and continues).
    #[must_use]
    pub(super) fn into_custom_notification(self) -> Option<rmcp::model::CustomNotification> {
        let params = serde_json::to_value(&self).ok()?;
        Some(rmcp::model::CustomNotification::new(METHOD, Some(params)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn watch_event(root: &str, batch_seq: u64, files: &[&str]) -> WatchEvent {
        WatchEvent {
            root: PathBuf::from(root),
            changed_files: files.iter().map(|s| (*s).to_owned()).collect(),
            batch_seq,
        }
    }

    #[test]
    fn from_watch_event_small_batch_no_truncation() {
        let ev = watch_event("/r", 7, &["src/a.rs", "src/b.rs"]);
        let g = GraphChangedEvent::from_watch_event(&ev);
        assert_eq!(g.event, "graphChanged");
        assert_eq!(g.v, 1);
        assert_eq!(g.root, "/r");
        assert_eq!(g.batch_seq, 7);
        assert_eq!(g.changed_files, vec!["src/a.rs", "src/b.rs"]);
        assert_eq!(g.changed_count, 2);
        assert!(!g.truncated);
        assert!(!g.hint.is_empty());
    }

    #[test]
    fn from_watch_event_truncates_at_50_with_flag() {
        let files: Vec<String> = (0..60).map(|i| format!("src/f_{i:02}.rs")).collect();
        let ev = WatchEvent {
            root: PathBuf::from("/r"),
            changed_files: files,
            batch_seq: 1,
        };
        let g = GraphChangedEvent::from_watch_event(&ev);
        assert_eq!(g.changed_files.len(), MAX_CHANGED_FILES);
        assert_eq!(g.changed_count, 60);
        assert!(g.truncated);
        // Truncation keeps the FIRST 50 (sort order is the engine's contract,
        // not this function's).
        assert_eq!(g.changed_files[0], "src/f_00.rs");
        assert_eq!(g.changed_files[49], "src/f_49.rs");
    }

    #[test]
    fn json_field_names_are_frozen_v1_shape() {
        let ev = watch_event("/r", 1, &["x"]);
        let g = GraphChangedEvent::from_watch_event(&ev);
        let json = serde_json::to_value(&g).unwrap();
        // The wire field names are part of the v1 contract — name-change = break.
        let obj = json.as_object().expect("object");
        for key in [
            "event",
            "v",
            "root",
            "batch_seq",
            "changed_files",
            "changed_count",
            "truncated",
            "hint",
        ] {
            assert!(obj.contains_key(key), "v1 contract requires field `{key}`");
        }
        assert_eq!(obj["event"], "graphChanged");
        assert_eq!(obj["v"], 1);
    }

    #[test]
    fn into_custom_notification_uses_frozen_method_name() {
        let ev = watch_event("/r", 1, &["x"]);
        let n = GraphChangedEvent::from_watch_event(&ev)
            .into_custom_notification()
            .expect("serializable");
        assert_eq!(n.method.as_str(), METHOD);
        assert_eq!(n.method.as_str(), "mycelium/graphChanged");
        let params = n.params.expect("params");
        assert_eq!(params["event"], "graphChanged");
        assert_eq!(params["v"], 1);
    }
}
