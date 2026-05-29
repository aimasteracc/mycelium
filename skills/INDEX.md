# Skills Coverage Matrix

> Generated artifact. Do not edit by hand. CI regenerates this file on every PR touching `crates/mycelium-cli/`, `crates/mycelium-mcp/`, or `skills/`.
>
> Source of truth for the **coverage invariant** in [RFC-0090](../rfcs/0090-cli-mcp-skill-parity.md): every (CLI, MCP) pair must appear in at least one Skill's `allowed-tools`.

## Status

| Phase | State |
|---|---|
| Phase 0 (RFC-0090 PR) | Coverage matrix scaffolded. No real Skills yet. Existing 88 MCP capabilities are unmapped. |
| Phase 1 (v0.1.2) | Parity-check CI + generator script land. INDEX.md becomes mechanical. |
| Phase 2 (v0.1.3, **complete for the 8 planned categories**) | All 9 category Skills (hyphae-query + the 8 from glm5.1 eval) shipped. 73/88 capabilities covered. |
| Phase 2.3 (v0.1.4, **complete**) | All 16 remaining capabilities triaged: 10 into existing Skills, 6 into new `index-management` Skill. Index total rises from 89 (query was added in v0.1.3). |
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
| `skills/index-management/` | 7 | `index_workspace`, `load_index`, `server_status` (also basic-queries), `watch_status`, `sync_file`, `set_compact_mode`, `get_token_stats` |
| **Total (planned)** | **79** | Phase 2.3 adds the index-management Skill + 10 capabilities into existing Skills. |

## Coverage matrix (real)

> One row per capability. CI MUST fail any PR that adds a capability without filling a row here. CI MUST fail any row whose Skill folder is missing on disk.

