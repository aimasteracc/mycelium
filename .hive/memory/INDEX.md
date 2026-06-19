# Hive Memory Index

> The PM agent curates this file. Every entry points to a JSONL line or a
> cluster of them. Use grep to find specifics; use this to find the right
> file to grep in.
>
> **Last synthesized**: 2026-06-19 (PM dispatch v302 weekly synthesis)

**Files in this directory:**

- `decisions.jsonl` — every notable decision (append-only, 286 lines as of v302)
- `anti-patterns.jsonl` — mistakes we promised not to repeat (append-only, 52 lines)
- `lessons.jsonl` — generalized learnings consolidated from anti-patterns (25 lines)
- `glossary.jsonl` — project terminology, canonical spellings, translations

---

## Reading Order for New Agents

If you are a new agent invocation, read in this order:

1. `CHARTER.md` (repo root)
2. `.hive/_orchestrator.md`
3. `.hive/<your-role>.agent.md`
4. **This file** — for the map of memory
5. The relevant slice of `anti-patterns.jsonl` (`grep` your domain)
6. The relevant slice of `decisions.jsonl` (recent 50 entries)

---

## Index by Domain

### DCO / commit signing

- `anti-patterns.jsonl`: search `"domain":"dco"` or `"domain":"release-ci"`
- `lessons.jsonl`: search `"domain":"dco-discipline"` — **MUST READ** before any commit/push
- **Key rule**: ONLY `git commit -s` for DCO commits. Never `push_files` for code/docs fixes.

### Release ceremony

