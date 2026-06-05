# RFC-0111: Node & Python bindings via thin CLI wrapper

- **Status**: **Implemented** — Phase 1 (Node SDK) merged (PR #559, Charter §3
  amendment founder-ratified). Phase 2 (Python SDK, `mycelium-rcig`) implemented
  in the follow-up PR.
- **Author(s)**: orchestrator (Hive AI agent)
- **Created**: 2026-06-05 (UTC)
- **Depends on**: [RFC-0110](0110-npm-bun-cli-distribution.md) (prebuilt CLI
  binary already shipped via npm), Charter §3 (Bindings — **amended by this
  RFC**), Charter §5.13 / [RFC-0090](0090-cli-mcp-skill-parity.md)
  (Three-Surface Rule), [RFC-0094](0094-token-efficient-output.md)
  (`--format json` stable contract)
- **Affected paths**: `npm/sdk/`, `bindings/python/` (Phase 2), `CHARTER.md` §3,
  `README.md`, `CHANGELOG.md`
- **Supersedes**: none

## Summary

Ship first-class **language SDKs** for Node/TypeScript and Python that let
applications call Mycelium *as a library* — `const m = new Mycelium();
await m.query("#login")` / `m.query("#login")` — without a Rust toolchain and
without learning the CLI's argv conventions.

The SDKs are **thin wrappers over the already-distributed CLI binary**: they
locate the `mycelium` executable (shipped via RFC-0110 on npm, and via PyPI in
Phase 2), spawn it with `--format json`, and parse the stdout JSON into native
objects. They are **not** native FFI addons (`napi-rs` / `pyo3`).

## Motivation

RFC-0110 made the `mycelium` **command** installable without cargo. But an
AI-agent or app developer who wants to *embed* code intelligence still has to:

- shell out manually and remember each subcommand's flags,
- append `--format json` and `JSON.parse` / `json.loads` by hand,
- handle non-zero exit codes, stderr, and binary discovery themselves.

A typical consumer (a TS agent framework, a Python LangChain tool, a CI script)
wants an ergonomic, typed client object — not a subprocess recipe. That is the
gap this RFC closes.

## Scope & relationship to Charter §3 and RFC-0110

| Concern | Owner | Status |
|---|---|---|
| Distribute the **CLI executable** without cargo | RFC-0110 | ✅ Implemented (npm) |
| Embed Mycelium as a **library** in Node/Python | **this RFC** | proposed |
| In-process **native FFI** addon (`.node` / `.so`) | future | deferred (see Alternatives) |

RFC-0110 deliberately left "embed as a library" out of scope and pointed at a
future `bindings/node` napi-rs path. This RFC fills that slot **but changes the
mechanism** from native FFI to a thin CLI wrapper, for the reasons below.

## Decision: thin CLI wrapper, not native FFI

The SDK spawns the prebuilt CLI and parses its JSON. Rationale:

1. **Three-Surface parity is inherited for free.** Charter §5.13 mandates
   CLI ↔ MCP byte-identical 1:1. A wrapper over the CLI is, by construction,
   1:1 with the CLI — so it is automatically 1:1 with MCP too. A native FFI
   binding would be a *fourth* surface that must be kept in lock-step by hand,
   multiplying the parity-drift surface the Charter exists to prevent.
2. **Zero core coupling.** The wrapper depends only on the **stable
   `--format json` output contract** (RFC-0094), never on `mycelium-core`
   internals. Core can refactor freely; the SDK only breaks if the *documented
   JSON* breaks — which CI already guards.
3. **Reuses RFC-0110 distribution.** No new per-platform native-addon build
   matrix (`.node` per Node-ABI × OS × arch is a combinatorial nightmare;
   `napi-rs` prebuilds help but still couple to N-API versions). The single
   prebuilt CLI binary already exists and is already published.
4. **Tiny maintenance + matches commercial positioning.** The product value is
   the *engine* (token-dense, reactive, cross-language context), surfaced as an
   embeddable layer. A thin wrapper is the minimum code that exposes that value
   to two huge ecosystems; it does not fork the engine into three codebases.

**Cost accepted:** one subprocess spawn per call (no warm in-process state, no
streaming across the boundary). For the SDK's use case — discrete context/query
calls from an agent — this is acceptable. The native-FFI path remains available
later as a *performance optimization* for hot-loop embedders (see Alternatives),
behind its own RFC, without breaking the SDK API.

