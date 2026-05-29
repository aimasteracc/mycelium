# Changelog

All notable changes to **Mycelium** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Day-0 project skeleton: charter, governance, GitFlow, code of conduct, security policy.
- `.hive/` definition of the autonomous AI development team.
- `.hive/memory/` persistent shared memory (append-only JSONL).
- RFC-0000 RFC template and RFC-0001 draft (Trunk + Synapse storage layer).
- GitHub workflows skeleton: `ci.yml`, `release.yml`, `nightly.yml`, `hive.yml`, `triage.yml`.
- Issue and PR templates.
- macOS `launchd` plists for autonomous Hive scheduling.
- Cargo workspace stub with `mycelium-core`, `mycelium-hyphae`, `mycelium-pack`, `mycelium-cli`, `mycelium-mcp` crates.
- First language packs: Python and TypeScript skeletons under `packs/`.
- `mycelium-core`: RFC-0002 `Extractor` — tree-sitter → Store bridge; parses Python source files and populates `Trunk` nodes + `Contains` edges for modules, functions, classes, methods, and imports.
- `mycelium-pack`: language pack loader (`LanguagePack::load`) with `pack.toml` manifest parsing and query-source validation.
- `mycelium index <path>`: first end-user-visible CLI command — walks a directory tree, extracts Python symbols via RFC-0002 `Extractor`, and reports file/error counts.
- TypeScript language pack (`packs/typescript/`) — `function_declaration`, `class_declaration`, methods, `interface_declaration`, `type_alias_declaration`, and import references.
- Extractor generic `definition.*` dispatch: any capture name starting with `definition.` (other than `module`/`method`) creates a top-level child node, enabling language-pack authors to use custom definition kinds.
- Rust language pack (`packs/rust/`) — functions, structs, enums, traits, type aliases, consts, inline mods, impl methods, and use declarations.
- `mycelium index` now indexes Python, TypeScript, and Rust source trees.
- RFC-0004 MCP server (`mycelium-mcp`): `mycelium serve --mcp` starts a stdio JSON-RPC 2.0 server with three tools — `mycelium_index_workspace`, `mycelium_search_symbol`, `mycelium_get_ancestors`.
- `Store::search_symbol` — case-insensitive substring search over all materialized path name-segments; returns sorted results up to a configurable limit.
- `Store::ancestors_of_path` — returns ancestor path strings (child-to-root) for a given trunk path string.
- RFC-0005: JavaScript language pack (`packs/javascript/`) — top-level functions, arrow functions, class declarations, methods, and import references for `.js` and `.jsx` files.
- RFC-0005: `.jsx` and `.tsx` extension dispatch in CLI and MCP indexing layers.
- RFC-0005: `mycelium_get_descendants` MCP tool — returns all symbols nested under a trunk path.
- RFC-0005: `mycelium_index_workspace` now includes a `"languages"` field listing all indexed language names.
- RFC-0005: `Store::descendants_of_path` — symmetric counterpart to `ancestors_of_path`; returns descendant path strings in unspecified order.
- RFC-0005: MCP server identity corrected — `get_info()` now reports `{"name":"mycelium-mcp","version":"0.0.1"}` instead of the rmcp library name.
- RFC-0006: `Store::save()` — serializes the full Trunk+Synapse graph to a `MessagePack` snapshot; creates parent directories automatically.
- RFC-0006: `Store::load()` — deserializes a `Store` from a `.mycelium/index.rmp` snapshot file.
- RFC-0006: `mycelium index` CLI auto-saves snapshot to `.mycelium/index.rmp` after indexing.
- RFC-0006: `mycelium_index_workspace` MCP tool auto-saves snapshot after indexing.
- RFC-0006: `mycelium_load_index` MCP tool — reloads a previously-saved index from `.mycelium/index.rmp` without re-parsing source files.
- RFC-0006: All core types (`NodeId`, `NodeKind`, `EdgeKind`, `Language`, `Trunk`, `Synapse`, `Store`) now implement `serde::Serialize` + `Deserialize`.
- RFC-0007: `MyceliumServer::with_root(path)` — new constructor that pre-loads a `.mycelium/index.rmp` snapshot, or falls back to a live index + auto-save.
- RFC-0007: `serve_stdio(root: Option<PathBuf>)` — passes `--root` through to `with_root`.
- RFC-0007: `mycelium serve --mcp --root <path>` CLI flag — server starts ready without needing `mycelium_index_workspace`.
- RFC-0007: `mycelium_server_status` MCP tool — returns `node_count`, `indexed_root`, and `is_loaded` for client diagnostics.
- RFC-0008: File-system watch mode — `MyceliumServer::start_watch(root)` spawns a background loop that debounces FSE events (300 ms window) and incrementally re-indexes changed/created/deleted files.
- RFC-0008: `with_root` now automatically starts the watch loop after loading.
- RFC-0008: `mycelium_watch_status` MCP tool — returns `watching`, `root`, and `batches_processed`.
- RFC-0008: `reindex_file` helper — single-file extraction used by the watch loop.
- RFC-0009: Gitignore-aware file walking — CLI `index_path` and MCP `run_index` now use `ignore::WalkBuilder` to respect `.gitignore` and `.myceliumignore` patterns.
- RFC-0009: `target/` and `.mycelium/` are always excluded from indexing, even without an ignore file.
- RFC-0009: Background FSE watch loop filters events for ignored paths before re-indexing.
- RFC-0009: `.myceliumignore` is registered as a custom ignore filename in `WalkBuilder`.
- RFC-0010: `Synapse::edge_count()` — total directed edges across all `EdgeKind` buckets.
- RFC-0010: `Store::edge_count()` — delegates to `Synapse::edge_count()`.
- RFC-0010: `mycelium_server_status` now includes `"edge_count"` alongside `"node_count"`.
- RFC-0011: Call graph edges — `reference.call` patterns added to Python, TypeScript, JavaScript, and Rust language packs.
- RFC-0011: `Extractor` now populates `EdgeKind::Calls` edges between caller and callee nodes.
- RFC-0011: Intra-file call resolution: callees defined before callers in the same file are resolved to their definition nodes rather than bare stubs.
- RFC-0012: `mycelium_get_callees` MCP tool — returns all symbols a given path calls, as a sorted list.
- RFC-0012: `mycelium_get_callers` MCP tool — returns all symbols that call a given path, as a sorted list.
- RFC-0013: Two-pass extraction — `Extractor::extract` now makes two sequential AST traversals (definitions first, references second) so forward-reference call edges always resolve to definition nodes rather than bare stubs.
- RFC-0014: Cross-file call stub resolution — `Store::resolve_bare_call_stubs()` runs after each full workspace index, rewiring `Calls` edges that point to bare stub nodes to their actual definition nodes (unambiguous matches only).
- RFC-0014: `AdjacencyList::redirect_node` and `Synapse::redirect_node` — edge-rewiring primitives used by stub resolution.
- RFC-0014: `mycelium_index_workspace` response now includes `"stubs_resolved"` count.
- RFC-0015: Watch-mode stub resolution — `resolve_bare_call_stubs()` is called at the end of each FSE debounce batch, so cross-file call edges are kept accurate during incremental re-indexing without requiring a full re-index.
- RFC-0016: `mycelium_get_symbol_info` MCP tool — returns ancestors, descendants, callers, and callees for any symbol path in a single call; all lists are sorted lexicographically.
- RFC-0017: `Store::find_call_path(from, to, max_depth)` — BFS shortest call path search; returns `Some(Vec<NodeId>)` including both endpoints, or `None` if unreachable; cycle-safe via visited set; `max_depth` limits hops.
- RFC-0017: `mycelium_find_call_path` MCP tool — BFS call chain tool; request `{ from_path, to_path, max_depth? }`; returns `{ path, hops }` on success or `{ path: [], hops: null, message }` when unreachable; unknown paths return `{ error }`.
- RFC-0018: `Store::all_file_paths()` — returns all trunk paths with no `>` separator (file-level nodes), sorted lexicographically.
- RFC-0018: `mycelium_get_files` MCP tool — enumerates all indexed source files; optional `path_prefix` parameter filters results; returns `{ files: [...] }` sorted.
- RFC-0019: `Store::top_callee_symbols(limit)` — returns top-N `(path, caller_count)` pairs sorted by caller count descending (ties by path ascending); symbols with 0 callers excluded.
- RFC-0019: `mycelium_rank_symbols` MCP tool — hot-spot analysis; request `{ limit? }`; returns `{ symbols: [{ path, caller_count }, ...] }`; limit defaults to 10, capped at 100.
- RFC-0020: `CalleeNode { id, children }` struct — DFS callee tree node; cycle-safe via per-traversal visited set with backtrack removal.
- RFC-0020: `Store::callee_tree(id, max_depth)` — depth-limited recursive DFS over Calls edges.
- RFC-0020: `mycelium_get_callee_tree` MCP tool — returns `{ root: { path, children: [...] } }`; max_depth defaults to 4, capped at 10; unknown path returns `{ error }`.
- RFC-0021: `CallerNode { id, callers }` struct — symmetric complement to `CalleeNode`; DFS up incoming Calls edges; cycle-safe via path-tracking visited set.
- RFC-0021: `Store::caller_tree(id, max_depth)` — depth-limited recursive DFS over incoming Calls edges.
- RFC-0021: `mycelium_get_caller_tree` MCP tool — returns `{ root: { path, callers: [...] } }`; max_depth defaults to 4, capped at 10; unknown path returns `{ error }`.
- RFC-0022: `Store::entry_points(prefix)` — returns all symbol paths (containing `>`) with zero incoming Calls edges, sorted lexicographically; optional prefix filter.
- RFC-0022: `mycelium_get_entry_points` MCP tool — returns `{ entry_points: [...] }`; optional `path_prefix` filter; excludes file-level nodes.
- RFC-0023: `Store::imports_of(id)` / `Store::imported_by(id)` — outgoing/incoming `Imports` edge resolvers; results sorted lexicographically.
- RFC-0023: `mycelium_get_imports` MCP tool — returns `{ imports: [...], imported_by: [...] }` for a path; unknown path returns `{ error }`.
- RFC-0024: `ImportNode { id, imports }` struct — DFS import dependency tree node; cycle-safe via path-tracking visited set.
- RFC-0024: `Store::import_tree(id, max_depth)` — depth-limited recursive DFS over outgoing `Imports` edges.
- RFC-0024: `mycelium_get_import_tree` MCP tool — returns `{ root: { path, imports: [...] } }`; max_depth defaults to 4, capped at 10; unknown path returns `{ error }`.
- RFC-0025: `mycelium_batch_symbol_info` MCP tool — batch variant of `mycelium_get_symbol_info`; accepts up to 50 paths in one call; returns `{ symbols: [{ path, ancestors, descendants, callers, callees }] }` in input order; unknown paths return `{ path, error }` without failing the whole request.
- RFC-0026: `mycelium_get_extends` MCP tool — returns `{ extends, extended_by }` for a path using `EdgeKind::Extends`; both lists sorted lexicographically; unknown path returns `{ error }`.
- RFC-0026: `mycelium_get_implements` MCP tool — returns `{ implements, implemented_by }` for a path using `EdgeKind::Implements`; both lists sorted lexicographically; unknown path returns `{ error }`.
- RFC-0027: `Store::find_import_path(from, to, max_depth)` — BFS shortest import-dependency path; returns `Some(Vec<NodeId>)` including both endpoints or `None` if unreachable; cycle-safe; `max_depth` limits hops.
- RFC-0027: `mycelium_find_import_path` MCP tool — BFS import chain tool; request `{ from_path, to_path, max_depth? }`; returns `{ path, hops }` on success or `{ path: [], hops: null, message }` when unreachable; unknown paths return `{ error }`.
- RFC-0028: `Store::kind_map` — per-node `NodeKind` metadata stored alongside each node; zero query-time cost.
- RFC-0028: `Store::set_kind(id, kind)`, `Store::kind_of(id) -> Option<NodeKind>`, `Store::symbols_of_kind(kind, prefix) -> Vec<String>` — kind storage and query methods.
- RFC-0028: `Extractor` now calls `set_kind` for every extracted node (file → `File`, functions → `Function`, classes → `Class`, methods → `Method`, etc.).
- RFC-0028: `mycelium_get_node_kind` MCP tool — returns `{ path, kind }` where kind is the wire string or `null` if unrecorded; unknown path returns `{ error }`.
- RFC-0028: `mycelium_get_symbols_by_kind` MCP tool — returns `{ symbols: [...] }` for all indexed symbols of a given kind; optional `path_prefix` filter; unknown kind returns `{ error }`.
- RFC-0029: `SourceSpan` now derives `Serialize` + `Deserialize` so it persists in the MessagePack snapshot.
- RFC-0029: `Store::set_span(id, span)`, `Store::span_of(id) -> Option<SourceSpan>` — source location storage and retrieval.
- RFC-0029: `Extractor` now calls `set_span` for every extracted node using tree-sitter node positions (rows converted to 1-indexed lines).
- RFC-0029: `mycelium_get_source_span` MCP tool — returns `{ path, start_line, start_col, end_line, end_col, start_byte, end_byte }` on hit, `{ path, span: null }` when unrecorded, or `{ error }` when path is not found.
- RFC-0030: `Store::find_extends_path(from, to, max_depth)` — BFS shortest extends-chain search over `EdgeKind::Extends`; completes the `find_*_path` triad.
- RFC-0030: `mycelium_find_extends_path` MCP tool — returns `{ path, hops }` on success, `{ path: [], hops: null, message }` when unreachable, or `{ error }` for unknown paths; `max_depth` defaults to 8, capped at 20.
- RFC-0031: `ExtendsNode { id, parents }` struct — DFS superclass tree node; cycle-safe via path-tracking visited set with backtrack removal.
- RFC-0031: `Store::extends_tree(id, max_depth)` — depth-limited recursive DFS over outgoing `Extends` edges.
- RFC-0031: `mycelium_get_extends_tree` MCP tool — returns `{ root: { path, parents: [...] } }`; `max_depth` defaults to 4, capped at 10; unknown path returns `{ error }`.
- RFC-0032: `SubclassNode { id, subclasses }` struct — DFS subclass forest node; cycle-safe via path-tracking visited set with backtrack removal.
- RFC-0032: `Store::subclasses_tree(id, max_depth)` — depth-limited recursive DFS over **incoming** `Extends` edges.
- RFC-0032: `mycelium_get_subclasses_tree` MCP tool — returns `{ root: { path, subclasses: [...] } }`; `max_depth` defaults to 4, capped at 10; unknown path returns `{ error }`. Complements `extends_tree` (outgoing) for full class-hierarchy exploration.
- RFC-0033: `Store::find_implements_path(from, to, max_depth)` — BFS shortest implements-chain search over `EdgeKind::Implements`; completes the `find_*_path` family (calls / imports / extends / implements).
- RFC-0033: `mycelium_find_implements_path` MCP tool — returns `{ path, hops }` on success, `{ path: [], hops: null, message }` when unreachable, or `{ error }` for unknown paths; `max_depth` defaults to 8, capped at 20.
- RFC-0034: `ImplementsNode { id, interfaces }` struct — DFS interface hierarchy node; cycle-safe via path-tracking visited set with backtrack removal.
- RFC-0034: `Store::implements_tree(id, max_depth)` — depth-limited recursive DFS over outgoing `Implements` edges.
- RFC-0034: `mycelium_get_implements_tree` MCP tool — returns `{ root: { path, interfaces: [...] } }`; `max_depth` defaults to 4, capped at 10; unknown path returns `{ error }`.
- RFC-0035: `ImplementorNode { id, implementors }` struct — DFS implementor forest node; cycle-safe via path-tracking visited set with backtrack removal.
- RFC-0035: `Store::implementors_tree(id, max_depth)` — depth-limited recursive DFS over **incoming** `Implements` edges.
- RFC-0035: `mycelium_get_implementors_tree` MCP tool — returns `{ root: { path, implementors: [...] } }`; `max_depth` defaults to 4, capped at 10; unknown path returns `{ error }`. Completes the Implements family.
- RFC-0036: `ImporterNode { id, importers }` struct — DFS reverse-dependency tree node; cycle-safe via path-tracking visited set with backtrack removal.
- RFC-0036: `Store::importers_tree(id, max_depth)` — depth-limited recursive DFS over **incoming** `Imports` edges.
- RFC-0036: `mycelium_get_importers_tree` MCP tool — returns `{ root: { path, importers: [...] } }`; `max_depth` defaults to 4, capped at 10; unknown path returns `{ error }`. Completes the Imports family and the full symmetric DFS coverage for all four `EdgeKind` variants.
- RFC-0037: `Store::dead_symbols(prefix)` — returns all symbol paths (containing `>`) with zero incoming `Calls` edges and zero incoming `Imports` edges; file-level nodes excluded; optional prefix filter; results sorted lexicographically.
- RFC-0037: `mycelium_get_dead_symbols` MCP tool — dead-code analysis tool; returns `{ dead_symbols: [...], count: N }`; optional `path_prefix` filter; dead symbols are candidates for deletion or documentation review.
- RFC-0038: `GraphStats { total_nodes, total_edges, nodes_by_kind, edges_by_kind }` struct — per-kind breakdown of the indexed graph.
- RFC-0038: `Synapse::edge_counts_by_kind()` — iterator over non-empty `(EdgeKind, usize)` pairs.
- RFC-0038: `Store::graph_stats()` — returns `GraphStats` with node counts grouped by `NodeKind` and edge counts grouped by `EdgeKind`; kinds with zero count are omitted.
- RFC-0038: `mycelium_get_stats` MCP tool — comprehensive per-kind statistics; extends `mycelium_server_status` with the breakdown needed for architectural analysis; returns `{ total_nodes, total_edges, nodes_by_kind, edges_by_kind }`.
- RFC-0039: `CrossRefs { callers, importers, extended_by, implemented_by }` struct — all incoming edges for a symbol grouped by `EdgeKind`.
- RFC-0039: `Store::cross_refs(id)` — collects incoming `Calls`, `Imports`, `Extends`, and `Implements` edges and resolves them to sorted path strings; all four lists always present.
- RFC-0039: `mycelium_get_cross_refs` MCP tool — unified "who references this?" primitive for impact analysis; returns `{ callers, importers, extended_by, implemented_by }` or `{ error }` for unknown paths.
- RFC-0040: `Store::nodes_in_cycles(edge_kind, prefix)` — iterative DFS with `in_stack` tracking; returns all paths participating in at least one cycle for the given `EdgeKind`; optional prefix filter; results sorted lexicographically.
- RFC-0040: `mycelium_detect_cycles` MCP tool — circular dependency detection; `edge_kind` must be `"calls"`, `"imports"`, `"extends"`, or `"implements"`; returns `{ cycle_nodes, count }` or `{ error }` for unknown edge kind.
- RFC-0041: `OutgoingRefs { callees, imports, extends, implements }` struct — all outgoing edges from a symbol grouped by `EdgeKind`; symmetric complement to `CrossRefs`.
- RFC-0041: `Store::outgoing_refs(id)` — collects outgoing `Calls`, `Imports`, `Extends`, `Implements` edges and resolves them to sorted path strings; all four lists always present.
- RFC-0041: `mycelium_get_outgoing_refs` MCP tool — "what does this reference?" primitive; paired with `mycelium_get_cross_refs` provides complete incoming/outgoing reference picture in two calls; returns `{ callees, imports, extends, implements }` or `{ error }`.
- RFC-0042: `Store::all_symbols(prefix, kind)` — returns all non-file symbol paths (paths containing `>`), sorted lexicographically, with optional path-prefix and `NodeKind` filters; file-level nodes are excluded.
- RFC-0042: `mycelium_get_all_symbols` MCP tool — enumerates every indexed symbol across all kinds; accepts optional `path_prefix` and `kind` parameters; returns `{ symbols, count }` or `{ error }` for an unknown kind string.
- RFC-0043: `Store::reachable_from(id, kind, max_depth)` — flat BFS reachability from a node via outgoing edges of any `EdgeKind`, depth-limited (cap 20), cycle-safe; starting node excluded; results sorted lexicographically.
- RFC-0043: `mycelium_get_reachable` MCP tool — transitive dependency enumeration in a single call; accepts `path`, `edge_kind`, and optional `max_depth`; returns `{ reachable, count }` or `{ error }` for unknown path or edge kind.
- RFC-0044: `Store::reachable_to(id, kind, max_depth)` — flat BFS backward reachability following incoming `EdgeKind` edges; depth-limited (cap 20), cycle-safe, starting node excluded; symmetric complement to `reachable_from`.
- RFC-0044: `mycelium_get_reachable_to` MCP tool — impact analysis primitive answering "who transitively depends on this symbol?"; paired with `mycelium_get_reachable` provides complete forward+backward reachability.
- RFC-0045: `Store::siblings(id)` — returns all direct siblings (other children of the same parent container in the containment tree), excluding the node itself; root nodes return empty `Vec`; results sorted lexicographically.
- RFC-0045: `mycelium_get_siblings` MCP tool — "what else is in this class/file?" query in a single call; returns `{ siblings, count }` or `{ error }` for unknown paths.
- RFC-0046: `NodeDegree` struct — per-node edge count summary: in/out degree for each of the four `EdgeKind`s (calls, imports, extends, implements).
- RFC-0046: `Store::node_degree(id)` — O(1) per-kind edge count summary without pulling full edge lists; useful for fast coupling analysis and hub-node detection.
- RFC-0046: `mycelium_get_node_degree` MCP tool — connectivity fingerprint for any path; returns `{ in_calls, out_calls, in_imports, out_imports, in_extends, out_extends, in_implements, out_implements }` or `{ error }`.
- RFC-0047: `Store::top_files(limit)` — returns top-N source files ranked by direct child symbol count (descending), ties broken alphabetically; files with no direct symbols excluded; limit capped at 100.
- RFC-0047: `mycelium_get_top_files` MCP tool — god-file detector identifying the most symbol-dense source files; returns `{ files: [{ path, symbol_count }], count }`.
- RFC-0048: `Store::most_connected(limit, kind)` — top-N symbol nodes ranked by total degree (in + out) for any EdgeKind; zero-degree nodes excluded; sorted descending by degree, ties broken alphabetically; limit capped at 100.
- RFC-0048: `mycelium_get_most_connected` MCP tool — hub-node detector for any edge kind; returns `{ symbols: [{ path, degree }], count }` or `{ error }` for unknown edge kind.
- RFC-0049: `Store::leaf_symbols(kind, limit)` — symbol nodes with out-degree 0 for any EdgeKind; symmetric complement to `entry_points` (RFC-0022, in-degree 0 for Calls); sorted alphabetically; limit capped at 100.
- RFC-0049: `mycelium_get_leaf_symbols` MCP tool — leaf-implementation detector for any edge kind; returns `{ symbols, count }` or `{ error }` for unknown edge kind.
- RFC-0050: `Store::shortest_path(from, to, kind)` — BFS minimum-hop path between two symbol nodes via outgoing EdgeKind edges; returns `Some(path_strings)` with both endpoints, or `None` if unreachable; cycle-safe.
- RFC-0050: `mycelium_get_shortest_path` MCP tool — "how does A reach B?" in a single call; returns `{ path, length }` if found, `{ path: null, length: null }` if no path, or `{ error }` for unknown edge kind or unrecognised node paths.
- RFC-0051: `Store::symbol_count_by_kind()` — per-`NodeKind` symbol histogram from `kind_map`; wire-string keys sorted alphabetically; zero-count kinds excluded.
- RFC-0051: `Store::upsert_node_with_kind(path, kind)` — convenience method: insert or retrieve a node and record its `NodeKind` in a single call.
- RFC-0051: `mycelium_get_symbol_count_by_kind` MCP tool — codebase composition histogram; returns `{ kinds: [{ kind, count }], total }`.
- RFC-0052: `Store::common_callers(target_ids, kind)` — set intersection of each target's incoming-neighbour set for any EdgeKind; answers "which symbols depend on ALL of these targets?"; results sorted alphabetically.
- RFC-0052: `mycelium_get_common_callers` MCP tool — shared-dependency detector; accepts `{ paths, edge_kind }` and returns `{ callers, count }` or `{ error }`.
- RFC-0053: `Store::fan_out_rank(kind, limit)` — top-N symbol nodes ranked by out-degree for any EdgeKind; "orchestrator detector" identifying symbols that call/import/extend many others; zero-degree nodes excluded; sorted descending by degree, ties broken alphabetically; limit capped at 100.
- RFC-0053: `mycelium_get_fan_out_rank` MCP tool — identifies orchestrating symbols; returns `{ symbols: [{ path, out_degree }], count }` or `{ error }` for unknown edge kind; limit defaults to 10.
- RFC-0054: `Store::fan_in_rank(kind, limit)` — top-N symbol nodes ranked by in-degree for any EdgeKind; "hotspot detector" identifying symbols depended upon by many others; zero-degree nodes excluded; sorted descending by degree, ties broken alphabetically; limit capped at 100. Symmetric complement to `fan_out_rank`.
- RFC-0054: `mycelium_get_fan_in_rank` MCP tool — identifies high-demand hotspot symbols; returns `{ symbols: [{ path, in_degree }], count }` or `{ error }` for unknown edge kind; limit defaults to 10.
- RFC-0055: `Store::common_callees(source_ids, kind)` — set intersection of each source's outgoing-neighbour set for any EdgeKind; answers "which symbols are called/imported by ALL of these sources?"; results sorted alphabetically. Symmetric complement to `common_callers` (RFC-0052).
- RFC-0055: `mycelium_get_common_callees` MCP tool — shared-dependency detector (outgoing direction); accepts `{ paths, edge_kind }` and returns `{ callees, count }` or `{ error }`.
- RFC-0056: `Store::isolated_symbols(prefix)` — symbol nodes with zero connectivity across all four EdgeKinds (Calls, Imports, Extends, Implements); stronger than `dead_symbols` (RFC-0037) which only checks incoming edges; optional path prefix filter; results sorted alphabetically.
- RFC-0056: `mycelium_get_isolated_symbols` MCP tool — completely-disconnected symbol detector; returns `{ isolated_symbols, count }`; optional `path_prefix` filter.
- RFC-0057: `Store::scc_groups(kind)` — Tarjan's iterative Strongly Connected Components algorithm over symbol nodes for a given EdgeKind; returns groups of size ≥ 2 (singletons excluded), sorted by size descending then by first path ascending; reveals mutually-recursive dependency clusters.
- RFC-0057: `mycelium_get_scc_groups` MCP tool — mutually-recursive symbol cluster detector; accepts `{ edge_kind }` and returns `{ groups, group_count, total_symbols }` or `{ error }` for unknown edge kind.
- RFC-0058: `Store::dependency_layers(kind)` — Kahn's BFS topological dependency layering; layer 0 = utility/leaf symbols (zero outgoing edges for `kind`), layer k+1 = symbols all of whose direct dependencies are in layers 0..=k; symbols in cycles excluded; paths within each layer sorted ascending.
- RFC-0058: `mycelium_get_dependency_layers` MCP tool — architectural layering inspector; accepts `{ edge_kind }` and returns `{ layers, layer_count, total_symbols, cycle_excluded_count }` or `{ error }` for unknown edge kind. Complements `scc_groups` (cycles) and `entry_points` (zero in-degree).
- RFC-0059: `Store::two_hop_neighbors(id, kind)` — symbol paths reachable in exactly 2 outgoing steps for `kind`; excludes source and direct (1-hop) neighbours; focused bridge detector without full reachability traversal; results sorted ascending.
- RFC-0059: `mycelium_get_two_hop_neighbors` MCP tool — indirect dependency bridge detector; accepts `{ path, edge_kind }` and returns `{ neighbors, count }`, `{ neighbors: [], count: 0 }` for unknown path, or `{ error }` for unknown edge kind.
- RFC-0060: `Store::symbol_neighborhood(id, kind)` + `SymbolNeighborhood` struct — ego-graph of a symbol for a single EdgeKind; returns path + direct incoming + direct outgoing, both lists sorted ascending; returns empty neighborhood for unknown id.
- RFC-0060: `mycelium_get_symbol_neighborhood` MCP tool — bidirectional single-kind ego-graph query; accepts `{ path, edge_kind }` and returns `{ path, incoming, outgoing, incoming_count, outgoing_count }`, empty neighborhood for unknown path, or `{ error }` for unknown edge kind.
- RFC-0061: `Store::hub_symbols(kind, min_in, min_out, limit)` — symbols with both in-degree ≥ `min_in` AND out-degree ≥ `min_out` for a given EdgeKind; returns `(path, in_degree, out_degree)` sorted by `in_degree + out_degree` descending (ties by path ascending); limit capped at 100; file nodes excluded.
- RFC-0061: `mycelium_get_hub_symbols` MCP tool — architectural hub detector identifying symbols that are both widely-used (high in-degree) and orchestrating (high out-degree); accepts `{ edge_kind, min_in?, min_out?, limit? }` and returns `{ hubs: [{ path, in_degree, out_degree }], count }` or `{ error }` for unknown edge kind; `min_in`/`min_out` default to 1.
- RFC-0062: `Store::singly_referenced(kind, limit)` — symbols with exactly one incoming edge for a given EdgeKind; returns `(symbol_path, referencing_path)` pairs sorted by symbol path ascending; limit capped at 100; file nodes excluded. Fills the in-degree=1 gap between `entry_points` (0) and `fan_in_rank` (top-N).
- RFC-0062: `mycelium_get_singly_referenced` MCP tool — inlining and privatisation candidate detector; accepts `{ edge_kind, limit? }` and returns `{ symbols: [{ path, referenced_by }], count }` or `{ error }` for unknown edge kind; limit defaults to 10.
- RFC-0063: `Store::batch_reachable_to(ids, kind, max_depth)` — union of transitive incoming dependents for a set of symbols; deduplicated, input nodes excluded, sorted ascending, max_depth capped at 20. Answers "what is the total blast radius if any of these symbols change?"
- RFC-0063: `mycelium_batch_reachable_to` MCP tool — total change-impact surface in one call; accepts `{ paths (up to 20), edge_kind, max_depth? }` and returns `{ reachable, count }` or `{ error }` for unknown edge kind; max_depth defaults to 10.
- RFC-0064: `Store::k_core(kind, k)` — k-core decomposition of the symbol graph; the maximal induced subgraph where every node has total degree (in + out within the subgraph) ≥ k; iterative peeling algorithm; k=0 returns all symbols; file nodes excluded; results sorted ascending.
- RFC-0064: `mycelium_get_k_core` MCP tool — hard-to-refactor core detector; accepts `{ edge_kind, k? }` and returns `{ core, count, k }` or `{ error }` for unknown edge kind; k defaults to 2.
- RFC-0065: `Store::batch_reachable_from(ids, kind, max_depth)` — union of symbols transitively reachable FROM a set of sources via outgoing edges; deduplicated, input nodes excluded, sorted ascending, max_depth capped at 20. Symmetric complement of `batch_reachable_to` (RFC-0063).
- RFC-0065: `mycelium_batch_reachable_from` MCP tool — collective forward-reachability in one call; accepts `{ paths (up to 20), edge_kind, max_depth? }` and returns `{ reachable, count }` or `{ error }` for unknown edge kind; max_depth defaults to 10.
- RFC-0066: `Store::batch_node_degree(ids)` — returns one `NodeDegree` per `NodeId` in input order; ids absent from the synapse return `NodeDegree::default()` (all counts zero). Batch version of `node_degree` (RFC-0046) eliminating N round trips when analysing a set of related symbols.
- RFC-0066: `mycelium_batch_node_degree` MCP tool — batch degree query for up to 50 symbols in one call; accepts `{ paths }` and returns `{ degrees: [{ path, in_calls, out_calls, in_imports, out_imports, in_extends, out_extends, in_implements, out_implements }], count }` with unknown paths returning `{ path, error: "path not found" }`; results in input order.
- RFC-0067: `Store::cycle_members(kind)` — paths of all symbol nodes participating in at least one directed cycle for a given EdgeKind; uses iterative Kosaraju's SCC algorithm (O(V+E)); file nodes excluded; results sorted ascending. Returns `[]` when no cycles exist.
- RFC-0067: `mycelium_find_cycle_members` MCP tool — circular dependency detector; accepts `{ edge_kind }` and returns `{ members, count }` (cycle-member symbol paths, sorted) or `{ error }` for unknown edge kind. Detects circular imports, mutually-recursive functions, and inheritance cycles.
- RFC-0068: `Store::weakly_connected_components(kind)` — groups symbol nodes into weakly-connected components (WCCs) treating edges as undirected; uses path-compressed Union-Find (O(α(V)·E)); components sorted by size descending (ties by first element); file nodes excluded. Surfaces isolated clusters and self-contained subsystems.
- RFC-0068: `mycelium_get_wcc` MCP tool — cluster detector; accepts `{ edge_kind, min_size? }` and returns `{ components, component_count, total_symbols }` or `{ error }` for unknown edge kind; `min_size` (default 1) filters singletons to focus on real clusters.
- RFC-0069: `Store::topological_sort(kind)` — topological ordering of the symbol graph via Kahn's BFS algorithm; returns `TopologicalOrder { order, cycle_members }` where `order` places each symbol after all its `kind`-predecessors (ties broken by path ascending) and `cycle_members` lists symbols that form directed cycles; file nodes excluded.
- RFC-0069: `mycelium_topological_sort` MCP tool — dependency order analysis; accepts `{ edge_kind }` and returns `{ order, cycle_members, ordered_count, cycle_count }` or `{ error }` for unknown edge kind. Useful for build order, initialization sequences, and layered architecture validation.
- RFC-0070: `Store::articulation_points(kind)` — cut vertices in the undirected symbol graph for a given EdgeKind via iterative Tarjan DFS (O(V+E)); file nodes excluded; singleton nodes (degree 0) never returned; results sorted ascending. A node is an articulation point if its removal disconnects its weakly-connected component.
- RFC-0070: `mycelium_find_articulation_points` MCP tool — single-point-of-failure detector; accepts `{ edge_kind }` and returns `{ points, count }` or `{ error }` for unknown edge kind. Identifies modules whose removal fragments the dependency graph — critical for safe refactoring and resilience analysis.
- RFC-0071: `Store::bridge_edges(kind)` — bridge edges (cut edges) in the undirected symbol graph via iterative Tarjan bridge-finding DFS (O(V+E)); file nodes excluded; multigraph-safe (parallel edges are not bridges); canonical `(from ≤ to)` pairs sorted ascending. Complements articulation points (RFC-0070): where APs are vertex cut-points, bridges are edge cut-points.
- RFC-0071: `mycelium_find_bridge_edges` MCP tool — fragile single-link connection detector; accepts `{ edge_kind }` and returns `{ bridges: [{ from, to }], count }` or `{ error }` for unknown edge kind. Identifies dependency edges whose removal would disconnect two subsystems.

### Fixed

- RFC-0013: Forward-reference calls (callee defined after caller in source order) no longer create duplicate bare stub nodes; `Calls` edges now always point to the definition node.

- RFC-0006 / RFC-0005: `.tsx` files were dispatched to `LANGUAGE_TYPESCRIPT` which cannot parse JSX syntax; corrected to use `tree_sitter_typescript::LANGUAGE_TSX`.

### Changed

- (none)

### Deprecated

- (none)

### Removed

- (none)

### Fixed

- (none)

### Security

- (none)

---

[Unreleased]: https://github.com/aimasteracc/mycelium/compare/...HEAD

