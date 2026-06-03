//! Per-subscription scoped `mycelium/subscriptionDelta` notification
//! (RFC-0107).
//!
//! Whereas PUSH (RFC-0106) emits **one** broadcast per batch with the changed
//! file list, SUBSCRIBE lets a client register an *Interest* (Files / Symbols /
//! Hyphae selector) and receive **only** the slice of each batch that matches
//! that Interest — as added / modified / removed trunk paths per file.
//!
//! The MCP server holds an in-memory `Subscriptions` map; the existing
//! `WatchEngine::on_batch` callback (RFC-0105 — widened in Phase A to
//! `FnMut(&WatchEvent, &BatchDelta, &Store)`) fan-outs at most one notification
//! per matching subscription via the captured `Peer<RoleServer>` from RFC-0106.
//!
//! Lifecycle is defence-in-depth (founder D3): rolling TTL bumped on every
//! delivery + per-client and server-wide caps + peer-close GC.
//!
//! Module shape mirrors `push.rs`: frozen-at-v1 constants, `pub(super)` items,
//! `into_custom_notification` builder. The capture of `Peer`, the spawn of the
//! actual send, and the lib.rs wiring live next to the rest of the watch
//! plumbing.

// `pub(super)` items inside a private module are flagged as redundant by
// `clippy::redundant_pub_crate`. They're not redundant *here* — the module is
// intentionally private and the items are intentionally callable from `lib.rs`.
#![allow(clippy::redundant_pub_crate)]

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

use globset::{Glob, GlobSet, GlobSetBuilder};
use mycelium_core::store::Store;
use mycelium_core::watch::{BatchDelta, WatchEvent};
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time::Instant;

// ── frozen-at-v1 constants ───────────────────────────────────────────────────

/// Method name of the per-subscription notification.
///
/// **Frozen at v1.** Changing this is a wire-contract break.
pub const METHOD: &str = "mycelium/subscriptionDelta";

/// Default subscription TTL when the client omits `ttl_seconds`. **Frozen.**
pub const DEFAULT_TTL_SECONDS: u64 = 3600;

/// Maximum permitted subscription TTL. **Frozen.**
pub const MAX_TTL_SECONDS: u64 = 86_400;

/// Server-wide cap across every connected client.
pub const MAX_SUBSCRIPTIONS: usize = 256;

/// Per-client cap.
pub const MAX_PER_CLIENT: usize = 32;

/// Selector-specific cap (server-wide).
pub const MAX_SELECTOR: usize = 64;

/// Per-array cap on `added` / `modified` / `removed` inside a `PerFileDelta`.
/// Matches RFC-0106's 50-item cap on `changed_files`.
pub const MAX_PER_ARRAY: usize = 50;

/// Cap on `per_file` entries per notification.
pub const MAX_FILES_PER_DELTA: usize = 16;

/// Maximum allowed Hyphae selector source length, in bytes.
pub const MAX_SELECTOR_SOURCE_LEN: usize = 4096;

/// Cap on the per-subscription `last_match_set` cached for Selector
/// subscriptions. Bounded ≈ 50 MB worst-case at 64 selector subs.
pub const MAX_SELECTOR_LAST_MATCH_SET: usize = 10_000;

/// Allowed-shape regex for client-supplied `subscription_id`.
pub const ID_REGEX_STR: &str = r"^[A-Za-z0-9_-]{1,64}$";

/// Human-friendly hint surfaced in `application_error` responses when an
/// invalid id is supplied.
pub const SUBSCRIPTION_ID_VALIDATION_HINT: &str = "id must match ^[A-Za-z0-9_-]{1,64}$";

/// Default `hint` field surfaced to the agent. Agents may ignore.
const DEFAULT_HINT: &str = "Apply the delta locally or re-query the affected paths.";

/// Compiled `subscription_id` regex (lazy, compiled once).
fn id_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(ID_REGEX_STR).expect("ID_REGEX_STR compiles"))
}

