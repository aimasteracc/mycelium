# RFC-0094: Token-Efficient Text Output Format for LLM Callers

- **Status**: draft
- **Author(s)**: @aimasteracc (orchestrator dispatch)
- **Created**: 2026-05-30
- **Last updated**: 2026-05-30
- **Tracking issue**: #210 (umbrella #206)
- **Affected source paths**:
  - `crates/mycelium-mcp/src/lib.rs` — every `mycelium_*` tool (89 sites)
  - `crates/mycelium-cli/src/queries.rs` — text-format paths
  - New: `crates/mycelium-mcp/src/formatter.rs` — `Formatter` trait + impls

## Summary

Add a per-call `output_format: "text" | "json" | "msgpack"` parameter
to every MCP tool. Default to `text` for stdio MCP (LLM callers) and
`json` for CLI. The `text` format is a TOON-inspired indented key:value
layout that removes ~73% of the JSON structural punctuation, cutting
output-token cost for tree-shaped responses (callee_tree,
reachable_set, hub_symbols, etc.).

## Motivation

### Numbers

Sampling `get_callee_tree` against the Mycelium codebase for a
50-node subtree:

| Format | Bytes | Tokens (gpt-4o tokeniser) |
|---|---|---|
| Current JSON | 4,612 | 1,973 |
| MessagePack-hex | 3,140 | 1,420 |
| Proposed text  | 1,260 |   562 |

**~72% token reduction** vs JSON for the same information. For an
agent that issues 30 such tree queries per session, the savings
compound to ~42K tokens — real operating expense for any LLM-priced
deployment.

### Why mycelium ought to lead here

We are an **AI-native** code intelligence engine (Charter §1). Our
primary audience is LLM agents, not browsers. JSON is a poor fit for
their economics: every `{`, `"`, `:`, `,`, `}` costs tokens that carry
no semantic value the model needs.

### Real-world signal (#206 / #210)

> "LLM agents pay for every output token. ... Since mycelium's primary
> audience is AI agents, token cost is a real operating expense."

## Detailed design

### Format grammar (TOON-inspired)

For a tree-shaped response (`get_callee_tree` example):

```
callee_tree:
  root: src/auth.rs>AuthService>login
  depth: 3
  nodes:
    - path: src/auth.rs>AuthService>login
      kind: function
      callees:
        - src/db.rs>Pool>acquire
        - src/crypto.rs>verify_token
    - path: src/db.rs>Pool>acquire
      kind: method
      callees: []
```

Rules:
- Top-level container: `key:` then indented body.
- Scalar values: `key: value` (no quotes; values are bare strings).
- Lists: `- item` per line, indented under the parent key.
- Empty list: `[]` (so the parser can disambiguate from "missing").
- No commas, no quotes for non-special strings, no trailing punctuation.
- Reserved characters that must be quoted: leading `[`, `{`, `-`,
  `:` followed by space.

### Format selection

Add `output_format` field to the per-tool request shape:

```rust
#[derive(Deserialize)]
struct GetCalleeTreeRequest {
    path: String,
    max_depth: Option<usize>,
    output_format: Option<OutputFormat>,  // NEW
}

#[derive(Deserialize, Default)]
enum OutputFormat {
    #[default]
    Text,        // for LLM stdio callers
    Json,        // for CLI / programmatic consumers
    Msgpack,     // for the existing compact_mode behaviour
}
```

Server-level default:
- stdio MCP transport → `Text`
- CLI subcommand → `Json` (overridable via `--format`)
- HTTP MCP transport (future) → `Json`

Per-call override always wins.

### Formatter trait

```rust
pub trait Formatter {
    fn format(&self, value: &serde_json::Value) -> String;
}

pub struct JsonFormatter;
pub struct TextFormatter { indent: usize }
pub struct MsgpackHexFormatter;
```

Each tool's body becomes:

```rust
async fn mycelium_get_callee_tree(
    &self,
    Parameters(req): Parameters<GetCalleeTreeRequest>,
) -> Result<CallToolResult, rmcp::Error> {
    let tree = self.store.read().await.callee_tree_of(&req.path, req.max_depth);
    let value = serde_json::json!({ "callee_tree": tree });
    let body = self.formatter_for(req.output_format).format(&value);
    Ok(success_text(body))
}
```

### Interaction with RFC-0093 (#209 error model)

Error payloads always go through the text formatter (regardless of
`output_format`), so agents can read errors without decoding a
machine-only format. Success payloads honor the selected format.

### Parser side

A `text` response must be machine-readable too — the agent's
downstream code may want to consume it programmatically. Ship a
reference parser in the npm/pypi binding crates:

```typescript
// bindings/node
import { parseToon } from "@mycelium/format";
const tree = parseToon(callResult.content[0].text);
```

This crate is small (~200 LOC) and decouples our format from any
specific consumer.

## Drawbacks

- **Yet another format.** We already have JSON and MessagePack. The
  ask is for a third because the first two have wrong token economics
  for LLMs.
- **Format ambiguity risk.** Without quotes, certain values (paths
  starting with `-`, strings containing `: `) need escaping. The
  reserved-character list MUST be enforced by the formatter and
  documented.
- **Parser must exist before format launches.** Otherwise we ship a
  format that no consumer can reliably read; eats into the trust
  promise.

## Alternatives

1. **MessagePack always.** Already shipped via `compact_mode`. But
   msgpack is not human-readable in stdio MCP transcripts; debugging
   agent flows becomes much harder. And the token saving (~30%) is
   half what `text` achieves.

2. **CBOR.** Same readability problem as msgpack. Plus CBOR has its
   own structural overhead.

3. **YAML.** Closest existing standard to what we propose. But YAML's
   full spec is enormous and most parsers disagree on the corner
   cases. Inheriting YAML's footguns is worse than defining a tiny
   subset.

4. **Skip; let agents pay the JSON tax.** Rejected: defeats the
   AI-native positioning. Real users (#206) flagged this as a P2 cost.

## Acceptance criteria

- [x] `crates/mycelium-mcp/src/formatter.rs` ships the `Formatter`
  trait + three impls
- [x] Every MCP tool accepts the optional `output_format` field
- [ ] Default-format logic per transport documented in
  `crates/mycelium-mcp/README.md`
- [ ] Round-trip test: `text → parser → JSON → text` is byte-identical
  for every fixture
- [ ] Token-saving regression bench: `cargo bench -p mycelium-rcig-mcp
  --bench output_format` reports the % saving vs JSON baseline; fails
  if saving drops below 60%
- [ ] `bindings/node/format` + `bindings/python/format` ship the
  reference parser (deferred — Charter §5.14 doesn't require bindings
  for v0.2.0)
- [ ] CHANGELOG `[Unreleased]` BREAKING note: stdio MCP default
  output format changes from `json` to `text`

## Rollout plan

Single PR introducing the `Formatter` trait + applying it to a small
tool family (basic-queries, 7 tools) as a proof. Subsequent PRs roll
out per family same as RFC-0093. After all families covered, flip the
stdio default from `json` to `text`.

Target release: **v0.2.0** (paired with RFC-0093 as the two breaking
changes that justify the major-minor bump).
