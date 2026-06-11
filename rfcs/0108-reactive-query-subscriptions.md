# RFC-0108: Reactive query subscriptions (Salsa Phase 2)

- **RFC**: 0108
- **Title**: Subscribe to a *query result* — receive a notification only when its value actually changes
- **Status**: **Implemented** *(merged to develop via PR #480, 2026-06-03; shipped in v0.1.18. Salsa Phase 2 reactive query subscriptions live in `crates/mycelium-mcp/src/subscription.rs` + `crates/mycelium-core/src/cortex.rs`.)*
- **Author**: rust-implementer (autonomous-mode draft)
- **Created**: 2026-06-03
- **Depends on**:
  - [RFC-0003](0003-hyphae-query-language.md) (Cortex Phase 1, `Cortex::query_file` already memoised)
  - [RFC-0105](0105-shared-watch-engine-cli-watch.md) (shared `WatchEngine`)
  - [RFC-0106](0106-push-graph-changed-notification.md) (`Peer` capture + `CustomNotification` transport)
  - [RFC-0107](0107-subscribe-scoped-delta.md) (subscription store + 3 MCP tools + CLI `--subscribe`)
- **Closes**: the reactive-completion roadmap, step **4/4** (watch → push → subscribe → **salsa**)

---

## 1. Summary

RFC-0107 lets an agent subscribe to **file or symbol changes** and receive scoped
deltas. RFC-0108 lets an agent subscribe to a **query result** — e.g. "the
callers of `src/auth.rs>fn:login`", "the result of this Hyphae selector", or
"the impact set of this symbol" — and receive a `mycelium/queryResultChanged`
notification **only when the value actually changes** (Salsa-style backdated
equality).

The mechanism is a tiny, additive bolt-on to RFC-0107:

- A new `Interest::Query { query: QuerySpec }` variant.
- A new method name `mycelium/queryResultChanged` (separate from RFC-0107's
  `mycelium/subscriptionDelta` so clients can route them differently).
- A small `cortex::query_cache` layer that hashes the post-batch query result
  and only emits when the hash changes.

No new transport. No new auth surface. The CLI `--subscribe` shorthand grows
one extra prefix (`query:<kind>:<args>`).

## 2. What's fixed regardless of founder answers

| Locked | Why |
|---|---|
| Reuses the RFC-0107 `Subscription` store, TTL machinery, peer-close GC, and caps (`MAX_SUBSCRIPTIONS`, `MAX_PER_CLIENT`). | One mechanism for the whole reactive contract; no parallel plumbing. |
| Stored as a new variant on the existing `Interest` enum (additive — older clients that don't send it still work). | Forward-compatible. |
| Result-change detection = **content hash of the canonical-JSON serialisation of the result** (BLAKE3-128 of `serde_json::to_vec(&result).unwrap()`, sorted-key). | Cheap, deterministic, language-agnostic, no `Eq` required on every query result type. |
| Re-evaluation triggered by **the watch batch boundary** (same `on_batch` seam as RFC-0107). | The watch loop is already the single source of "the graph just changed"; query subscriptions piggy-back on it. |
| **Best-effort, fire-and-forget delivery**; dead client never aborts the watch loop. | Same discipline as RFC-0106 / RFC-0107. |
| **One notification per subscription per batch maximum** — even if a query depends on 100 files in the batch, the agent gets one delta. | Prevents notification storms. |
| Sub fan-out happens AFTER the write lock drops (same discipline as RFC-0107). | Watch loop is never blocked on query evaluation. |
| Per-query evaluation budget: **soft 50 ms**, hard 200 ms. Above hard cap, the subscription is **paused** for `cooldown_seconds=60` and an `application_warning` notification is delivered. | Pathological queries cannot starve the watch loop. |

## 3. The four founder decisions

### D1 — Query catalogue (v1 scope)

Which query kinds does v1 of `Interest::Query` accept?

**Options**:
- (a) `selector` only — reuse the Hyphae source string from RFC-0107
- (b) `selector` + `callers` + `callees` — the three most useful for agents
- **(c) `selector` + `callers` + `callees` + `impact` + `context`** *(recommended)*
- (d) every existing MCP query tool (~30+)

**Recommendation**: **(c)**. The 5 query kinds in (c) cover the 90% case for an agent watching a refactor land. `impact` and `context` are where Salsa-style deduplication matters most (cheap to compute, expensive in tokens to re-fetch). (d) is forward-compatible — additive — so deferring it costs nothing.

```rust
pub enum QuerySpec {
    Selector { hyphae: String },                // reuses RFC-0107 Selector
    Callers  { path: String, hops: Option<u32> },
    Callees  { path: String, hops: Option<u32> },
    Impact   { path: String, max_paths: Option<u32> },
    Context  { task: String, focus: Vec<String>, max_tokens: Option<u32> },
}
```

Each variant maps directly to an existing MCP tool's pure-function body — no new query logic invented.

### D2 — Result-change reporting shape

When the result changes, what does the notification carry?

**Options**:
- (i) `{ new_result: <whole value> }` — simple, full re-send
- **(ii) `{ new_result, summary: { added: [...], removed: [...] } }` for set-like results; `{ new_result }` for scalar results** *(recommended)*
- (iii) `{ diff_patch: <jsonpatch> }` — most efficient, most complex client-side

**Recommendation**: **(ii)**. Hybrid: when the query result is naturally a *set of trunk paths* (Callers/Callees/Impact/Selector), include `summary.added` and `summary.removed` (RFC-0107 §4 §1-style). When the result is a structured tree (Context/CalleeTree/CallerTree), just include `new_result` — diff'ing trees inline is too complex for v1. Both shapes share a `result_hash_old` / `result_hash_new` pair so the client can reconcile.

### D3 — Quiet-period (rate limit)

A subscriber to `Context(task="auth")` could receive a notification every batch if any file in the focus area changes. How do we prevent thrashing?

**Options**:
- (a) None — fire on every batch where hash changes
- (b) Per-subscription `min_interval_seconds` (default 5s) — coalesce; emit the latest result at the next tick
- **(c) Default `min_interval_seconds=2s` server-side; subscriber can override up to `max_min_interval=300s`** *(recommended — defence in depth)*

**Recommendation**: **(c)**. 2 s default protects the agent from thrashing during burst edits without delaying meaningful change feedback. The `max_min_interval` cap prevents an attacker from setting `min_interval=10000s` and effectively pausing their subscription indefinitely while still holding a cap slot.

### D4 — Three-Surface posture

- **(a) Extend the RFC-0105 EXCEPTION (which RFC-0107 already extended)** *(recommended)* — CLI `mycelium watch --subscribe 'query:<kind>:<args>'` prints the new notification as NDJSON, identical to the MCP wire.
- (b) New EXCEPTION line for query subscriptions specifically.
- (c) MCP-only — query subscriptions have no CLI equivalent.

**Recommendation**: **(a)**. Same lifecycle binding (watch loop) as RFC-0107. Same Skill (`index-management`). One CLI surface stays a single conceptual reactive surface. The byte-identical contract test from RFC-0107 (`three_surface_cli_mcp_byte_identical_payload`) is extended with a `query:` case.

## 4. Frozen wire contracts (v1)

### `mycelium/queryResultChanged` payload

```jsonc
{
  "method": "mycelium/queryResultChanged",
  "params": {
    "event": "queryResultChanged",
    "v": 1,
    "subscription_id": "...",
    "root": "/abs/path",
    "batch_seq": 42,
    "query_kind": "callers",   // "selector" | "callers" | "callees" | "impact" | "context"
    "result_hash_old": "b3:...",  // BLAKE3-128 hex; null on first delivery
    "result_hash_new": "b3:...",
    "new_result": { /* shape depends on query_kind; matches the equivalent MCP tool's response.data */ },
    "summary": {            // present iff result is set-shaped (callers/callees/impact/selector)
      "added":   ["src/foo.rs>fn:bar"],
      "added_count": 1,
      "added_truncated": false,
      "removed": [],
      "removed_count": 0,
      "removed_truncated": false
    },
    "evaluation_ms": 7,
    "hint": "Result of `callers(src/auth.rs>fn:login)` changed."
  }
}
```

- Per-array cap = 50 (matches RFC-0107).
- `summary` field omitted entirely for tree-shaped results (`context` / `callee_tree` / `caller_tree`).
- `result_hash_*` are BLAKE3-128 hex strings, prefix `b3:`, **frozen at v1**.

### `Interest::Query` variant on the existing `Interest` enum

```rust
pub enum Interest {
    Files    { paths: Vec<String> },            // RFC-0107
    Symbols  { paths: Vec<String> },            // RFC-0107
    Selector { hyphae: String },                // RFC-0107
    Query    { query: QuerySpec, min_interval_seconds: Option<u64> }, // NEW (RFC-0108)
}
```

With serde tag `kind = "query"`. Old clients that don't send `kind:"query"` still
work. New clients can also send the older 3 kinds.

### MCP tool changes

`mycelium_subscribe` accepts the new `Interest::Query` variant — no new tool, no
new request envelope, no breaking change to existing fields. `SubscribeResponse`
gains an optional `query_kind: Option<String>` field (omitted when not a query
subscription).

### CLI shorthand

```
mycelium watch --subscribe 'query:callers:src/auth.rs>fn:login'
mycelium watch --subscribe 'query:callers:src/auth.rs>fn:login,hops=2'
mycelium watch --subscribe 'query:selector:fn[name="login"]'
mycelium watch --subscribe 'query:impact:src/auth.rs>fn:login,max_paths=100'
mycelium watch --subscribe 'query:context:auth,focus=src/auth.rs+src/db.rs,max_tokens=4000'
```

Grammar is comma-separated `key=value` pairs after the path; round-tripped via
a contract test (`three_surface_cli_mcp_byte_identical_payload` extended).

## 5. The evaluation seam

```text
watch engine commits batch
        │
        ├── on_batch(&WatchEvent, &BatchDelta, &Store)             ← RFC-0107 seam
        │       │
        │       ├── persist_watch_batch                            ← RFC-0105 (existing)
        │       ├── PUSH: spawn mycelium/graphChanged              ← RFC-0106 (existing)
        │       ├── SUBSCRIBE (file/symbol/selector): spawn        ← RFC-0107 (existing)
        │       │   one mycelium/subscriptionDelta per match
        │       └── QUERY-SUBSCRIBE: for each Interest::Query sub  ← NEW (RFC-0108)
        │           ├── if min_interval not elapsed → skip
        │           ├── evaluate QuerySpec against &Store
        │           ├── hash(canonical_json(result))
        │           ├── if hash == sub.last_hash → skip
        │           ├── compute summary (set-shaped queries only)
        │           ├── spawn mycelium/queryResultChanged
        │           └── update sub.{last_hash, last_emit_at}
```

Evaluation happens INSIDE the `on_batch` callback on a read-lock of the post-
batch store. Each query evaluator must be **read-only** — any attempt to mutate
the store from inside a tracked query is a bug (compile-checked via `&Store`).

## 6. RED-first test plan (8 tests)

1. `query_spec_parsing_round_trips_for_all_5_kinds` — serde + CLI parser.
2. `result_hash_stable_across_serde_orderings` — canonical-JSON canary.
3. `callers_subscription_fires_only_on_actual_change` — file with no
   semantic delta but a whitespace change does NOT fire (hash stays).
4. `min_interval_coalesces_burst_edits` — 5 rapid batches → 1 emit.
5. `set_shaped_summary_added_removed_consistent` — Callers result goes
   from `{A, B}` → `{A, C}`; summary reports `added=[C], removed=[B]`.
6. `tree_shaped_omits_summary` — Context result change carries `new_result`
   only, no `summary` field.
7. `evaluation_budget_pauses_runaway_subscription` — a query that takes
   500 ms triggers `subscription_paused` warning + `cooldown` field;
   subsequent batches skip eval until cooldown elapses.
8. `three_surface_query_cli_mcp_byte_identical_payload` — round-trip
   identity for one each of `callers`, `selector`, and `context`.

Plus regression: all RFC-0105 + RFC-0106 + RFC-0107 tests still pass.

## 7. Risks

- **Query evaluation cost on hot batches.** Mitigated by `min_interval` (D3),
  per-query budget (§2), and the per-client cap inherited from RFC-0107.
- **`Context` result shape stability.** RFC-0101 fixed the `mycelium_context`
  response shape but downstream changes could destabilise hashes. Mitigation:
  the canonical-JSON canary test catches it as a regression.
- **Storage of `last_hash` per subscription.** Tiny — 16 bytes BLAKE3 per
  query subscription. Caps inherit from RFC-0107.
- **No real Salsa graph yet.** This RFC delivers Salsa *behaviour* (result-
  deduplication, change-driven notification) without lifting the trunk store
  into Salsa. A follow-up RFC (post-redb full reactive integration) can do that
  with no wire-shape change.

## 8. What this RFC does NOT do (deferred)

- **Lifting the trunk Store into Salsa.** A genuine `salsa::input` for every
  node would let queries skip evaluation entirely when none of their reads
  invalidated. Deferred — the v1 wire is forward-compatible.
- **Cross-query memoisation.** Two subscriptions to identical `callers` queries
  re-evaluate independently. Deferred — fix when telemetry shows pressure.
- **Streaming partial results for large queries.** v1 emits whole values once
  per change. Deferred.
- **Persistence across restarts.** Subscriptions still live in memory only
  (matches RFC-0107).

## 9. Acceptance criteria

- [x] Founder ratifies D1 / D2 / D3 / D4. *(autonomous-mode recommendations
      applied: D1=(c) 5 query kinds, D2=(ii) hybrid summary, D3=(c) 2 s
      default min_interval, D4=(a) extends RFC-0105 EXCEPTION)*
- [x] Implementation PR opened. *(branch `feature/rfc-0108-impl`; commits
      `feat(mcp): scaffolding`, `test(mcp): 8 RED-first tests`,
      `feat(cli): query:<kind>:<args> shorthand`)*
- [x] 8 RED-first tests pass; all RFC-0105 + RFC-0106 + RFC-0107 tests still
      green. *(30 subscription tests, 387 MCP lib tests, 2 contract tests —
      all green)*
- [x] `skills/index-management/SKILL.md` updated with the new method + the
      new CLI shorthand.
- [x] `CHANGELOG.md` "Unreleased / Added" entry includes the frozen v1 wire.
- [x] Quality gate clean (clippy `-D warnings`, fmt, cross-OS CI).
- [x] Reactive-completion roadmap §0 declared complete (watch ✅ push ✅
      subscribe ✅ salsa ✅).

## 10. The four questions, restated

1. **D1 — Query catalogue**: (c) 5 kinds *(selector, callers, callees, impact, context)* recommended? Or smaller / larger?
2. **D2 — Result reporting**: (ii) hybrid `summary` for set-shaped, `new_result` only for tree-shaped recommended? Or simpler?
3. **D3 — Quiet-period**: (c) 2 s default + per-sub override capped at 300 s recommended?
4. **D4 — Three-Surface**: (a) extend the RFC-0105 EXCEPTION (which RFC-0107 already extended) recommended?

Once ratified, the implementation is a focused MCP-side build (~250 LOC on top
of RFC-0107's subscription module + 5 query-spec evaluator shims that each call
the existing MCP tool's pure-function body) + 8 RED-first tests + CLI grammar.

**No new transport. No new RFC after this. No new Skill file.** This RFC closes
the reactive-completion roadmap.
