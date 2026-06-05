# mycelium-rcig

Thin, typed **Python SDK** for [Mycelium](https://github.com/aimasteracc/mycelium) —
the reactive, AI-native code-intelligence graph. Embed code intelligence in any
Python app **without a Rust toolchain**.

The SDK is a thin wrapper over the prebuilt `mycelium` CLI: it locates the
binary, spawns it with `--format json`, and returns parsed objects. Because it
wraps the CLI, it inherits the CLI ↔ MCP byte-identical parity guaranteed by the
Charter's Three-Surface Rule — see
[RFC-0111](https://github.com/aimasteracc/mycelium/blob/develop/rfcs/0111-node-py-bindings-thin-cli-wrapper.md).

> **Naming.** The PyPI distribution is **`mycelium-rcig`** (the short name
> `mycelium` is taken by an unrelated package), mirroring the crates.io
> `mycelium-rcig-*` prefix. The import package is **`mycelium_rcig`**.

## Install

```bash
pip install mycelium-rcig
```

You also need the `mycelium` CLI on your machine. Install it via npm
(`npm install -g @aimasteracc/mycelium`), cargo
(`cargo install mycelium-rcig-cli`), or point the SDK at a binary with the
`MYCELIUM_BIN` environment variable.

## Quickstart

```python
from mycelium_rcig import Mycelium

m = Mycelium(root=".")

m.index()                                   # build/refresh the index
hits = m.query("#login")                    # Hyphae selector → parsed JSON
info = m.get_symbol_info("src/lib.rs>App>render")
ctx  = m.context("trace ServeHTTP to HandlerFunc", max_nodes=30)
```

## API

| Method | CLI twin |
|---|---|
| `version()` | `mycelium version` |
| `index(path=None)` | `mycelium index` |
| `query(expr)` | `mycelium query --format json` |
| `search_symbol(query, limit=None)` | `mycelium search-symbol --format json` |
| `get_symbol_info(path)` | `mycelium get-symbol-info --format json` |
| `get_callers(path, edge_kind=None, include_virtual=False, budget=None)` | `mycelium get-callers --format json` |
| `get_callees(path, edge_kind=None, budget=None)` | `mycelium get-callees --format json` |
| `context(task, max_nodes=None, max_code_blocks=None, budget=None)` | `mycelium context --format json` |
| `server_status()` | `mycelium server-status --format json` |
| `run(args)` | any subcommand — raw argv escape hatch |

Every command the CLI exposes is reachable via
`run(["<subcommand>", ..., "--format", "json"])`, even before it has a typed
convenience method.

## Binary resolution

The binary is located in this order:

1. `MYCELIUM_BIN` environment variable (explicit override),
2. `mycelium` on your `PATH`,
3. the bare command name (the OS resolves it at spawn time).

Pass `bin="/path/to/mycelium"` to the constructor to pin one directly.

## Errors

Failures raise a `MyceliumError` carrying `code`, `signal`, `stderr`, `stdout`,
and `args_` (the CLI argv).

## License

MIT