- `decisions.jsonl`: search `"agent":"release"` or `"action":"release-prep"`
- `anti-patterns.jsonl`: search `"domain":"release-governance"` (4 entries)
- `lessons.jsonl`: search `"domain":"release-ceremony"` — Charter §5.12 four-step definition
- **Current state** (2026-06-19): v0.3.0 registries published; ceremony blocked on founder `finalize` dispatch (PR #568, ×162 escalations).

### CI / GitHub Actions

- `anti-patterns.jsonl`: search `"domain":"ci"` — 3+ entries on portability, DCO, workflow_call
- `lessons.jsonl`: search `"domain":"ci-portability"` — four recurring runner gotchas
- **Key rule**: test CI changes on a real runner before declaring correct.

### Memory / Hive discipline

- `anti-patterns.jsonl`: search `"domain":"memory-discipline"` or `"domain":"hive/dispatch"`
- `lessons.jsonl`: search `"domain":"hive-memory-integrity"` — push_files corruption patterns
- **Key rule**: append-only files must use `cat >>` + `git commit -s` + `git push`, never `push_files`.

### PM dispatch

- `anti-patterns.jsonl`: search `"domain":"pm-dispatch"` or `"domain":"autonomous-loop"`
- `lessons.jsonl`: search `"domain":"pm-dispatch-discipline"`
- `decisions.jsonl`: search `"action":"pm-dispatch"` (v28–v302)
- **Key rule**: always pre-flight before branching. When no code work → do weekly synthesis, never spin idle.

### Storage (redb)

- `anti-patterns.jsonl`: search `"domain":"storage"` (3 entries)
- `lessons.jsonl`: search `"domain":"storage-atomicity"`
- **Key rule**: one logical operation = one WriteTransaction. No auto-commit-per-sub-op.

### Engine architecture

- `decisions.jsonl`: search `"domain":"engine"` or RFC refs 0001, 0100
- See also: `rfcs/0100-redb-migration.md`, `docs/adr/0008-redb-default-backend.md`

### Reactivity (Salsa, subscriptions)

- `decisions.jsonl`: search `"domain":"reactivity"` or RFC refs 0107, 0108
- `anti-patterns.jsonl`: search `"domain":"async"`

### Hyphae query language

- `decisions.jsonl`: search `"domain":"hyphae"` or RFC refs 0003, 0091, 0124
- See also: `rfcs/0124-hyphae-attr-after-pseudo.md` (Implemented — attr+pseudo any order)

### Language packs

- `decisions.jsonl`: search `"domain":"pack"` or RFC refs 0096, 0113, 0125, 0126
- `lessons.jsonl`: search `"agent":"pack-author"`
- **Key rule**: ≤3 files per pack under `packs/<lang>/`. Never modify core for language onboarding.

### Three-Surface Rule (CLI ↔ MCP ↔ Skill)

- `decisions.jsonl`: search RFC refs 0090, 0105
- See also: `skills/INDEX.md`, `rfcs/0090-cli-mcp-skill-parity.md`
- **Current state**: all 93+ tools covered; RFC-0105 EXCEPTION ratified for WatchEngine.

### Context tool / ranking

- `decisions.jsonl`: search RFC refs 0101, 0119
- `rfcs/0119-context-importance-ranking.md` — **Partially Implemented**; AC-12/AC-13/AC-17 open

### Security / dependencies

- `decisions.jsonl`: search `"agent":"security"`
- `lessons.jsonl`: search `"type":"advisory|ban|secret"`

---

## Recent Highlight Decisions (curated, as of v302)

- **2026-06-04**: RFC-0110 npm/bun CLI distribution (no Rust required) — Implemented
- **2026-06-04**: v0.2.0 release prepared (founder authorized)
- **2026-06-05**: v0.3.0 registries published (crates.io + npm); ceremony blocked since
- **2026-06-06–11**: RFCs 0112–0126 designed and implemented (Implemented or Draft status)
- **2026-06-14**: PyPI v0.3.0 published; nightly.yml ENOTDIR fix merged (PR #861)
- **2026-06-18–19**: RFC backlog confirmed empty (all RFCs 0112–0126 Implemented or governance-Draft)
- **2026-06-19**: Weekly synthesis complete — 6 new lessons appended; INDEX.md updated

---

## Crosswalk: RFC ↔ ADR ↔ Decisions

| RFC | Status | ADR | Key decision |
|---|---|---|---|
| 0001 (Trunk + Synapse) | Implemented | 0001 (Rust), 0002 (tree-sitter) | Core architecture |
| 0090 (Three-Surface Rule) | Implemented | 0007 | CLI↔MCP 1:1; every pair in ≥1 Skill |
| 0100 (redb migration) | Implemented | 0008 | redb as default backend (replaces WAL) |
| 0102 (OutputBudget) | Implemented | — | Budget knob on all 7 graph-list tools |
| 0105 (WatchEngine EXCEPTION) | Ratified | — | Three-Surface exception for watch engine |
| 0107/0108 (Subscribe/Reactive) | Implemented | — | Reactive query subscriptions |
| 0109 (graph-list parity) | Implemented | — | 7/7 tools byte-identical CLI↔MCP |
| 0110 (npm/bun distribution) | Implemented | — | Cargo-less install via npm/bun |
| 0111 (Node+Python SDKs) | Implemented | — | In v0.3.0 (release/v0.3.0 pending ceremony) |
| 0113 (stdlib callee classify) | Implemented | — | 66.4% overall callee classification |
| 0119 (context importance rank) | **Partial** | — | AC-12/AC-13/AC-17 open — dogfood pending |
| 0121 (Charter §2 SLA amend) | Draft | — | PR #763 awaiting founder un-draft |
| 0123 (MCP facade consolidation) | Draft | — | Governance change, team ratification needed |
| 0125/0126 (JS CJS/browser globs) | Implemented | — | CJS require() + browser-global callees |

---

## Glossary Snapshot

> Pull from `glossary.jsonl` for full list. Highlights:

- **Mycelium**: the project name and the underground network metaphor
- **Hyphae**: the query language; literally, a single thread of fungal mycelium
- **Trunk**: the containment-tree storage layer (radix trie)
- **Synapse**: the cross-cutting-edge storage layer (CSR adjacency)
- **The Hive**: the team of AI agents developing Mycelium
- **Pre-flight / Post-flight**: mandatory startup / shutdown rituals for every agent invocation
- **Three-Surface Rule (1:1:1)**: CLI ↔ MCP must be 1:1 byte-identical; every pair must appear in ≥1 Skill
- **TOON format**: Mycelium's token-efficient MCP output (tree responses ≤35% of JSON tokens)
- **ceremony**: the Charter §5.12 four-step release ritual (merge→tag→crates.io→back-merge)

---

*Memory is what makes the Hive smarter than any one of its members.*