// ── public-shape (wire) types ────────────────────────────────────────────────

/// A subscription's Interest — frozen tagged union, mutually exclusive
/// (founder decision D1=(d)). Adding a future `Compound { all_of: [...] }`
/// combinator is additive — no v2 wire bump required.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Interest {
    /// File-glob match against `delta.per_file[*].file`.
    Files {
        /// One or more file globs, e.g. `"src/auth/**/*.rs"`.
        paths: Vec<String>,
    },
    /// Trunk-path glob match against every added/modified/removed symbol.
    Symbols {
        /// One or more trunk-path globs, e.g. `"src/auth.rs>fn:*"`.
        paths: Vec<String>,
    },
    /// A Hyphae selector source string (RFC-0003 grammar).
    Selector {
        /// Hyphae selector source, e.g. `'fn[name="login"]'`. Capped at
        /// [`MAX_SELECTOR_SOURCE_LEN`].
        hyphae: String,
    },
}

/// Wire-stable name of the Interest variant.
#[must_use]
pub const fn interest_kind_str(i: &Interest) -> &'static str {
    match i {
        Interest::Files { .. } => "files",
        Interest::Symbols { .. } => "symbols",
        Interest::Selector { .. } => "selector",
    }
}

/// Per-file diff payload inside a `subscriptionDelta` notification.
///
/// Field order matches RFC-0107 §4 exactly. **Frozen v1.**
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PerFileDelta {
    /// Repository-relative file path (`/`-normalized).
    pub file: String,
    /// Trunk paths added in this batch that match the Interest.
    pub added: Vec<String>,
    /// Pre-cap count of `added`.
    pub added_count: u64,
    /// `true` when `added` was truncated to [`MAX_PER_ARRAY`].
    pub added_truncated: bool,
    /// Trunk paths modified in this batch that match the Interest.
    pub modified: Vec<String>,
    /// Pre-cap count of `modified`.
    pub modified_count: u64,
    /// `true` when `modified` was truncated.
    pub modified_truncated: bool,
    /// Trunk paths removed in this batch that match the Interest.
    pub removed: Vec<String>,
    /// Pre-cap count of `removed`.
    pub removed_count: u64,
    /// `true` when `removed` was truncated.
    pub removed_truncated: bool,
}

/// Wire-shape of the `mycelium/subscriptionDelta` notification payload.
///
/// **Frozen at v1.** Any breaking change increments `v` and gets a new RFC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubscriptionDeltaEvent {
    /// Constant `"subscriptionDelta"` — disambiguates from other Mycelium events.
    pub event: String,
    /// Schema version.
    pub v: u32,
    /// Subscription that produced this delta.
    pub subscription_id: String,
    /// Absolute path of the watched root.
    pub root: String,
    /// Monotonic batch counter from RFC-0105.
    pub batch_seq: u64,
    /// One entry per touched file, capped at [`MAX_FILES_PER_DELTA`].
    pub per_file: Vec<PerFileDelta>,
    /// `true` when the underlying batch touched more than
    /// [`MAX_FILES_PER_DELTA`] matching files.
    pub files_truncated: bool,
    /// `"files" | "symbols" | "selector"` — disambiguates handling on the
    /// client side.
    pub interest_kind: String,
    /// Free-text human-friendly suggestion.
    pub hint: String,
}

impl SubscriptionDeltaEvent {
    /// Build the rmcp `CustomNotification` envelope. Best-effort — returns
    /// `None` on serialization failure (effectively OOM only).
    #[must_use]
    pub(super) fn into_custom_notification(self) -> Option<rmcp::model::CustomNotification> {
        let params = serde_json::to_value(&self).ok()?;
        Some(rmcp::model::CustomNotification::new(METHOD, Some(params)))
    }
}

// ── MCP request / response shapes ────────────────────────────────────────────

