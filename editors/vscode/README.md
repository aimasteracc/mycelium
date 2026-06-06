# Mycelium for VS Code

Bring [Mycelium](https://github.com/aimasteracc/mycelium) — the reactive,
AI-native code-intelligence graph — into your editor. A **thin client** over the
prebuilt `mycelium` CLI (via [`@aimasteracc/mycelium-sdk`](https://www.npmjs.com/package/@aimasteracc/mycelium-sdk)),
so there's **no Rust toolchain** to install. Implements
[RFC-0112](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0112-ide-plugin-vscode-thin-client.md),
Phase 1 (MVP).

> **Not a language server.** Mycelium surfaces *structural* graph intelligence
> (callers, callees, context), not type-resolved navigation — see
> [ADR-0010](https://github.com/aimasteracc/mycelium/blob/develop/docs/adr/0010-no-live-lsp-prefer-scip-ingestion.md).

## ✨ Headline: Copy context for AI

Put your cursor in code (or select a region), run **`Mycelium: Copy context for
AI`**, and a **token-dense context bundle** for that task lands on your
clipboard — ready to paste into Claude, ChatGPT, Cursor, or any assistant. One
click turns "explain how login reaches the database" into the exact graph slice
your AI needs.

## Commands

| Command | What it does |
|---|---|
| **Mycelium: Copy context for AI** | Cursor/selection → `context()` → clipboard |
| **Mycelium: Find callers of symbol at cursor** | structural callers (incoming `Calls`) |
| **Mycelium: Find callees of symbol at cursor** | structural callees (outgoing `Calls`) |
| **Mycelium: Show symbol info at cursor** | ancestors/descendants/callers/callees in one call |
| **Mycelium: Index this workspace** | build/refresh the `.mycelium` index |

Right-click in the editor for *Copy context for AI* and *Find callers*.

## Requirements

The `mycelium` CLI binary. It's pulled in automatically by the bundled
`@aimasteracc/mycelium-sdk` (per-platform prebuilt binary) — no `cargo`. To pin
a specific binary, set **`mycelium.binaryPath`** in Settings.

## Settings

- `mycelium.binaryPath` — absolute path to a `mycelium` binary (else resolved from the SDK/PATH).
- `mycelium.indexOnActivate` — index the workspace automatically on activation (default off).

## Develop

```bash
npm install
npm run compile      # tsc → out/
# press F5 in VS Code to launch the Extension Development Host
```

## License

MIT
