# RFC-0116: pre-edit safety verdict — "safe-to-edit" before you touch (design)

- **Status**: **Partially Implemented** (Phase 1 pure `edit_verdict` core done — `crates/mycelium-core/src/verdict.rs`; Phase 2 — Store adapter + `mycelium safe-to-edit` CLI/MCP surfaces pending)
- **Author(s)**: orchestrator (Hive AI agent)
- **Created**: 2026-06-06 (UTC)
- **Depends on**: [RFC-0112](0112-ide-plugin-vscode-thin-client.md) (IDE plugin —
  the headline *consumer* of this verdict at the cursor), RFC-0114
  (graph-native HealthReport: grade A–F + dimensions — an *optional input* to a
  full verdict; ✅ on develop), RFC-0115 (test-gap signal — the third input to a
  full verdict; Phase 1 ✅ on develop; Phase 2 pending), `mycelium_get_reachable_to`
  (existing blast-radius/transitive-dependents — the **primary** input),
  [ADR-0010](../docs/adr/0010-no-live-lsp-prefer-scip-ingestion.md)
  (no live LSP — this verdict is pure graph arithmetic), Charter §5.13 (Three-Surface Rule)
- **Affected paths** (when implemented): `crates/mycelium-core/src/verdict/`
  (pure `edit_verdict` core ✅ done), later `crates/mycelium-core/src/store/`
  (adapter), CLI + MCP request layers, `skills/`

## Summary

Add a deterministic **pre-edit safety verdict**: given a symbol an agent (or a
human in the IDE) is about to change, compose Mycelium's existing
**blast-radius / impact** (how many symbols transitively depend on it) plus
**caller count** plus — when available — the RFC-0114 health signal and RFC-0115
test-gap signal into a single **`SAFE | CAUTION | REVIEW | UNSAFE`** verdict with
a short reason list and a pre-edit checklist. This is THE "agents know before they
touch" story and directly powers the planned IDE plugin ([RFC-0112](0112-ide-plugin-vscode-thin-client.md)).

## Motivation

1. **Agents edit blind.** The built-in `Edit`/`Write` tools give an agent zero
   visibility into downstream impact. Mycelium *already knows* the difference
   (`mycelium_get_reachable_to`, caller counts) — but only as raw numbers the agent must
   interpret. A single verdict turns "47 transitive dependents" into an
   actionable **`UNSAFE` + checklist**.
2. **Raw metrics are not a decision.** Charter's commercial positioning is
   Mycelium-as-context-layer: the value is pre-digested, token-dense answers.
   A verdict is the densest possible answer — one token (`SAFE`) the agent can
   branch on without a second call.
3. **TSA proved the shape.** The founder's tree-sitter-analyzer ships exactly
   this (`safe_to_edit` + `modification_guard`): an impact-level → verdict map
   with a pre-edit checklist and a documented "VERDICT INTEGRITY" rule.
4. **It is the IDE plugin's headline gate.** [RFC-0112](0112-ide-plugin-vscode-thin-client.md)'s
   one-click "context for your AI" wants a traffic-light at the cursor. This
   verdict *is* that traffic-light.

## Decision: pure verdict core first, thin adapter later

Phase 1 is **complete**: `crates/mycelium-core/src/verdict.rs` implements
`edit_verdict(metrics: &EditMetrics) -> EditVerdict` with full TDD coverage.

### Phase 1 — pure `edit_verdict(metrics) -> EditVerdict` (DONE ✅)

See `crates/mycelium-core/src/verdict.rs` for the implementation.

```rust
pub struct EditMetrics {
    pub symbol_found: bool,
    pub parse_broken: bool,
    pub direct_callers: u32,
    pub blast_radius: u32,
    pub health: Option<crate::health::HealthGrade>,  // RFC-0114, optional
    pub test_gap_uncovered: Option<bool>,             // RFC-0115, optional
}
pub enum Verdict { Safe, Caution, Review, Unsafe, Error, NotFound }
pub struct EditVerdict { pub verdict: Verdict, pub reasons: Vec<String>, pub checklist: Vec<String> }
pub fn edit_verdict(m: &EditMetrics) -> EditVerdict { ... }
```