/// Request shape for `mycelium_subscribe`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SubscribeRequest {
    /// Optional client-supplied subscription id (regex `^[A-Za-z0-9_-]{1,64}$`).
    /// When omitted, the server mints a UUID-v4 simple form.
    #[serde(default)]
    pub subscription_id: Option<String>,
    /// What to match. Mutually-exclusive tagged union (founder D1=(d)).
    pub interest: Interest,
    /// Optional rolling TTL in seconds (default `DEFAULT_TTL_SECONDS`, max
    /// `MAX_TTL_SECONDS`). Bumped on every successful delivery.
    #[serde(default)]
    pub ttl_seconds: Option<u64>,
    /// Optional explicit root override; falls back to the server's indexed
    /// root when omitted.
    #[serde(default)]
    pub root: Option<String>,
}

/// Response shape for `mycelium_subscribe`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SubscribeResponse {
    /// The server-canonical subscription id.
    pub subscription_id: String,
    /// Absolute path of the root this subscription is scoped to.
    pub root: String,
    /// Effective rolling TTL in seconds.
    pub ttl_seconds: u64,
    /// `"files" | "symbols" | "selector"`.
    pub interest_kind: String,
    /// Active subscription count after this insert (server-wide).
    pub active_count: u64,
}

/// Request shape for `mycelium_unsubscribe`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct UnsubscribeRequest {
    /// Subscription id to remove. Unknown ids are idempotent no-ops.
    pub subscription_id: String,
}

/// Response shape for `mycelium_unsubscribe`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct UnsubscribeResponse {
    /// `true` when an existing subscription was removed; `false` when the id
    /// was unknown (idempotent).
    pub removed: bool,
    /// Active subscription count after the operation.
    pub active_count: u64,
}

/// Request shape for `mycelium_subscription_status`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct SubscriptionStatusRequest {
    /// When `Some`, return only that subscription (or empty list). When
    /// `None`, return every active subscription.
    #[serde(default)]
    pub subscription_id: Option<String>,
}

/// Single subscription's status row.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct SubscriptionInfo {
    /// Subscription id.
    pub subscription_id: String,
    /// Absolute path of the scoped root.
    pub root: String,
    /// `"files" | "symbols" | "selector"`.
    pub interest_kind: String,
    /// Effective rolling TTL.
    pub ttl_seconds: u64,
    /// Seconds until expiry (approximate; 0 when already expired but not
    /// yet reaped by the periodic eviction task).
    pub seconds_until_expiry: u64,
}

/// Response shape for `mycelium_subscription_status`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct SubscriptionStatusResponse {
    /// Active subscription count, server-wide.
    pub active_count: u64,
    /// Server-wide cap.
    pub max_subscriptions: u64,
    /// Per-client cap.
    pub max_per_client: u64,
    /// Server-wide Selector-subscription cap.
    pub max_selector: u64,
    /// `true` when the file-watch loop is active.
    pub watching: bool,
    /// One row per subscription. When the request had `subscription_id =
    /// Some(...)`, contains at most one row.
    pub subscriptions: Vec<SubscriptionInfo>,
}

// ── error model ──────────────────────────────────────────────────────────────

/// Errors returnable from `subscribe` / `unsubscribe` / `status`.
#[derive(Debug, Error)]
pub enum SubscribeError {
    /// Client-supplied id collides with an active subscription.
    #[error("subscription id collides with an active subscription")]
    IdCollision,
    /// Invalid Interest shape (e.g. empty paths, malformed id).
    #[error("invalid interest: {0}")]
    InvalidInterest(String),
    /// Hyphae selector source exceeded [`MAX_SELECTOR_SOURCE_LEN`].
    #[error("selector source exceeds {MAX_SELECTOR_SOURCE_LEN}-byte cap")]
    SelectorTooLarge,
    /// Cap reached at the scope reported by `scope` (`client | server | selector`).
    #[error("subscription limit reached ({scope})")]
    SubscriptionLimit {
        /// `"client" | "server" | "selector"`.
        scope: &'static str,
    },
    /// Root is not under the server's allowed-roots set.
    #[error("root is not under allowed roots: {0}")]
    RootNotAllowed(String),
}

