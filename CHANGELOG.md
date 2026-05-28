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
