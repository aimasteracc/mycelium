# RFC-0107: SUBSCRIBE — scoped per-batch delta notifications

- **RFC**: 0107
- **Title**: Agents register an *interest*; the server pushes only the matching slice of each batch
- **Status**: **Accepted (all 5 recommendations ratified by founder 2026-06-03)**. D1=(d) tagged union mutually exclusive; D2=(ii-strict) phantom-removal-free; D3=TTL+caps+peer-close GC defence-in-depth; D4=(a) extend `on_batch` to `FnMut(&WatchEvent, &BatchDelta, &Store)`; D5=(a) extend RFC-0105 EXCEPTION with `mycelium watch --subscribe '<spec>'`. Implementation tracked separately.
- **Author**: .hive team (design workflow) + rust-implementer
- **Created**: 2026-06-03
- **Depends on**: [RFC-0105](0105-shared-watch-engine-cli-watch.md) (`on_batch` seam), [RFC-0106](0106-push-graph-changed-notification.md) (Peer capture + CustomNotification transport)
- **Closes**: the reactive-completion roadmap, step 3/4 (watch → push → **subscribe** → salsa)

---

## 1. Summary

Today every connected agent receives the **same** `mycelium/graphChanged`
notification on every batch (RFC-0106). SUBSCRIBE lets an agent register an
**Interest** (Files / Symbols / Hyphae selector) and receive **only** the slice
of each batch that matches it — as a `mycelium/subscriptionDelta` custom
notification carrying added/modified/removed trunk paths per file.

Three new MCP tools — `mycelium_subscribe`, `mycelium_unsubscribe`,
`mycelium_subscription_status` — manage an in-memory map on `MyceliumServer`.
The CLI gets a `mycelium watch --subscribe '<spec>'` form printing the same
payloads as NDJSON to stdout.

The watch engine evolves *additively*: per batch, it computes **one**
`BatchDelta` inside the write-lock it already holds, then the MCP server
fan-outs at most one notification per matching subscription via RFC-0106's
captured Peer. **No new transport. No new core seam. Same `on_batch` callback
with one additional argument.**

## 2. What's fixed regardless of founder answers

