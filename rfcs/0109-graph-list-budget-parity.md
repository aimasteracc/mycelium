# RFC-0109: Graph-list tool output-shape parity + budget knob roll-out

- **Status**: **Accepted — Option A** (ratified 2026-06-03 UTC under the founder's
  standing autonomous-development mandate + "all rights" grant). Rationale:
  ADR-0009 records the founder's pre-launch principle to "shed conservative
  backward-compat baggage we don't owe anyone yet," and Option A closes a real
  Three-Surface gap rather than codifying a permanent split. Implementation
  proceeds incrementally, one tool per PR, each behind a green byte-identical
  contract test. *(Founder may downgrade to Option B on review; the EXCEPTION
  path is preserved in §Decision.)*
- **Author(s)**: orchestrator (Hive AI agent)
- **Created**: 2026-06-03 (UTC; commit `2026-06-03T19:25Z`)
- **Supersedes**: none
- **Depends on**: RFC-0090 (Three-Surface Rule), RFC-0102 (adaptive output budget)
- **Tracking issue**: TBD (#380 follow-up)
- **Affected source paths**:
  - `crates/mycelium-mcp/src/lib.rs` — list-tool handlers
  - `crates/mycelium-cli/src/queries.rs` — list-tool twins, `print_string_list`
  - `crates/mycelium-cli/src/main.rs` — clap `--budget` flags
  - `crates/mycelium-core/src/budget.rs` — shared resolve/apply (already done)

## Summary

Rolling the RFC-0102 per-call `budget` knob across the graph-list tools
(`get_callees`, `get_callers`, `get_dead_symbols`, `get_isolated_symbols`,
`get_all_symbols`, `get_reachable`, `get_reachable_to`, `search_symbol`)
surfaced a **pre-existing Three-Surface discrepancy** that blocks the roll-out
and must be decided first.

## Motivation / the discovered problem

RFC-0102's Status note called the roll-out "mechanical, no new design." It is
not — dogfooding the knob (PRs #497–#499) revealed two facts:

1. **CLI list tools emit a bare JSON array; the MCP twins emit an object.**
   - MCP `mycelium_get_callees` → `{"callee_paths":[…]}`
     (`crates/mycelium-mcp/src/lib.rs:2346`).
   - CLI `mycelium get-callees --format json` → `["…","…"]`
     (`print_string_list`, `crates/mycelium-cli/src/queries.rs:1924`).
   - These are **not byte-identical**, and no contract test asserts they are.
     The RFC-0101 byte-identical contract only covers `mycelium_context`
     (which both surfaces build through the shared `mycelium_core::context`).

2. **Budget metadata cannot ride on a bare array.** `truncated` /
   `total_available` / the nested `budget {}` object (RFC-0102) are object
   keys. A CLI tool that prints `[…]` has nowhere to attach them, so it cannot
   express a budgeted, truncation-aware response at all.

Therefore the budget knob **cannot** be rolled out to these tools on the CLI
without first deciding their output shape. This is a Charter §5.13 /
RFC-0090 question (byte-identical CLI↔MCP JSON), i.e. non-trivial → RFC-gated.

## Decision (BDFL)

Two coherent options. Both keep `search_symbol`/`context` (already object-shaped
and parity-correct) as-is.

### Option A — Unify CLI list tools onto the MCP object shape *(recommended)*

CLI `--format json` for the list tools emits the **same object** as MCP
(`{"callee_paths":[…], "truncated":…, "budget":{…}}`), built by routing both
surfaces through one shared core helper (the pattern already proven for
`context` and `watch`). Then the budget knob + metadata roll out uniformly and
Three-Surface byte-identical becomes *real* (and testable) for these tools.

- **Pro**: closes a latent Three-Surface gap; one shared builder per tool kills
  future drift; budget/knob roll-out becomes truly mechanical afterward.
- **Con**: breaking change to CLI `--format json` output for ~7 commands
  (bare array → object). Text mode is unchanged.
- **Mitigation / fit**: Mycelium is pre-launch alpha; ADR-0009 records the
  founder's principle to "shed conservative backward-compat baggage we don't
  owe anyone yet." The break is acceptable now and cheap later it would not be.

### Option B — Document a Three-Surface EXCEPTION for list tools

Keep CLI bare arrays; declare (per RFC-0090 `EXCEPTION:`) that list tools are
CLI↔MCP *semantically* equivalent but not byte-identical, like the RFC-0105
watch exception. The budget knob then lands **MCP-only** for these tools, with
the CLI relying on its existing `--limit`/`--offset` pagination as the
size-control equivalent.

- **Pro**: no CLI output break.
- **Con**: permanently bifurcates the surfaces; "MCP-only arg" dents the strict
  1:1 arg rule; agents and humans get different truncation semantics.

## Detailed design (Option A)

For each list tool, introduce `mycelium_core::<tool>::build_payload`-style
shared builders (mirroring `context`) returning the object shape, then:

1. MCP handler calls the shared builder + `apply_budget(resolve(over, n))`.
2. CLI twin calls the **same** builder + the **same** resolve/apply, prints the
   object in `--format json`; text mode renders the list as today.
3. Add `--budget` (CLI) / `budget` (MCP) to each, parsed via the shared
   `BudgetOverride::from_str` (already shipped in #498).
4. Add a byte-identical contract test per tool (extend the `context` pattern).

Roll out incrementally, one tool per PR, RED-first, each behind a green
byte-identical contract test.

### Roll-out progress

| Tool | Shared builder | CLI object shape | `--budget`/`budget` | PR |
|---|---|---|---|---|
| `get_callees` | `mycelium_core::queries::callees_payload` | ✅ | ✅ | (this RFC's first impl) |
| `get_callers` | `mycelium_core::queries::callers_payload` | ✅ | ✅ | done |
| `get_dead_symbols` | `mycelium_core::queries::dead_symbols_payload` | ✅ | ✅ | done |
| `get_isolated_symbols` | `mycelium_core::queries::isolated_symbols_payload` | ✅ | ✅ | done |
| `get_reachable` / `get_reachable_to` | — | — | — | pending (already object-shaped on MCP) |
| `get_all_symbols` | — | — | — | pending (bespoke pagination — reconcile) |

## Acceptance criteria

- [x] BDFL decision recorded — **Option A** (see Status; ratified 2026-06-03 UTC
      under the autonomous-development mandate, citing ADR-0009's pre-launch
      principle).
- [~] (Option A) A shared core builder exists per rolled-out list tool; CLI and
      MCP both call it; a byte-identical contract test guards each. **Started:
      `get_callees` routes both surfaces through
      `mycelium_core::queries::callees_payload`; remaining tools pending.**
- [~] (Option A) `--budget`/`budget` accepted on each rolled-out tool, resolving
      via the shared `OutputBudget::resolve`; unknown value fails fast on both
      surfaces. **Started: `get_callees` done on both surfaces.**
- [ ] CHANGELOG `[Unreleased]` notes the CLI JSON shape change (Option A) or the
      documented EXCEPTION (Option B).
- [ ] RFC-0102's "roll knob across remaining graph-list tools" item is closed by
      this RFC's implementation.

## Three-Surface implications

This RFC *is* the Three-Surface reconciliation for list tools. Option A makes
the strict 1:1 byte-identical contract true where it is currently only assumed;
Option B records the deviation explicitly so it stops being an invisible gap.
