# RFC-0117: architectural-constraint DSL — forbid-rule layering checks over synapse edges (design)

- **Status**: **Draft** (design — no implementation in this PR)
- **Author(s)**: orchestrator (Hive AI agent)
- **Created**: 2026-06-06 (UTC)
- **Depends on**: [RFC-0103](0103-import-aware-cross-file-resolution.md) +
  [RFC-0092](0092-cross-language-alias-resolution.md) (import-aware resolution —
  a forbid rule is only as good as the resolved callee/import file it matches
  against), [ADR-0010](../docs/adr/0010-no-live-lsp-prefer-scip-ingestion.md)
  (no live LSP; static precision — this RFC reads the *already-resolved* synapse
  graph, adds zero runtime), Charter §4 (≤3-file packs — N/A here, this is
  language-agnostic core), Charter §5.13 (Three-Surface Rule)
- **Affected paths** (when implemented): `crates/mycelium-core/src/constraints/`
  (new module: frozen rule types + pure evaluator), later
  `crates/mycelium-core/src/store/` (edge adapter), CLI/MCP/skill surfaces
  (Phase 2). Project config lives at `.mycelium/constraints.yml` (repo-owned, not
  core).
- **Reuses**: tree-sitter-analyzer (TSA) `tree_sitter_analyzer/constraints/`
  — `schema.py` (frozen `Constraint` / `Violation`), `parser.py`
  (`compile_constraints` → pre-compiled glob regexes), `evaluator.py`
  (`evaluate(constraints, edges) → list[Violation]`), and the root
  `architectural-constraints.yml` (the YAML shape). Same founder, MIT. TSA is
  Python over a SQLite `edges` table; Mycelium is Rust over the synapse CSR
  graph — this is a **port-concept + port-schema**, not a source dependency.

## Summary

Add a YAML **forbid-rule DSL** that lets a team assert architectural layering as
data: *"modules matching `from` must NOT reach modules matching `to`, except for
`exception` seams."* The rules are evaluated over Mycelium's synapse **Calls** and
**Imports** edges and produce a list of **Violations** — each naming which edge
broke which rule. This makes invariants like *"UI must not import DB directly"* or
*"domain must not depend on web"* machine-checkable against the real resolved
graph, not a linter's guess. The core is a **pure evaluator** `evaluate(rules,
edges) -> Vec<Violation>` over plain inputs; YAML loading and the Store/CLI/MCP
wiring are a deliberate second phase.

## Motivation

Mycelium already knows *what reaches what* — the synapse edge graph is the exact
substrate an architecture-fitness check needs, and import-aware resolution
(RFC-0092/0103) means a Calls/Imports edge now carries a **resolved callee file**,
not a bare stub. What is missing is a way to *assert* the shape that graph should
have. Teams encode layering in tribal knowledge and README prose; nothing fails
when someone imports the database client straight into a React component.

TSA solved exactly this for its own codebase: a tiny frozen-`Constraint` DSL
(`from_glob`/`to_glob`/`exceptions`) evaluated against `calls` edges, yielding
`Violation`s that escalate a `safe_to_edit` verdict. The schema is ~30 lines and
the evaluator is a pure O(rules × edges) glob match (TSA
`constraints/evaluator.py`). Mycelium has a richer edge graph (Imports *and*
Calls, CSR-laid-out, import-resolved) so the same concept ports cleanly and
gets *more* precise: we can forbid on the import edge, the call edge, or both.

## Decision: a pure evaluator core, with the adapter deferred

Mirror RFC-0113/0114: ship the **pure, testable core first** over plain inputs,
defer all wiring. This keeps the load-bearing logic (rule semantics, glob
matching, exception handling) fully unit-testable with hand-built fixtures and no
Store, no YAML, no CLI.

### Phase 1 — frozen types + pure evaluator (this design's core)

Three frozen structs (immutability convention — no in-place mutation; new values
only), ported 1:1 from TSA `constraints/schema.py`:

```rust
// crates/mycelium-core/src/constraints/types.rs  (frozen — fields are pub-read, constructed once)
pub enum Severity { Error, Warn, Info }
pub enum EdgeKindFilter { Calls, Imports, Any }   // which synapse edge kinds the rule inspects