## Charter §3 amendment (requires this RFC per Charter §3 "Locked")

Charter §3's tech-stack table currently reads:

> | Bindings | napi-rs (npm) + maturin/pyo3 (PyPI) | Reach both ecosystems |

This RFC amends that row to:

> | Bindings | **thin CLI-wrapper SDKs** (npm `@aimasteracc/mycelium-sdk`, PyPI `mycelium`) over the RFC-0110 prebuilt binary; native FFI (napi-rs / maturin·pyo3) reserved for a future in-process performance RFC | Reach both ecosystems with one engine and inherited CLI↔MCP parity |

The *goal* of §3 ("reach both ecosystems") is unchanged; only the *mechanism*
changes. napi-rs/pyo3 are not deleted from the roadmap — they are re-scoped to a
later performance concern.

## Architecture

### Node SDK (`@aimasteracc/mycelium-sdk`) — Phase 1

```
npm/sdk/
  package.json            # name @aimasteracc/mycelium-sdk; peerDep on the CLI pkg
  index.js                # public entry: re-exports Mycelium + errors
  index.d.ts              # hand-written TS types (no build step)
  src/
    resolve-binary.js     # MYCELIUM_BIN env → CLI optionalDep package → PATH
    run.js                # spawn binary, capture stdout, JSON.parse, error model
    client.js             # Mycelium class: low-level run() + typed convenience methods
  test/                   # node:test, injected fake spawn (hermetic, no real binary)
  README.md
```

- **Binary resolution** (`resolve-binary.js`): in order — (1) `MYCELIUM_BIN`
  env var (explicit override / monorepo / CI), (2) the RFC-0110 per-platform
  optionalDependency package (`@aimasteracc/mycelium-<platform>`), (3) `mycelium`
  on `PATH`. The platform→package map is the **same table** the RFC-0110
  launcher uses. Resolver is dependency-injected for hermetic unit tests.
- **Runner** (`run.js`): async `execFile`-style spawn; captures stdout/stderr;
  on exit 0 → `JSON.parse(stdout)`; on non-zero exit or unparseable JSON →
  throw `MyceliumError` carrying `{ code, stderr, args }`. Spawn fn injected.
- **Client** (`client.js`): `new Mycelium({ root?, bin?, budget? })`.
  - Low-level escape hatch: `run(args: string[]) → Promise<unknown>` — appends
    `--format json` / `--root` where applicable; covers **all** CLI commands,
    including ones without a typed convenience method.
  - Typed convenience methods for the core set (Phase 1): `version()`,
    `index(path?)`, `query(expr, {format?})`, `searchSymbol(q, {limit?})`,
    `getSymbolInfo(path)`, `getCallers(path, opts?)`, `getCallees(path, opts?)`,
    `context(task, opts?)`, `serverStatus()`. The remaining commands are reached
    via `run()` until promoted to typed methods (purely additive, never
    breaking).

### Python SDK (`mycelium-rcig` on PyPI) — Phase 2

Same architecture, Pythonic surface: `from mycelium_rcig import Mycelium`,
`m.query("#login")`, `MyceliumError`. Distributed as a **pure-Python wheel**
(hatchling, no `maturin` — there is no Rust extension); binary resolution is
`MYCELIUM_BIN` → `PATH` (Python has no npm-style per-platform optional package;
binary **bundling** via platform wheels is a deferred follow-up — for now the
user installs the CLI via npm/cargo or points `MYCELIUM_BIN` at a binary). The
PyPI **distribution** name is `mycelium-rcig` (the short `mycelium` is taken by
an unrelated package, mirroring the crates.io `mycelium-rcig-*` prefix); the
**import** package is `mycelium_rcig` to avoid shadowing it. **No core or Rust
changes** — same thin-wrapper contract as Node.

## Three-Surface Rule compliance (Charter §5.13)

The SDKs add **no new capabilities** — every method maps onto an existing
CLI+MCP pair. Therefore:

- **CLI ↔ MCP**: unchanged, still strict 1:1.
- **SDK**: a *consumer* of the CLI surface, not a new capability surface. No
  orphan tools, no Skill coverage gap. This RFC introduces no command that
  lacks a CLI/MCP twin.

If a future SDK convenience method ever composes multiple CLI calls into one
new capability, that capability MUST first exist as a CLI+MCP pair (an
`EXCEPTION:` line would be required otherwise). Phase 1 introduces none.

