//! Query evaluators for RFC-0108 Reactive Query Subscriptions.
//!
//! Each variant of [`super::subscription::QuerySpec`] maps to the equivalent
//! existing MCP-tool pure-function body — we re-use the underlying
//! `Store::incoming` / `Store::outgoing` / `mycelium_hyphae::Evaluator`
//! paths, never re-implement query logic.
//!
//! All evaluators are read-only (`&Store`) — any attempt to mutate the
//! store from inside a tracked query is a bug, compile-checked at the
//! borrow level.
//!
//! `match_query_batch` is the public entry point: it gates on the
//! quiet-period + paused-until + touched-set heuristics, runs the
//! evaluator, hashes the result, and constructs the
//! [`super::query_delta::QueryResultChangedEvent`] payload only when the
//! hash changed.

#![allow(clippy::redundant_pub_crate)]

use std::collections::{BTreeSet, VecDeque};
use std::time::Duration;

use mycelium_core::store::Store;
use mycelium_core::trunk::TrunkPath;
use mycelium_core::types::EdgeKind;
use mycelium_core::watch::{BatchDelta, WatchEvent};
use serde_json::{Value, json};
use tokio::time::Instant;
use tracing::warn;

use super::query_delta::{
    DEFAULT_HINT, QueryResultChangedEvent, SetSummary, canonical_json_hash, cap_set, hash_hex,
};
use super::subscription::{
    QUERY_BUDGET_HARD_MS, QUERY_BUDGET_SOFT_MS, QUERY_DEFAULT_HOPS, QUERY_DEFAULT_MAX_PATHS,
    QUERY_DEFAULT_MAX_TOKENS, QUERY_MAX_HOPS, QUERY_MAX_PATHS, QUERY_PAUSE_COOLDOWN_SECONDS,
    QuerySpec, Subscription, query_is_set_shaped, query_kind_str,
};

/// Test-only hook: a global Duration that the Context evaluator will sleep
/// for before returning, so the eval-budget runaway test can synthesise a
/// > 200 ms wall-clock without depending on real query weight.
#[cfg(test)]
pub(crate) static TEST_FORCE_EVAL_DELAY_MS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Evaluate a `QuerySpec` against the post-batch `Store`. Returns a
/// `serde_json::Value` whose shape matches the equivalent MCP tool's
/// response data:
///
/// - Selector / Callers / Callees / Impact → `Value::Array(Vec<String>)`
///   of trunk paths.
/// - Context → seven-key minimal payload (full Cortex integration deferred
///   to RFC-0108 §8).
#[must_use]
pub(super) fn evaluate_query(spec: &QuerySpec, store: &Store) -> Value {
    match spec {
        QuerySpec::Selector { hyphae } => {
            let mut paths: Vec<String> = super::subscription::evaluate_selector_set(hyphae, store)
                .into_iter()
                .collect();
            paths.sort();
            json!(paths)
        }
        QuerySpec::Callers { path, hops } => {
            let h = clamp_hops(*hops);
            let paths = bfs_one_direction(store, path, h, BfsDir::Incoming, &[EdgeKind::Calls]);
            json!(paths)
        }
        QuerySpec::Callees { path, hops } => {
            let h = clamp_hops(*hops);
            let paths = bfs_one_direction(store, path, h, BfsDir::Outgoing, &[EdgeKind::Calls]);
            json!(paths)
        }
        QuerySpec::Impact { path, max_paths } => {
            let cap = (*max_paths)
                .unwrap_or(QUERY_DEFAULT_MAX_PATHS)
                .min(QUERY_MAX_PATHS) as usize;
            let paths = bfs_impact(store, path, cap);
            json!(paths)
        }
        QuerySpec::Context {
            task,
            focus,
            max_tokens,
        } => {
            // Test-only delay hook: lets the eval-budget runaway test
            // synthesise > 200 ms without owning a real Salsa graph.
            #[cfg(test)]
            {
                let d = TEST_FORCE_EVAL_DELAY_MS.load(std::sync::atomic::Ordering::Relaxed);
                if d > 0 {
                    std::thread::sleep(Duration::from_millis(d));
                }
            }
            let tokens = (*max_tokens).unwrap_or(QUERY_DEFAULT_MAX_TOKENS);
            // RFC-0108 §8: full Cortex integration deferred. v1 returns a
            // minimal placeholder structure stable enough for hashing —
            // any change in (task, focus, tokens, resolved symbol set)
            // produces a different hash and the agent is notified.
            let resolved: Vec<String> = focus
                .iter()
                .filter_map(|f| {
                    if store.lookup(f).is_some() || TrunkPath::parse(f).is_ok() {
                        Some(f.clone())
                    } else {
                        None
                    }
                })
                .collect();
            json!({
                "task": task,
                "focus": focus,
                "max_tokens": tokens,
                "symbols": resolved,
            })
        }
    }
}

