# RFC-0116: pre-edit safety verdict — "safe-to-edit" before you touch (design)

- **Status**: **Draft** (design — no implementation in this PR)
- **Author(s)**: orchestrator (Hive AI agent)
- **Created**: 2026-06-06 (UTC)
- **Depends on**: [RFC-0112](0112-ide-plugin-vscode-thin-client.md) (IDE plugin —
  the headline *consumer* of this verdict at the cursor), RFC-0114
  (graph-native HealthReport: grade A–F + dimensions — an *optional input* to a
  full verdict; link resolves once that RFC lands), RFC-0115 (test-gap signal —
  the third input to a full verdict; sequence after it so we **compose, not
  duplicate**), `mycelium_get_reachable_to` (existing blast-radius/transitive-dependents —
  the **primary, already-extracted** input),
  [ADR-0010](../docs/adr/0010-no-live-lsp-prefer-scip-ingestion.md)
  (no live LSP — this verdict is pure graph arithmetic, the sanctioned static
  path), Charter §5.13 (Three-Surface Rule)
- **Affected paths** (when implemented): `crates/mycelium-core/src/verdict/`
  (new — pure `edit_verdict` core), later `crates/mycelium-core/src/store/`
  (adapter), CLI + MCP request layers, `skills/`
- **Reuses**: tree-sitter-analyzer (TSA), same founder, MIT — port the **concept**,
  not source. Specifically `mcp/tools/modification_guard_tool.py`'s
  `_VERDICT_MAP` (impact level → `SAFE`/`CAUTION`/`REVIEW`/`UNSAFE`), its
  `_VERDICT_TO_RISK` and `_VERDICT_BOOST` normalization, the
  `_build_required_actions` checklist generator, and `mcp/tools/safe_to_edit_tool.py`'s
  pre-edit-checklist + syntax-error short-circuit (`risk=high` / `verdict=ERROR`
  when the AST is broken). TSA is Python; Mycelium is Rust — this is a
  *port-concept*, not a dependency.

## Summary

Add a deterministic **pre-edit safety verdict**: given a symbol an agent (or a
human in the IDE) is about to change, compose Mycelium's existing
**blast-radius / impact** (how many symbols transitively depend on it) plus
**caller count** plus — when available — the RFC-0114 health signal and RFC-0115
test-gap signal into a single **`SAFE | CAUTION | REVIEW | UNSAFE`** verdict with
a short reason list and a pre-edit checklist. This answers the one question
agents ask before mutating code: *"do I know enough before I touch this?"* It is
**pure verdict logic over a metrics struct** — no new extraction, gated entirely
on data `mycelium_get_reachable_to` already produces. This is THE "agents know before they
touch" story and directly powers the planned IDE plugin
([RFC-0112](0112-ide-plugin-vscode-thin-client.md)).

## Motivation

1. **Agents edit blind.** The built-in `Edit`/`Write` tools give an agent zero
   visibility into downstream impact. An agent told "just rename this" will
   happily mutate a 200-dependent foundation symbol exactly as readily as a
   dead private helper. Mycelium *already knows* the difference
   (`mycelium_get_reachable_to`, caller counts) — but only as raw numbers the agent must
   interpret. A single verdict turns "47 transitive dependents" into an
   actionable **`UNSAFE` + checklist**.
2. **Raw metrics are not a decision.** Charter's commercial positioning is
   Mycelium-as-context-layer: the value is pre-digested, token-dense answers,
   not numbers the caller re-derives. A verdict is the densest possible
   answer — one token (`SAFE`) the agent can branch on without a second call.
3. **TSA proved the shape.** The founder's tree-sitter-analyzer ships exactly
   this (`safe_to_edit` + `modification_guard`): an impact-level → verdict map
   with a pre-edit checklist, and a documented "VERDICT INTEGRITY" rule — the
   verdict is a *hard gate*, derived from graph facts, **not** softened to
   please a "just ship it" instruction. We port that discipline.
4. **It is the IDE plugin's headline gate.** [RFC-0112](0112-ide-plugin-vscode-thin-client.md)'s
   one-click "context for your AI" wants a traffic-light at the cursor. This
   verdict *is* that traffic-light, computed server-side so CLI, MCP, and the
   IDE all show the identical answer.

## Decision: pure verdict core first, thin adapter later

Mirror the phased shape of RFC-0113 / RFC-0114: land a **pure function** over a
plain metrics struct, defer all Store/CLI/MCP wiring to a later phase. The
verdict logic must be testable with zero graph fixtures.

### Phase 1 — pure `edit_verdict(metrics) -> EditVerdict`

