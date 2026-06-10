# RFC-0123: MCP Facade Consolidation — 95 tools → 11 action facades

- **Status**: Draft
- **Created**: 2026-06-10
- **Author**: architect (fresh-context design session)
- **Amends**: RFC-0090 (Three-Surface Rule), Charter §5.13 — **governance change,
  requires team-consensus ratification** (investor-mode: founder delegated;
  acceptance of this RFC + a companion ADR is the ratification mechanism)
- **Depends on**: RFC-0090 (parity machinery), RFC-0094 Phase 4 (output formats),
  RFC-0102 (budget), RFC-0107/0108 (SUBSCRIBE)
- **Implementation**: phased follow-up PRs (Phase 1–3 below), **not** this RFC.
  This RFC is design + governance only.

---

## Summary

The Mycelium MCP server exposes **95 tools**. A single `tools/list` response
measures **189,624 bytes ≈ 47.4K tokens** (measured live, see §Motivation) —
a tax that schema-injecting MCP clients pay on **every** request. We propose
consolidating the 95 tools into **11 facade tools**, each selecting its
capability via a required `action` enum parameter. The wire payload of every
capability is unchanged — responses stay **byte-identical** to today's
per-tool outputs. Only the *tool surface* shrinks.

This requires amending the Three-Surface Rule (RFC-0090 / Charter §5.13):
CLI ↔ MCP parity moves from **tool-level 1:1** to **action-level 1:1**.
Every CLI subcommand still has exactly one MCP twin — it is now a
`(facade, action)` pair instead of a standalone tool, with the action name
mechanically derivable from the CLI subcommand name.

In one line: **same 95 capabilities, same 95 contracts, 11 doors instead
of 95.**

## Motivation

### Measured numbers (live server, develop @ ce85709)

Methodology: `cargo build --release -p mycelium-rcig-cli`, then drive
`mycelium serve --mcp` over stdio with a JSON-RPC script
(`initialize` → `notifications/initialized` → `tools/list`) and measure
the responses. Reproduction script in §Appendix A.

| Metric | Value |
|---|---:|
| Registered MCP tools (`#[tool(...)]` in `crates/mycelium-mcp/src/lib.rs`) | **95** |
| `tools/list` result object, compact JSON | **189,624 bytes** |
| ≈ tokens (bytes ÷ 4) | **≈ 47.4K tokens** |
| Average schema bytes per tool | 1,996 |
| Largest single tool schema | `mycelium_subscribe` (5,526 B) |
| `initialize.instructions` | 3,590 B (cheap; paid once) |

