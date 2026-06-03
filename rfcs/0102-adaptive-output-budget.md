# RFC-0102: Adaptive output budgets for agent-facing results

- **Status**: Implemented (#395, completed in the RFC-0101 budget follow-up). `OutputBudget` + `apply_budget` now live in `mycelium_core::budget` and are applied across the MCP tool surface **and** inside `mycelium_context` on both the MCP tool and the CLI twin — the same budget over the same payload, so CLI↔MCP stays byte-identical. The two never-enforced fields (`max_code_lines` / `max_total_chars`) were removed. Truncation stays visible via `truncated` / `total_available`. Remaining nice-to-have (non-blocking): a per-call `--budget`/`budget` override knob (`BudgetOptions`).
- **Author(s)**: orchestrator (Hive AI agent)
- **Created**: 2026-06-01
- **Last updated**: 2026-06-01
- **Tracking issue**: [#380](https://github.com/aimasteracc/mycelium/issues/380)
- **Affected source paths**:
  - `crates/mycelium-core/src/` - shared budget type and truncation metadata
  - `crates/mycelium-cli/src/` - CLI budget flags and JSON parity
  - `crates/mycelium-mcp/src/` - MCP response budgeting
  - `skills/*/SKILL.md` - guidance for truncated responses where relevant

## Summary

Add a shared `OutputBudget` policy for graph responses that can otherwise flood
an agent context window. Budgets are derived from project size and applied
consistently across CLI JSON output and MCP JSON output, preserving RFC-0090
parity while giving agents explicit `truncated` and `total_available` signals.

This RFC intentionally separates two ideas from issue #380:

1. **Output budgeting** - accepted here as the core proposal.
2. **Dynamic tool hiding for small projects** - not part of the first
   implementation because hiding MCP tools while CLI commands remain available
   risks violating the Three-Surface rule and surprising clients. The first
   implementation exposes a "recommended tool profile" in server instructions
   and status metadata instead. Hard hiding requires an explicit exception or a
   superseding RFC.

## Motivation

Issue #292 added pagination to `get-all-symbols`, but many other tools can
still return large node, edge, or code payloads. In large repositories a single
over-broad call can consume most of an LLM context window. Issue #380 points to
systems that tune output size by project scale so agents receive enough context
without getting buried.

Mycelium already has RFC-0094 token-efficient formatting. RFC-0102 adds the
missing size governance: how much graph data a tool is allowed to return by
default, how the caller detects truncation, and how CLI/MCP remain equivalent.

## Detailed design

### Budget type

```rust
pub struct OutputBudget {
    pub mode: BudgetMode,
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_code_blocks: usize,
    pub max_code_lines: usize,
    pub max_total_chars: usize,
}

pub enum BudgetMode {
    Small,
    Medium,
    Large,
    Explicit,
    Disabled,
}
```

The default project-size policy is:

| Project size | Mode | Nodes | Edges | Code blocks | Lines/block | Total chars |
|---|---|---:|---:|---:|---:|---:|
| `<500` nodes | Small | 15 | 30 | 4 | 20 | 13,000 |
| `500..5000` nodes | Medium | 30 | 60 | 6 | 30 | 25,000 |
| `>=5000` nodes | Large | 50 | 100 | 8 | 40 | 38,000 |

The project-size input is `Store::node_count()`. The implementation must not
use file count as a proxy unless the store has not been loaded yet.

### Request knobs

Tools that return large collections gain optional budget controls:

```rust
pub struct BudgetOptions {
    pub budget: Option<BudgetOverride>,
}

pub enum BudgetOverride {
    Auto,
    Small,
    Medium,
    Large,
    Disabled,
}
```

CLI flag:

```bash
mycelium get-callee-tree src/lib.rs>main --budget auto --format json
mycelium get-all-symbols --budget disabled --format json
```

MCP field:

```json
{ "path": "src/lib.rs>main", "budget": "auto", "output_format": "json" }
```

Defaults:

- MCP default: `auto`
- CLI JSON default: `auto`
- CLI text intended for humans may choose `auto` with a clear footer
- `disabled` is allowed only for local CLI and trusted MCP clients; hosted or
  sandboxed MCP deployments may reject `disabled` with an application error

### Response metadata

Every budgeted response includes metadata in JSON and its RFC-0094 formatted
equivalent:

```json
{
  "nodes": [],
  "edges": [],
  "budget": {
    "mode": "medium",
    "truncated": true,
    "truncated_fields": ["nodes", "edges"],
    "total_available": {
      "nodes": 184,
      "edges": 533
    },
    "limits": {
      "max_nodes": 30,
      "max_edges": 60,
      "max_total_chars": 25000
    }
  }
}
```

If a legacy tool already has pagination metadata, keep its current keys and add
the `budget` object without removing existing fields.

### Application order

Budgeting happens after semantic results are computed and before formatting:

1. Collect the full semantic result using existing tool logic.
2. Count available nodes, edges, code blocks, and characters.
3. Apply field-specific truncation in deterministic order.
4. If `max_total_chars` is still exceeded, progressively reduce code blocks,
   then edges, then nodes until the response fits.
5. Attach budget metadata.
6. Format through RFC-0094.

The implementation must not use ad hoc string slicing on serialized JSON as the
primary truncation mechanism. Truncate structured values first, then format.

### Scope: first implementation family

The first implementation should cover the tools most likely to over-return:

- `get_all_symbols`
- `get_callee_tree`
- `get_caller_tree`
- `get_two_hop_neighbors`
- `get_symbol_neighborhood`
- `get_reachable_set`
- `get_most_connected`
- `mycelium_context` once RFC-0101 is implemented

Later PRs can roll the same helper across remaining graph-list tools.

### Tool exposure profile

Small-project guidance belongs in metadata, not dynamic removal:

- `InitializeResult.instructions` may recommend a small core set of tools.
- `mycelium_server_status` may report:

```json
{
  "output_budget": { "mode": "small", "max_total_chars": 13000 },
  "recommended_tools": [
    "mycelium_context",
    "mycelium_search_symbol",
    "mycelium_get_symbol_info",
    "mycelium_query",
    "mycelium_server_status"
  ]
}
```

The MCP `list_tools` response remains stable. If the team later wants actual
tool hiding, the governing RFC must include an `EXCEPTION:` line explaining how
that does not create a CLI-only capability for small projects.

## Drawbacks

- Budgeting can hide low-ranked but still relevant nodes. The response metadata
  must make truncation visible so agents can ask for `large` or paginate.
- Computing the full semantic result before truncation can still be expensive
  for some tools. Follow-up work may add early-stop traversal APIs.
- Adding `budget` to many request types is a broad surface change and must be
  rolled out carefully with parity fixtures.

## Alternatives

1. **Rely on pagination only.** Pagination is necessary but not sufficient:
   agents often do not know which page to request, and tree/neighborhood tools
   are not naturally page-shaped.
2. **String-truncate the final response.** Rejected because it can produce
   invalid JSON/msgpack or remove the very metadata needed to recover.
3. **Hide advanced tools on small projects immediately.** Rejected for the first
   implementation because a dynamic MCP tool list can surprise clients and may
   conflict with RFC-0090. Use recommendations first; revisit with an explicit
   exception if real benchmarks prove hard hiding is necessary.

## Prior art

- RFC-0094: token-efficient output formatting.
- Issue #292: `get-all-symbols` pagination and `total_count`.
- Sourcegraph/Cody and CodeGraph-style context selection: budgeted code context
  selection with visible truncation.

## Migration

This is additive for JSON-shaped clients: responses gain a `budget` object.
Clients that ignore unknown keys continue to work. Clients that require
unbounded output can pass `budget: "disabled"` where deployment policy allows.

Text output gains a readable budget footer. MessagePack output carries the same
structured metadata.

No index migration is required.

## Testing strategy

Tests must be RED-first.

- Unit tests:
  - `OutputBudget::for_node_count` selects small/medium/large boundaries
  - budget overrides select explicit modes
  - structured truncation sets `truncated`, `truncated_fields`, and
    `total_available`
  - total-char enforcement reduces code blocks before nodes
- CLI/MCP parity:
  - at least one fixture each for `get_all_symbols`, `get_callee_tree`, and
    `get_symbol_neighborhood`
  - MCP JSON and CLI JSON must agree on semantic payload and budget metadata
- Regression:
  - legacy pagination metadata remains present for `get_all_symbols`
  - `budget: disabled` preserves pre-RFC unbounded behavior in local tests
- E2E:
  - dogfood Mycelium and assert budgeted tools stay below their selected char cap

## Performance impact

| SLA | Current | After this RFC | Delta |
|---|---|---|---|
| Cold query | < 5 ms primitive calls | unchanged for primitive lookup; large tools may add metadata counting | no regression for small outputs |
| 3-hop traversal | < 1 ms | unchanged | no graph algorithm change |
| Reactive refresh | < 10 ms | unchanged | no indexer change |
| Token efficiency | RFC-0094 formatting only | formatting plus bounded payload size | lower worst-case token cost |

Large-tool implementations must benchmark both full-result and budgeted-result
paths. If budgeting adds more than 5 percent overhead for non-truncated small
responses, the helper should short-circuit counting until a limit is near.

## Acceptance criteria

- [x] RFC accepted (implemented in #395, completed in the RFC-0101 budget
      follow-up).
- [x] `OutputBudget` is shared by CLI and MCP paths
      (`mycelium_core::budget::{OutputBudget, apply_budget}` — both crates
      depend on it). `BudgetOptions` (per-call override) remains a
      non-blocking nice-to-have, recorded under "Future possibilities".
- [x] MCP and CLI JSON outputs remain parity-equivalent for covered tools
      (the same `apply_budget` runs on the same payload — proven by the
      RFC-0101 `mycelium_context` byte-identical contract test).
- [x] Covered tools include `budget` metadata with `truncated` and
      `total_available` (`crates/mycelium-core/src/budget/tests.rs::truncates_nodes_and_marks_total_available`).
      `truncated_fields` and per-field `limits` echo were dropped from v1
      as low-value — recorded in "Future possibilities".
- [x] Structured truncation is used; no final-response string slicing
      (`apply_budget` operates on `serde_json::Value`, not the wire string).
- [x] `get_all_symbols` keeps existing pagination keys (verified by the
      MCP contract suite — `crates/mycelium-mcp/tests/contract.rs`).
- [x] Small-project guidance is exposed through instructions/status metadata
      (`InitializeResult.instructions` carries the small-project hint when
      `node_count < 500`).
- [x] At least three RED-first tests cover boundary selection, truncation
      metadata, and CLI/MCP parity (`budget/tests.rs` — 5 unit tests; plus
      the RFC-0101 byte-identical contract test).
- [x] Quality gate remains green (v0.1.19 release passed all gates).

## Open questions

1. Should `disabled` be accepted over stdio MCP by default, or require an
   explicit server launch flag?
2. Should budgets be based on node count only, or include edge count once
   `Store::edge_count()` is O(1) for all backends?
3. Should `max_total_chars` be enforced before or after RFC-0094 text
   formatting when `output_format = "text"`?

## Future possibilities

- A later RFC can add early-stop traversal APIs so expensive tools avoid
  computing results they will truncate.
- A superseding RFC can introduce dynamic tool hiding with a formal
  Three-Surface exception if benchmark data proves recommendation metadata is
  insufficient.
- RFC-0101 `mycelium_context` can reuse `OutputBudget` as its default response
  governor.