| Locked | Why |
|---|---|
| **Storage** = `Arc<RwLock<HashMap<String, Subscription>>>` on `MyceliumServer`. | In-memory only; server restart = re-subscribe (mirrors watch). |
| **BatchDelta computed once per batch**, fan-out N matches. | Avoids quadratic recompute; cost amortised over subscribers. |
| **OLD symbol set captured inside the same write-lock as `remove_file`**, before per-file mutation runs (`watch/mod.rs` ~282-334). | The store has no historical view post-remove; doing it any other way races. |
| **`Store::symbols_in_file(file_rel) -> Vec<String>`** new core helper, file root excluded, sorted. | Cheap O(d) over `trunk.descendants`; reuses the existing index. |
| **Wire transport = RFC-0106's CustomNotification path**, just a different method name (`mycelium/subscriptionDelta`). | One mechanism for the whole reactive contract; no second peer plumbing. |
| **Subscriptions are best-effort, fire-and-forget, dead-client tolerant** — same discipline as RFC-0106. | Watch loop must never block on network IO. |
| **`mycelium_unsubscribe` is idempotent** — unknown id returns `{removed: false}`, not an error. | Agents can run cleanup blindly on reconnect. |
| **subscription_id**: server-minted [ULID](https://github.com/ulid/spec) by default; clients MAY supply an id matching `^[A-Za-z0-9_-]{1,64}$` for idempotent reconnect. | Reasonable round-trip default. |

## 3. The five founder decisions

Each one is a permanent wire/policy contract once shipped. Pick fast; I've
recommended an answer + one-sentence reasoning for each.

### D1 — Interest grammar (mutually exclusive vs combinator)

A subscription's **Interest** can be one of:

```rust
pub enum Interest {
    Files    { paths: Vec<String> },   // file globs, e.g. "src/auth/**/*.rs"
    Symbols  { paths: Vec<String> },   // trunk-path globs, e.g. "src/auth.rs>fn:*"
    Selector { hyphae: String },       // a Hyphae selector source
}
```

**Question**: exactly-one variant per subscription (`d`), or add a `Compound { all_of: [...] }` combinator (`e`)?

**Options**:
- (a) Files only — cheapest, narrowest
- (b) Symbols only
- (c) Selector only — most expressive but expensive
- **(d) tagged union, mutually exclusive** *(recommended)*
- (e) combinator over (a)+(b)+(c)

**Recommendation**: **(d)**. Files/Symbols are cheap fast paths for the dominant cases; Selector subsumes any compound need (Hyphae already supports path+name predicates). Adding a combinator later is additive (no v2 wire bump); removing one is painful.

### D2 — Selector-removal semantics

When a Hyphae-selector subscription's match-set changes, what counts as a "removed" entry?

**Options**:
- (i) `delta = NEW match-set only` — drops removals silently
- (ii) `delta = NEW ∪ (OLD match-set − NEW match-set)` — unrestricted
- **(ii-strict) `delta = NEW ∪ ((OLD − NEW) ∩ this-batch's actually-removed paths)`** *(recommended)*

**Recommendation**: **(ii-strict)**. Plain (ii) over-reports — a path may leave the match-set because a selector's predicate flipped on unrelated state, producing **phantom removals every batch**. (ii-strict) only reports a removal if the path was both in OLD AND its file was actually touched this batch. One extra set intersection per Selector — the only honest answer.

### D3 — Subscription lifecycle / leak avoidance

Crashed agents can leak subscriptions; selector subscriptions are expensive.

**Options**:
- TTL only (e.g. 30 min idle, rolling refresh on delivery)
- Hard caps only (per-client + server-wide)
- **TTL + caps + `peer.is_closed()` GC** *(recommended — defence in depth)*

**Recommendation**: **defence in depth**:
- **TTL** default `3600s`, max `86400s` (per-subscription, rolling on delivery)
- **Caps**: `MAX_SUBSCRIPTIONS = 256` server-wide, **per-client = 32**, **Selector-specific = 64**
- **Peer-close GC**: when `peer.is_closed()` detected, clear all that peer's subscriptions
- Excess returns `application_error code="subscription_limit"` (**never silent-drop-oldest**)

### D4 — `on_batch` signature change

Today the WatchEngine emit seam is `FnMut(&WatchEvent, &Store)`. SUBSCRIBE needs the **per-file BatchDelta** computed inside the same write-lock. Three ways to plumb it:

- **(a) Extend `on_batch` to `FnMut(&WatchEvent, &BatchDelta, &Store)`** *(recommended)* — additive third arg; RFC-0106 PUSH closure ignores it; single source of truth.
- (b) Side-channel: a separate `on_delta` callback registered in parallel — two emit paths with their own ordering hazards.
- (c) Recompute BatchDelta in MCP's `on_batch` via a `Store::diff_against_snapshot` helper — requires snapshotting the entire trunk before every batch (orders of magnitude more expensive than capturing changed-file old-sets inside the existing write-lock).

**Recommendation**: **(a)**. The only solution that respects the verified `watch/mod.rs:282-334` lock discipline. The signature bump is a **coordinated breaking change to `mycelium_core::watch::WatchEngine`** landed atomically with RFC-0107; the RFC-0106 PUSH closure adds `_delta` and ignores it (zero-cost).

### D5 — Three-Surface posture

- **(a) Extend the RFC-0105 EXCEPTION** *(recommended)*: CLI face = `mycelium watch --subscribe '<spec>'` printing NDJSON identical to MCP wire.
- (b) New EXCEPTION declaring SUBSCRIBE has no CLI twin.
- (c) Skill-only category (`skills/subscriptions/SKILL.md`) with no CLI surface.

**Recommendation**: **(a)**. Subscription lifetime is bounded by the watch loop lifetime — they're conceptually one reactive surface. CLI shorthand is genuinely useful for human debugging; one Skill (`index-management`, already updated for PUSH) keeps the trio together.

## 4. Frozen wire contracts (v1)

### `mycelium/subscriptionDelta` payload

```jsonc
{
  "method": "mycelium/subscriptionDelta",
  "params": {
    "event": "subscriptionDelta",
    "v": 1,
    "subscription_id": "01HZ...",
    "root": "/abs/path",
    "batch_seq": 42,
    "per_file": [
      {
        "file": "src/auth.rs",
        "added": ["src/auth.rs>fn:login"],
        "added_count": 1,
        "added_truncated": false,
        "modified": [],
        "modified_count": 0,
        "modified_truncated": false,
        "removed": ["src/auth.rs>fn:legacy_signin"],
        "removed_count": 1,
        "removed_truncated": false
      }
    ],
    "files_truncated": false,
    "interest_kind": "selector",
    "hint": "Apply the delta locally or re-query the affected paths."
  }
}
```

- **Per-array cap** = 50 (matches RFC-0106).
- **Per-file-list cap** = 16 (more than 16 files in one subscription's delta ⇒ `files_truncated: true`).
- All lists **sorted + deduped** by the server before send.

### MCP tool requests/responses

```rust
// Subscribe
pub struct SubscribeRequest {
    pub root: Option<String>,
    pub interest: Interest,
    pub subscription_id: Option<String>,  // regex ^[A-Za-z0-9_-]{1,64}$
    pub ttl_seconds: Option<u64>,         // default 3600, max 86400
}
pub struct SubscribeResponse {
    pub subscription_id: String,
    pub root: String,
    pub ttl_seconds: u64,
    pub interest_kind: String,            // "files" | "symbols" | "selector"
    pub active_count: u64,
}

// Error codes: "subscription_limit" (scope:"client"|"server"|"selector"),
//              "id_collision", "invalid_interest", "selector_too_large",
//              "root_not_allowed".
```

```rust
// Unsubscribe (idempotent)
pub struct UnsubscribeRequest  { pub subscription_id: String }
pub struct UnsubscribeResponse { pub removed: bool, pub active_count: u64 }

// Status
pub struct SubscriptionStatusRequest  { pub subscription_id: Option<String> }
pub struct SubscriptionStatusResponse {
    pub active_count: u64,
    pub max_subscriptions: u64,
    pub max_per_client: u64,
    pub max_selector: u64,
    pub watching: bool,
    pub subscriptions: Vec<SubscriptionInfo>,
}
```

### CLI shorthand

```
mycelium watch --subscribe files:src/a.rs,src/b.rs
mycelium watch --subscribe 'symbols:src/auth.rs>fn:*'
mycelium watch --subscribe 'selector:fn[name="login"]'
mycelium watch --subscribe-id my-id --subscribe '<spec>'
```

The shorthand parser produces a `SubscribeRequest`. A round-trip test asserts CLI NDJSON ≡ MCP JSON for the same input.

## 5. The lock seam (critical correctness point)

```text
write_lock acquired                                          ← watch/mod.rs:282
  ├─ for each changed_file:
  │    OLD = Store::symbols_in_file(file)     ← captured BEFORE remove_file
  │    Store::remove_file(file)
  │    reindexer.reindex(file, src, ext, store)
  │    NEW = Store::symbols_in_file(file)
  │    BatchDelta.per_file.push(SymbolDelta { file, added=NEW−OLD, modified=OLD∩NEW (by name, span changed), removed=OLD−NEW })
  ├─ Store::resolve_bare_call_stubs()
drop(write_lock)                                              ← engine drops lock
on_batch(&WatchEvent, &BatchDelta, &Store)                   ← MCP fan-out happens here
```

Doing the OLD snapshot lazily (per-file as `remove_file` is called) yields empty OLD sets for files processed early. The RFC mandates **batch-scoped pre-lock-entry snapshotting** — covered by `test_batch_delta_classifies_added_modified_removed_per_file`.

## 6. RED-first test plan (10 tests, distilled from team output)

1. `Store::symbols_in_file_returns_sorted_descendants_excluding_root`
2. `batch_delta_classifies_added_modified_removed_per_file` — the lock-discipline canary
3. `on_batch_signature_carries_batch_delta` — compile + observable value
4. `subscribe_files_emits_one_notification_per_matching_batch`
5. `subscribe_symbols_glob_matches_trunk_paths`
6. **`selector_removal_strict_ii`** — fails under plain (ii) and (i)
7. `dead_peer_gc_clears_all_subscriptions`
8. `per_client_and_total_caps_return_application_error`
9. `ttl_eviction_and_id_reuse`
10. **`three_surface_cli_mcp_byte_identical_payload`** — round-trip identity

Plus a regression test that all existing RFC-0105 watch tests + RFC-0106 push tests still pass (the `on_batch` signature change is the only public-API touch).

## 7. Risks (with mitigations)

- **Engine API break.** `on_batch` signature changes. Mitigation: land atomically; the third arg is purely additive; non-consumers add `_delta`. All in one PR.
- **OLD-set capture timing.** A lazy implementation yields empty OLD sets. Mitigation: §5 lock discipline spec'd + canary test.
- **Selector eval cost.** 64 Selector subs × ~1ms each = ~64 ms per batch — brushes Charter §2 reactive <10 ms SLA. Mitigation: 4096-char selector source cap + 64 Selector subs cap + documented; defer a selector→files reverse index to a follow-up.
- **Symbols-glob hotspot.** 5k subs × 200 deltas × 4 globs ≈ 600 ms worst-case. Mitigation: per-client cap = 32 enforced; document cost; bucketing deferred.
- **rmcp peer-close detection** reliability varies by transport. Mitigation: TTL is the primary mechanism; `peer.is_closed()` is the fast-path optimization.
- **Notification ordering.** PUSH and SUBSCRIBE fire on separate `tokio::spawn`s — order within a `batch_seq` is **not guaranteed**. Mitigation: v1 contract states `batch_seq` is the sole ordering primitive; clients reconcile by it.
- **Selector `last_match_set` memory.** 64 subs × 10000-path cap × ~80 B ≈ 50 MB worst-case. Mitigation: 10000-path cap per Selector subscription + degradation mode (mark `removed_truncated`); Files/Symbols subs hold `None`.

## 8. What this RFC does NOT do (deferred)

- **Selector→files reverse index.** If telemetry shows real pressure, a follow-up RFC adds it; the v1 wire shape is forward-compatible.
- **Bucketing Symbols subscriptions by file-glob prefix.** Same — defer until proven needed.
- **Persistence across server restarts.** Subscriptions live in memory only. Persistence is a post-salsa RFC.

## 9. Why this slot in the loop, again

```
watch engine commits batch
        │
        ├── on_batch(&WatchEvent, &BatchDelta, &Store)    ← new seam
        │       │
        │       ├── persist_watch_batch              ← RFC-0105 (existing)
        │       ├── PUSH: spawn mycelium/graphChanged ← RFC-0106 (existing; ignores _delta)
        │       └── SUBSCRIBE: match BatchDelta against
        │           every Subscription's Interest →
        │           spawn one mycelium/subscriptionDelta
        │           per matching sub                  ← NEW (RFC-0107)
```

Everything new fan-outs **after** the write lock drops (same discipline as PUSH), so the watch loop is **never** blocked on subscription matching or send.

## 10. Acceptance criteria

- [ ] Founder ratifies D1 / D2 / D3 / D4 / D5.
- [ ] Implementation PR opened against this RFC.
- [ ] 10 RED-first tests pass; all RFC-0105 + RFC-0106 tests still green.
- [ ] `skills/index-management/SKILL.md` documents the three new tools and the CLI shorthand.
- [ ] `CHANGELOG.md` "Unreleased / Added" entry includes the frozen v1 wire shapes.
- [ ] Quality gate clean (clippy -D warnings, fmt, cross-OS CI).

## 11. The five questions, restated

1. **D1 — Interest grammar**: tagged union, mutually exclusive (recommended (d)), or compound combinator (e)?
2. **D2 — Selector removal**: (ii-strict) recommended; or (i) / (ii)?
3. **D3 — Lifecycle**: TTL + caps + peer-close GC recommended; or simpler?
4. **D4 — `on_batch` signature change**: (a) recommended; or (b)/(c)?
5. **D5 — Three-Surface**: (a) extend RFC-0105 EXCEPTION recommended; or (b)/(c)?

Once you answer, the implementation PR is a focused MCP-side build (~400 lines + 10 tests) plus the one coordinated WatchEngine signature change. **No new transport. No new RFC. No new Skill file.**
