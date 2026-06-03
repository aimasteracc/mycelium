# RFC-0106: PUSH — server-initiated graph-changed notifications

- **RFC**: 0106
- **Title**: Server pushes a single `graphChanged` notification per committed watch batch
- **Status**: **Accepted (Option B — custom JSON-RPC method `mycelium/graphChanged`)** ratified by founder 2026-06-03. GraphChangedEvent v1 shape (§4) frozen; `changed_files` cap at 50. Implementation tracked separately.
- **Author**: rust-implementer
- **Created**: 2026-06-03
- **Depends on**: [RFC-0105](0105-shared-watch-engine-cli-watch.md) (provides the `on_batch` emit seam this RFC attaches to)
- **Blocks**: [RFC-0107](0107-…) (SUBSCRIBE — scoped delta push reuses this notification's payload shape)
- **Tracking**: reactive-completion roadmap, step 2 of 4
- **Decision gate**: this RFC selects a permanent, hard-to-reverse **wire contract** between the Mycelium MCP server and every connected agent. The contract chosen here is what subscribe (step 3) extends.

---

## 1. Summary

When the watch loop commits a batch (RFC-0105's `on_batch` callback), the MCP
server emits **one** server-initiated MCP notification carrying a frozen
`GraphChangedEvent` JSON payload that tells the connected agent: "the graph
just changed; here is what touched". The agent decides whether to re-query.

Today the agent is **pull-only**: it must re-query (`mycelium_context`,
`mycelium_search_symbol`, …) to discover changes. This RFC delivers the
blueprint's "last mile" — *"the mycelium network — invisible but everyone feels
it when one strand moves"*.

The watch loop changes **zero behavior**; this RFC only attaches a side-effect
to the `on_batch(&WatchEvent, &Store)` seam RFC-0105 deliberately left open.

## 2. What is fixed regardless of the founder's choice

These are not options — they are non-negotiable parts of the contract:

- **One notification per committed batch** (not per file). Coalescing happens
  inside RFC-0105's debounce; this RFC inherits that.
- **Best-effort delivery**: send failures (dead client, no client connected)
  are logged via `tracing::warn` and **never abort the watch loop**. Mirrors
  the existing best-effort `persist_watch_batch`.
- **Late client-attach is OK**: if a batch fires before any client connects,
  the notification is silently dropped (the server holds an
  `Arc<Mutex<Option<Peer<RoleServer>>>>` and gates on `if let Some(peer)`).
- **No request → response**: this is a **notification**, never a request. The
  agent never has to acknowledge.
- **The payload is JSON, capped, and carries a `truncated` flag** — see §4.

## 3. The transport-contract choice — **founder decides one**

`rmcp 1.7` (the SDK Mycelium uses) supports two outbound mechanisms on
`Peer<RoleServer>` that can carry our event. The choice is permanent: it goes
into every existing agent integration once shipped. The third option is a
hybrid.

### Option A — Standard `notifications/message` (LoggingMessage)

```rust
peer.notify_logging_message(LoggingMessageNotificationParam {
    level: LoggingLevel::Info,
    logger: Some("mycelium/graphChanged".into()),
    data: serde_json::to_value(&event)?,
}).await
```

Capability bit: `ServerCapabilities::builder().enable_logging()`.

**Pros**
- **Standards-compliant.** Every MCP client that implements the spec at all
  receives `notifications/message`. Most show it in a log pane by default.
- **Zero client-side coordination.** Existing agents just see "a log line" —
  they don't crash, they don't error.
- **Easy to inspect** in any MCP client UI for debugging.

**Cons**
- **Semantically wrong.** A graph-changed event is not a log message; it is a
  domain event. Agents that ignore logging (or filter to ≥ `Warning`) silently
  miss every notification.
- **Discoverability is awkward.** The agent has to filter `notifications/message`
  by `logger == "mycelium/graphChanged"` and parse `data` — heavier than reacting
  to a typed method name.
- **No first-class capability advertisement.** Clients can't statically know
  the server is going to emit this event-shaped log.

### Option B — Custom JSON-RPC method `mycelium/graphChanged` (**recommended**)

```rust
peer.send_notification(ServerNotification::CustomNotification(
    CustomNotification {
        method: "mycelium/graphChanged".into(),
        params: Some(serde_json::to_value(&event)?),
        extensions: Default::default(),
    }
)).await
```

**Pros**
- **Semantically correct.** A named method models a domain event, not a log
  line. Agents react by registering a notification handler for the method
  name.
- **Cleanly filterable.** No string-matching on `logger`; no level-filter
  collisions.
- **Future-proof.** SUBSCRIBE (RFC-0107) uses the same custom-method shape
  with a different name (`mycelium/subscriptionDelta`) — one mechanism for the
  whole reactive contract.
- **JSON-RPC 2.0 compliant for notifications**: unknown methods are silently
  dropped on the client side per the spec — no client crashes.

**Cons**
- **Adoption cost.** Agents that don't register a `mycelium/graphChanged`
  handler simply ignore the event. Documentation + Skill update needed.
- **Not visible in generic MCP UIs** that only render the standard
  notification types — debug visibility is lower than Option A.

### Option C — Hybrid (send both)

Emit Option A *and* Option B for the same batch. Maximum reach (works for
logging-only clients AND custom-handler clients).

**Pros**
- **Maximum compatibility** during a transition window.

**Cons**
- **Doubles the wire traffic** per batch (matters on high-churn watch streams).
- **Two contracts to maintain** — two places to drift.
- Best treated as a **migration window**, not a permanent contract.

### My recommendation

**Option B (`mycelium/graphChanged` custom method).** Reasons:
1. The reactive feature is a **domain capability**, not a log stream — modelling
   it as a typed method is the honest contract.
2. SUBSCRIBE (RFC-0107) will need a custom method either way (per-subscription
   delta payloads don't fit the logging shape semantically). Picking Option B
   now means **one mechanism for both push and subscribe**, not two.
3. Discoverability is a documentation problem (cheap to solve via the Skill +
   release notes), not a protocol problem.

If the founder values maximum-compat over clean semantics, Option A is the
honest fallback. **Option C should only be used as a time-boxed transition**
(e.g., "emit both for 2 releases, then drop A"), not a permanent contract.

## 4. The `GraphChangedEvent` payload (frozen at v1)

Same shape regardless of which transport is chosen:

```json
{
  "event": "graphChanged",
  "v": 1,
  "root": "/abs/path/to/workspace/root",
  "batch_seq": 17,
  "changed_files": [
    "src/auth.rs",
    "src/db/query.rs"
  ],
  "changed_count": 2,
  "truncated": false,
  "hint": "Re-query mycelium_context for the area you care about."
}
```

| Field | Type | Notes |
|---|---|---|
| `event` | `"graphChanged"` | Constant string — disambiguates this from future Mycelium events. |
| `v` | `1` | Schema version. Increments on any breaking change to this shape. |
| `root` | string | Absolute path of the watched root (the `WatchEvent.root` from RFC-0105). |
| `batch_seq` | u64 | Monotonic batch counter from RFC-0105 — clients can detect dropped notifications. |
| `changed_files` | `[]string` | Repository-relative, `/`-normalized. **Capped at 50** (sorted, deduped — engine already does this). |
| `changed_count` | u64 | Total number of changed files **before** the cap. Equals `changed_files.len()` when not truncated. |
| `truncated` | bool | `true` when `changed_count > 50`. Agents should treat as "many files changed; re-query broadly". |
| `hint` | string | A free-text human-friendly suggestion. Not load-bearing; agents may ignore. |

**Why capped at 50?** A single massive batch (e.g., `git checkout` on a 5k-file
project) should not blast a JSON-RPC frame the size of the diff. The cap +
`truncated` is the standard agent-context-friendly compromise.

**What is NOT in v1** (deliberately deferred):
- Per-file *what changed* (added/modified/removed counts) — SUBSCRIBE (RFC-0107)
  scope.
- Affected symbol summaries — also SUBSCRIBE scope.
- Causality / ordering across multiple watched roots — single-root assumption
  for v1; we don't run multi-watch in one server today.

## 5. Why this slot in the loop

RFC-0105 deliberately left the `on_batch(&WatchEvent, &Store)` callback as the
*post-mutation, post-lock-drop* emit point. PUSH attaches **inside that
callback** — *after* the store has its new state and the write lock is
released:

```rust
let on_batch = move |ev: &WatchEvent, _store_r: &Store| {
    watch_state.batches_processed.fetch_add(1, Ordering::Relaxed);
    if let Err(e) = persist_watch_batch(&ev.root, _store_r, &ev.changed_files) {
        warn!("could not persist watch batch: {e}");
    }
    // NEW: PUSH (RFC-0106).
    if let Some(peer) = notifier.lock().ok().and_then(|g| g.as_ref().cloned()) {
        let payload = GraphChangedEvent::from(ev);
        tokio::spawn(async move {
            let _ = peer.<send-call-from-option-A-or-B>(payload).await;
        });
    }
};
```

This means: a batch that **never wakes the agent** (e.g., agents that ignore
the method) still gets fully persisted; conversely a slow agent doesn't slow
the watch loop (notification is fire-and-forget via `tokio::spawn`).

## 6. Capturing the peer

`Peer<RoleServer>` only materializes **after** `server.serve(transport).await`
returns a `RunningService`. The watch loop launches from constructors that run
**before** `serve()`. Resolution:

```rust
// MyceliumServer gains:
notifier: Arc<Mutex<Option<Peer<RoleServer>>>>,

// In Cmd::Serve after .serve():
let running = server.serve(transport).await?;
*server.notifier.lock().await = Some(running.peer().clone());
running.waiting().await?;
```

A batch that fires in the pre-peer window (theoretically possible if a watch
starts during `with_root`'s pre-`serve` index seeding) simply skips the send,
guarded by `Option::is_some`. This is the same race the original MCP loop
silently exhibited (it had no peer to push to at all); we just make it
explicit and bounded.

## 7. Capability advertisement

- **Option A path**: add `enable_logging()` to the `ServerCapabilities`
  builder (line ~4995). Standards-clean; agents who don't read it still get
  the data.
- **Option B path**: no capability bit exists for custom methods. We advertise
  via `InitializeResult.instructions` (the same field the routing table already
  uses) — one sentence: *"This server emits `mycelium/graphChanged`
  notifications whenever the watched index re-builds."*

## 8. Three-Surface (Charter §5.13)

PUSH is intrinsically **a server-only capability** — the CLI is a one-shot
process with no long-lived client to push to. The CLI's reactive face is
already covered by RFC-0105's `mycelium watch` stdout (each batch prints a
line). So:

- MCP: emits the notification per §3.
- CLI: stdout line per batch from RFC-0105's `on_batch`.
- Skill: the existing `skills/index-management/SKILL.md` is updated to
  document the notification + the recommendation to register a handler for it.

This carries the **same `EXCEPTION:` line RFC-0105 already established** —
no new Three-Surface gate to cross.

## 9. Test plan (RED-first; deferred to the implementation PR)

1. `core` (none — PUSH is server-side only; no shared-core change required).
2. `mcp` `push_emits_one_notification_per_batch` — drive the watch loop with a
   recording fake client; write a file; assert exactly one outbound
   notification carrying a `GraphChangedEvent` whose `changed_files` includes
   the rel path and whose `batch_seq` matches RFC-0105's counter.
3. `mcp` `push_truncates_at_50_with_flag` — synthesize a batch of 60 files;
   assert payload has 50 files + `truncated: true` + `changed_count: 60`.
4. `mcp` `push_is_best_effort` — drop the captured peer; fire a batch; assert
   the watch loop continues (next batch still persists, `batches_processed`
   keeps incrementing).
5. `mcp` `push_skipped_before_peer_captured` — fire a batch before
   `Cmd::Serve` injects the peer; assert no panic and no notification (peer is
   `None`).
6. **No-regression**: the full RFC-0105 watch test suite passes unchanged.

## 10. Acceptance criteria

- [x] Founder ratified Option B (custom method `mycelium/graphChanged`) 2026-06-03.
- [x] Founder approved the `GraphChangedEvent` v1 shape (§4) including 50-file cap.
- [ ] Implementation PR opened against this RFC.
- [ ] 4 RED-first MCP tests pass; RFC-0105 watch tests still green.
- [ ] `skills/index-management/SKILL.md` documents how an agent registers a
      handler for `mycelium/graphChanged`.
- [ ] `CHANGELOG.md` "Unreleased / Added" entry includes the new transport.

## 11. What this RFC does NOT do (deferred)

- **No per-subscription filtering.** Every notification goes to every connected
  client. SUBSCRIBE (RFC-0107) layers filtered scoped pushes on top of this
  mechanism.
- **No retry / replay.** A dead client misses the event; on reconnect it should
  re-query. Reliable delivery is out of scope for v1.
- **No multi-root coordination.** Single watched root per server, matching
  current MCP behavior.

## 12. Pre-implementation founder questions

These three answers unlock the implementation PR:

1. **Transport**: A, B, or C? (Recommendation: **B**.)
2. **Payload v1 shape**: ratify §4 as written, or amend?
3. **Cap value**: 50 looks right to me — confirm or pick a different number.

Once these are settled, the implementation PR lands as a small addition to
`MyceliumServer` (notifier field + peer capture in `Cmd::Serve` + ~10 lines in
the `on_batch` closure) plus 4 RED-first tests. No `mycelium-core` change. No
CLI change.
