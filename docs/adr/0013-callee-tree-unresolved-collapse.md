# ADR-0013: Callee tree collapses unresolved callees into a per-node count

**Status**: Accepted
**Date**: 2026-06-10
**RFC**: [RFC-0020](../../rfcs/0020-callee-tree.md) (callee tree), [RFC-0118](../../rfcs/0118-resolver-receiver-disambiguation.md) (Part A — `Unresolved` phantoms)
**Supersedes / extends**: extends [ADR-0012](0012-graph-query-real-symbol-induced-subgraph.md) to the tree payload

---

## Context

`mycelium_get_callee_tree` / `get-callee-tree` emitted **every** outgoing
`Calls` edge as a child node — including edges to the resolver's
`NodeKind::Unresolved` phantoms (stdlib calls like `unwrap`/`map`/`collect`,
ambiguous names) and, on stale snapshots, dangling edge targets rendered as
`{"path":"<unknown>","children":[]}`. Measured on a real repo
(`crates/mycelium-cli/src/index.rs>index_path`, depth 3): 142 of 173 nodes
(82%) were such content-free placeholder leaves. For an AI agent the signal is
buried and the token cost is dominated by noise.

ADR-0012 already established that graph-theory queries operate on the
real-symbol induced subgraph; the tree tools were the remaining surface that
leaked phantoms. Unlike ADR-0012's silent exclusion, a *tree* consumer still
needs to know that calls were left out — silently dropping them would make a
function with 10 unresolved calls look identical to one with none.

Per Charter §3 this is a public output-contract change, hence this ADR.

## Decision

1. **Collapse, don't enumerate.** `Store::callee_tree` no longer emits a child
   for a callee that is not a real, displayable symbol — i.e. its kind is
   `NodeKind::Unresolved` **or** it has no trunk path (dangling edge). Instead
   each `CalleeNode` carries `unresolved_callees: usize`, the number of direct
   callees collapsed at that node.
2. **Serialization (both surfaces, byte-identical node shape).** The JSON node
   gains `"unresolved_callees": N`, **omitted when 0** (token economy). The
   existing `path` / `children` fields are unchanged; if all callees are
   unresolved the node has `"children": []` plus the count.
3. **Gating in core** keeps CLI ↔ MCP identical by construction (Charter
   §5.13) — both serializers read the same `CalleeNode`.
4. **Caller tree unchanged.** Verified: phantoms are only ever call *targets*;
   the extractor never mints a phantom as an edge *source*, so
   `caller_tree` contains no phantom nodes (empirical check: 614-node caller
   tree, 0 stubs). Its shape is left untouched.

## Rationale

- **Token economy / signal-to-noise**: dogfood sanity run dropped the tree
  from 173 nodes to 31 (payload 7,251 → 2,976 bytes), with zero information
  loss — the 142 collapsed callees survive as per-node counts (Σ = 142).
- **Consistency**: extends RFC-0118 Part A / ADR-0012's real-symbol principle
  to the last phantom-leaking query surface, while staying honest about
  omissions via the count.
- **Back-compatible by construction**: `is_real_symbol` is a negative gate
  (excludes only `Unresolved`), so kind-less programmatic/legacy stores are
  unchanged (count stays 0, children identical).

## Consequences

- Output contract change (intended): consumers no longer see `<unknown>` or
  bare stub-name leaves; they must read `unresolved_callees` for the omitted
  count. `skills/call-graph/SKILL.md` documents the new shape.
- Cycle/depth-limit leaves report `unresolved_callees: 0` (omitted) because
  their children are not expanded at all — the count only describes expanded
  nodes.

## Alternatives considered

- **Keep the leaves but mark them** (`"unresolved": true`): still O(N)
  placeholder objects; rejected — the noise *is* the problem.
- **Silently drop the leaves** (pure ADR-0012 treatment): loses the "this
  function also makes N unresolved calls" fact; rejected for honesty.
- **Collapse in the serializers only**: duplicates the rule in CLI and MCP and
  risks drift; rejected — core is the single source of truth.