fn clamp_hops(h: Option<u32>) -> u32 {
    h.unwrap_or(QUERY_DEFAULT_HOPS).min(QUERY_MAX_HOPS)
}

#[derive(Clone, Copy)]
enum BfsDir {
    Incoming,
    Outgoing,
}

/// BFS one direction across the supplied edge kinds; cap depth at `hops`.
fn bfs_one_direction(
    store: &Store,
    start_path: &str,
    hops: u32,
    dir: BfsDir,
    kinds: &[EdgeKind],
) -> Vec<String> {
    let Some(start) = store.lookup(start_path) else {
        return Vec::new();
    };
    let mut visited: BTreeSet<u64> = BTreeSet::new();
    let mut out: BTreeSet<String> = BTreeSet::new();
    let mut frontier: VecDeque<(u64, u32)> = VecDeque::new();
    frontier.push_back((start.0, 0));
    visited.insert(start.0);
    while let Some((node_raw, d)) = frontier.pop_front() {
        if d >= hops {
            continue;
        }
        let node = mycelium_core::types::NodeId(node_raw);
        for kind in kinds {
            let neighbours: Vec<u64> = match dir {
                BfsDir::Incoming => store.incoming(node, *kind).iter().map(|n| n.0).collect(),
                BfsDir::Outgoing => store.outgoing(node, *kind).iter().map(|n| n.0).collect(),
            };
            for n_raw in neighbours {
                if visited.insert(n_raw) {
                    if let Some(p) = store.path_of(mycelium_core::types::NodeId(n_raw)) {
                        out.insert(p.to_owned());
                    }
                    frontier.push_back((n_raw, d + 1));
                }
            }
        }
    }
    out.into_iter().collect()
}

/// BFS impact frontier (both directions, multi-edge-kind), capped at
/// `max_paths`.
fn bfs_impact(store: &Store, start_path: &str, max_paths: usize) -> Vec<String> {
    let Some(start) = store.lookup(start_path) else {
        return Vec::new();
    };
    let kinds = [EdgeKind::Calls, EdgeKind::Imports, EdgeKind::Extends];
    let mut visited: BTreeSet<u64> = BTreeSet::new();
    let mut out: BTreeSet<String> = BTreeSet::new();
    let mut frontier: VecDeque<u64> = VecDeque::new();
    frontier.push_back(start.0);
    visited.insert(start.0);
    while let Some(node_raw) = frontier.pop_front() {
        if out.len() >= max_paths {
            break;
        }
        let node = mycelium_core::types::NodeId(node_raw);
        for kind in &kinds {
            for n in store
                .incoming(node, *kind)
                .iter()
                .chain(store.outgoing(node, *kind).iter())
            {
                if visited.insert(n.0) {
                    if let Some(p) = store.path_of(*n) {
                        out.insert(p.to_owned());
                        if out.len() >= max_paths {
                            break;
                        }
                    }
                    frontier.push_back(n.0);
                }
            }
            if out.len() >= max_paths {
                break;
            }
        }
    }
    out.into_iter().take(max_paths).collect()
}

/// Outcome of one Query-subscription match attempt.
///
/// The `Emit` variant is boxed because `QueryResultChangedEvent` is large
/// relative to the unit variants — boxing keeps `QueryOutcome` cheap to
/// move on the no-op happy path.
#[derive(Debug, Clone)]
pub(super) enum QueryOutcome {
    /// Emit this payload.
    Emit(Box<QueryResultChangedEvent>),
    /// Hard-budget breach — caller MUST pause the subscription for
    /// [`QUERY_PAUSE_COOLDOWN_SECONDS`].
    Pause,
    /// One of the gates short-circuited; no-op.
    Skip,
}

