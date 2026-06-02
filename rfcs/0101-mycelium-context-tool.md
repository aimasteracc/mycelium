# RFC-0101: One-shot architecture context tool (`mycelium_context`)

- **Status**: Implemented
- **Author(s)**: orchestrator (Hive AI agent)
- **Created**: 2026-06-01
- **Last updated**: 2026-06-01
- **Tracking issue**: [#379](https://github.com/aimasteracc/mycelium/issues/379)
- **Affected source paths**:
  - `crates/mycelium-core/src/store/` - context graph assembly helpers
  - `crates/mycelium-cli/src/` - `mycelium context`
  - `crates/mycelium-mcp/src/lib.rs` - `mycelium_context`
  - `skills/architecture-context/SKILL.md` - agent-facing workflow coverage

## Summary

Add a Three-Surface capability named `context`: CLI command
`mycelium context`, MCP tool `mycelium_context`, and Skill coverage in a new
architecture-context Skill. The capability accepts a natural-language
architecture task, finds likely entry points, expands a bounded subgraph, and
returns the exact context an agent usually needs in one call:
`entry_points`, `nodes`, `edges`, `code_blocks`, `related_files`, `stats`, and
`agent_summary`.

The goal is not to turn Mycelium into an LLM. The goal is to package the graph
operations Mycelium already does well so agents stop spending 5-20 turns
manually chaining `search_symbol`, callers/callees, `symbol_info`, and source
span reads for common architecture-tracing questions.

## Motivation

Issue #379 records the practical gap found in side-by-side agent benchmarks:
Mycelium's individual graph queries are fast, but agents need too many tool
round-trips to answer questions such as "trace `ServeHTTP` to `HandlerFunc`" or
"show how auth requests reach token verification". Competing systems reduce
this by exposing a one-shot context tool that returns the relevant entry points,
graph neighborhood, and source snippets together.

Mycelium has the better foundation for this because Synapse adjacency traversal
is in-process and Hyphae can express precise graph intent. This RFC makes that
advantage reachable through a single, governed surface.

## Detailed design

### Public API

The CLI and MCP surfaces are byte-identical per RFC-0090.

```rust
pub struct ContextRequest {
    /// Natural-language task or a Hyphae selector.
    pub task: String,
    /// Maximum graph nodes in the returned neighborhood. Default: 30.
    pub max_nodes: Option<usize>,
    /// Maximum source snippets to include. Default: 6.
    pub max_code_blocks: Option<usize>,
    /// Edge kinds to traverse. Default: Calls, Imports, Extends.
    pub edge_kinds: Option<Vec<EdgeKind>>,
    /// Response encoding. Default follows RFC-0094 transport defaults.
    pub output_format: Option<OutputFormat>,
}
```

CLI:

```bash
mycelium context --task "trace ServeHTTP to HandlerFunc"
mycelium context --task 'function:calls(#AuthService)' --format json
```

MCP:

```json
{
  "task": "trace ServeHTTP to HandlerFunc",
  "max_nodes": 30,
  "max_code_blocks": 6,
  "edge_kinds": ["calls", "imports", "extends"],
  "output_format": "text"
}
```

### Response contract

Every successful response has these seven top-level keys:

```rust
pub struct ContextResponse {
    pub entry_points: Vec<ContextNode>,
    pub nodes: Vec<ContextNode>,
    pub edges: Vec<ContextEdge>,
    pub code_blocks: Vec<CodeBlock>,
    pub related_files: Vec<String>,
    pub stats: ContextStats,
    pub agent_summary: String,
}
```

`ContextNode` includes `id`, `path`, `kind`, `score`, and an optional `span`.
`ContextEdge` includes `source_id`, `target_id`, `source_path`,
`target_path`, `kind`, and an optional line/span if known. `CodeBlock`
includes `path`, `symbol_path`, `start_line`, `end_line`, and `text`.

When no useful context can be found, return an application-level NOT_FOUND
payload, not a successful empty graph:

```json
{
  "verdict": "NOT_FOUND",
  "reason": "No indexed symbol or Hyphae match was found for the task.",
  "next_step": "Try quoting an exact symbol name or run mycelium_search_symbol first."
}
```

### Pipeline

1. **Classify the task.** If `task` parses as Hyphae, or starts with a clear
   Hyphae selector prefix, run the Hyphae path first. Do not run natural-language
   extraction when the user gave a valid selector.
2. **Extract symbol candidates.** For natural language, collect quoted names,
   backticked names, CamelCase/PascalCase identifiers, snake_case identifiers
   with at least four characters, and path-like fragments containing `::`, `->`,
   `.`, `/`, or `>`. Drop common stop words.
3. **Find entry points.** Search each candidate using the same ranking semantics
   as `search_symbol`. Prefer exact path/name matches, then high-rank fuzzy
   matches, then kind-relevant matches inferred from words like "class",
   "method", "handler", or "service".
4. **Expand a bounded neighborhood.** Starting from entry points, traverse
   selected edge kinds with breadth-first expansion. Default depth is 2. Stop
   once `max_nodes` is reached. Keep traversal deterministic by sorting
   candidates by distance, edge kind order, score, and path.
5. **Build induced edges.** Return edges among selected nodes only. Preserve edge
   kind and source/target paths so agents can explain the relationship without
   another lookup.
6. **Attach code blocks.** Pick up to `max_code_blocks` nodes by score and graph
   centrality, read their source spans, and cap each snippet to 40 lines. MCP
   must apply the RFC-0097 filesystem boundary. If source text cannot be read,
   keep the node/span and omit that code block instead of failing the whole
   response.
7. **Format output.** Reuse RFC-0094 `OutputFormat` and formatter plumbing.
   `text`, `json`, and `msgpack` must all carry the same semantic payload.

### Hyphae path

Hyphae selectors are not a separate feature bolted onto this tool. They are the
precision mode:

1. Execute the selector with the same semantics as `mycelium_query`.
2. Treat matches as entry points.
3. Apply the same bounded expansion and code-block selection as the natural
   language path.

This keeps `mycelium_context` useful for both vague user language and exact
agent-authored graph queries.

### Budgets

RFC-0101 defines only context-tool-local defaults:

| Budget | Default | Hard cap |
|---|---:|---:|
| `max_nodes` | 30 | 100 |
| `max_code_blocks` | 6 | 12 |
| code lines per block | 40 | 80 |
| BFS depth | 2 | 3 |

Adaptive project-wide output sizing is intentionally left to issue #380 and a
separate RFC. This RFC must not hide or remove existing tools based on project
size.

### Three-Surface rule

Implementation must ship in one PR with:

- CLI command: `mycelium context`
- MCP tool: `mycelium_context`
- Skill coverage: `skills/architecture-context/SKILL.md`
- Skill parity fixture proving CLI/MCP semantic equivalence

No MCP-only or CLI-only implementation is allowed. If implementation needs to
split for review size, the non-public core helper PR may land first, but the
public tool cannot be exposed until all three surfaces are present.

## Drawbacks

- The tool can return plausible but incomplete context if candidate extraction
  misses the user's intended symbol. The response must expose `stats` and
  selected entry points so agents can see uncertainty.
- Adding source snippets means MCP filesystem access matters. The tool must
  respect allowed roots and degrade to spans when source text is unavailable.
- This does not replace specialist tools. Agents still need exact callers,
  callees, shortest paths, and Hyphae for deep follow-up work.

## Alternatives

1. **Improve instructions only.** Issue #382 already strengthens server
   instructions, but instructions cannot remove the round-trip cost of fetching
   entry points, graph edges, and snippets separately.
2. **Expose a generic batch tool.** Batch calls save transport overhead but still
   require the agent to choose the right sequence. `context` encodes the common
   sequence directly.
3. **Add an LLM-powered summarizer.** Rejected for now. Mycelium should remain a
   deterministic code graph engine; `agent_summary` is a concise deterministic
   summary of selected graph facts, not generated prose.

## Prior art

- CodeGraph-style one-shot context tools: natural-language task in, entry
  points plus graph neighborhood plus code blocks out.
- Sourcegraph Cody context selection: ranking code locations and snippets around
  user intent.
- rust-analyzer workspace symbol search: fast candidate discovery with precise
  follow-up lookups.
- Mycelium RFC-0094: token-efficient text output and shared `OutputFormat`.
- Mycelium RFC-0090: Three-Surface rule.

## Migration

This is non-breaking. Existing CLI commands, MCP tools, Skills, indexes, and
wire formats remain valid. `mycelium_context` is additive. Agent instructions
may recommend it first when present, but older servers without the tool remain
usable through the existing search/query/call-graph workflow.

## Testing strategy

Tests must be written RED-first before implementation.

- Unit tests:
  - candidate extraction handles quotes, backticks, CamelCase, snake_case, and
    path fragments
  - gibberish returns NOT_FOUND with `next_step`
  - response budget clamps user-provided values above hard caps
- Integration tests:
  - natural-language "trace X to Y" returns at least one entry point plus related
    nodes and edges
  - Hyphae selector path bypasses natural-language extraction and returns
    selector matches as entry points
  - code blocks respect line caps and omit unreadable files without failing
  - output formats `text`, `json`, and `msgpack` preserve equivalent payloads
- Three-Surface parity:
  - `skills/architecture-context/tests/parity.test.json` proves CLI JSON and MCP
    JSON agree on a fixture graph
- E2E:
  - dogfood Mycelium itself with a task such as "trace mycelium_query execution"
    and assert response size stays within budget

## Performance impact

| SLA | Current | After this RFC | Delta |
|---|---|---|---|
| Cold query | < 5 ms per primitive query | < 50 ms for <500 files; < 200 ms for 500-3000 files; < 500 ms for larger repos | New aggregate call; bounded by search + BFS + source reads |
| 3-hop traversal | < 1 ms | Unchanged for existing tools; context BFS cap depth <= 3 | No regression expected |
| Reactive refresh | < 10 ms | Unchanged | No indexer change |
| Token efficiency | <= 30% JSON when text/msgpack is requested | Uses RFC-0094 formatter; response budget limits total payload | Improves agent turn cost |

If a fixture cannot meet the target because source reads dominate, the first
implementation must lower default code-block count before raising the SLA.

## Acceptance criteria

- [x] RFC accepted before public API implementation starts.
- [x] MCP tool `mycelium_context` registered and callable.
- [x] CLI command `mycelium context --task "..."` is byte-equivalent with MCP
      JSON output for the same indexed fixture.
- [x] `skills/architecture-context/SKILL.md` lists the new capability in
      `allowed-tools` and includes examples for natural-language and Hyphae
      tasks.
- [x] Natural-language "trace X to Y" returns at least one entry point plus
      related nodes and edges.
- [x] Valid Hyphae task goes through the DSL path and does not run candidate
      extraction. (`mycelium_core::context::looks_like_hyphae` → DSL evaluator on
      both surfaces; `routing` field reports `"natural"` / `"hyphae"`.)
- [x] `related_files` key and `edge_kinds` request field implemented; both
      surfaces share `mycelium_core::context` so JSON is identical by
      construction; `skills/architecture-context/tests/parity.test.json` added.
- [x] Apply `OutputBudget` to the context payload (RFC-0102). Done: `OutputBudget`
      moved into `mycelium_core::budget`; both the MCP tool and the CLI twin run
      the identical `for_project(node_count)` over the same payload, so the
      truncated JSON stays byte-identical.
- [x] Gibberish task returns NOT_FOUND with a `next_step` hint.
- [x] `output_format` supports `text` and `json` (CLI); `msgpack` via MCP.
- [x] At least three RED-first tests cover: no-index error, NOT_FOUND on empty
      store, and required JSON keys (Phase 2 tests in queries.rs).
- [x] `related_files` key present in both NOT_FOUND and success responses.
- [x] `edge_kinds` optional request parameter accepted (field exists on `GetContextRequest`).
- [x] `apply_budget` wired into the success response path.
- [x] Five RED-first integration tests in `context_contract_tests` module (NOT_FOUND
      has related_files, success has related_files, all 7 RFC keys present,
      budget truncation fires, edge_kinds param accepted).
- [x] Existing quality gate remains green: fmt, clippy, tests pass.

## Open questions

1. Should `agent_summary` be a terse deterministic bullet list or a structured
   object that clients render themselves?
2. Should edge traversal include `Implements` by default along with `Extends`?
3. Should source snippets default to symbols only, or may they include
   surrounding imports when helpful?

These questions are intentionally implementation-level. They must be answered
before FCP ends, but they do not block drafting the RFC.

## Future possibilities

- Issue #380 can make budgets adaptive by project size once the one-shot context
  payload is stable.
- Issue #381 can improve entry-point and inheritance accuracy by resolving more
  cross-file references before context selection.
- A future "explain path" mode can use shortest-path and causal edge ranking to
  produce a smaller, path-focused response for specific "how does A reach B"
  questions.