impl SubscribeError {
    /// Wire-stable error code.
    #[must_use]
    pub(super) const fn code(&self) -> &'static str {
        match self {
            Self::IdCollision => "id_collision",
            Self::InvalidInterest(_) => "invalid_interest",
            Self::SelectorTooLarge => "selector_too_large",
            Self::SubscriptionLimit { .. } => "subscription_limit",
            Self::RootNotAllowed(_) => "root_not_allowed",
        }
    }
}

// ── in-memory store ──────────────────────────────────────────────────────────

/// One live subscription (in-memory only — server restart = re-subscribe).
#[derive(Debug, Clone)]
pub struct Subscription {
    /// Server-canonical id.
    pub id: String,
    /// Absolute path of the scoped root.
    pub root: PathBuf,
    /// What this subscription matches.
    pub interest: Interest,
    /// Effective TTL in seconds (bumped on every delivery).
    pub ttl_seconds: u64,
    /// Rolling deadline — set to `Instant::now() + ttl` on every delivery.
    pub expires_at: Instant,
    /// Opaque per-peer identity tag for dead-peer GC.
    pub client_tag: String,
    /// `Some(...)` only for Selector subscriptions — the previous batch's
    /// match-set, used for (ii-strict) removal computation.
    pub last_match_set: Option<BTreeSet<String>>,
}

/// In-memory subscription store.
#[derive(Debug, Default)]
pub struct Subscriptions {
    /// All active subscriptions keyed by id.
    pub by_id: HashMap<String, Subscription>,
    /// Count of `Selector`-kind subscriptions (cached so the cap check is
    /// O(1)).
    pub selector_count: usize,
}

/// Shared handle into the subscription store.
pub type Store_ = Arc<RwLock<Subscriptions>>;

/// Build a fresh empty subscription store.
#[must_use]
pub fn new_store() -> Store_ {
    Arc::new(RwLock::new(Subscriptions::default()))
}

// ── pure helpers ─────────────────────────────────────────────────────────────

/// Validate the Interest payload (does not check caps — see [`subscribe`]).
fn validate_interest(interest: &Interest) -> Result<(), SubscribeError> {
    match interest {
        Interest::Files { paths } | Interest::Symbols { paths } => {
            if paths.is_empty() {
                return Err(SubscribeError::InvalidInterest(
                    "paths must be non-empty".to_owned(),
                ));
            }
            for p in paths {
                if p.is_empty() {
                    return Err(SubscribeError::InvalidInterest(
                        "empty path string".to_owned(),
                    ));
                }
                Glob::new(p).map_err(|e| {
                    SubscribeError::InvalidInterest(format!("invalid glob `{p}`: {e}"))
                })?;
            }
            Ok(())
        }
        Interest::Selector { hyphae } => {
            if hyphae.len() > MAX_SELECTOR_SOURCE_LEN {
                return Err(SubscribeError::SelectorTooLarge);
            }
            if hyphae.trim().is_empty() {
                return Err(SubscribeError::InvalidInterest(
                    "selector source must be non-empty".to_owned(),
                ));
            }
            Ok(())
        }
    }
}

/// Build a `GlobSet` for the supplied patterns. Caller has already validated
/// each pattern compiles.
fn build_globset(patterns: &[String]) -> GlobSet {
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        if let Ok(g) = Glob::new(p) {
            b.add(g);
        }
    }
    b.build().unwrap_or_else(|_| GlobSet::empty())
}

/// Extract the "file" prefix of a trunk path (everything before the first `>`).
/// Returns the input unchanged when no `>` is present (i.e. the path *is* a
/// file).
fn file_of_trunk_path(p: &str) -> &str {
    p.split_once('>').map_or(p, |(file, _)| file)
}

// ── core API: subscribe / unsubscribe / status ───────────────────────────────

