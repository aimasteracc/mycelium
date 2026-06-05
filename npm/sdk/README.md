# @aimasteracc/mycelium-sdk

Thin, typed **Node / TypeScript SDK** for [Mycelium](https://github.com/aimasteracc/mycelium) —
the reactive, AI-native code-intelligence graph. Embed code intelligence in any
JS/TS app **without a Rust toolchain**.

The SDK is a thin wrapper over the prebuilt `mycelium` CLI ([RFC-0110](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0110-npm-bun-cli-distribution.md)):
it locates the binary, spawns it with `--format json`, and returns parsed
objects. Because it wraps the CLI, it inherits the CLI ↔ MCP byte-identical
parity guaranteed by the Charter's Three-Surface Rule — see
[RFC-0111](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0111-node-py-bindings-thin-cli-wrapper.md).

## Install

```bash
npm install @aimasteracc/mycelium-sdk
# or: bun add @aimasteracc/mycelium-sdk
```

The matching prebuilt binary is pulled in automatically via per-platform
`optionalDependencies`. No `cargo` required.

## Quickstart

```js
const { Mycelium } = require("@aimasteracc/mycelium-sdk");

const m = new Mycelium({ root: "." });

await m.index();                       // build/refresh the index
const hits = await m.query("#login");  // Hyphae selector → parsed JSON
const info = await m.getSymbolInfo("src/lib.rs>App>render");
const ctx  = await m.context("trace ServeHTTP to HandlerFunc", { maxNodes: 30 });
```

TypeScript types ship in the box (`index.d.ts`) — no build step:

```ts
import { Mycelium, MyceliumError } from "@aimasteracc/mycelium-sdk";
```

## API

| Method | CLI twin |
|---|---|
| `version()` | `mycelium version` |
| `index(path?)` | `mycelium index` |
| `query(expr)` | `mycelium query --format json` |
| `searchSymbol(q, { limit? })` | `mycelium search-symbol --format json` |
| `getSymbolInfo(path)` | `mycelium get-symbol-info --format json` |
| `getCallers(path, { edgeKind?, includeVirtual?, budget? })` | `mycelium get-callers --format json` |
| `getCallees(path, { edgeKind?, budget? })` | `mycelium get-callees --format json` |
| `context(task, { maxNodes?, maxCodeBlocks?, budget? })` | `mycelium context --format json` |
| `serverStatus()` | `mycelium server-status --format json` |
| `run(args)` | any subcommand — raw argv escape hatch |

Every command the CLI exposes is reachable via `run(["<subcommand>", …, "--format", "json"])`,
even before it has a typed convenience method.

## Binary resolution

The binary is located in this order:

1. `MYCELIUM_BIN` environment variable (explicit override),
2. the matching `@aimasteracc/mycelium-<platform>` package,
3. `mycelium` on your `PATH`.

Pass `{ bin: "/path/to/mycelium" }` to the constructor to pin one directly.

## Errors

Failures throw a `MyceliumError` carrying `{ code, signal, stderr, stdout, args }`.

## License

MIT
