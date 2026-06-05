# RFC-0112: IDE plugin — VS Code extension as a thin client (design)

- **Status**: **Draft** (design — no implementation in this PR)
- **Author(s)**: orchestrator (Hive AI agent)
- **Created**: 2026-06-05 (UTC)
- **Depends on**: [RFC-0111](0111-node-py-bindings-thin-cli-wrapper.md) (Node SDK
  — the plugin's engine client), [RFC-0105](0105-shared-watch-engine-cli-watch.md)
  /[RFC-0107](0107-subscribe-scoped-delta.md)/[RFC-0108](0108-reactive-query-subscriptions.md)
  (reactive watch/subscribe — live updates), [ADR-0010](../docs/adr/0010-no-live-lsp-prefer-scip-ingestion.md)
  (no live LSP — **binding constraint**), Charter §5.13 (Three-Surface Rule)
- **Affected paths** (when implemented): `editors/vscode/` (new), `README.md`
- **Supersedes**: none

## Summary

Bring Mycelium into the editor where developers actually work, as a **thin VS
Code extension** that is a *client* of the existing engine surfaces — the Node
SDK ([RFC-0111](0111-node-py-bindings-thin-cli-wrapper.md)) for queries and the
reactive `watch`/`subscribe` stream ([RFC-0105](0105-shared-watch-engine-cli-watch.md)/[RFC-0107](0107-subscribe-scoped-delta.md))
for live updates. It surfaces Mycelium's call/inheritance/import graph, dead-code
and structure views, and — the headline — **one-click token-dense "context for
your AI assistant"** at the cursor.

It is **not** a Language Server and does **not** wrap live LSPs (ADR-0010). It
adds **no new engine capability** — every action maps onto an existing CLI+MCP
pair (Charter §5.13), so it is a consumer surface like the SDKs, not a fourth
capability surface.

## Motivation

1. **Developers live in the IDE, not the terminal.** The CLI and MCP reach
   scripts and AI agents; the SDKs reach app developers. The one audience still
   unserved by a first-class UX is the human writing code in an editor.
2. **AI-assisted coding needs token-dense context at the cursor.** The fastest-
   growing dev workflow is "ask an AI about this code." Mycelium's
   differentiator is *token-dense, cross-language, reactive* context
   (`mycelium_context`). A one-click "copy context for AI" turns that engine
   value into a daily-habit feature — directly serving the commercial
   positioning (Mycelium as the embeddable **context layer**, not a linter).
3. **Reactive shines in-editor.** Mycelium's watch/subscribe delta stream
   (RFC-0105/0107/0108) was built for exactly this: update the graph as the user
   types/saves, with scoped deltas instead of re-indexing. An IDE is where
   "reactive code intelligence" becomes visible.
4. **The AI-IDE half is already done.** Cursor, Claude Code, Windsurf, Zed et al.
   consume **MCP** — Mycelium's MCP server is *already* their integration path.
   This RFC covers the **human-facing** editor UX that MCP does not provide.

## Scope & relationship to existing decisions

| Concern | Owner | Status |
|---|---|---|
| AI-agent IDE integration (Cursor/Claude/…) | existing **MCP server** | ✅ shipped — no work here |
| Human-facing editor UX (VS Code) | **this RFC** | proposed (design) |
| Type-level precision in the editor | ADR-0010 (optional static SCIP) | out of scope; the plugin shows what the engine knows |
| Live LSP / wrapping rust-analyzer etc. | **rejected** by ADR-0010 | not an option |

### Binding constraint — ADR-0010 (no live LSP)

The extension MUST NOT implement the Language Server Protocol as a Mycelium
server, MUST NOT spawn or proxy language servers, and MUST NOT present itself as
a "language server for X." It is a **graph/context client**. Editor features that
look LSP-ish (go-to, hover) are powered by Mycelium's *graph* (callers/callees/
extends/imports), explicitly labelled as structural — not type-resolved — to set
correct expectations. This RFC is the recorded guard against a future
contributor "adding IDE support" by reaching for `vscode-languageclient`.

## Decision: thin VS Code extension over the Node SDK + reactive stream

```
┌──────────────────────────── VS Code extension (TS) ─────────────────────────┐
│  Tree views · CodeLens · hover · commands · "context for AI" · status bar    │
│         │ on-demand queries                    │ live deltas                 │
│         ▼                                       ▼                             │
│  @aimasteracc/mycelium-sdk (RFC-0111)     child: `mycelium watch --subscribe`│
│         │ spawn `mycelium <cmd> --format json`        │ scoped delta stream   │
└─────────┼───────────────────────────────────────────┼───────────────────────┘
          ▼                                            ▼
                    one prebuilt `mycelium` CLI binary (RFC-0110)
                                     │
                              the engine (RCIG)
```

- **Queries** go through the **Node SDK** — `query`, `getCallers`/`getCallees`,
  `getSymbolInfo`, `context`, `getDeadSymbols`, etc. Zero new wire contracts; the
  extension inherits CLI↔MCP parity for free.
