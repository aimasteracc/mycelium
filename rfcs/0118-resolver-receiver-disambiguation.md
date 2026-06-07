# RFC-0118: Resolver receiver disambiguation and unresolved-node isolation

- **Status**: Draft
- **Created**: 2026-06-06
- **Depends on / supersedes**: Builds on RFC-0092 (alias-aware dispatch) and RFC-0103 (import-aware cross-file resolution). Orthogonal to — and explicitly NOT folded into — RFC-0113 (`classify.rs` / `CalleeClass`). Must land **before** RFC-0114 Phase 2 (PR #606) finalizes health-grade thresholds, or coordinate in the same PR (see Risks). RFC-0118 precedes RFC-0119. Honors ADR-0010 (no live LSP) and Charter §4 (≤3-file packs) / §5.13 (Three-Surface).

---

## Summary

RFC-0118 fixes **two distinct correctness bugs** that share the unresolved-receiver code path, plus a third — the over-broad **all-edges-share-one-stub** redirect model — that the original draft's Part B design would have made worse.

- **Part A (de-noise the symbol universe).** Unresolved callees today are minted as kind-less placeholder nodes (`store.upsert_node(...)` at `extractor/mod.rs:393` and `:401-402`). They carry a real `>`-qualified path, so `all_symbols`, `page_rank`, and `rank_symbols` count them as real code. Part A gives unresolved callees a dedicated `NodeKind::Unresolved` marker so the **three** genuinely-leaking queries exclude them by kind, not by the fragile `p.contains('>')` string heuristic.
- **Part B (bind method calls to definitions).** A method like `upsert_node` is defined on many types, so the simple resolver's `if matches.len() == 1` gate (`store/mod.rs:1209`) refuses to bind it and `get-callers` returns 0. Part B adds **static receiver-type inference** to disambiguate the multi-match case via a **two-phase** design: **capture** per-call-site receiver context into a `Store` side table at extraction time (where the receiver identifier is in scope), then **resolve** it in a post-merge pass against the complete store — so it is correct regardless of file/chunk indexing order, and never mis-binds the shared stub.

Everything is landed **pure-core-first**: a side-effect-free `resolver::receiver` module with unit tests, then the `NodeKind::Unresolved` set-query work, then extractor-time wiring plus the pack captures the headline Rust example actually requires.

---

## Motivation

Two dogfood findings were initially attributed to one root cause. Independent review against `develop@755c204` showed the F1 attribution was **partly empirically wrong**, and that the proposed Part B fix had a **category error**. This motivation reflects the corrected, code-verified picture.

### F1 — phantom nodes pollute set-queries (CORRECTED SCOPE)

When a call's receiver/callee cannot be resolved, the extractor mints a placeholder node and unconditionally links a `Calls` edge to it (`extractor/mod.rs:407`) so the caller is not falsely "dead". Two placeholder shapes are minted:

- **bare, container-less, kind-less**: `TrunkPath::parse(callee_name)` → `store.upsert_node(bare)` at `extractor/mod.rs:401-402` (e.g. `unwrap`, `HashMap`).
- **qualified-but-undefined**: when a receiver alias resolves to a prefix, `format!("{prefix}>{callee_name}")` is upserted at `extractor/mod.rs:393` even though no definition with that path exists.

The original draft claimed these phantoms are "counted as real symbols by `dead_symbols`, `isolated_symbols`, `entry_points`". **This is refuted by the code.** All three queries gate on **incoming edges being empty**:

- `dead_symbols` (`store/mod.rs:2419-2426`): requires `incoming(Calls).is_empty() && incoming(Imports).is_empty()`.
- `isolated_symbols` (`store/mod.rs:3282+`): requires zero in/out across all kinds.
- `entry_points` (`store/mod.rs:929+`): requires `incoming(Calls)` empty.

Every minted phantom is the **target** of a `Calls` edge, so it has in-degree ≥ 1 and is **already excluded** from all three. The genuinely-leaking queries are **three** (the draft said two; Codex P2 on PR #609 added the third):

- **`all_symbols(None, None)`** (`store/mod.rs:2459`): filters `p.contains('>')` with **no edge-degree gate**, so the `prefix>callee` phantom is counted.
- **`page_rank`** (`store/mod.rs:3937`): seeds from `symbol_nodes()` (a pure `p.contains('>')` filter, `trunk/mod.rs:244`), so the phantom enters the rank universe and is allotted PageRank mass; the output filter has no kind exclusion.
- **`rank_symbols`** (`top_symbols_by_incoming` / `top_callee_symbols`): these iterate `trunk.all_paths()` and rank by incoming-edge count, and the extractor deliberately adds a `Calls` edge to every phantom — so both bare and qualified unresolved targets get ranked and can outrank real symbols in `mycelium rank-symbols` / `mycelium_rank_symbols`. Part A's `is_real_symbol` predicate gates this query too (with a CLI↔MCP parity snapshot — see AC-20).

> The bare phantom (no `>`, e.g. `unwrap`) does **not** enter `all_symbols`/`page_rank`/`symbol_nodes()` (no `>`), and `all_file_paths` is already interim-guarded by a `NodeKind::File` presence-gate (`store/mod.rs:904`). Part A generalizes that presence-gate pattern from one query to the two that actually leak, and replaces the heuristic with an authoritative predicate.

### F5 — method callers = 0

`resolve_bare_call_stubs_simple` (`store/mod.rs:1191`) redirects a bare stub only when **exactly one** definition shares the simple name (`if matches.len() == 1`, line 1209). Free functions (unique names) resolve fine. A method like `upsert_node` is defined on many types, so `matches.len() > 1`, the gate refuses, the stub dangles, and the real `…>Store>upsert_node` never receives the redirected edge. `get-callers` on any popular method returns 0 — the single most valuable edge class for impact/blast-radius queries is silently dropped.

### F5 root cause is deeper than the draft assumed (CONFIRMED architecture flaw)

The original draft proposed building a `ReceiverContext` **per stub** in the post-pass and calling `disambiguate`. **This is a category error.** The bare stub `upsert_node` is a **single** node in the trie (paths are unique). Every unresolved `x.upsert_node()` across the whole codebase links its `Calls` edge to that one shared node. Synapse edges are plain `(NodeId, NodeId)` pairs with **no call-site/receiver context** (`synapse/mod.rs:26-49`), and `redirect_node` rewires **all** edges touching the stub to one target (`synapse/mod.rs:230-234`). So a stub has no single receiver — it aggregates many call sites with potentially different receiver types (`store: Store`, `t: Trunk`). Redirecting the shared stub to `…>Store>upsert_node` would silently re-point the `t.upsert_node()` callers too, **manufacturing wrong edges** — the exact precision regression Part B claims to avoid.

The fix: per-call-site receiver context is only recoverable **at extraction time** (`extractor/mod.rs:324-407`), where the receiver identifier and the local/param/field/import context for *that* call site are in hand — so RFC-0118 **captures** it there into a side table, but **resolves** it post-merge (see Decision Part B), because the candidate definitions may live in files indexed later. The shared-stub post-pass remains strictly the conservative fallback for contexts that stay ambiguous.

Both F1 and F5 are correctness bugs in static resolution — the governing concern of RFC-0092 and RFC-0103. RFC-0113 (`classify.rs`) is adjacent but **not** the mechanism: it labels the unresolved tail (stdlib/builtin/external) for output; it does not bind method calls to definitions nor remove phantom nodes from set-queries.

---

## Decision

Adopt a three-part static fix, landed pure-core-first.

**Part A — Unresolved nodes get a distinct kind; never members of the leaking symbol-universe queries.**
Add `NodeKind::Unresolved` (a non-File, non-symbol marker). The extractor's two unresolved fallbacks (`extractor/mod.rs:393` and `:401-402`) call `upsert_node_with_kind(path, NodeKind::Unresolved)` instead of bare `upsert_node`. The three genuinely-leaking queries — **`all_symbols`, `page_rank`** (via `symbol_nodes`), **and `rank_symbols`** (`top_symbols_by_incoming`/`top_callee_symbols`) — exclude `NodeKind::Unresolved` by **kind**, via an authoritative `is_real_symbol(id)` predicate. This is presence-gated exactly like the existing `all_file_paths` guard: a purely programmatic test store that never set kinds keeps the legacy string contract. `dead_symbols` / `isolated_symbols` / `entry_points` are **also** kind-gated for defense-in-depth and forward-safety (so a future zero-in-degree Unresolved node can never leak), but the RED tests for those three are founded on the **`all_symbols`/`page_rank`** leak that is real today (see Acceptance), not on a pre-state that is already green.

**Part B — Static receiver-type inference at EXTRACTION TIME to disambiguate multi-match method calls.**
A pure function `infer_receiver_type(ctx) -> Option<TypeName>` operates on plain structs extracted statically (no LSP, ADR-0010). When inference yields a concrete type `T` and exactly one candidate definition is `…>T>method`, the extractor binds the `Calls` edge directly to that definition for **that call site**. When inference is `None`/ambiguous, the extractor falls back to today's behavior (mint the shared stub), and the post-pass resolver remains exactly as conservative as today. Net effect: per-call-site method calls with a statically inferable receiver type bind correctly (F5 fixed for the common case, **without** cross-receiver mis-binding); everything else is unchanged.

**Part C — Resolver-pass hygiene (orphan-kind fix + pass ordering).**
The draft's claim that "the marker disappears with redirect_node + trunk.remove — no orphan kinds" is **false**: `resolve_bare_call_stubs_simple` calls raw `self.trunk.remove(stub_id)` (`store/mod.rs:1212`), which mutates only `by_id` + the trie node (`trunk/mod.rs:223-232`) and never touches `kind_map`. Only `Store::remove_node` (`store/mod.rs:729-734`) cleans `kind_map`/`span_map`. Since `NodeId` is content-derived from the path, a stale `kind_map[stub_id] = Unresolved` survives across re-index cycles. RFC-0118 routes all three resolution passes' stub removal through `Store::remove_node` (or an explicit `kind_map.remove` + `span_map.remove` alongside `trunk.remove`), and adds an explicit kind-cleanup acceptance criterion.

---

## Design

### PURE CORE — new module `crates/mycelium-core/src/resolver/receiver.rs`

Data model (plain structs, owned `String`s, immutable inputs):

```rust
pub struct ReceiverContext {
    pub receiver: String,            // the receiver identifier at THIS call site
    pub method: String,
    pub imports: Vec<AliasBinding>,
    pub locals: Vec<LocalBinding>,
    pub self_type: Option<String>,   // enclosing impl/class type
    pub params: Vec<ParamBinding>,
    pub fields: Vec<FieldBinding>,
}
pub struct AliasBinding { pub local: String, pub resolved_path: String }   // mirrors extractor alias_table
pub struct LocalBinding { pub name: String, pub ctor_type: Option<String> } // from `let x = T::new()` / `x = T(...)` / `new T()`
pub struct ParamBinding { pub name: String, pub declared_type: Option<String> }
pub struct FieldBinding { pub name: String, pub declared_type: Option<String> }
pub enum TypeName { Path(String), Simple(String) }
pub struct Candidate { pub node_path: String }   // a `…>Type>method` definition path
pub enum Resolution { Unique(String), Ambiguous }
```

Pure functions (deterministic, side-effect-free, ≥90% covered):

**1. `infer_receiver_type(ctx: &ReceiverContext) -> Option<TypeName>`** — precedence, highest first:

- **a.** `self`/`cls`/`this` receiver → `ctx.self_type` (enclosing impl/class type, already discoverable via `enclosing_class_chain`, `extractor/mod.rs:591`).
- **b.** receiver matches a `ParamBinding.name` with a `declared_type` → that type (annotation-driven, high precision).
- **c.** receiver matches a `LocalBinding.name` with a `ctor_type` → that type (the `let store = Store::new()` case — the exact F5 example; constructor-driven, high precision).
- **d.** receiver matches a `FieldBinding.name` with a `declared_type` → that type (`self.store.upsert(...)` with field `store: Store`).
- **e.** receiver matches an `AliasBinding.local` → the imported path's terminal segment as a type (import-driven, medium precision).

Returns `None` when no rule fires. **No** rule crosses function boundaries, tracks reassignment, or resolves overloads — recall is deliberately capped to keep precision at 100% for the new tier.

**2. `disambiguate(inferred: Option<TypeName>, candidates: &[Candidate]) -> Resolution`**:

- `Resolution::Unique(path)` when `candidates.len() == 1` (preserves today's single-match behavior byte-identical), **or** when `inferred == Some(T)` and exactly one candidate path matches `>{T}>{method}`.
- `Resolution::Ambiguous` otherwise (multi-match with no/ambiguous inference). No guess.

> **Container-name matching caveat (reviewer-surfaced).** Rust impl-block container names come from `container_name(impl_item)` via `enclosing_class_chain` (`extractor/mod.rs:591-603`). For `impl<T> Foo<T>` or `impl Trait for Store`, the recorded segment and the inferred ctor type may not be byte-identical (generics, trait-impl forms), so `disambiguate` declines to `Ambiguous`. This is consistent with conservative-decline (no wrong edge) but narrows F5 recall; the limitation is stated explicitly in Risks rather than hidden behind "common case fixed".

### STATIC inputs — what inference MAY and MAY NOT use

**MAY**: import/alias tables (already built, `extractor/mod.rs`), local `let`/`const`/assignment bindings whose RHS is a constructor call (`T::new`, `T(...)`, `new T()`), `self`/`cls`/`this` mapped to the enclosing impl/class type, parameter type annotations, struct/class field type declarations, and (future) consumed external SCIP/LSIF artifacts per ADR-0010's sanctioned escape hatch.
**MAY NOT**: spawn or query any language server; perform flow-sensitive reassignment tracking; cross function boundaries; do trait/overload resolution.

### EXTRACTOR-SIDE wiring (Part B) — at the call site, Charter §4 compliant

The extractor already reads the receiver identifier at `extractor/mod.rs:326-332` and computes a per-call-site `resolved_target` at `:366`.

> **CORRECTION (Codex P1, PR #609): extraction-time *binding* is order-dependent and must NOT be done inline.** The CLI extracts every file/chunk first and only calls `resolve_bare_call_stubs()` after all files are merged. For `let s = Store::new(); s.upsert_node();` where `Store` is defined in a *later* file/chunk, an inline `disambiguate` at extraction time would see **no candidate**, fall back to the shared bare stub, and — because the shared stub carries no per-call-site context — the receiver type would be **lost**, leaving F5 unresolved and order-dependent. RFC-0118 therefore **captures** receiver context at extraction time but **resolves** it post-merge, two-phase:

1. **Extraction (capture, no binding).** For every method call site, the extractor appends a `CallSiteContext { caller_id, method_name, receiver_ctx }` to a new `Store` side table (`call_site_contexts: Vec<CallSiteContext>`), where `receiver_ctx` holds the in-scope alias table + binding facts (locals/params/fields/self) for *that* call site. It still mints the kinded placeholder `upsert_node_with_kind(.., NodeKind::Unresolved)` + the conservative `Calls` edge exactly as today (so the caller is never falsely "dead"). No binding decision yet — later-file definitions do not exist at this point.
2. **Post-merge resolution.** A new pass `resolve_call_site_contexts()` runs **after** the trie/synapse hold every definition (alongside the existing `resolve_bare_call_stubs()`). For each `CallSiteContext`: `infer_receiver_type(receiver_ctx)` → `disambiguate` over the now-complete candidate set.
   - `Resolution::Unique(path)`: **rewire that call site** — add `Calls(caller_id → path)`; the caller→stub edge is removed only once *all* of that caller's call sites targeting the stub have resolved (a caller with both `store.upsert_node()` and `trunk.upsert_node()` produces two contexts → two precise edges; the shared stub edge is dropped only when none remain unresolved).
   - `Resolution::Ambiguous`/`None`: leave the conservative stub edge; the existing single-match post-pass still applies.

This makes F5 **order-independent**: a receiver type defined anywhere resolves, because resolution runs against the merged store, not mid-extraction. The `call_site_contexts` table is Codex's "post-merge call-site context table".

The binding facts come from **new tree-sitter captures** added to each `packs/<lang>/queries.scm`: `@binding.local`, `@binding.ctor`, `@param.type`, `@field.type`. **Data only — no core edit adds a language** (1 file/pack, well within ≤3, Charter §4). The pure resolver stays language-agnostic.

> **CORRECTION (reviewer-confirmed): the Rust pack does NOT have a receiver capture.** The draft claimed "Rust pack already has the call/receiver captures (`packs/rust/queries.scm:151-169`)". Verified false: `packs/rust/queries.scm:152-156` captures only `@name` on the `field_expression` method form — there is **no** `@call.receiver`. `@call.receiver` exists **only** in `packs/python/queries.scm:165` and `packs/typescript/queries.scm:144`. For Rust, `receiver` is always `None`, so the F5 headline fixture cannot resolve today. **Phase 2b MUST add `@call.receiver` to the Rust method-call query** (capture the `value`/`object` node of the `field_expression`) **in addition to** the four binding captures. This is the prerequisite for acceptance criterion AC-8.

### STORE INTEGRATION (Part A + Part C — thin)

- **Part A**: switch the two extractor fallbacks to `upsert_node_with_kind(..., NodeKind::Unresolved)` (`upsert_node_with_kind` already exists, `store/mod.rs:625`); add `Store::is_real_symbol(id)` / `is_real_file(id)`; gate `all_symbols`, `symbol_nodes`/`page_rank`, and (defensively) `dead_symbols`, `dead_symbols_for_kind`, `isolated_symbols`, `entry_points`, `all_file_paths` by kind, presence-gated for legacy stores.
- **Part C**: rewrite stub removal in all three passes (`resolve_bare_call_stubs_simple` `store/mod.rs:1191`, `resolve_import_aware_stubs`, `resolve_import_aware_extends_stubs`) to route through `Store::remove_node` (or explicit `kind_map`/`span_map` removal) so the `Unresolved` marker is cleaned, not orphaned.

### CHANGES REQUIRING EXPLICIT CALL-OUT

- **STORAGE FORMAT**: `NodeKind::Unresolved` is a new variant. `redb_codec.rs` maps `NodeKind` ↔ `u8`; the tags are **dense `0..=18`** (`node_kind_tag` `redb_codec.rs:40-67`; `tag_to_node_kind` `:71-94`). **Pin the new tag to `19`** (the next free discriminant). `#[non_exhaustive]` on `NodeKind` (`types.rs:56`) permits the additive variant; `node_kind_tag` panics fail-loud on an unmapped variant; `tag_to_node_kind` already returns `None` for unknown tags. Add `as_str`/`try_from_wire` arms (`"unresolved"`) **and** add `NodeKind::Unresolved` to the all-variants exhaustiveness test array (`redb_codec.rs:125+`), or the exhaustiveness test will drift. Additive and backward-compatible: old DBs never contain tag 19, so they decode unchanged; **no re-index required**. The reviewer must confirm no out-of-tree branch already claimed 19 at merge time.
- **PUBLIC API**: `NodeKind::Unresolved` (graph NODE marker) vs `CalleeClass` (`classify.rs:28`, RFC-0113, output TAIL label) are **orthogonal** — a node may be `NodeKind::Unresolved` and carry a `CalleeClass` label. RFC-0118 defers any RFC-0113 wiring and only guarantees they do not conflict; documented in the module header as a bridge note per the Charter transitional-code rule.
- **PERF SLA**: `all_symbols`/`page_rank` gain one `kind_of` lookup per node (O(1) hashbrown); extraction-time inference is O(bindings-in-scope) per call site, bounded by file size. Benchmarked against a concrete RFC-0104 warm-query number in Phase 2 acceptance (see AC-13).
- **SERIALIZATION**: covered by the redb_codec call-out; MessagePack wire form gains the additive `"unresolved"` string via `as_str`.

---

## Phased plan

### Phase 1 — Pure receiver-inference core (no Store/CLI/MCP)

**Scope.** New file `crates/mycelium-core/src/resolver/receiver.rs` with the structs and the pure `infer_receiver_type` + `disambiguate`. Unit tests only, over hand-built structs. ≥90% line coverage on the new module. No edits to extractor/store.
**Collision note.** Isolated new module under a new `resolver/` dir; touches no existing function bodies, so it cannot collide with in-flight RFC-0113/0114 work in `store/` or `classify.rs`. Mirrors the RFC-0113/0114 "pure function over plain structs first" shape.

### Phase 2a — `NodeKind::Unresolved` + set-query exclusion + resolver hygiene (Part A + Part C)

**Scope.** Add `NodeKind::Unresolved` (`types.rs` `as_str`/`try_from_wire`; `redb_codec.rs` tag 19 + all-variants test). Add `Store::is_real_symbol`/`is_real_file`. Switch the two extractor fallbacks to `upsert_node_with_kind(NodeKind::Unresolved)`. Kind-gate `all_symbols` + `symbol_nodes`/`page_rank` (the real leaks) and defensively `dead_symbols`/`dead_symbols_for_kind`/`isolated_symbols`/`entry_points`/`all_file_paths`. Route all three resolver passes' removal through `remove_node` (Part C).
**Collision note.** Edits `store/mod.rs` set-query fns + extractor fallbacks. **PR #606 (RFC-0114 Phase 2, OPEN) edits `store/mod.rs`, `store/tests.rs`, and `health.rs`** — `health.rs` consumes dead/isolated counts. This is a **live** merge collision. Sequence explicitly: **merge #606 first, then rebase Part A onto it**, OR fold the health-grade threshold recalibration against the de-noised universe into the same PR. The additive enum variant is non-breaking to RFC-0113 `classify.rs`.

### Phase 2b — Extraction-time disambiguation + pack captures (Part B)

**Scope.** Add `@call.receiver` to the **Rust** method-call query (prerequisite, previously absent) plus `@binding.local`/`@binding.ctor`/`@param.type`/`@field.type` to `packs/rust|python|typescript/queries.scm` (1 file/pack, Charter §4). Extractor builds a per-call-site `ReceiverContext` at `extractor/mod.rs:~366`, calls `disambiguate`, and binds the `Calls` edge directly on `Unique`. The post-pass `resolve_bare_call_stubs_simple` stays the conservative fallback — **single-match path byte-identical to today**.
**Collision note.** Receiver inference runs at **extraction time** (per call site), not in the post-pass, so it does **not** pre-empt or reorder `resolve_import_aware_stubs`/`resolve_import_aware_extends_stubs` — those still run unchanged over whatever stubs remain. A call site that inference binds never becomes a stub, so import-aware never sees it; a call site inference declines becomes a stub exactly as today and flows to the unchanged import-aware passes. AC-9/AC-10 assert no behavior change for stubs the import-aware passes resolve today.

### Phase 2c — Python + TypeScript local-ctor bindings (Part B cross-language) — **landed**

**Scope.** The Pass-1c binding/receiver wiring keys on capture *names*
(`binding.local`/`binding.ctor`/`call.receiver`), not Rust syntax, and the
post-merge `resolve_call_site_contexts()` pass is language-agnostic — so
extending Part B to a new language is **pack-only** (Charter §4): add the
local-constructor-binding captures to that pack's `queries.scm`. Python (`x = Ctor()`
→ `assignment` with `right: (call function: (identifier))`) and TypeScript
(`const x = new Ctor()` → `variable_declarator` with `value: (new_expression)`,
plus the `assignment_expression` reassignment form so structural-typing rebinds
**decline** rather than trust a stale declarator). `@call.receiver` already
existed in both packs. `FUNCTION_KINDS` already covers `function_definition`
(Python) and `function_declaration`/`function_expression`/`method_definition`
(TS/JS), so scoping needs no core change. Conservative behavior (Title-case ctor
filter, conflict-decline de-shadow) is inherited unchanged. Covered by AC-22.

### Phase 3 — Surface parity

**Scope.** Parts A/B/C are correctness fixes to **existing** surfaces (`get-files`/`get-dead`/`get-isolated`/`get-callers`/PageRank) and add **no new CLI/MCP verb**, so the Three-Surface Rule is satisfied transitively (CLI↔MCP share the Store query). Because the **output** of these verbs changes, add a CLI↔MCP byte-identical golden/snapshot test on at least one affected verb (e.g. `get-callers`) so de-noising lands identically on both surfaces and neither layer post-filters divergently (AC-12). IF a future decision exposes `Unresolved`/`CalleeClass` as a filterable output, that addition must be CLI↔MCP 1:1 (byte-identical name/description/args/JSON) and covered by a `skills/<category>/SKILL.md` `allowed-tools` entry (Charter §5.13); file an `EXCEPTION:` line here if ever skipped.

---

## Acceptance criteria (RED-testable)

- [ ] **AC-1 (Part B core, RED).** `infer_receiver_type(&ReceiverContext{ receiver:"store", method:"upsert_node", locals:[LocalBinding{name:"store", ctor_type:Some("Store")}], .. })` returns `Some(TypeName::Simple("Store"))`. Fails before `receiver.rs` exists.
- [ ] **AC-2 (Part B disambiguation, RED).** `disambiguate(Some(Simple("Store")), &[Candidate{"a.rs>Store>upsert_node"}, Candidate{"b.rs>Trunk>upsert_node"}])` returns `Resolution::Unique("a.rs>Store>upsert_node")`. Fails before `disambiguate` exists.
- [ ] **AC-3 (no-regression, stays GREEN).** `disambiguate(None, &[single_candidate])` returns `Resolution::Unique` with that candidate's path (byte-identical to today's `matches.len()==1` path). Must remain GREEN through every phase.
- [ ] **AC-4 (conservative fallback, RED).** `disambiguate(None, &[two_candidates])` returns `Resolution::Ambiguous`; and `disambiguate(Some(Simple("X")), &candidates)` where zero candidates contain `>X>{method}` returns `Ambiguous`.
- [ ] **AC-5 (Part A kind, RED).** After the extractor processes a call to an unresolved callee, `kind_of(id) == Some(NodeKind::Unresolved)` (not `None`, not `File`) — for **both** the bare-phantom (no `>`, e.g. `unwrap`) and the qualified-phantom (`prefix>callee`) fallback sites. Fails before the `upsert_node_with_kind` switch.
- [ ] **AC-6 (Part A real leak, RED).** Build a store with one real symbol and one `NodeKind::Unresolved` node carrying a phantom `Calls` edge; assert **`all_symbols(None, None)`** and **`page_rank(Calls, ..)`** both EXCLUDE the Unresolved path (no member, no rank mass). Fails today: `all_symbols` (`store/mod.rs:2459`) and `symbol_nodes`/`page_rank` (`store/mod.rs:3937`) admit the `>`-qualified phantom with no edge/kind gate. (These are the only two queries that actually leak — verified against `develop@755c204`.)
- [ ] **AC-7 (Part A defensive gate, GREEN-locking).** For the same store, assert `dead_symbols(None)`, `isolated_symbols(None)`, `entry_points(None)` EXCLUDE the Unresolved path **and** continue to do so after a *future* zero-in-degree Unresolved node is added (construct an Unresolved node with **no** incoming edge and assert all three still exclude it). This locks the kind-gate's forward-safety — note the original draft's claim that these three leak today for a phantom-with-edge is **refuted**; the RED state here is the no-incoming-edge case, which would leak without kind-gating.
- [ ] **AC-8 (F5 single-caller end-to-end, RED — Phase 2b).** Index a Rust fixture with `struct Store; impl Store { fn upsert_node(){} }`, a second type with a same-named method, and a caller `let s = Store::new(); s.upsert_node();`. Assert `get-callers` on `…>Store>upsert_node` returns the caller (count == 1, not 0). Requires the new Rust `@call.receiver` + `@binding.ctor` captures; gated to Phase 2b.
- [ ] **AC-9 (F5 multi-caller / multi-receiver, RED — the real failure mode).** Index a fixture with **two** call sites of the same method name on **different** receiver types (`let store = Store::new(); store.upsert_node();` and `let t = Trunk::new(); t.upsert_node();`). Assert `get-callers` on `…>Store>upsert_node` returns **only** the `store` caller and `…>Trunk>upsert_node` returns **only** the `t` caller. This is impossible under the draft's shared-stub post-pass model and is the criterion that forces the extraction-time placement.
- [ ] **AC-10 (import-aware no-regression).** Take a multi-def method stub currently resolved by `resolve_import_aware_stubs` on `develop`; snapshot its resolved target; after Part B lands, assert the same stub resolves to the **same** target (extraction-time inference must not silently re-bind import-aware-resolved stubs to a different definition).
- [ ] **AC-11 (Part C kind cleanup, RED).** After `resolve_bare_call_stubs_simple` (and each of the two import-aware passes) binds a `NodeKind::Unresolved` stub, assert `kind_of(freed_id) == None` **and** `kind_map` contains no entry for the freed id. Fails today because the passes call raw `trunk.remove`, which never touches `kind_map`.
- [ ] **AC-12 (storage round-trip, RED).** A store containing a `NodeKind::Unresolved` node persisted via `redb_codec` (tag 19) and reloaded yields `kind_of == Some(NodeKind::Unresolved)`; an OLD DB (no tag 19) loads unchanged; the all-variants exhaustiveness test (`redb_codec.rs:125+`) includes `NodeKind::Unresolved`.
- [ ] **AC-13 (legacy presence-gate).** A programmatic store that never sets any kind keeps the historical contract — `all_symbols`/`page_rank`/`dead_symbols`/`isolated_symbols`/`all_file_paths` behave exactly as on `develop` (no silent emptying).
- [ ] **AC-14 (CLI↔MCP parity snapshot).** A golden test asserts `get-callers` (and one set-query, e.g. `get-isolated`) produce byte-identical JSON on the CLI and MCP surfaces after de-noising (§5.13 1:1 by construction; lock it so neither layer post-filters divergently).
- [ ] **AC-15 (dogfood corpus validation).** Re-run `get-files`/`get-dead`/`get-isolated`/PageRank/`all-symbols` on the Mycelium repo itself after the fix; assert the phantom count on the queries that actually leaked (**`all_symbols`, `page_rank`**) drops to ~0, validating the fix against the corpus that motivated F1 — not just synthetic structs.
- [ ] **AC-16 (receiver-inference recall report).** Emit a recall report on the dogfood corpus: fraction of multi-match method call sites that `infer_receiver_type` binds vs. declines, broken down by rule (a–e). No numeric floor is mandated (recall is intentionally capped), but the report makes "F5 fixed for the common case" falsifiable and tracks generic/trait-impl decline cases.
- [ ] **AC-17 (perf SLA).** Benchmark `page_rank` and `all_symbols` warm-query latency before/after Part A on the dogfood corpus; assert within the RFC-0104 warm-query budget (≤ the documented warm-query SLA number; no regression beyond noise). Benchmark extraction-time inference overhead per file is bounded and reported.
- [ ] **AC-18 (quality gate).** `cargo fmt --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test --all`; `cargo llvm-cov` on `crates/mycelium-core/src/resolver/receiver.rs` reports ≥90% lines.
- [ ] **AC-19.** RFC acceptance-criteria checkboxes flipped `[ ]→[x]` as each lands; Status → Implemented when all green.
- [ ] **AC-20 (rank_symbols de-noised — Codex P2 #609).** A multi-def fixture with a phantom unresolved callee asserts the phantom appears in neither `mycelium rank-symbols` nor `mycelium_rank_symbols` output after Part A, and that the CLI and MCP results are byte-identical (§5.13 1:1). RED today: the phantom currently ranks because it has an incoming `Calls` edge.
- [ ] **AC-21 (order-independent F5 — Codex P1 #609).** A two-file fixture where the receiver type is defined in a file indexed *after* the call site (`let s = Store::new(); s.upsert_node();` with `Store` in a later chunk) asserts `get-callers` on `…>Store>upsert_node` includes the caller. RED with any extraction-time-inline binding; GREEN only with the post-merge `resolve_call_site_contexts()` pass.
- [x] **AC-22 (Part B cross-language: Python + TypeScript — Phase 2c).** With local-ctor binding captures added to the Python and TypeScript packs, a multi-type method (`upsert_node` on two classes) bound via `s = Store()` (Python) / `const s = new Store()` (TypeScript) resolves `s.upsert_node()` to `…>Store>upsert_node` and **not** `…>Trunk>upsert_node` (`extractor_{python,typescript}_receiver_type_binds_multi_match_method_f5`); and a same-name binding rebound to a conflicting type **declines** with no edge to either type (`extractor_{python,typescript}_shadowed_binding_declines_no_misbind`, the TS case exercising the `assignment_expression` reassignment form). Pack-only change (Charter §4); core unchanged.

---

## Charter / ADR compliance

- **ADR-0010 (no live LSP).** Receiver-type inference is 100% static — it reads tree-sitter captures (imports, `let`-bindings, param/field annotations, enclosing impl/class) and graph arithmetic only. No language server is spawned or queried; the design explicitly enumerates what inference MAY NOT do (no flow-sensitivity, no overload resolution, no LSP). Verified against `docs/adr/0010-*.md` Decision: live LSP forbidden, static SCIP/LSIF is the sanctioned escape hatch (named as a future precision input, not required here).
- **Charter §4 (≤3-file packs).** New `@call.receiver` (Rust) + `@binding.*`/`@param.type`/`@field.type` captures live in `packs/<lang>/queries.scm` — **1 file per pack**, data only, no core edit adds a language. The pure resolver is language-agnostic.
- **Charter §5.13 / RFC-0090 Three-Surface.** Phases 1–2 add no new CLI/MCP verb; CLI↔MCP 1:1 and Skill coverage are preserved transitively. Because the *output* of existing verbs changes, AC-14 adds a CLI↔MCP byte-identical snapshot. Any future surfacing of `Unresolved`/`CalleeClass` must be 1:1 + appear in a category `SKILL.md` `allowed-tools` (flagged in Phase 3).
- **TDD §5.1.** Every criterion is a concrete RED-testable assertion with a stated pre-state. AC-6/AC-11 fail on `develop`; AC-7 is re-founded on the no-incoming-edge case so its RED state is genuine (the draft's phantom-with-edge pre-state was refuted as already-green). Coverage ≥90% on `receiver.rs`.
- **STORAGE/API/SERIALIZATION.** `NodeKind::Unresolved` is an additive `#[non_exhaustive]` variant with redb tag **19** (next free; tags dense 0..=18), `as_str`/`try_from_wire`/MessagePack `"unresolved"`, and an updated all-variants exhaustiveness test. Old DBs decode unchanged; no re-index.
- **PERF SLA.** One O(1) `kind_of` per node in `all_symbols`/`page_rank`; O(bindings-in-scope) per call site in extraction-time inference — within the RFC-0104 warm-query budget, benchmarked numerically in AC-17.
- **Bridge note.** `CalleeClass` (RFC-0113) vs `NodeKind::Unresolved` orthogonality is documented in the `resolver/receiver.rs` and `classify.rs` module headers per the Charter transitional-code rule.

---

## Alternatives considered

- **Drop the placeholder entirely (mint no node for unresolved callees).** Rejected: removing the `Calls` edge re-introduces the false-dead-code problem (Issue #229/#247 lineage) — a function whose only outgoing calls are unresolved would wrongly appear isolated. Tagging with `NodeKind::Unresolved` keeps the edge for liveness while excluding the target from the symbol-universe queries that leak.
- **Keep the `p.contains('>')` heuristic and extend it to strip qualified phantoms by naming convention.** Rejected: a path-text heuristic is exactly what produced the F1 over-report; a real `NodeKind` is authoritative and survives refactors, and the codebase already chose kind-over-heuristic in `all_file_paths` (`store/mod.rs:892-904`).
- **Per-stub receiver disambiguation in the post-pass (the original draft's Part B).** Rejected as a **category error**: the bare stub is shared across all call sites and edges carry no receiver context, so a single `redirect_node` re-points every caller — manufacturing wrong edges for differently-typed receivers (`store: Store` vs `t: Trunk`). Disambiguation must happen at extraction time where per-call-site context exists. (Reviewer 3, confirmed against `synapse/mod.rs:230-234`.)
- **Resolve multi-match by most-imported / highest-PageRank candidate (statistical guess).** Rejected: violates no-regression — it would introduce wrong edges for the ambiguous case. Conservative DECLINE keeps precision at 100% for the new tier.
- **Wait for SCIP/LSIF ingestion (ADR-0010 escape hatch) for precise types.** Rejected as the primary mechanism: SCIP ingestion is a larger, unbuilt effort; static let-ctor/annotation inference covers the dominant F5 case today with zero new dependencies. SCIP remains a future precision upgrade plugged into the same pure interface.
- **Fold this into RFC-0113's `classify.rs` cascade.** Rejected: classification LABELS the unresolved tail for output; it neither binds method calls to definitions nor removes phantom nodes from set-queries. Different mechanism, different module; kept orthogonal.
- **Attribute F1 to the `@reference.arg_callback` recall>precision edges (`.hive/memory/lessons.jsonl:16`).** Considered as the "real" F1 cause and rejected as the scope of *this* RFC: that is a separate mechanism (data args recorded as callees) and a separate fix. RFC-0118 corrects the F1 attribution to the two queries (`all_symbols`, `page_rank`) that the unresolved-receiver path actually pollutes, and leaves the arg-callback precision work to a follow-up.

---

## Risks & mitigations

- **Recall is capped (ctor-let / annotated-param / declared-field / self only).** Dynamically-typed code (untyped Python locals, reassignment across branches) and Rust generic/trait-impl container forms (`impl<T> Foo<T>`, `impl Trait for Store`) fall to `Ambiguous`. F5 is fixed for the common case, not universally. *Mitigation*: by design (precision over recall); AC-16 reports recall by rule on the dogfood corpus; the generic/trait-impl decline is stated explicitly, not hidden.
- **Rust pack lacked `@call.receiver` (corrected from draft).** Without it the headline Rust F5 case cannot resolve. *Mitigation*: Phase 2b adds `@call.receiver` to the Rust method-call query as an explicit prerequisite; AC-8 is gated to Phase 2b and would have caught the draft's false "already has" claim.
- **redb additive tag must be append-only at the next free discriminant (19).** A mistaken reuse would corrupt old DBs. *Mitigation*: AC-12 round-trips both new and legacy DBs and asserts the all-variants exhaustiveness test gains the variant; reviewer confirms 19 is unclaimed at merge.
- **Orphan-kind on resolution (corrected from draft).** Raw `trunk.remove` leaves `kind_map[stub_id] = Unresolved` stale across re-index cycles. *Mitigation*: Part C routes all three passes through `remove_node`; AC-11 asserts `kind_of(freed_id) == None` and no `kind_map` entry, across all three passes.
- **Over-exclusion if a real symbol is ever tagged Unresolved.** *Mitigation*: only the two extractor fallback sites set the kind; the presence-gate preserves legacy stores; AC-13 covers it.
- **Live collision with RFC-0114 / PR #606** (open, edits `store/mod.rs` + `health.rs`, consumes dead/isolated counts). The draft's "land Part A first" is unenforceable while #606 is already open. *Mitigation*: sequence explicitly — merge #606 first then rebase Part A, OR recalibrate health-grade thresholds against the de-noised universe in the same PR (Phase 2a collision note).
- **Pass-ordering / double-redirect.** Extraction-time inference removes per-call-site disambiguation from the post-pass entirely, so the import-aware passes run unchanged over remaining stubs; `redirect_node` is idempotent on removed nodes. *Mitigation*: AC-10 snapshots a multi-def import-resolved stub before/after Part B and asserts the same target — guarding against silent re-binding, the substantive risk beyond mere double-redirect.
- **Perf.** Kind lookups and inference are O(1)/O(bindings-in-scope). *Mitigation*: AC-17 ties the budget to a concrete RFC-0104 warm-query number with a fixture, so it cannot be skipped.

---

## Review incorporated

What changed versus the original draft, and which reviewer drove it:

- **F1 scope corrected from 4 queries to 2 (Reviewer 1 & 3, REFUTED + CONFIRMED).** Removed the false claim that `dead_symbols`/`isolated_symbols`/`entry_points` count qualified phantoms — verified those gate on empty incoming edges and every phantom has an incoming `Calls` edge. The genuine leaks are `all_symbols` (`store/mod.rs:2459`, no edge gate) and `page_rank`/`symbol_nodes` (`store/mod.rs:3937`). Motivation and AC-6 re-founded on these two. AC-7 re-founded on the no-incoming-edge case so its RED state is real (the draft's pre-state was already green, a TDD §5.1 violation).
- **Part B moved from post-pass to extraction time (Reviewer 3, CONFIRMED architecture flaw).** The shared bare stub aggregates all call sites and `redirect_node` rewires all edges (`synapse/mod.rs:230-234`), so per-stub disambiguation would mis-bind differently-typed receivers. Disambiguation now runs per call site in the extractor (`extractor/mod.rs:~366`), where the receiver identifier already exists. Added AC-9 (multi-caller/multi-receiver) — the criterion the draft's single-caller AC-8 could not catch.
- **Orphan-kind bug fixed (Reviewer 1 & 2, REFUTED).** The "marker disappears automatically" claim is false; raw `trunk.remove` (`trunk/mod.rs:223`) never touches `kind_map`. Added Part C (route all three passes through `remove_node`) and AC-11.
- **Rust `@call.receiver` prerequisite added (Reviewer 1 & 3, REFUTED/NUANCED).** Corrected the false "Rust already has receiver captures" claim; Phase 2b now adds `@call.receiver` to the Rust method-call query alongside the binding captures.
- **PR #606 live collision made explicit (Reviewer 3).** Phase 2a now mandates merging #606 first or co-recalibrating health thresholds, instead of the unenforceable "land first".
- **Import-aware no-regression criterion added (Reviewer 1 & 3).** AC-10 snapshots a multi-def import-resolved stub before/after.
- **Missing criteria added (all reviewers).** Dogfood-corpus validation (AC-15), recall report (AC-16), concrete perf budget (AC-17), CLI↔MCP parity snapshot for the changed output (AC-12 storage round-trip pinning tag 19 + exhaustiveness test, AC-14 parity), bare-phantom exclusion folded into AC-5.
- **Retained, confirmed-sound from the draft (Reviewers 1 & 2, CONFIRMED).** The redb additive-tag design (tag 19, fail-loud, `#[non_exhaustive]`), the `NodeKind::Unresolved` vs `CalleeClass` orthogonality bridge note, and `is_real_symbol`/`is_real_file` over the `p.contains('>')` heuristic — all verified accurate and kept.