pub struct Constraint {            // one parsed forbid rule
    pub id: String,
    pub severity: Severity,
    pub applies_to: EdgeKindFilter,    // default: Any
    pub from_glob: String,             // caller / importer side
    pub to_glob: String,               // callee / imported side
    pub reason: String,
    pub exceptions: Vec<String>,       // caller-side allow-list globs (never None — empty Vec)
}

pub struct EdgeRef<'a> {           // a plain, Store-free view of one synapse edge
    pub kind: EdgeKindFilter,      // Calls | Imports
    pub from_path: &'a str,        // resolved source symbol/file path: "src/ui/page.rs>Page>render"
    pub to_path: &'a str,          // resolved target path (import-aware): "src/db/pool.rs>Pool>get"
    pub from_line: u32,
}

pub struct Violation {             // one offending edge × rule
    pub rule_id: String,
    pub severity: Severity,
    pub kind: EdgeKindFilter,
    pub from_path: String,
    pub to_path: String,
    pub from_line: u32,
}

/// PURE: no I/O, no Store, no clock. Compiles globs once, then O(rules × edges).
pub fn evaluate(rules: &[Constraint], edges: &[EdgeRef<'_>]) -> Vec<Violation>;
```

Evaluator semantics (ported from TSA `evaluator.py::_iter_violations`):

1. **Compile once.** Glob → matcher per rule up front (TSA `compile_constraints`),
   so the hot loop is pure matching. Reuse the existing path-glob matcher already
   in the codebase (the same one backing file/module prefix matching) rather than
   pulling a new regex dep.
2. **Edge-kind gate.** A rule with `applies_to: imports` only inspects Imports
   edges; `calls` only Calls; `any` (default) inspects both. (TSA only had
   `calls`; the Imports edge is Mycelium's addition and is where most layering
   breaches actually live — a forbidden *import* precedes the forbidden call.)
3. **Match.** Edge is a violation when `from_path` matches `from_glob` **and**
   `to_path` matches `to_glob`.
4. **Exception suppresses.** If any `exceptions` glob matches the `from_path`, the
   edge is skipped (TSA `_is_excepted` — whitelisted seam).
5. **Skip unresolved.** An edge whose `to_path` is an unresolved bare stub
   (no resolved file from RFC-0092/0103) is skipped — exactly TSA's
   "no callee file → skip" rule, to avoid false positives on dynamic/external
   targets.
6. **Pure & deterministic.** No timestamp inside the core (TSA stamps
   `detected_at` in the evaluator; we move that to the adapter so the core stays a
   pure function of its inputs — easier to snapshot-test).

### Glob matching over symbol / file paths

Mycelium symbol paths are `src/foo.rs>Bar>baz` and file/module prefixes are
glob-matchable. Globs match against the **path prefix up to the first `>`** for
file-level rules (`src/ui/**`), and against the full symbol path when the glob
contains `>` (`src/db/**>*>connect`). `**` is recursive descent; `*` is a single
segment. This is the same two-level model TSA uses (`fnmatch` with `**`), extended
to span Mycelium's `file>Type>member` symbol grammar so a rule can target a whole
layer *or* one method.

### YAML schema (concrete) — loaded in Phase 2, specified now

Project config at `.mycelium/constraints.yml` (repo-owned, like `.editorconfig`):

```yaml
version: 1
constraints:
  - id: ui-must-not-import-db
    severity: error
    rule: forbid           # only `forbid` in v1 (require/layer reserved)
    applies_to: any        # any | calls | imports   (default: any)
    from: "src/ui/**"
    to:   "src/db/**"
    reason: "UI must reach the DB through the service layer, never directly."
    exceptions:
      - "src/ui/admin/migrate.rs"   # one sanctioned seam

  - id: domain-must-not-depend-on-web
    severity: warn
    rule: forbid
    applies_to: imports
    from: "src/domain/**"
    to:   "src/web/**"
    reason: "Domain is framework-agnostic; web depends on domain, never the reverse."
```

`from:`/`to:` are renamed to `from_glob`/`to_glob` on load (TSA `parser.py` does
the same — `from` is a keyword). Loader validates `version == 1`, non-empty
`id`/`from`/`to`, known `severity`, known `applies_to`, and `rule == forbid`,
failing fast with the offending key (input-validation-at-the-boundary).

### Phase 2 — Store adapter + Three-Surface wiring

A thin adapter walks the synapse graph (`edges by kind` for Calls + Imports),
projects each into an `EdgeRef`, calls the pure `evaluate`, and stamps
`detected_at` on the way out. Then the **new capability** is exposed under the
Three-Surface Rule (Charter §5.13):

- **CLI ↔ MCP 1:1 strict.** A new `check-architecture` (working name) pair —
  byte-identical name, description, args, and JSON `Vec<Violation>` output. It
  reads `.mycelium/constraints.yml`, runs the evaluator over the current graph,
  prints violations (and exits non-zero on any `error`-severity hit, for CI).
- **Skill coverage (N:1).** The CLI+MCP pair is listed in the `allowed-tools` of
  a governance/quality category `skills/<category>/SKILL.md` — no orphan, no
  Skill-only.

## Acceptance criteria

**Phase 1 — pure core (this PR's scope when promoted):**
- [ ] `Constraint` / `EdgeRef` / `Violation` frozen types in
      `crates/mycelium-core/src/constraints/types.rs`; no setters; constructed once.
- [ ] `evaluate(rules, edges) -> Vec<Violation>` pure (no I/O, no clock). TDD with
      hand-built fixtures (RED first): `ui→db` Calls edge → 1 violation; same edge
      with `exceptions` covering the caller → 0; `applies_to: imports` ignores a
      Calls edge; unresolved `to_path` stub → skipped; non-matching layer → 0.
- [ ] Glob matcher reuses the existing path-glob util (no new regex dependency);
      covers `**`, `*`, and `file>Type>member` symbol-path targeting. Snapshot tests.
- [ ] `clippy -D warnings`, `fmt --check`, ≥90% line coverage on the new module.

**Phase 2 — adapter + surfaces:**
- [ ] Store adapter projects synapse Calls+Imports edges → `EdgeRef`; integration
      test on a fixture graph with a real forbidden import.
- [ ] YAML loader for `.mycelium/constraints.yml` with fail-fast validation; rejects
      unknown keys, bad `version`, missing `id`/`from`/`to`.
- [ ] `check-architecture` CLI ↔ MCP pair, byte-identical, JSON `Vec<Violation>`,
      non-zero exit on `error` severity; covered by a category Skill `allowed-tools`.
- [ ] Three-Surface parity asserted in the parity test harness.

## Alternatives considered

- **External arch-fitness linter (ArchUnit / import-linter / dependency-cruiser).**
  Rejected: each re-parses the repo and re-derives a dependency graph Mycelium
  *already has* resolved and CSR-laid-out. Evaluating over the synapse graph is
  cheaper, import-aware for free (RFC-0092/0103), and needs no second toolchain.
- **A live LSP / type-checker plugin to derive dependencies.** Rejected by
  **ADR-0010** — this RFC reads only the already-static synapse graph; zero server.
- **Richer rule types now (`require`, `layer`, cycle detection).** Deferred: v1
  ships only `forbid` (TSA's MVP scope too). `require`/`layer` are reserved keys so
  the schema is forward-compatible without a `version` bump for additive rules.
- **Bake rules into core code instead of YAML.** Rejected: layering policy is
  per-project and changes often; data-as-config (the TSA model) lets teams own
  `.mycelium/constraints.yml` without recompiling, and keeps core language-agnostic.
- **Embed a timestamp in the pure core (as TSA does).** Rejected: keeping
  `detected_at` out of `evaluate` makes the core a deterministic pure function —
  the adapter stamps time, the snapshot tests stay stable.

## Conflicts with binding constraints

- **ADR-0010 (no live LSP):** ✅ fully compliant — pure read over the existing
  static synapse graph; no server, no subprocess, no per-repo index step.
- **Charter §4 (≤3-file packs):** N/A — this is language-agnostic core
  (`constraints/`), not a language pack. No pack files added; rules are
  cross-language by construction (a glob can span any pack's symbol paths).
- **Charter §5.13 (Three-Surface):** the new capability ships CLI↔MCP 1:1 +
  Skill coverage in **Phase 2**; Phase 1 adds no surface (pure core only), so it
  introduces no orphan. The `.mycelium/constraints.yml` file is project config,
  not a fourth surface — analogous to how other tools read repo-local config.
- **Immutability / small files:** ✅ frozen structs, new values only; `types.rs` +
  `evaluate.rs` + `glob.rs` (adapter and loader as separate small files in Phase 2).