- **Live updates** use the reactive surface: the extension runs one long-lived
  `mycelium watch --subscribe <scope>` child (RFC-0105/0107) and applies scoped
  deltas to its views on save — no full re-index. (Degrades gracefully to
  on-save re-query if the watch child is unavailable.)
- **Binary discovery** reuses the SDK's resolver (`MYCELIUM_BIN` → bundled/`PATH`).
  The extension does not bundle the engine in v1 — it resolves a user/workspace
  `mycelium` (installed via npm/cargo), with a guided install prompt if missing.

### Feature set

**Phase 1 — read-only MVP (on-demand via SDK):**
- **Structure / call-graph tree view** in the sidebar: callers, callees,
  extends/implements, for the symbol at the cursor.
- **Commands** (command palette + context menu): *Find callers / callees*,
  *Go to definition-by-path*, *Show symbol info*, *Dead code in file/folder*.
- **CodeLens / hover**: "N callers · M callees" above functions (structural,
  labelled as such), click to expand the tree.
- **🟢 Headline — "Copy context for AI"**: at the cursor (or for a selection/
  symbol), call `context(task)` and copy the token-dense bundle to the clipboard
  (and/or open it), ready to paste into any AI chat. The differentiator feature.

**Phase 2 — reactive (live):**
- Drive a `watch --subscribe` child; refresh the tree/CodeLens from scoped deltas
  as files change. Status-bar indicator for index freshness.

**Phase 3 — reach:**
- JetBrains plugin (same thin-client architecture over the CLI/SDK).
- Optional engine-binary bundling for a zero-install experience.

## Three-Surface Rule compliance (Charter §5.13)

The extension introduces **no new capability** — every action is an existing
CLI+MCP command surfaced in the UI. It is a *consumer* of the surface (like the
RFC-0111 SDKs), so there is no new 1:1 pair to maintain and no Skill-coverage
gap. If a future editor feature ever needs a new capability, that capability MUST
land first as a CLI+MCP pair (with Skill coverage) before the UI consumes it.

## Acceptance criteria (when promoted from design to implementation)

**Phase 1 (MVP):**
- [ ] `editors/vscode/` extension scaffold (TS, esbuild-bundled, `vsce`-packagable).
- [ ] Engine client built on `@aimasteracc/mycelium-sdk`; binary discovery with a
      missing-binary install prompt.
- [ ] Sidebar tree for callers/callees/extends/implements at the cursor.
- [ ] Commands: find callers/callees, symbol info, dead code; CodeLens caller/
      callee counts (labelled *structural*).
- [ ] "Copy context for AI" command → `context()` → clipboard, with task framing.
- [ ] Tests (extension host integration) + README + a short demo GIF.
- [ ] No `vscode-languageclient` dependency (ADR-0010 guard, CI-checked).

**Phase 2 (reactive):** `watch --subscribe` child, delta-driven view refresh,
freshness status bar.

**Phase 3:** JetBrains plugin; optional bundled binary.

## Alternatives considered

- **A real Language Server (LSP) for Mycelium.** Rejected by **ADR-0010** and by
  the thin-client philosophy: it would duplicate editor-protocol surface, invite
  "type-precision" expectations tree-sitter can't meet, and create a heavyweight
  resident server. The graph-client model gives the high-value navigation without
  the LSP baggage.
- **Re-implement queries natively in the extension (TS).** Rejected: forks engine
  logic, breaks parity, contradicts RFC-0111. The SDK already exists.
- **Only ship the MCP path and call it "IDE support."** Rejected as incomplete:
  MCP serves AI agents, not the human editor UX (tree views, CodeLens, one-click
  context). Both are wanted; MCP is already done, this adds the human half.

## Open questions

1. **Binary delivery** — v1 resolves a user-installed CLI; should v2 bundle a
   per-platform binary in the `.vsix` (like the npm SDK's platform packages) for
   zero-install? (Leaning yes for Phase 3.)
2. **Marketplace identity** — publisher id + extension name (`mycelium`?), and
   whether to also publish to the Open VSX registry (VSCodium/Cursor).
3. **"Context for AI" framing** — fixed task templates vs. a free-text prompt box
   feeding `context(task)`; how much to pre-shape for popular assistants.
4. **Reactive scope** — per-file vs. per-open-editor `subscribe` scope to bound
   delta volume on large repos.

## Security considerations

- The extension only spawns the resolved `mycelium` binary with an argv array
  (via the SDK) — no shell, no injection from file paths/selectors.
- It reads workspace files only through the engine; it opens no network
  connections of its own. "Copy context for AI" places code context on the
  **clipboard** — the UI must make that explicit so users don't paste proprietary
  code into an external assistant unknowingly.
- `MYCELIUM_BIN` / workspace binary settings let teams pin an audited engine.