/// Try to derive a `QueryResultChangedEvent` for one Query subscription
/// against one batch. See [`QueryOutcome`] for the three possible results.
///
/// Gates checked, in order:
/// 1. Subscription is paused (hard-budget cooldown still active).
/// 2. Quiet-period (`min_interval`) has not elapsed since `last_emit_at`.
/// 3. The batch touched no symbols (touched-set gate — v1 simplification).
/// 4. Hard-budget breach during evaluation → `Pause`.
/// 5. The newly-evaluated hash equals `last_hash` → `Skip`.
#[must_use]
pub(super) fn match_query_batch_outcome(
    sub: &Subscription,
    spec: &QuerySpec,
    ev: &WatchEvent,
    delta: &BatchDelta,
    store: &Store,
) -> QueryOutcome {
    let now = Instant::now();

    // Gate 1: paused?
    if let Some(until) = sub.paused_until {
        if now < until {
            return QueryOutcome::Skip;
        }
    }

    // Gate 2: quiet-period.
    if let Some(t) = sub.last_emit_at {
        let elapsed = now.duration_since(t);
        if elapsed < Duration::from_millis(sub.min_interval_ms) {
            return QueryOutcome::Skip;
        }
    }

    // Gate 3: touched-set heuristic (RFC-0108 §7 defer note: per-query
    // reverse-index would be more precise; v1 = "batch has any symbol
    // change at all").
    //
    // First delivery (last_hash.is_none()) bypasses the gate so the agent
    // always gets an initial snapshot on the first batch where the
    // subscription is alive.
    if sub.last_hash.is_some() && delta.per_file.is_empty() {
        return QueryOutcome::Skip;
    }

    // Evaluate with wall-clock timing.
    let t0 = Instant::now();
    let new_value = evaluate_query(spec, store);
    // `as_millis()` is u128; clamp to u64 for the wire payload.
    let evaluation_ms = u64::try_from(t0.elapsed().as_millis()).unwrap_or(u64::MAX);

    if evaluation_ms > QUERY_BUDGET_HARD_MS {
        // Hard-cap breach: log + tell the caller to pause us.
        // (RFC-0108 §2 — v1 simplification: no `subscription_paused`
        // notification; just `tracing::warn!` + cooldown.)
        warn!(
            target: "mycelium::subscription::query",
            subscription_id = %sub.id,
            query_kind = query_kind_str(spec),
            evaluation_ms,
            "RFC-0108 hard-budget breach; subscription paused for {QUERY_PAUSE_COOLDOWN_SECONDS}s",
        );
        return QueryOutcome::Pause;
    }
    if evaluation_ms > QUERY_BUDGET_SOFT_MS {
        warn!(
            target: "mycelium::subscription::query",
            subscription_id = %sub.id,
            query_kind = query_kind_str(spec),
            evaluation_ms,
            "RFC-0108 soft-budget breach",
        );
    }

    // Hash compare.
    let new_hash = canonical_json_hash(&new_value);
    if sub.last_hash == Some(new_hash) {
        return QueryOutcome::Skip;
    }

    // Set-shaped summary computation.
    let summary = if query_is_set_shaped(spec) {
        let new_set: BTreeSet<String> = new_value
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let empty = BTreeSet::new();
        let old_set = sub.last_set_value.as_ref().unwrap_or(&empty);
        let added: Vec<String> = new_set.difference(old_set).cloned().collect();
        let removed: Vec<String> = old_set.difference(&new_set).cloned().collect();
        let (added, added_count, added_truncated) = cap_set(added);
        let (removed, removed_count, removed_truncated) = cap_set(removed);
        Some(SetSummary {
            added,
            added_count,
            added_truncated,
            removed,
            removed_count,
            removed_truncated,
        })
    } else {
        None
    };

    QueryOutcome::Emit(Box::new(QueryResultChangedEvent {
        event: "queryResultChanged".to_owned(),
        v: 1,
        subscription_id: sub.id.clone(),
        root: ev.root.to_string_lossy().into_owned(),
        batch_seq: ev.batch_seq,
        query_kind: query_kind_str(spec).to_owned(),
        result_hash_old: sub.last_hash.as_ref().map(hash_hex),
        result_hash_new: hash_hex(&new_hash),
        new_result: new_value,
        summary,
        evaluation_ms,
        hint: DEFAULT_HINT.to_owned(),
    }))
}