| Capability | CLI | MCP | Covering Skill(s) | Status |
|---|---|---|---|---|
| `query` | `mycelium query <expr>` | `mycelium_query` | [`hyphae-query`](hyphae-query/SKILL.md) | ✅ Three-Surface, v0.1.3 (#151) |
| `search_symbol` | `mycelium search-symbol` | `mycelium_search_symbol` | [`basic-queries`](basic-queries/SKILL.md) | ✅ Three-Surface v0.1.4 (CLI batch 1) |
| `get_symbol_info` | `mycelium get-symbol-info` | `mycelium_get_symbol_info` | [`basic-queries`](basic-queries/SKILL.md) | ✅ Three-Surface v0.1.4 (CLI batch 1) |
| `get_ancestors` | `mycelium get-ancestors` | `mycelium_get_ancestors` | [`basic-queries`](basic-queries/SKILL.md) | ✅ Three-Surface v0.1.4 (CLI batch 1) |
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
| `get_imports` | `mycelium get-imports` | `mycelium_get_imports` | [`import-graph`](import-graph/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_import_tree` | `mycelium get-import-tree` | `mycelium_get_import_tree` | [`import-graph`](import-graph/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_importers_tree` | `mycelium get-importers-tree` | `mycelium_get_importers_tree` | [`import-graph`](import-graph/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_shortest_path` | `mycelium get-shortest-path` | `mycelium_get_shortest_path` | [`reachability`](reachability/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_reachable` | `mycelium get-reachable` | `mycelium_get_reachable` | [`reachability`](reachability/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_reachable_to` | `mycelium get-reachable-to` | `mycelium_get_reachable_to` | [`reachability`](reachability/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_k_hop_neighbors` | `mycelium get-k-hop-neighbors` | `mycelium_get_k_hop_neighbors` | [`reachability`](reachability/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_two_hop_neighbors` | `mycelium get-two-hop-neighbors` | `mycelium_get_two_hop_neighbors` | [`reachability`](reachability/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_symbol_neighborhood` | `mycelium get-symbol-neighborhood` | `mycelium_get_symbol_neighborhood` | [`reachability`](reachability/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_cross_refs` | `mycelium get-cross-refs` | `mycelium_get_cross_refs` | [`reachability`](reachability/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_outgoing_refs` | `mycelium get-outgoing-refs` | `mycelium_get_outgoing_refs` | [`reachability`](reachability/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_dependency_depth` | `mycelium get-dependency-depth` | `mycelium_get_dependency_depth` | [`reachability`](reachability/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_reachable_set` | `mycelium get-reachable-set` | `mycelium_get_reachable_set` | [`reachability`](reachability/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_reaches_into` | `mycelium get-reaches-into` | `mycelium_get_reaches_into` | [`reachability`](reachability/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_singly_referenced` | `mycelium get-singly-referenced` | `mycelium_get_singly_referenced` | [`reachability`](reachability/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `rank_symbols` | `mycelium rank-symbols` | `mycelium_rank_symbols` | [`centrality`](centrality/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_top_files` | `mycelium get-top-files` | `mycelium_get_top_files` | [`centrality`](centrality/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_most_connected` | `mycelium get-most-connected` | `mycelium_get_most_connected` | [`centrality`](centrality/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_hub_symbols` | `mycelium get-hub-symbols` | `mycelium_get_hub_symbols` | [`centrality`](centrality/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_fan_out_rank` | `mycelium get-fan-out-rank` | `mycelium_get_fan_out_rank` | [`centrality`](centrality/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_fan_in_rank` | `mycelium get-fan-in-rank` | `mycelium_get_fan_in_rank` | [`centrality`](centrality/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `betweenness_centrality` | `mycelium betweenness-centrality` | `mycelium_betweenness_centrality` | [`centrality`](centrality/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending; ⚠️ #153 perf |
| `closeness_centrality` | `mycelium closeness-centrality` | `mycelium_closeness_centrality` | [`centrality`](centrality/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `degree_centrality` | `mycelium degree-centrality` | `mycelium_degree_centrality` | [`centrality`](centrality/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `clustering_coefficient` | `mycelium clustering-coefficient` | `mycelium_clustering_coefficient` | [`centrality`](centrality/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `eccentricity` | `mycelium eccentricity` | `mycelium_eccentricity` | [`centrality`](centrality/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `page_rank` | `mycelium page-rank` | `mycelium_page_rank` | [`centrality`](centrality/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending; ⚠️ #153 perf |
| `harmonic_centrality` | `mycelium harmonic-centrality` | `mycelium_harmonic_centrality` | [`centrality`](centrality/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `neighbor_similarity` | `mycelium neighbor-similarity` | `mycelium_neighbor_similarity` | [`centrality`](centrality/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_extends` | `mycelium get-extends` | `mycelium_get_extends` | [`inheritance`](inheritance/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `extends_tree` | `mycelium extends-tree` | `mycelium_extends_tree` | [`inheritance`](inheritance/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `subclasses_tree` | `mycelium subclasses-tree` | `mycelium_subclasses_tree` | [`inheritance`](inheritance/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `find_extends_path` | `mycelium find-extends-path` | `mycelium_find_extends_path` | [`inheritance`](inheritance/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_implements` | `mycelium get-implements` | `mycelium_get_implements` | [`inheritance`](inheritance/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `implements_tree` | `mycelium implements-tree` | `mycelium_implements_tree` | [`inheritance`](inheritance/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `implementors_tree` | `mycelium implementors-tree` | `mycelium_implementors_tree` | [`inheritance`](inheritance/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `find_implements_path` | `mycelium find-implements-path` | `mycelium_find_implements_path` | [`inheritance`](inheritance/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_stats` | `mycelium get-stats` | `mycelium_get_stats` | [`graph-structure`](graph-structure/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_graph_metrics` | `mycelium get-graph-metrics` | `mycelium_get_graph_metrics` | [`graph-structure`](graph-structure/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending; ⚠️ #153 perf |
| `detect_cycles` | `mycelium detect-cycles` | `mycelium_detect_cycles` | [`graph-structure`](graph-structure/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_scc_groups` | `mycelium get-scc-groups` | `mycelium_get_scc_groups` | [`graph-structure`](graph-structure/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `topological_sort` | `mycelium topological-sort` | `mycelium_topological_sort` | [`graph-structure`](graph-structure/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `find_articulation_points` | `mycelium find-articulation-points` | `mycelium_find_articulation_points` | [`graph-structure`](graph-structure/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `find_bridge_edges` | `mycelium find-bridge-edges` | `mycelium_find_bridge_edges` | [`graph-structure`](graph-structure/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_biconnected_components` | `mycelium get-biconnected-components` | `mycelium_get_biconnected_components` | [`graph-structure`](graph-structure/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_k_core` | `mycelium get-k-core` | `mycelium_get_k_core` | [`graph-structure`](graph-structure/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_dependency_layers` | `mycelium get-dependency-layers` | `mycelium_get_dependency_layers` | [`graph-structure`](graph-structure/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_strongly_connected_components` | `mycelium get-strongly-connected-components` | `mycelium_get_strongly_connected_components` | [`graph-structure`](graph-structure/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `get_wcc` | `mycelium get-wcc` | `mycelium_get_wcc` | [`graph-structure`](graph-structure/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending; ⚠️ #153 perf |
| `get_degree_histogram` | `mycelium get-degree-histogram` | `mycelium_get_degree_histogram` | [`graph-structure`](graph-structure/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending; ⚠️ #153 perf |
| `find_cycle_members` | `mycelium find-cycle-members` | `mycelium_find_cycle_members` | [`graph-structure`](graph-structure/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `batch_symbol_info` | `mycelium batch-symbol-info` | `mycelium_batch_symbol_info` | [`batch-ops`](batch-ops/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `batch_node_degree` | `mycelium batch-node-degree` | `mycelium_batch_node_degree` | [`batch-ops`](batch-ops/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `batch_reachable_from` | `mycelium batch-reachable-from` | `mycelium_batch_reachable_from` | [`batch-ops`](batch-ops/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `batch_reachable_to` | `mycelium batch-reachable-to` | `mycelium_batch_reachable_to` | [`batch-ops`](batch-ops/SKILL.md) | 🟡 Skill landed v0.1.3; CLI subcommand pending |
| `index_workspace` | `mycelium index` | `mycelium_index_workspace` | [`index-management`](index-management/SKILL.md) | 🟡 Skill landed v0.1.4; CLI `index` subcommand is the equivalent |
| `load_index` | `mycelium load-index` | `mycelium_load_index` | [`index-management`](index-management/SKILL.md) | 🟡 Skill landed v0.1.4; CLI subcommand pending |
| `watch_status` | `mycelium watch-status` | `mycelium_watch_status` | [`index-management`](index-management/SKILL.md) | 🟡 Skill landed v0.1.4; CLI subcommand pending |
| `sync_file` | `mycelium sync-file` | `mycelium_sync_file` | [`index-management`](index-management/SKILL.md) | 🟡 Skill landed v0.1.4; CLI subcommand pending |
| `set_compact_mode` | `mycelium set-compact-mode` | `mycelium_set_compact_mode` | [`index-management`](index-management/SKILL.md) | 🟡 Skill landed v0.1.4; CLI subcommand pending |
| `get_token_stats` | `mycelium get-token-stats` | `mycelium_get_token_stats` | [`index-management`](index-management/SKILL.md) | 🟡 Skill landed v0.1.4; CLI subcommand pending |
| `get_node_degree` | `mycelium get-node-degree` | `mycelium_get_node_degree` | [`basic-queries`](basic-queries/SKILL.md) | 🟡 Skill landed v0.1.4; CLI subcommand pending |
| `get_files` | `mycelium get-files` | `mycelium_get_files` | [`basic-queries`](basic-queries/SKILL.md) | 🟡 Skill landed v0.1.4; CLI subcommand pending |
| `get_symbol_count_by_kind` | `mycelium get-symbol-count-by-kind` | `mycelium_get_symbol_count_by_kind` | [`basic-queries`](basic-queries/SKILL.md) | 🟡 Skill landed v0.1.4; CLI subcommand pending |
| `get_leaf_symbols` | `mycelium get-leaf-symbols` | `mycelium_get_leaf_symbols` | [`call-graph`](call-graph/SKILL.md) | 🟡 Skill landed v0.1.4; CLI subcommand pending; ⚠️ #153 perf |
| `find_call_path` | `mycelium find-call-path` | `mycelium_find_call_path` | [`call-graph`](call-graph/SKILL.md) | 🟡 Skill landed v0.1.4; CLI subcommand pending; ⚠️ #153 perf |
| `get_common_callers` | `mycelium get-common-callers` | `mycelium_get_common_callers` | [`call-graph`](call-graph/SKILL.md) | 🟡 Skill landed v0.1.4; CLI subcommand pending |
| `get_common_callees` | `mycelium get-common-callees` | `mycelium_get_common_callees` | [`call-graph`](call-graph/SKILL.md) | 🟡 Skill landed v0.1.4; CLI subcommand pending |
| `find_import_path` | `mycelium find-import-path` | `mycelium_find_import_path` | [`import-graph`](import-graph/SKILL.md) | 🟡 Skill landed v0.1.4; CLI subcommand pending |
| `get_mutual_reachability` | `mycelium get-mutual-reachability` | `mycelium_get_mutual_reachability` | [`reachability`](reachability/SKILL.md) | 🟡 Skill landed v0.1.4; CLI subcommand pending |
| `get_common_reachable` | `mycelium get-common-reachable` | `mycelium_get_common_reachable` | [`reachability`](reachability/SKILL.md) | 🟡 Skill landed v0.1.4; CLI subcommand pending |

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