An earlier measurement on ce85709+ recorded 195,388 bytes ≈ 48.8K tokens —
same order; the surface hovers around **190–195 KB** and grows with every
capability (PR #743, in flight, adds `mycelium_test_gap` → 96).

### Thematic breakdown of the 95

graph-theory 26 · callers/callees/refs 14 · symbols/search 10 · trees 8 ·
index/admin 7 · paths 5 · imports/extends/implements 3 · analyze 2 ·
subscribe 2 (+1 status) · query/context 2 · misc 16.

### Why this is a product problem, not a cosmetic one

1. **Token tax on every request.** Schema-injecting clients (Claude Desktop,
   many SDK-based agents) resend the full tool list with each model call.
   47K tokens is ~24% of a 200K context window spent before the agent reads
   a single line of code — for a product whose pitch is *token density*.
2. **Tool-selection paralysis.** Our own agent-experience QA found agents
   stalling between 5 overlapping "who calls X" tools
   (`get_callers`, `get_caller_tree`, `get_common_callers`,
   `get_cross_refs`, `get_reachable_to`). Industry experience converges on
   agents degrading beyond ~40 tools.
3. **Sibling projects already consolidated.** tree-sitter-analyzer ships
   **8 facade tools** with `action` params; codegraph ships ~10 tools. Both
   are directly comparable code-intelligence MCP servers, and both route by
   intent with no measured capability loss.
4. **Count drift is already visible.** The hard-coded instructions header
   says "93 tools", README.md says 93, `docs/walkthrough.md` says 93 — the
   real count is 95. Per-tool surfaces rot; a fixed facade set with a
   generated action inventory cannot drift the same way.

### Root cause

RFC-0090's Three-Surface Rule makes CLI ↔ MCP **strict 1:1 at tool level**.
Every CLI subcommand mechanically spawns an MCP tool, so the MCP tool count
grows linearly with CLI subcommands. The rule's *purpose* — no
single-surface capabilities, no drift — is sound. Its *granularity* (one
MCP tool per capability) is the bug. The CLI itself does not suffer: 95
subcommands under one `mycelium` binary cost nothing until invoked, and
`--help` is hierarchical. MCP has no hierarchy — `tools/list` is flat and
always fully expanded. The facade pattern *is* the hierarchy MCP lacks.

## Design

### Facade set (11 tools)

Facade boundaries follow the existing Skill categories (RFC-0090 Phase 2)
and the routing intents already encoded in `initialize.instructions` —
agents are *already taught* to think in these categories; the facades make
the taught structure physical.

| # | Facade tool | Actions | Intent |
|---|---|---:|---|
| 1 | `mycelium_query` | 1 | Hyphae DSL query (unchanged, stays standalone — flagship) |
| 2 | `mycelium_context` | 1 | Task-focused context bundle (unchanged, stays standalone — primary entry) |
| 3 | `mycelium_symbols` | 12 | Find/describe symbols: search, info, kinds, spans, containment |
| 4 | `mycelium_callgraph` | 10 | Who-calls-whom: direct, transitive, paths, common, hygiene |
| 5 | `mycelium_reach` | 16 | Reachability, neighborhoods, refs, shortest paths |
| 6 | `mycelium_hierarchy` | 12 | Imports / extends / implements: neighbors, trees, paths |
| 7 | `mycelium_graph` | 16 | Graph structure: cycles, SCC/WCC, layers, topology, stats |
| 8 | `mycelium_rank` | 15 | Centrality & ranking: PageRank, betweenness, hubs, fan-in/out |
| 9 | `mycelium_analyze` | 2 | Verdicts: project health, safe-to-edit (absorbs `test_gap` when PR #743 lands → 3) |
| 10 | `mycelium_admin` | 7 | Index lifecycle, status, diagnostics, token stats |
| 11 | `mycelium_subscribe` | 3 | RFC-0107/0108 reactive subscriptions |
| | **Total** | **95** | |

`query` and `context` stay standalone (action count 1, no `action` param)
deliberately: they are the two tools the instructions route to FIRST, and
burying the flagship behind an enum would cost selection accuracy for zero
schema savings (their schemas are big because of Hyphae/context docs, not
tool-count overhead).

### Action naming rule (deterministic, CI-checkable)

> **action = legacy MCP tool name minus the `mycelium_` prefix**
> (equivalently: the CLI subcommand name, kebab→snake case).

`mycelium_get_callers` → `mycelium_callgraph {action: "get_callers"}`;
CLI `mycelium get-callers` ↔ action `get_callers`. No renames, no
prettification (`callers` vs `get_callers` bikeshed explicitly rejected):
keeping the canonical capability name makes (a) the CLI↔action derivation a
pure string function, (b) the parity script extension mechanical, (c) every
existing doc/skill/test reference greppable across the migration, and
(d) the legacy→facade rewrite a regex (`mycelium_X(args)` →
`facade(action:"X", args)`).

The facade-membership table (§Complete mapping) is the one non-derivable
fact; it is checked in as a generated artifact
(`skills/INDEX.md` gains a `facade` column) and CI fails on drift.

### Complete mapping: all 95 tools → facade.action

#### `mycelium_query` (1)

| Legacy tool | Facade.action |
|---|---|
| `mycelium_query` | `mycelium_query` (unchanged) |

#### `mycelium_context` (1)

| Legacy tool | Facade.action |
|---|---|
| `mycelium_context` | `mycelium_context` (unchanged) |

#### `mycelium_symbols` (12)

| Legacy tool | Facade.action |
|---|---|
| `mycelium_search_symbol` | `symbols.search_symbol` |
| `mycelium_get_symbol_info` | `symbols.get_symbol_info` |
| `mycelium_batch_symbol_info` | `symbols.batch_symbol_info` |
| `mycelium_get_ancestors` | `symbols.get_ancestors` |
| `mycelium_get_descendants` | `symbols.get_descendants` |
| `mycelium_get_siblings` | `symbols.get_siblings` |
| `mycelium_get_node_kind` | `symbols.get_node_kind` |
| `mycelium_get_symbols_by_kind` | `symbols.get_symbols_by_kind` |
| `mycelium_get_source_span` | `symbols.get_source_span` |
| `mycelium_get_all_symbols` | `symbols.get_all_symbols` |
| `mycelium_get_symbol_count_by_kind` | `symbols.get_symbol_count_by_kind` |
| `mycelium_get_files` | `symbols.get_files` |

#### `mycelium_callgraph` (10)

| Legacy tool | Facade.action |
|---|---|
| `mycelium_get_callees` | `callgraph.get_callees` |
| `mycelium_get_callers` | `callgraph.get_callers` |
| `mycelium_get_callee_tree` | `callgraph.get_callee_tree` |
| `mycelium_get_caller_tree` | `callgraph.get_caller_tree` |
| `mycelium_find_call_path` | `callgraph.find_call_path` |
| `mycelium_get_common_callers` | `callgraph.get_common_callers` |
| `mycelium_get_common_callees` | `callgraph.get_common_callees` |
| `mycelium_get_entry_points` | `callgraph.get_entry_points` |
| `mycelium_get_dead_symbols` | `callgraph.get_dead_symbols` |
| `mycelium_get_isolated_symbols` | `callgraph.get_isolated_symbols` |

#### `mycelium_reach` (16)

| Legacy tool | Facade.action |
|---|---|
| `mycelium_get_reachable` | `reach.get_reachable` |
| `mycelium_get_reachable_to` | `reach.get_reachable_to` |
| `mycelium_get_reachable_set` | `reach.get_reachable_set` |
| `mycelium_get_reaches_into` | `reach.get_reaches_into` |
| `mycelium_get_common_reachable` | `reach.get_common_reachable` |
| `mycelium_get_mutual_reachability` | `reach.get_mutual_reachability` |
| `mycelium_batch_reachable_from` | `reach.batch_reachable_from` |
| `mycelium_batch_reachable_to` | `reach.batch_reachable_to` |
| `mycelium_get_k_hop_neighbors` | `reach.get_k_hop_neighbors` |
| `mycelium_get_two_hop_neighbors` | `reach.get_two_hop_neighbors` |
| `mycelium_get_shortest_path` | `reach.get_shortest_path` |
| `mycelium_get_symbol_neighborhood` | `reach.get_symbol_neighborhood` |
| `mycelium_get_cross_refs` | `reach.get_cross_refs` |
| `mycelium_get_outgoing_refs` | `reach.get_outgoing_refs` |
| `mycelium_get_dependency_depth` | `reach.get_dependency_depth` |
| `mycelium_get_singly_referenced` | `reach.get_singly_referenced` |

#### `mycelium_hierarchy` (12)

| Legacy tool | Facade.action |
|---|---|
| `mycelium_get_imports` | `hierarchy.get_imports` |
| `mycelium_get_import_tree` | `hierarchy.get_import_tree` |
| `mycelium_get_importers_tree` | `hierarchy.get_importers_tree` |
| `mycelium_find_import_path` | `hierarchy.find_import_path` |
| `mycelium_get_extends` | `hierarchy.get_extends` |
| `mycelium_get_extends_tree` | `hierarchy.get_extends_tree` |
| `mycelium_get_subclasses_tree` | `hierarchy.get_subclasses_tree` |
| `mycelium_find_extends_path` | `hierarchy.find_extends_path` |
| `mycelium_get_implements` | `hierarchy.get_implements` |
| `mycelium_get_implements_tree` | `hierarchy.get_implements_tree` |
| `mycelium_get_implementors_tree` | `hierarchy.get_implementors_tree` |
| `mycelium_find_implements_path` | `hierarchy.find_implements_path` |

#### `mycelium_graph` (16)

| Legacy tool | Facade.action |
|---|---|
| `mycelium_get_stats` | `graph.get_stats` |
| `mycelium_get_graph_metrics` | `graph.get_graph_metrics` |
| `mycelium_get_node_degree` | `graph.get_node_degree` |
| `mycelium_batch_node_degree` | `graph.batch_node_degree` |
| `mycelium_detect_cycles` | `graph.detect_cycles` |
| `mycelium_find_cycle_members` | `graph.find_cycle_members` |
| `mycelium_get_scc_groups` | `graph.get_scc_groups` |
| `mycelium_get_strongly_connected_components` | `graph.get_strongly_connected_components` |
| `mycelium_get_wcc` | `graph.get_wcc` |
| `mycelium_get_dependency_layers` | `graph.get_dependency_layers` |
| `mycelium_topological_sort` | `graph.topological_sort` |
| `mycelium_find_articulation_points` | `graph.find_articulation_points` |
| `mycelium_find_bridge_edges` | `graph.find_bridge_edges` |
| `mycelium_get_biconnected_components` | `graph.get_biconnected_components` |
| `mycelium_get_k_core` | `graph.get_k_core` |
| `mycelium_get_degree_histogram` | `graph.get_degree_histogram` |

#### `mycelium_rank` (15)

| Legacy tool | Facade.action |
|---|---|
| `mycelium_rank_symbols` | `rank.rank_symbols` |
| `mycelium_page_rank` | `rank.page_rank` |
| `mycelium_get_betweenness_centrality` | `rank.get_betweenness_centrality` |
| `mycelium_get_closeness_centrality` | `rank.get_closeness_centrality` |
| `mycelium_get_degree_centrality` | `rank.get_degree_centrality` |
| `mycelium_get_harmonic_centrality` | `rank.get_harmonic_centrality` |
| `mycelium_get_eccentricity` | `rank.get_eccentricity` |
| `mycelium_get_clustering_coefficient` | `rank.get_clustering_coefficient` |
| `mycelium_get_neighbor_similarity` | `rank.get_neighbor_similarity` |
| `mycelium_get_hub_symbols` | `rank.get_hub_symbols` |
| `mycelium_get_most_connected` | `rank.get_most_connected` |
| `mycelium_get_top_files` | `rank.get_top_files` |
| `mycelium_get_leaf_symbols` | `rank.get_leaf_symbols` |
| `mycelium_get_fan_out_rank` | `rank.get_fan_out_rank` |
| `mycelium_get_fan_in_rank` | `rank.get_fan_in_rank` |

#### `mycelium_analyze` (2)

| Legacy tool | Facade.action |
|---|---|
| `mycelium_project_health` | `analyze.project_health` |
| `mycelium_safe_to_edit` | `analyze.safe_to_edit` |

*(PR #743's `mycelium_test_gap` → `analyze.test_gap` when it lands.)*

#### `mycelium_admin` (7)

| Legacy tool | Facade.action |
|---|---|
| `mycelium_index_workspace` | `admin.index_workspace` |
| `mycelium_load_index` | `admin.load_index` |
| `mycelium_server_status` | `admin.server_status` |
| `mycelium_watch_status` | `admin.watch_status` |
| `mycelium_sync_file` | `admin.sync_file` |
| `mycelium_set_compact_mode` | `admin.set_compact_mode` |
| `mycelium_get_token_stats` | `admin.get_token_stats` |

#### `mycelium_subscribe` (3)

| Legacy tool | Facade.action |
|---|---|
| `mycelium_subscribe` | `subscribe.subscribe` |
| `mycelium_unsubscribe` | `subscribe.unsubscribe` |
| `mycelium_subscription_status` | `subscribe.subscription_status` |

**Allocation audit**: 1+1+12+10+16+12+16+15+2+7+3 = **95**. Every tool from
the live registration list (§Appendix B) appears exactly once.

### Normative facade schema: `mycelium_callgraph`

The pattern below is normative for all 9 multi-action facades. Measured
size: **2,721 bytes** compact JSON (vs ~20,000 bytes for the 10 legacy
tools it replaces).

```json
{
  "name": "mycelium_callgraph",
  "description": "Call-graph queries: who calls X, what X calls, transitive trees, call paths, shared callers/callees, and call-graph hygiene (entry points / dead / isolated symbols). Select a capability with `action`; per-action parameter requirements are listed on each enum value. Responses are byte-identical to the v0.x per-tool outputs.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "action": {
        "type": "string",
        "enum": [
          "get_callees", "get_callers", "get_callee_tree",
          "get_caller_tree", "find_call_path", "get_common_callers",
          "get_common_callees", "get_entry_points", "get_dead_symbols",
          "get_isolated_symbols"
        ],
        "description": "get_callees: direct callees of `path` | get_callers: direct callers of `path` (opt: include_virtual) | get_callee_tree: transitive callee tree from `path` (opt: max_depth) | get_caller_tree: transitive caller tree to `path` (opt: max_depth) | find_call_path: shortest call path `from` -> `to` | get_common_callers: callers shared by all `paths` | get_common_callees: callees shared by all `paths` | get_entry_points: symbols with callers=0 (opt: limit) | get_dead_symbols: unreachable from any entry point (opt: limit) | get_isolated_symbols: no callers and no callees (opt: limit)"
      },
      "path": {
        "type": "string",
        "description": "Trunk path of the subject symbol, e.g. \"src/lib.rs>process\". Required by: get_callees, get_callers, get_callee_tree, get_caller_tree."
      },
      "paths": {
        "type": "array", "items": { "type": "string" },
        "description": "Trunk paths (2..=50). Required by: get_common_callers, get_common_callees."
      },
      "from": { "type": "string", "description": "Start symbol trunk path. Required by: find_call_path." },
      "to":   { "type": "string", "description": "End symbol trunk path. Required by: find_call_path." },
      "max_depth": { "type": "integer", "description": "Maximum traversal depth for tree actions. Defaults to 4, capped at 10." },
      "include_virtual": { "type": "boolean", "description": "get_callers only: also include callers reaching the symbol via virtual dispatch (ancestor method of the same name). Default false." },
      "edge_kind": { "type": "string", "description": "Edge kind to traverse for get_callees/get_callers: \"calls\" (default), \"imports\", \"extends\", \"implements\"." },
      "limit": { "type": "integer", "description": "Maximum number of results for list actions. Server default applies when omitted." },
      "output_format": { "type": "string", "enum": ["json", "text", "msgpack"], "description": "Response format override. Omit for transport default — \"text\" (TOON) on stdio MCP, \"json\" for programmatic callers (RFC-0094 Phase 4)." },
      "budget": { "type": "string", "description": "Per-call output budget (RFC-0102): \"auto\" (default), \"small\", \"medium\", \"large\", \"disabled\"." }
    },
    "required": ["action"]
  }
}
```

Schema design decisions (normative):

1. **`action` is the only `required` field.** Per-action requireds are
   validated server-side (§Errors). A JSON-Schema `oneOf` branch per action
   would express this statically but multiplies schema size by ~the action
   count — defeating the purpose — and `oneOf` dispatch support is uneven
   across MCP client schema renderers. Rejected.
2. **Flat parameter bag = union of the facade's per-action params**, each
   field documenting which actions consume it. Shared params keep their
   RFC-established names and docs verbatim: `path`, `paths`, `from`, `to`,
   `max_depth`, `limit`, `edge_kind`, `output_format` (RFC-0094),
   `budget` (RFC-0102).
3. **The `action` enum description is the routing table** — one `|`-separated
   line per action: what it does + its required/optional params. This is the
   single most token-dense place to teach selection; it replaces 10 separate
   2KB schemas with one 1KB string.
4. **Param name collisions across actions in one facade are forbidden** —
   if two actions need a parameter with the same name it must have the same
   type and meaning (true today for all 95 by construction, since shared
   params were already standardized by RFC-0094/0102; CI re-checks).

### Errors (normative)

Unknown or missing `action` → the existing structured error shape
(`crates/mycelium-mcp/src/error.rs`), with the valid action list inline:

```json
{ "error": "UNKNOWN_ACTION", "message": "unknown action \"callers\" for mycelium_callgraph — did you mean \"get_callers\"? Valid actions: get_callees, get_callers, get_callee_tree, get_caller_tree, find_call_path, get_common_callers, get_common_callees, get_entry_points, get_dead_symbols, get_isolated_symbols" }
```

Missing per-action param → `MISSING_PARAM` naming the action, the param,
and the action's full signature. Both error shapes are RED-testable and
follow the explicit-error precedent of PR #703 (no silent empty sets).

### Response contract: unchanged, byte-identical

A facade action is a pure dispatcher to the same handler the legacy tool
calls today. For every legacy tool `mycelium_X` and its facade twin
`F(action="X")`, identical inputs MUST produce **byte-identical** responses
(modulo timestamps, same carve-out as RFC-0090 I4). This is the **twin
invariant** and is enforced by generated tests (§Acceptance). TOON/text,
JSON, msgpack, budget behavior, compact mode — all unchanged.

### `initialize.instructions` rewrite

The instructions (currently 3,590 B, hard-coding 60+ tool names and a
stale "93 tools" header) are rewritten to route by facade + action, in the
style tree-sitter-analyzer ships ("Intent → tool + action" table). The
small-project-mode appendix routes to `(facade, action)` pairs. Estimated
size: roughly unchanged (≤4 KB) — instructions are paid once per session,
not per request, so we optimize them for routing accuracy, not bytes.

## Three-Surface Rule amendment (RFC-0090 / Charter §5.13)

> **This section is a governance change.** It amends a Charter Hard Rule
> and therefore requires explicit ratification: founder acceptance of this
> RFC + a companion ADR (`docs/adr/00XX-mcp-facade-consolidation.md`)
> recording the decision. Until ratified, RFC-0090's tool-level wording
> remains in force and Phase 1 cannot merge.

### Current wording (RFC-0090)

> CLI ↔ MCP is 1:1 — strict. Each capability has exactly one CLI subcommand
> and exactly one MCP tool. Name, description, argument schema, and JSON
> output are byte-identical across the two.

### Proposed wording

> **CLI ↔ MCP is 1:1 — strict, at action level.** Each capability has
> exactly one CLI subcommand and exactly one MCP **facade action**
> `(facade, action)`. The action name is mechanically derivable from the
> CLI subcommand name (kebab-case CLI ↔ snake_case action; e.g.
> `mycelium get-callers` ↔ `mycelium_callgraph {action: "get_callers"}`).
> Per-capability argument names/types/docs and JSON output are
> byte-identical across the two surfaces. The facade-membership table
> (capability → facade) is a generated, CI-checked artifact in
> `skills/INDEX.md`.
>
> **(CLI, MCP-action) ↔ Skill is N:1 — covered.** Every
> (CLI subcommand, facade action) pair MUST be referenced by ≥1 Skill. A
> Skill's `allowed-tools` lists the **facade** tool name; the SKILL.md body
> MUST document each covered **action** in its per-capability sections.
> Orphan = a capability whose action appears in no SKILL.md body. No
> orphans. No Skill-only.

What does **not** change: the definition of "capability", the four pair
invariants (name/description/argument/output parity — now applied at action
granularity), the exception mechanism (`EXCEPTION:` lines; RFC-0105's watch
exception carries over verbatim to `admin.watch_status` /
`subscribe.*`), and the no-orphan coverage invariant.

Enforcement changes (`scripts/check_skill_parity.py`):

- `_extract_mcp_tools` extracts `(facade, action)` pairs from the action
  enum registrations instead of `async fn mycelium_*` names.
- I2 ("every allowed-tool exists") checks facade names in frontmatter
  **and** action coverage in SKILL.md bodies.
- New invariant **I5 (membership freshness)**: the capability → facade
  table in `skills/INDEX.md` matches the code.
- New invariant **I6 (twin byte-identity)**: for every action, a generated
  test calls legacy tool and facade action with identical inputs and
  asserts byte-identical output (Phase 1–2 only; retired with the legacy
  surface in Phase 3, where I6 degrades to CLI ↔ action output parity —
  the existing I4 at action level).

## Token economics

### Estimated post-consolidation `tools/list`

Method: the callgraph facade schema above was built in full and measured
(2,721 B compact). Other facades estimated from it by action count
(≈1,100 B base + ≈160 B per action enum-line + param fields), except
`query`/`context` (kept as-is, measured today) and `subscribe` (dominated
by the RFC-0107 tagged-union spec, which must survive consolidation
intact).

| Facade | Actions | Est. bytes |
|---|---:|---:|
| `mycelium_query` (as today) | 1 | 2,698 |
| `mycelium_context` (as today) | 1 | 2,737 |
| `mycelium_symbols` | 12 | ~3,000 |
| `mycelium_callgraph` (measured) | 10 | 2,721 |
| `mycelium_reach` | 16 | ~3,700 |
| `mycelium_hierarchy` | 12 | ~3,100 |
| `mycelium_graph` | 16 | ~3,700 |
| `mycelium_rank` | 15 | ~3,600 |
| `mycelium_analyze` | 2 | ~1,400 |
| `mycelium_admin` | 7 | ~2,400 |
| `mycelium_subscribe` | 3 | ~6,200 |
| envelope | | ~300 |
| **Total** | **95** | **≈ 35,600 B ≈ 8.9K tokens** |

**Reduction: 189,624 → ≈35,600 bytes ≈ 81%** (47.4K → ≈8.9K tokens).
Honest accounting: action enums + per-action routing lines + per-action
"Required by:" docs add back ~15 KB versus a naive "11 × 400 B" fantasy;
the estimate above includes them. The acceptance gate is set at the 80%
line (≤ 37,925 B) with the estimate ~6% under it, so the gate is tight but
not aspirational.

### Expected selection-accuracy benefit

- 95 → 11 top-level choices puts the surface back under the ~40-tool
  degradation threshold with 4× headroom.
- The 5-way "who calls X" paralysis becomes a 2-way choice
  (`callgraph` vs `reach`) whose enum descriptions disambiguate inline —
  at the moment of choice, not in a far-away instructions blob.
- Facade = Skill category = instructions intent: one taxonomy everywhere.
- New capabilities stop costing top-level slots: `test_gap` (PR #743)
  lands as one enum value (+~160 B), not a 96th tool (+~2 KB).

## Migration & back-compat

### Phasing

| Phase | Release | Server behavior |
|---|---|---|
| **1** | next minor (v0.x+1) | Facades ship **alongside** all 95 legacy tools. New flag `mycelium serve --mcp --tool-surface=facade\|legacy\|both` (env `MYCELIUM_TOOL_SURFACE`), default **`both`**. Twin tests (I6) gate the PR. Instructions rewritten for facades; legacy tools get a one-line "prefer `<facade>(action=...)`" prefix in `both` mode. |
| **2** | v0.x+2 (2 minors later) | Default flips to **`facade`**. Legacy available behind `--tool-surface=legacy\|both`. CHANGELOG + README migration table. Skills flipped to facade names. |
| **3** | next **major** | Legacy tool registrations removed; flag values `legacy`/`both` return a startup error naming this RFC. I6 twin tests retire; action-level I4 parity remains. |

Default `both` in Phase 1 is safe for clients that merely *see* more tools
(both surfaces resolve to the same handlers) but doubles the schema bytes;
that is acceptable for one transition window because agents configured for
facades (`--tool-surface=facade`) get the full savings immediately, and
unconfigured agents lose nothing they have today.

### In-repo consumer inventory (verified by grep, this worktree)

| Consumer | What it references | Update (phase) |
|---|---|---|
| `skills/*/SKILL.md` × 11 + `_template` | `mcp__mycelium__<tool>` in `allowed-tools` (e.g. call-graph lists 8); tool names throughout bodies | Frontmatter → facade names; bodies → action-documented sections (2) |
| `skills/INDEX.md` | generated coverage matrix | regenerate with facade column (1) |
| `scripts/check_skill_parity.py` | extracts `async fn mycelium_*` | action-level extraction + I5/I6 (1) |
| `MCP_INSTRUCTIONS_BASE` (`crates/mycelium-mcp/src/lib.rs:4160`) | 60+ tool names; stale "93 tools" header | full rewrite (1) |
| `crates/mycelium-mcp/src/tests.rs` | 92 distinct `mycelium_*` names | add generated twin tests (1); flip canonical names (3) |
| `crates/mycelium-cli/tests/` parity/purity tests | tool names via CLI twins | extend to actions (1) |
| `README.md:51` | "MCP server (93 tools)" — already stale | "11 facade tools, 95 capabilities" (1) |
| `docs/walkthrough.md:153` | "exposes 93 tools" — already stale | same (1) |
| `rfcs/0004, 0012, 0016, 0018, 0019, 0101, 0102, 0106, 0107, 0109, …` | historical tool names | no edits (historical record); this RFC is the forward pointer |
| `CHANGELOG.md` | historical names | append-only; no rewrites |
| `.github/workflows/` (`parity` job in `ci.yml`, `release.yml`) | runs parity script | inherits script changes (1) |

**Non-consumers (verified):** `editors/vscode` calls the **CLI** via
`@aimasteracc/mycelium-sdk` (`editors/vscode/src/engine.ts` imports the SDK,
which locates the prebuilt binary — no MCP tool names anywhere).
`npm/sdk` and `bindings/python` wrap the CLI; each mentions
`mycelium_context` once, in a docstring ("the `mycelium_context` twin") —
cosmetic, updated in Phase 2. **The entire SDK/IDE surface is untouched by
this consolidation** — a direct payoff of RFC-0111's thin-CLI-wrapper
decision.

**External consumers** (Claude Desktop / agent configs pointing at
`mycelium serve --mcp`): no action required. In Phase 1 they see both
surfaces; in Phase 2 they see 11 tools. Nothing they could have scripted
against breaks while `both` exists, and tool *payloads* never change at any
phase.

## Risks

1. **Schema-validation weakening** (biggest real cost): per-action required
   params move from JSON Schema (`"required"`) to server-side checks. A
   client can no longer statically know `find_call_path` needs `from`/`to`.
   Mitigated by the enum routing line, "Required by:" field docs, and
   `MISSING_PARAM` errors that teach the full signature. tree-sitter-analyzer
   runs this exact trade in production without observed agent confusion.
2. **Enum-description bloat over time.** Each new action adds a routing
   line; a 30-action facade's enum description would degrade. Guard: CI
   warns when any facade exceeds 20 actions — split the facade (a new
   facade is a minor-version event, not a contract break).
3. **`both` window doubles schema** (~225 KB). Time-boxed to two minors;
   agents can opt into `facade` on day one.
4. **Twin-test surface is large** (95 × N format/budget combos). Generated,
   not hand-written; runs as one integration suite over the fixture index
   already used by `token_bench.rs`.
5. **Charter-amendment failure mode**: if ratification stalls, nothing
   ships (Phase 1 is gated on this RFC's acceptance) — by design, per the
   "never bypass RFC for governance" Hard Rule.

## Alternatives considered

### (A) Dynamic tool discovery / lazy schemas

Expose a `search_tools` meta-tool; serve full schemas on demand.
**Rejected**: client support is uneven — some clients have deferred
tool-search (Claude Code's ToolSearch), but most schema-injecting clients
(and every "dumb" MCP host) call `tools/list` once and inject everything.
We would optimize for the one client class that already solved the problem
and leave the rest paying full price. Also adds a protocol-shaped runtime
dependency on client behavior we don't control.

### (B) Server-side tool filtering via config

`--expose-tools=callers,callees,…` lets each deployment trim the list.
**Rejected as primary fix**: pushes the curation burden onto every user,
defaults still pay 190 KB, and partial surfaces break the Three-Surface
audit story (which capabilities exist becomes deployment-dependent).
Note `--tool-surface` is *not* this: it selects between two complete,
parity-checked surfaces, never a subset.

### (C) Keep 95 tools, improve instructions only

Cheapest. **Rejected**: instructions already route by intent (the current
3.6 KB text is good) and the paralysis was observed *with* it. The 47K-token
schema tax is structural — no prose fixes it.

### (D) MCP resources instead of tools

Model read-only queries as `resources/read` URIs. **Rejected**: resources
have no input schema (parameters would be stringly-typed URI conventions),
many clients don't auto-surface resources to the model, and we would
trade a flat-but-typed surface for an invisible-and-untyped one.
RFC-0107's subscription URIs already use resources where they fit
(push-updated state); query capabilities are tool-shaped.

### (E) Fewer, coarser facades (e.g. tree-sitter-analyzer's 8)

Merging `rank` into `graph` (31 actions) and `reach` into `callgraph` (26)
was sketched. **Rejected**: both merged enums cross the 20-action guard
immediately, the enum routing lines stop fitting in one screen, and the
Skill-category alignment (one facade ↔ 1–2 skills) breaks. 11 is already
≤ the sibling-project band (8–10) plus our two deliberate standalones.

### (F) Renamed, prettified actions (`callers` not `get_callers`)

**Rejected** — see §Action naming rule: breaks mechanical CLI↔action
derivation, makes migration non-greppable, and invites a 95-name bikeshed.
A future major may revisit naming wholesale; orthogonal to consolidation.

## Acceptance criteria (RED-testable)

- [ ] **Facade size gate**: with `--tool-surface=facade`, the `tools/list`
      result object (compact JSON) is **≤ 37,925 bytes** (80% of the
      measured 189,624 B baseline); CI captures the measured number in the
      job summary. Stretch target: ≤ 36,000 B.
- [ ] **Twin invariant (I6)**: a generated test suite calls every one of
      the 95 legacy tools and its facade action with identical inputs and
      asserts **byte-identical** responses (modulo RFC-0090's timestamp
      carve-out), across `output_format` ∈ {json, text} and default budget.
      RED first: the suite exists and fails before facade dispatch is
      implemented.
- [ ] **Unknown-action error**: calling any facade with an invalid `action`
      returns `UNKNOWN_ACTION` naming every valid action; missing per-action
      params return `MISSING_PARAM` naming action + param + signature.
- [ ] **Parity script at action level**: `check_skill_parity.py` extracts
      `(facade, action)` pairs; I1/I2 + new I5 (membership table freshness)
      + I6 wiring pass in `--strict` mode on the Phase 1 tree.
- [ ] **`--tool-surface` flag**: `facade|legacy|both` each serve the exact
      promised tool set (asserted by a `tools/list` name-set test);
      default is `both` in Phase 1.
- [ ] **Instructions rewritten**: `MCP_INSTRUCTIONS_BASE` routes by
      facade.action, contains zero stale tool counts (count is interpolated,
      not hard-coded), and ≤ 4,096 bytes.
- [ ] **Skills updated**: all 11 SKILL.md frontmatters reference facade
      names; every action has a documented section (no-orphan at action
      level); `skills/INDEX.md` regenerated with the facade column.
- [ ] **Governance ratified**: companion ADR merged; RFC-0090 amended with
      a pointer to this RFC; Charter §5.13 + CLAUDE.md Hard Rule wording
      updated in the same PR as the ADR.
- [ ] **Docs**: README.md / docs/walkthrough.md tool counts corrected to
      "11 facade tools, 95 capabilities" (fixing the existing "93" staleness).

## Implementation plan (follow-up PRs, not this RFC)

| Phase | Content | Gate |
|---|---|---|
| 0 (this RFC) | Design + governance amendment text | Founder ratification |
| 1 | ADR + Charter/CLAUDE.md/RFC-0090 amendments; facade dispatch + `--tool-surface` (default `both`); twin tests; parity script v2; instructions rewrite | I6 green; size gate measured (informational) |
| 2 | Default → `facade`; Skills + SDK docstrings + README/walkthrough flips | size gate **required**; two-minor deprecation notice for `legacy` |
| 3 (next major) | Remove legacy registrations; retire I6 | action-level I4 green |

## Appendix A — Measurement reproduction

```bash
cargo build --release -p mycelium-rcig-cli
BIN=target/release/mycelium
{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"bench","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
  sleep 2
} | "$BIN" serve --mcp 2>/dev/null
# then: len(json.dumps(resp2["result"], separators=(",",":")).encode())
```

Measured 2026-06-10 on develop @ ce85709: 95 tools, 189,624 bytes,
avg 1,996 B/tool, max `mycelium_subscribe` 5,526 B,
`initialize.instructions` 3,590 B.

## Appendix B — The 95 registered tools (extraction)

Extracted via `awk '/#\[tool\(/{t=1} t && /async fn /{...}'` over
`crates/mycelium-mcp/src/lib.rs` (95 `#[tool(...)]` attributes, 95 fns):

`index_workspace, search_symbol, get_ancestors, get_descendants,
load_index, server_status, watch_status, subscribe, unsubscribe,
subscription_status, get_callees, get_callers, get_symbol_info,
batch_symbol_info, get_callee_tree, get_caller_tree, get_imports,
get_import_tree, get_node_kind, get_symbols_by_kind, get_source_span,
get_extends, get_implements, get_entry_points, get_dead_symbols,
get_isolated_symbols, project_health, safe_to_edit, get_stats,
get_cross_refs, get_outgoing_refs, get_all_symbols, get_reachable,
get_reachable_to, get_siblings, query, get_node_degree, detect_cycles,
get_scc_groups, get_dependency_layers, get_two_hop_neighbors,
get_symbol_neighborhood, get_hub_symbols, get_singly_referenced,
batch_reachable_to, batch_reachable_from, batch_node_degree,
topological_sort, find_articulation_points, find_bridge_edges,
get_biconnected_components, get_degree_histogram, get_graph_metrics,
get_neighbor_similarity, get_clustering_coefficient, get_eccentricity,
get_harmonic_centrality, get_mutual_reachability, get_reachable_set,
get_reaches_into, get_common_reachable, get_k_hop_neighbors,
get_betweenness_centrality, page_rank, get_wcc, find_cycle_members,
get_k_core, rank_symbols, get_top_files, get_most_connected,
get_leaf_symbols, get_shortest_path, get_symbol_count_by_kind,
get_common_callers, get_common_callees, get_fan_out_rank, get_fan_in_rank,
get_files, find_call_path, find_import_path, find_extends_path,
get_extends_tree, get_subclasses_tree, find_implements_path,
get_implements_tree, get_implementors_tree, get_importers_tree,
get_closeness_centrality, get_dependency_depth, get_degree_centrality,
get_strongly_connected_components, sync_file, set_compact_mode, context,
get_token_stats` (all prefixed `mycelium_`).

## 备注

The colloquial framing for the founder: RFC-0090 made CLI and MCP
co-twins; this RFC keeps every twin pair but moves the MCP twins from 95
separate houses into 11 shared houses with name-plated rooms. The Skill
umbrellas now shelter houses instead of individuals — same coverage, far
cheaper rent (47.4K → ≈9K tokens per request).