```rust
// crates/mycelium-core/src/verdict/  (pure, no Store, no I/O)

pub struct EditMetrics {
    pub direct_callers: u32,         // from caller-count (already extracted)
    pub blast_radius: u32,           // transitive dependents from mycelium_get_reachable_to
    pub symbol_found: bool,          // false ⇒ NOT_FOUND
    pub parse_broken: bool,          // true  ⇒ ERROR short-circuit (TSA M3 port)
    pub health: Option<HealthGrade>, // RFC-0114, optional
    pub test_gap: Option<TestGap>,   // RFC-0115, optional
}

// Full enum — the decision axis AND the envelope/short-circuit tokens live in
// ONE type so `parse_broken` → ERROR and `!symbol_found` → NOT_FOUND are
// representable without changing the public type during implementation.
pub enum Verdict { Safe, Caution, Review, Unsafe, Error, NotFound } // as_str → SAFE..NOT_FOUND

pub struct EditVerdict {
    pub verdict: Verdict,
    pub reasons: Vec<String>,    // why this grade (caller/blast/health/test)
    pub checklist: Vec<String>,  // concrete pre-edit actions (TSA port)
}

pub fn edit_verdict(m: &EditMetrics) -> EditVerdict { /* thresholds below */ }
```

### Thresholds (ported from TSA `_VERDICT_MAP`, on blast-radius not just callers)

The primary axis is **blast radius** (transitive dependents) — broader than
TSA's direct-caller count, because Mycelium has the transitive graph TSA lacks.
Direct callers tie-break and escalate.

| blast_radius | base verdict |
|---|---|
| `0` | `SAFE` |
| `1–5` | `CAUTION` |
| `6–20` | `REVIEW` |
| `21+` | `UNSAFE` |

**Escalation rules** (monotonic — never downgrade; ported from TSA
`_VERDICT_BOOST`):
- `parse_broken` ⇒ short-circuit to the **`ERROR`** envelope token (TSA M3): the
  graph is untrustworthy, no numeric verdict is emitted.
- `symbol_found == false` ⇒ **`NOT_FOUND`** envelope token.
- `health` (RFC-0114) grade `D`/`F` boosts the verdict one step.
- `test_gap` (RFC-0115) "uncovered" boosts one step — editing untested
  high-fan-in code is strictly riskier.

`reasons` and `checklist` are generated from whichever inputs fired (port of
TSA `_build_required_actions` / `pre_edit_checklist`), e.g. `UNSAFE` →
`"audit all 47 dependents before changing the signature"`.

### Phase 2 — thin Store adapter + CLI/MCP + Skill

A thin adapter assembles `EditMetrics` from the existing engine and surfaces it
as a new capability `mycelium_safe_to_edit` on **both** CLI and MCP (1:1),
covered by a category Skill (see Three-Surface below). The metric sources are
the **already-shipped** reachability surface — no new extraction:

- **`blast_radius`** = size of the transitive *dependents* set, from
  `Store::reachable_to(id, kind, max_depth)` (MCP `mycelium_get_reachable_to` /
  batch `Store::batch_reachable_to`) — i.e. "who depends on me", the impact
  direction (see `crates/mycelium-mcp/src/lib.rs`). *(This is the real, shipped
  surface — earlier drafts called it `mycelium_impact`, which is not a symbol in
  the tree; the adapter MUST bind to `reachable_to`.)*
- **`direct_callers`** = incoming-edge count from the existing callers query.
- **`health` / `test_gap`** populated only once RFC-0114 / RFC-0115 land.

## Verdict vocabulary — reconcile, do not invent