## Acceptance criteria

**Phase 1 — Node SDK (this RFC's first PR):**

- [x] `npm/sdk/` scaffolding: `package.json`, `index.js`, `index.d.ts`, and the
      three `src/` modules.
- [x] `resolve-binary.js`: env → CLI-package → PATH resolution order, with the
      RFC-0110 platform map; injected resolver; unit-tested for hit/miss/override.
- [x] `run.js`: spawn + capture + `JSON.parse`; `MyceliumError` on non-zero exit
      / bad JSON / signal; injected spawn; unit-tested for each path.
- [x] `client.js`: `Mycelium` with `run()` + the Phase-1 typed methods; argv
      assembly (incl. `--format json`, `--root`, `--budget`) unit-tested against
      an injected fake spawn (hermetic — no real binary needed).
- [x] `index.d.ts` gives TS consumers full types with no build step.
- [x] `README.md` (install + quickstart) and an `npm test` (`node:test`) green
      (28 hermetic unit tests + 2 guarded integration tests).
- [x] CI runs the SDK unit tests **and** a live integration test against the
      release binary (`.github/workflows/ci.yml` `unit` job).
- [x] CHARTER §3 bindings row amended per this RFC.
- [x] README "Use as a library" section + CHANGELOG `[Unreleased]` entry.

**Phase 2 — Python SDK (follow-up PR, same RFC):**

- [x] `bindings/python/` thin wrapper with the same resolution + run + client
      shape (`mycelium_rcig` package: `_resolve` + `_run` + `_client`); 32
      stdlib-`unittest` tests (30 hermetic + 2 guarded integration) green; typed
      (`py.typed` + inline hints); binary-location strategy documented
      (`MYCELIUM_BIN` → `PATH`; bundling deferred).
- [x] PyPI packaging (`mycelium-rcig` — the short `mycelium` is taken, mirroring
      the crates prefix; import `mycelium_rcig`) wired into release automation
      (`release.yml` `publish-pypi`: version-pinned `python -m build` + Trusted
      Publishers, idempotent via `skip-existing`). CI runs the unit + integration
      tests against the release binary.
- [x] README + CHANGELOG updated for the Python channel; Charter §3 PyPI name
      corrected to `mycelium-rcig`.

## Rollout

Incremental, each behind green CI:

1. RFC + Node SDK (`resolve-binary` + `run` + `client` + tests + README) +
   Charter §3 amendment + CI (unit + integration) + **release packaging**
   (`build-npm.mjs` assembles `mycelium-sdk` with version-pinned platform
   optionalDependencies; `release.yml` publishes it after the main package).
   **← this PR.** The SDK goes live at the next release that runs `release.yml`.
2. Python SDK (Phase 2) under this same RFC.
4. (Future, separate RFC) optional native-FFI fast path for hot-loop embedders,
   API-compatible with the wrapper SDK.

## Alternatives considered

- **napi-rs (Node) + pyo3/maturin (Python) native FFI — the original Charter §3
  plan.** In-process, no subprocess overhead, streaming-capable. Rejected for
  Phase 1 because it (a) creates a fourth parity surface to hand-maintain
  against CLI/MCP, (b) couples bindings to `mycelium-core` internals and N-API /
  Python-ABI versions, (c) needs a combinatorial prebuild matrix, and (d)
  triples the engine's effective API surface — all to optimize a cost (one
  spawn per call) that the SDK's discrete-call usage does not feel. Retained as
  a **future opt-in performance path** behind its own RFC, API-compatible with
  the wrapper so embedders can switch without rewrites.
- **No SDK, document the subprocess recipe instead.** Rejected: pushes binary
  discovery, JSON parsing, and the error model onto every consumer; no types;
  high friction for the primary JS/Python audience.
- **A single cross-language SDK generator.** Over-engineered for two targets;
  deferred until a third ecosystem (e.g. Go) actually appears.

## Security considerations

- The SDK only ever spawns the resolved `mycelium` binary with an **argv array**
  (never a shell string) — no shell interpolation, no injection from
  user-supplied selectors/paths.
- `MYCELIUM_BIN` lets a consumer pin an audited binary; otherwise resolution is
  confined to the signed/published CLI package or `PATH`.
- The wrapper reads only the binary's stdout/stderr; it writes nothing and opens
  no network connections of its own.
