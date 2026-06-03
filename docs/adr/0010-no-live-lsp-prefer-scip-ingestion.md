# ADR-0010: No Live LSP — Prefer Optional Static SCIP/LSIF Ingestion for Semantic Precision

## Status

Accepted — decision recorded 2026-06-04. **Rejects** adopting the Language Server
Protocol (live, server-based) as Mycelium's path to type-level semantic precision.
**Prefers** an optional, static, file-based semantic-index ingestion layer
(SCIP / LSIF) if and when type-aware precision is prioritized.

This ADR records a *negative* architecture decision (a road not taken) so that a
future contributor — human or AI — asked to "improve extraction precision" or
"match Serena" does not silently re-implement live LSP. It directly guards against
the failure mode documented in `.hive/memory/anti-patterns.jsonl` (RFC-0099/0100:
a worker implementing a retired/un-nailed approach because nothing recorded the
decision).

Relates to: ADR-0002 (tree-sitter as parser), Charter §1 (≤3-file language packs),
Charter §2 (performance SLA), Charter §3 (locked tech stack), RFC-0092
(cross-language alias resolution), RFC-0103 (import-aware cross-file resolution).

## Context

Mycelium extracts symbols and edges via declarative tree-sitter `queries.scm`
files (one per language pack, 71–184 lines each). Tree-sitter is a *syntactic*
parser: it cannot perform *semantic* resolution — it does not resolve method calls
through traits/interfaces, generics, or dynamic dispatch, and cannot do
type-directed cross-file reference resolution. This caps extraction precision on
mainstream languages. Cross-file accuracy today is approximated heuristically
(RFC-0014 stub resolution, RFC-0103 import-aware resolution).

A competing MCP server, **Serena**, achieves type-level precision by wrapping
**live LSP** servers (`rust-analyzer`, `gopls`, `pyright`, `clangd`, …). This
raised the question: *should Mycelium adopt LSP?*

The question conflates two distinct things:

- **The capability** — type-level / semantic resolution precision. The need is
  real.
- **The mechanism** — *live LSP*, a long-running per-language server process
  spoken to over JSON-RPC. This is only one way to get the capability.

A second mechanism exists: **static semantic-index formats** — **SCIP** (Sourcegraph
Code Intelligence Protocol) and **LSIF**. These are language-agnostic, file-based
indexes generated offline (e.g. in CI), with **no resident server process**.
Sourcegraph's precise navigation is built on SCIP, not on live LSP at query time.

## Decision

1. **Do NOT adopt live LSP.** Mycelium will not spawn or speak to language-server
   subprocesses at index or query time.

2. **If type-level precision is later prioritized, get it via an *optional, static*
   SCIP/LSIF ingestion pass** that enriches the tree-sitter-built graph with
   precise, type-resolved edges. Tree-sitter remains the always-on fast path; SCIP
   ingestion is an optional enrichment layer, not a dependency. (This would itself
   require its own RFC; this ADR only fixes the *direction*, not the design.)

### Why live LSP is rejected — mapped to binding constraints

| # | Constraint (source) | Why live LSP violates it |
|---|---|---|
| 1 | **§2 SLA: cold small query < 5 ms** | `rust-analyzer`/`clangd`/`gopls` cold-start is seconds-to-minutes on large repos. Fronting a sub-ms engine with a minute-scale IPC server voids the SLA. (cf. RFC-0104, where even redb mmap cold pages needed a separate cold budget — LSP is orders of magnitude worse.) |
| 2 | **README pillar: "single Rust binary, no server, no cloud", embeddable** | Live LSP = one external server process per language; users must install and version-manage `rust-analyzer`, `gopls`, `pyright`, `clangd`, … This destroys the embeddability that is Mycelium's primary differentiation and commercial moat (be the embeddable context layer). |
| 3 | **§1 hard rule: add a language in ≤3 files, 0 core-code lines** | An LSP integration per language is not a declarative `queries.scm` — it requires locating/spawning external binaries, capability negotiation, and lifecycle management: imperative core complexity per language. Forbidden by Charter and CLAUDE.md. |
| 4 | **§3 tech stack: Parser locked to "tree-sitter + declarative .scm"** | Adopting LSP changes the locked parser layer → requires a `meta` RFC amending Charter §3 + founder authorization, not a feature RFC. |
| 5 | **Reactive identity (RFC-0108 Salsa incremental)** | LSP is itself an incremental engine. Wrapping it makes query latency the LSP's latency and degrades Mycelium's reactive layer to a proxy — two redundant incremental engines. |

### Why static SCIP/LSIF ingestion is the coherent alternative

- **No resident process** → preserves "no server", sub-ms warm queries, and
  embeddability.
- **Optional enrichment** → can be a self-contained ingest module, not a
  per-language core change; tree-sitter stays the default.
- **Composes with existing work** → SCIP-precise edges layer on top of RFC-0092
  (cross-language aliases) and RFC-0103 (import-aware resolution), yielding a
  "tree-sitter fast path + SCIP precise path" two-tier model — a story Serena
  (single-language LSP silos) structurally cannot tell.

## Consequences

**Positive**
- Architectural identity (no-server, sub-ms, embeddable, ≤3-file packs) preserved.
- The precision gap has a sanctioned, coherent path (static SCIP) instead of an
  ad-hoc LSP bolt-on.
- A future "improve precision / match Serena" task is steered to the right
  mechanism by this record.

**Negative / accepted trade-offs**
- Until SCIP ingestion exists, mainstream-language extraction precision remains
  tree-sitter-capped (mitigated by RFC-0014/0092/0103 heuristics and per-pack
  query quality, e.g. the Rust pack 67%→99.8% improvement in #492).
- Mycelium will not match Serena's *live*, in-editor, type-exact navigation in the
  IDE-agent editing use case. This is an accepted scope boundary, not a defect:
  Mycelium targets the embeddable read-time context layer, not live editing.

### Conditions that would re-open this decision

Revisit **only if all three hold** — and even then the mechanism is SCIP, not
live LSP:
1. The product direction shifts to competing head-on with Serena in the
   IDE-agent *editing* scenario (vs. the embeddable context-layer strategy).
2. Resources exist to own the cold-start + multi-server distribution cost.
3. The founder formally amends the locked Charter §1/§2/§3 clauses via a `meta` RFC.

## References

- ADR-0002 — tree-sitter as parser
- Charter §1 (language-pack constraint), §2 (performance SLA), §3 (locked stack)
- RFC-0092 — cross-language alias resolution
- RFC-0103 — import-aware cross-file resolution
- RFC-0104 — Charter warm/cold SLA split (precedent: mmap cold pages needed a
  separate budget; live LSP cold-start is far worse)
- PR #492 — Rust pack precision 67%→99.8% (evidence that query quality, not LSP, is
  the near-term precision lever)