/// Insert a new subscription. Caps enforced in the order
/// `id_collision` → `invalid_interest` → [`MAX_SUBSCRIPTIONS`] →
/// per-client → [`MAX_SELECTOR`] (founder D3 defence-in-depth).
///
/// # Errors
///
/// Returns a [`SubscribeError`] when any validation gate (id shape /
/// id collision / Interest shape / Selector cap / server cap / per-client
/// cap / Selector cap / root not in allowed roots) rejects the request.
pub async fn subscribe(
    store: &Store_,
    req: SubscribeRequest,
    client_tag: String,
    root: PathBuf,
) -> Result<SubscribeResponse, SubscribeError> {
    // 1. id validation (collision check is later, against live state)
    let id = match req.subscription_id.as_deref() {
        Some(supplied) => {
            if !id_regex().is_match(supplied) {
                return Err(SubscribeError::InvalidInterest(
                    SUBSCRIPTION_ID_VALIDATION_HINT.to_owned(),
                ));
            }
            supplied.to_owned()
        }
        None => uuid::Uuid::new_v4().simple().to_string(),
    };

    // 2. interest validation (shape-only)
    validate_interest(&req.interest)?;

    let ttl_seconds = req
        .ttl_seconds
        .unwrap_or(DEFAULT_TTL_SECONDS)
        .min(MAX_TTL_SECONDS);
    let kind_str = interest_kind_str(&req.interest).to_owned();
    let is_selector = matches!(&req.interest, Interest::Selector { .. });
    let last_match_set: Option<BTreeSet<String>> = if is_selector {
        Some(BTreeSet::new())
    } else {
        None
    };

    let mut w = store.write().await;

    // 3. caps + collision (LIVE — under write lock)
    if w.by_id.contains_key(&id) {
        return Err(SubscribeError::IdCollision);
    }
    if w.by_id.len() >= MAX_SUBSCRIPTIONS {
        return Err(SubscribeError::SubscriptionLimit { scope: "server" });
    }
    let per_client = w
        .by_id
        .values()
        .filter(|s| s.client_tag == client_tag)
        .count();
    if per_client >= MAX_PER_CLIENT {
        return Err(SubscribeError::SubscriptionLimit { scope: "client" });
    }
    if is_selector && w.selector_count >= MAX_SELECTOR {
        return Err(SubscribeError::SubscriptionLimit { scope: "selector" });
    }

    let sub = Subscription {
        id: id.clone(),
        root: root.clone(),
        interest: req.interest,
        ttl_seconds,
        expires_at: Instant::now() + std::time::Duration::from_secs(ttl_seconds),
        client_tag,
        last_match_set,
    };
    w.by_id.insert(id.clone(), sub);
    if is_selector {
        w.selector_count = w.selector_count.saturating_add(1);
    }

    let active_count = w.by_id.len() as u64;
    drop(w);

    Ok(SubscribeResponse {
        subscription_id: id,
        root: root.to_string_lossy().into_owned(),
        ttl_seconds,
        interest_kind: kind_str,
        active_count,
    })
}

/// Remove a subscription. Idempotent — unknown id returns `removed=false`.
pub(super) async fn unsubscribe(store: &Store_, id: &str) -> UnsubscribeResponse {
    let mut w = store.write().await;
    let prev = w.by_id.remove(id);
    if let Some(s) = &prev {
        if matches!(s.interest, Interest::Selector { .. }) {
            w.selector_count = w.selector_count.saturating_sub(1);
        }
    }
    UnsubscribeResponse {
        removed: prev.is_some(),
        active_count: w.by_id.len() as u64,
    }
}

