# Skills Coverage Matrix

> Generated artifact. Do not edit by hand. CI regenerates this file on every PR touching `crates/mycelium-cli/`, `crates/mycelium-mcp/`, or `skills/`.
>
> Source of truth for the **coverage invariant** in [RFC-0090](../rfcs/0090-cli-mcp-skill-parity.md): every (CLI, MCP) pair must appear in at least one Skill's `allowed-tools`.

## Status

| Phase | State |
|---|---|
| Phase 0 (RFC-0090 PR) | Coverage matrix scaffolded. No real Skills yet. Existing 88 MCP capabilities are unmapped. |
| Phase 1 (v0.1.2) | Parity-check CI + generator script land. INDEX.md becomes mechanical. |
| Phase 2 (v0.1.3, in progress) | **First real Skill landed: `hyphae-query`** (#151 PR). Authoring proceeds one category per PR. |
| Phase 3 (v0.2.0) | Coverage invariant CI-gated on `main`. Orphans block release. |

## Proposed Skill categories for v0.2

Seeded from an independent end-to-end evaluation of all 88 MCP tools
(see [decisions.jsonl 2026-05-30 #glm51-eval](../.hive/memory/decisions.jsonl)).
These are *planned* category Skills. The actual SKILL.md files are
authored in Phase 2 PRs, one PR per category.

| Skill (planned) | Capability count | Notes |
|---|---:|---|
| `skills/basic-queries/` | 10 | `search_symbol`, `get_symbol_info`, `get_ancestors`, `get_descendants`, `get_node_kind`, `get_symbols_by_kind`, `get_source_span`, `get_siblings`, `get_all_symbols`, `server_status` |
| `skills/call-graph/` | 7 | `get_callees`, `get_callers`, `get_callee_tree`, `get_caller_tree`, `get_entry_points`, `get_dead_symbols`, `get_isolated_symbols` |
| `skills/import-graph/` | 3 | `get_imports`, `get_import_tree`, `get_importers_tree` |
| `skills/inheritance/` | 8 | `get_extends`, `extends_tree`, `subclasses_tree`, `find_extends_path`, `get_implements`, `implements_tree`, `implementors_tree`, `find_implements_path` |
| `skills/graph-structure/` | 14 | `get_stats`, `get_graph_metrics`, `detect_cycles`, `get_scc_groups`, `topological_sort`, `find_articulation_points`, `find_bridge_edges`, `get_biconnected_components`, `get_k_core`, `get_dependency_layers`, `get_scc`, `get_wcc`, `get_degree_histogram`, `find_cycle_members` |
| `skills/centrality/` | 14 | `rank_symbols`, `get_top_files`, `get_most_connected`, `get_hub_symbols`, `get_fan_out_rank`, `get_fan_in_rank`, `betweenness_centrality`, `closeness_centrality`, `degree_centrality`, `clustering_coefficient`, `eccentricity`, `page_rank`, `harmonic_centrality`, `neighbor_similarity` |
| `skills/reachability/` | 12 | `get_reachable`, `get_reachable_to`, `get_k_hop_neighbors`, `get_two_hop_neighbors`, `get_shortest_path`, `get_symbol_neighborhood`, `get_cross_refs`, `get_outgoing_refs`, `get_dependency_depth`, `get_reachable_set`, `get_reaches_into`, `get_singly_referenced` |
| `skills/batch-ops/` | 4 | `batch_symbol_info`, `batch_node_degree`, `batch_reachable_from`, `batch_reachable_to` |
| **Total** | **72** | Remaining ~16 to be inventoried during Phase 2 categorization. |

## Coverage matrix (real)

> One row per capability. CI MUST fail any PR that adds a capability without filling a row here. CI MUST fail any row whose Skill folder is missing on disk.

| Capability | CLI | MCP | Covering Skill(s) | Status |
|---|---|---|---|---|
| `query` | `mycelium query <expr>` | `mycelium_query` | [`hyphae-query`](hyphae-query/SKILL.md) | ✅ Three-Surface, v0.1.3 (#151) |
| `search_symbol` | `mycelium search-symbol` | `mycelium_search_symbol` | [`basic-queries`](basic-queries/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_symbol_info` | `mycelium get-symbol-info` | `mycelium_get_symbol_info` | [`basic-queries`](basic-queries/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_ancestors` | `mycelium get-ancestors` | `mycelium_get_ancestors` | [`basic-queries`](basic-queries/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_descendants` | `mycelium get-descendants` | `mycelium_get_descendants` | [`basic-queries`](basic-queries/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_node_kind` | `mycelium get-node-kind` | `mycelium_get_node_kind` | [`basic-queries`](basic-queries/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_symbols_by_kind` | `mycelium get-symbols-by-kind` | `mycelium_get_symbols_by_kind` | [`basic-queries`](basic-queries/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_source_span` | `mycelium get-source-span` | `mycelium_get_source_span` | [`basic-queries`](basic-queries/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_siblings` | `mycelium get-siblings` | `mycelium_get_siblings` | [`basic-queries`](basic-queries/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_all_symbols` | `mycelium get-all-symbols` | `mycelium_get_all_symbols` | [`basic-queries`](basic-queries/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `server_status` | `mycelium server-status` | `mycelium_server_status` | [`basic-queries`](basic-queries/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_callees` | `mycelium get-callees` | `mycelium_get_callees` | [`call-graph`](call-graph/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_callers` | `mycelium get-callers` | `mycelium_get_callers` | [`call-graph`](call-graph/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_callee_tree` | `mycelium get-callee-tree` | `mycelium_get_callee_tree` | [`call-graph`](call-graph/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_caller_tree` | `mycelium get-caller-tree` | `mycelium_get_caller_tree` | [`call-graph`](call-graph/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_entry_points` | `mycelium get-entry-points` | `mycelium_get_entry_points` | [`call-graph`](call-graph/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_dead_symbols` | `mycelium get-dead-symbols` | `mycelium_get_dead_symbols` | [`call-graph`](call-graph/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_isolated_symbols` | `mycelium get-isolated-symbols` | `mycelium_get_isolated_symbols` | [`call-graph`](call-graph/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |

**Status legend:** 🟡 Skill bundle written + MCP tool exists, but the CLI half is still missing — `parity-backfill` epic tracks these. ✅ Three-Surface = all three surfaces shipped, parity-CI green. The CLI subcommands ship in v0.1.4–v0.1.5 alongside the parity-CI workflow.

## Orphan detection

CI fails if either:

- A `(CLI, MCP)` pair exists in `crates/` with no row in the matrix above.
- A row in the matrix above references a Skill that does not exist on disk.
- A `SKILL.md` lists a tool in `allowed-tools` that does not correspond to a real `(CLI, MCP)` pair.

## Reference

- [Charter §5.13](../CHARTER.md#513--the-three-surface-rule-cli--mcp-parity--skill-coverage)
- [RFC-0090](../rfcs/0090-cli-mcp-skill-parity.md)
- [ADR-0007](../docs/adr/0007-cli-mcp-skill-parity.md)