Blast-radius bands (primary axis):
| blast_radius | base verdict |
|---|---|
| `0` | `SAFE` |
| `1–5` | `CAUTION` |
| `6–20` | `REVIEW` |
| `21+` | `UNSAFE` |

Escalation rules (monotonic — never downgrade):
- `parse_broken` ⇒ `ERROR` short-circuit
- `symbol_found == false` ⇒ `NOT_FOUND`
- `health` grade `D`/`F` boosts one step
- `test_gap_uncovered == Some(true)` boosts one step

### Phase 2 — thin Store adapter + CLI/MCP + Skill (PENDING)

A thin adapter assembles `EditMetrics` from the existing engine and surfaces it
as a new capability `mycelium_safe_to_edit` on **both** CLI and MCP (1:1).

**Metric sources (all existing, no new extraction):**
- **`blast_radius`** = `Store::reachable_to(id, kind, max_depth)` — transitive dependents
- **`direct_callers`** = incoming-edge count from the existing callers query
- **`health` / `test_gap_uncovered`** = `None` until RFC-0114/0115 Phase 2 land

## Three-Surface compliance (Charter §5.13)

- **CLI ↔ MCP 1:1 strict.** Phase 2 ships `mycelium safe-to-edit <symbol>` and
  MCP `mycelium_safe_to_edit` with **byte-identical** name, description, args,
  and JSON output (`{ verdict, reasons, checklist, blast_radius, direct_callers }`).
- **Skill coverage (N:1).** The new CLI+MCP pair MUST appear in ≥1
  `skills/<category>/SKILL.md` `allowed-tools`. No orphan, no Skill-only.

## Acceptance criteria

**Phase 1 — pure verdict core (DONE ✅):**
- [x] `edit_verdict(metrics)` exists in `crates/mycelium-core/src/verdict.rs`,
      pure, no Store/I/O. TDD (RED first): `blast=0` → `SAFE`; `blast=3` →
      `CAUTION`; `blast=12` → `REVIEW`; `blast=40` → `UNSAFE`;
      `parse_broken` → `ERROR`; `symbol_found=false` → `NOT_FOUND`.
- [x] Health/test-gap escalation is monotonic (never downgrades); fixtures for
      `D`-grade and `uncovered` each boosting one step.
- [x] `reasons` + `checklist` are non-empty for every non-`SAFE` verdict and
      name the concrete count.
- [x] Verdict tokens are a strict subset of the reconciled vocabulary;
      snapshot test pins them against `context/mod.rs`'s set.

**Phase 2 — adapter + surfaces (PENDING):**
- [ ] Thin Store adapter assembles `EditMetrics` from existing caller-count +
      `mycelium_get_reachable_to`; `health`/`test_gap` left `None` until RFC-0114/0115 land.
- [ ] `mycelium safe-to-edit` (CLI) + `mycelium_safe_to_edit` (MCP), byte-identical
      (parity snapshot test).
- [ ] ≥1 category Skill lists the new pair in `allowed-tools`.

**Phase 3 (sequenced after RFC-0114 + RFC-0115):**
- [ ] Wire `health` + `test_gap` inputs; re-snapshot the escalation paths.

## Alternatives considered

- **Live LSP / language-server for "real" edit safety.** Rejected: ADR-0010.
- **Static SCIP/LSIF ingestion as the impact source.** Complementary: if/when
  SCIP lands it makes `blast_radius` more precise — but the verdict function is
  unchanged.
- **Let the agent eyeball raw `mycelium_get_reachable_to` numbers.** Rejected: that is
  the status quo and it fails.
- **A new clashing verdict enum.** Rejected: would fracture the `verdict` field
  already shipped by `mycelium_context`.

## Conflicts with binding constraints

- **ADR-0010 (no live LSP):** ✅ fully compliant — pure arithmetic over existing graph.
- **Charter §4 (≤3-file packs):** ✅ language-agnostic core logic.
- **Charter §5.13 (Three-Surface):** ✅ Phase 2 ships CLI ↔ MCP 1:1 + Skill.
- **No new extraction / SLA:** ✅ gated on existing `mycelium_get_reachable_to` + caller-count data.