/// Look up subscription status. When `id` is `Some`, returns at most one row.
pub(super) async fn status(
    store: &Store_,
    id: Option<&str>,
    watching: bool,
) -> SubscriptionStatusResponse {
    let r = store.read().await;
    let now = Instant::now();
    let subscriptions: Vec<SubscriptionInfo> = id.map_or_else(
        || {
            let mut v: Vec<SubscriptionInfo> = r.by_id.values().map(|s| sub_info(s, now)).collect();
            v.sort_by(|a, b| a.subscription_id.cmp(&b.subscription_id));
            v
        },
        |want| {
            r.by_id
                .get(want)
                .map(|s| sub_info(s, now))
                .into_iter()
                .collect()
        },
    );
    let active_count = r.by_id.len() as u64;
    drop(r);
    SubscriptionStatusResponse {
        active_count,
        max_subscriptions: MAX_SUBSCRIPTIONS as u64,
        max_per_client: MAX_PER_CLIENT as u64,
        max_selector: MAX_SELECTOR as u64,
        watching,
        subscriptions,
    }
}

fn sub_info(s: &Subscription, now: Instant) -> SubscriptionInfo {
    let seconds_until_expiry = s
        .expires_at
        .checked_duration_since(now)
        .map_or(0, |d| d.as_secs());
    SubscriptionInfo {
        subscription_id: s.id.clone(),
        root: s.root.to_string_lossy().into_owned(),
        interest_kind: interest_kind_str(&s.interest).to_owned(),
        ttl_seconds: s.ttl_seconds,
        seconds_until_expiry,
    }
}

// ── lifecycle (TTL + peer-close) ─────────────────────────────────────────────

/// Drop subscriptions whose rolling TTL has elapsed. Returns count evicted.
pub(super) async fn evict_expired(store: &Store_) -> usize {
    let now = Instant::now();
    let mut w = store.write().await;
    let to_drop: Vec<String> = w
        .by_id
        .iter()
        .filter(|(_, s)| s.expires_at <= now)
        .map(|(k, _)| k.clone())
        .collect();
    let mut n = 0_usize;
    for id in to_drop {
        if let Some(s) = w.by_id.remove(&id) {
            if matches!(s.interest, Interest::Selector { .. }) {
                w.selector_count = w.selector_count.saturating_sub(1);
            }
            n += 1;
        }
    }
    drop(w);
    n
}

/// Drop every subscription owned by a peer that has gone away.
///
/// Reserved for the rmcp peer-disconnect hook once that signal is plumbed
/// through (RFC-0107 D3 defence-in-depth). Until then, TTL eviction is the
/// primary mechanism — this fn is exercised by unit tests + ready for the
/// follow-up wire-up.
#[allow(dead_code)]
pub(super) async fn evict_for_dead_peer(store: &Store_, client_tag: &str) -> usize {
    let mut w = store.write().await;
    let to_drop: Vec<String> = w
        .by_id
        .iter()
        .filter(|(_, s)| s.client_tag == client_tag)
        .map(|(k, _)| k.clone())
        .collect();
    let mut n = 0_usize;
    for id in &to_drop {
        if let Some(s) = w.by_id.remove(id) {
            if matches!(s.interest, Interest::Selector { .. }) {
                w.selector_count = w.selector_count.saturating_sub(1);
            }
            n += 1;
        }
    }
    drop(w);
    n
}

/// Bump `expires_at = now() + ttl` on the named subscription (rolling TTL).
/// Silent no-op for unknown ids.
pub(super) async fn bump_ttl(store: &Store_, id: &str) {
    let mut w = store.write().await;
    if let Some(s) = w.by_id.get_mut(id) {
        s.expires_at = Instant::now() + std::time::Duration::from_secs(s.ttl_seconds);
    }
}

/// Persist a freshly-computed match-set onto a Selector subscription.
/// Silent no-op for non-Selector subs / unknown ids.
pub(super) async fn update_last_match_set(store: &Store_, id: &str, new_set: BTreeSet<String>) {
    let mut w = store.write().await;
    if let Some(s) = w.by_id.get_mut(id) {
        if matches!(s.interest, Interest::Selector { .. }) {
            // Bound the cached set; once over the cap we still store the
            // truncated view so subsequent diffs degrade gracefully (a removal
            // is still detectable when the underlying path is touched again).
            let trimmed: BTreeSet<String> = new_set
                .into_iter()
                .take(MAX_SELECTOR_LAST_MATCH_SET)
                .collect();
            s.last_match_set = Some(trimmed);
        }
    }
}