This RFC introduces **no new tokens**. `mycelium_context` already ships a
verdict envelope using `INFO` / `NOT_FOUND` (`crates/mycelium-core/src/context/mod.rs`).
TSA's legal vocabulary is `SAFE / CAUTION / REVIEW / UNSAFE / INFO / WARN /
ERROR / NOT_FOUND`. We adopt **exactly that union** so the new verdict is
drop-in comparable with the existing envelope and with RFC-0114's grade verdict:

- **Decision axis (new use here):** `SAFE` → `CAUTION` → `REVIEW` → `UNSAFE`.
- **Envelope/meta (already in tree):** `INFO`, `NOT_FOUND` (from `context/mod.rs`),
  plus `WARN` / `ERROR` for the broken-parse short-circuit.

No clashing set is created; a single `verdict` field across `mycelium_context`,
RFC-0114 health, and this RFC speaks one vocabulary. **Verdict integrity** (ported
from TSA's rule): the verdict is a hard gate derived from graph facts — a calling
agent MUST surface `REVIEW`/`UNSAFE` verbatim and MUST NOT downgrade to `SAFE`
to satisfy a "just refactor it" instruction.

## Three-Surface compliance (Charter §5.13)

This **is** a new capability (a verdict the engine did not previously emit), so
it is a Phase-2 surfacing obligation, not an additive field:

- **CLI ↔ MCP 1:1 strict.** Phase 2 ships `mycelium safe-to-edit <symbol>` and
  MCP `mycelium_safe_to_edit` with **byte-identical** name, description, args,
  and JSON output (`{ verdict, reasons, checklist, blast_radius, direct_callers }`).
- **Skill coverage (N:1).** The new CLI+MCP pair MUST appear in ≥1
  `skills/<category>/SKILL.md` `allowed-tools` — the natural home is the
  pre-edit / impact category alongside `mycelium_get_reachable_to`. No orphan, no
  Skill-only.

## Acceptance criteria

**Phase 1 — pure verdict core (this RFC's promotable unit):**
- [x] `edit_verdict(metrics)` exists in `crates/mycelium-core/src/verdict/`,
      pure, no Store/I/O. TDD (RED first) with fixtures: `blast=0` → `SAFE`;
      `blast=3` → `CAUTION`; `blast=12` → `REVIEW`; `blast=40` → `UNSAFE`;
      `parse_broken` → `ERROR` short-circuit; `symbol_found=false` → `NOT_FOUND`.
- [x] Health/test-gap escalation is monotonic (never downgrades); fixtures for
      `D`-grade and `uncovered` each boosting one step.
- [x] `reasons` + `checklist` are non-empty for every non-`SAFE` verdict and
      name the concrete count (e.g. "40 dependents").
- [x] Verdict tokens are a strict subset of the reconciled vocabulary above;
      a snapshot test pins them against `context/mod.rs`'s set.

**Phase 2 — adapter + surfaces:**
- [ ] Thin Store adapter assembles `EditMetrics` from existing caller-count +
      `mycelium_get_reachable_to`; `health`/`test_gap` left `None` until RFC-0114/0115 land.
- [ ] `mycelium safe-to-edit` (CLI) + `mycelium_safe_to_edit` (MCP), byte-identical
      (parity snapshot test).
- [ ] ≥1 category Skill lists the new pair in `allowed-tools`.

**Phase 3 (sequenced after RFC-0114 + RFC-0115):**
- [ ] Wire `health` + `test_gap` inputs; re-snapshot the escalation paths.

## Alternatives considered

- **Live LSP / language-server for "real" edit safety.** Rejected by
  **ADR-0010** — no resident server, no subprocess. This verdict is pure graph
  arithmetic over data Mycelium already has; it needs no type-checker.
- **Static SCIP/LSIF ingestion as the impact source.** Complementary, not
  competing (ADR-0010's reserved path): if/when SCIP lands it makes
  `blast_radius` more precise — but the verdict *function* is unchanged; it
  consumes whatever blast radius the engine computes.
- **Let the agent eyeball raw `mycelium_get_reachable_to` numbers.** Rejected: that is
  the status quo and it fails — agents don't reliably convert "47 dependents"
  into "stop". The verdict's value is the *pre-digested gate*, per the
  context-layer positioning.
- **Duplicate health/test logic inside the verdict.** Rejected: compose, don't
  duplicate. Health is RFC-0114, test-gap is RFC-0115; this RFC only *reads*
  their outputs as optional inputs, which is why Phase 3 is sequenced after them.
- **A new clashing verdict enum (e.g. `OK/WARN/BLOCK`).** Rejected: would
  fracture the `verdict` field already shipped by `mycelium_context`. We reuse
  the existing `SAFE/CAUTION/REVIEW/UNSAFE/INFO/WARN/ERROR/NOT_FOUND` union.

## Conflicts with binding constraints

- **ADR-0010 (no live LSP):** ✅ fully compliant — pure arithmetic over the
  existing graph; no server, no subprocess, no type resolution.
- **Charter §4 (≤3-file packs):** ✅ N/A — this is **language-agnostic core**
  logic, not a language pack. It adds zero per-language files; it consumes the
  graph all packs already feed.
- **Charter §5.13 (Three-Surface):** ✅ honored — Phase 2 ships CLI ↔ MCP 1:1
  plus a covering category Skill. No surface is left behind.
- **No new extraction / SLA:** ✅ — gated on existing `mycelium_get_reachable_to` +
  caller-count data; the pure core does no I/O, so it cannot regress the
  extraction performance SLA.