// ── match_batch ──────────────────────────────────────────────────────────────

/// Per-file bucket for the Selector matcher: (added, modified, removed) trunk
/// paths grouped by the file prefix of each trunk path.
type Buckets = (Vec<String>, Vec<String>, Vec<String>);

/// Build the set of paths the batch actually touched (used by (ii-strict)).
fn touched_paths(delta: &BatchDelta) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    for f in &delta.per_file {
        for p in f
            .added
            .iter()
            .chain(f.modified.iter())
            .chain(f.removed.iter())
        {
            s.insert(p.clone());
        }
    }
    s
}

/// Cap-and-sort one (added/modified/removed) array; returns `(vec, pre_cap,
/// truncated)`.
fn cap_array(mut v: Vec<String>) -> (Vec<String>, u64, bool) {
    v.sort();
    v.dedup();
    let pre = v.len() as u64;
    let truncated = v.len() > MAX_PER_ARRAY;
    if truncated {
        v.truncate(MAX_PER_ARRAY);
    }
    (v, pre, truncated)
}

/// Try to derive an event for a subscription against one batch. Returns
/// `None` when nothing matches.
///
/// `trunk_store` is the read-locked post-batch [`mycelium_core::store::Store`]
/// supplied by `WatchEngine::drive`'s `on_batch` third arg.
#[allow(clippy::too_many_lines)] // single coherent match-and-cap pass
#[must_use]
pub fn match_batch(
    sub: &Subscription,
    ev: &WatchEvent,
    delta: &BatchDelta,
    trunk_store: &Store,
) -> Option<SubscriptionDeltaEvent> {
    let mut per_file_payload: Vec<PerFileDelta> = Vec::new();

    match &sub.interest {
        Interest::Files { paths } => {
            let gs = build_globset(paths);
            for f in &delta.per_file {
                if gs.is_match(&f.file) {
                    let (added, added_count, added_truncated) = cap_array(f.added.clone());
                    let (modified, modified_count, modified_truncated) =
                        cap_array(f.modified.clone());
                    let (removed, removed_count, removed_truncated) = cap_array(f.removed.clone());
                    if added_count == 0 && modified_count == 0 && removed_count == 0 {
                        continue;
                    }
                    per_file_payload.push(PerFileDelta {
                        file: f.file.clone(),
                        added,
                        added_count,
                        added_truncated,
                        modified,
                        modified_count,
                        modified_truncated,
                        removed,
                        removed_count,
                        removed_truncated,
                    });
                }
            }
        }
        Interest::Symbols { paths } => {
            let gs = build_globset(paths);
            for f in &delta.per_file {
                let added: Vec<String> =
                    f.added.iter().filter(|p| gs.is_match(p)).cloned().collect();
                let modified: Vec<String> = f
                    .modified
                    .iter()
                    .filter(|p| gs.is_match(p))
                    .cloned()
                    .collect();
                let removed: Vec<String> = f
                    .removed
                    .iter()
                    .filter(|p| gs.is_match(p))
                    .cloned()
                    .collect();
                if added.is_empty() && modified.is_empty() && removed.is_empty() {
                    continue;
                }
                let (added, added_count, added_truncated) = cap_array(added);
                let (modified, modified_count, modified_truncated) = cap_array(modified);
                let (removed, removed_count, removed_truncated) = cap_array(removed);
                per_file_payload.push(PerFileDelta {
                    file: f.file.clone(),
                    added,
                    added_count,
                    added_truncated,
                    modified,
                    modified_count,
                    modified_truncated,
                    removed,
                    removed_count,
                    removed_truncated,
                });
            }
        }
        Interest::Selector { hyphae } => {
            // Evaluate the selector against the post-batch trunk.
            let ast = mycelium_hyphae::parse(hyphae).ok()?;
            let evaluator = mycelium_hyphae::evaluator::Evaluator::new(trunk_store);
            let new_matches = evaluator.eval(&ast);
            let new_set: BTreeSet<String> = new_matches.into_iter().collect();

            let touched = touched_paths(delta);
            let empty = BTreeSet::new();
            let old_set = sub.last_match_set.as_ref().unwrap_or(&empty);

            // added = NEW − OLD; intersect with this batch's touched paths to
            // avoid reporting symbols a Selector now matches purely because of
            // unrelated state. Mirror of (ii-strict) for the added side.
            let added: Vec<String> = new_set
                .difference(old_set)
                .filter(|p| touched.contains(*p))
                .cloned()
                .collect();
            // modified = (OLD ∩ NEW) ∩ touched.
            let modified: Vec<String> = new_set
                .intersection(old_set)
                .filter(|p| touched.contains(*p))
                .cloned()
                .collect();
            // removed = (OLD − NEW) ∩ touched   ← (ii-strict)
            let removed: Vec<String> = old_set
                .difference(&new_set)
                .filter(|p| touched.contains(*p))
                .cloned()
                .collect();

            if added.is_empty() && modified.is_empty() && removed.is_empty() {
                return None;
            }

            // Bucket by file prefix.
            let mut by_file: std::collections::BTreeMap<String, Buckets> =
                std::collections::BTreeMap::new();
            for p in added {
                by_file
                    .entry(file_of_trunk_path(&p).to_owned())
                    .or_default()
                    .0
                    .push(p);
            }
            for p in modified {
                by_file
                    .entry(file_of_trunk_path(&p).to_owned())
                    .or_default()
                    .1
                    .push(p);
            }
            for p in removed {
                by_file
                    .entry(file_of_trunk_path(&p).to_owned())
                    .or_default()
                    .2
                    .push(p);
            }
            for (file, (a, m, r)) in by_file {
                let (added, added_count, added_truncated) = cap_array(a);
                let (modified, modified_count, modified_truncated) = cap_array(m);
                let (removed, removed_count, removed_truncated) = cap_array(r);
                per_file_payload.push(PerFileDelta {
                    file,
                    added,
                    added_count,
                    added_truncated,
                    modified,
                    modified_count,
                    modified_truncated,
                    removed,
                    removed_count,
                    removed_truncated,
                });
            }
            // NOTE: caller is expected to persist the freshly-computed
            // `new_set` via `update_last_match_set` after delivery succeeds.
        }
    }

    if per_file_payload.is_empty() {
        return None;
    }

    // Sort per-file by file path, then apply MAX_FILES_PER_DELTA cap.
    per_file_payload.sort_by(|a, b| a.file.cmp(&b.file));
    let files_truncated = per_file_payload.len() > MAX_FILES_PER_DELTA;
    if files_truncated {
        per_file_payload.truncate(MAX_FILES_PER_DELTA);
    }

    Some(SubscriptionDeltaEvent {
        event: "subscriptionDelta".to_owned(),
        v: 1,
        subscription_id: sub.id.clone(),
        root: ev.root.to_string_lossy().into_owned(),
        batch_seq: ev.batch_seq,
        per_file: per_file_payload,
        files_truncated,
        interest_kind: interest_kind_str(&sub.interest).to_owned(),
        hint: DEFAULT_HINT.to_owned(),
    })
}

/// For Selector subscriptions, recompute (without diffing) the current match
/// set against the supplied store — used by the fan-out path to update
/// `last_match_set` after a successful delivery.
pub(super) fn evaluate_selector_set(hyphae: &str, trunk_store: &Store) -> BTreeSet<String> {
    mycelium_hyphae::parse(hyphae).map_or_else(
        |_| BTreeSet::new(),
        |ast| {
            mycelium_hyphae::evaluator::Evaluator::new(trunk_store)
                .eval(&ast)
                .into_iter()
                .collect()
        },
    )
}

#[cfg(test)]
mod tests;
