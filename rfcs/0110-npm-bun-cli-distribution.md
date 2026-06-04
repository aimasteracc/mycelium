# RFC-0110: npm / bun distribution of the Mycelium CLI (prebuilt binary)

- **Status**: **Accepted** (ratified 2026-06-03 UTC under the founder's
  autonomous-development mandate; goal: "讓客戶沒有 cargo 環境也能使用我們的項目。
  npm 安裝 bun 安裝支持"). Implementation proceeds incrementally.
- **Author(s)**: orchestrator (Hive AI agent)
- **Created**: 2026-06-03 (UTC)
- **Depends on**: release.yml (existing `publish-npm` job stub), Charter §3
  (Bindings), Charter §5.12 (release gate)
- **Affected paths**: `npm/`, `.github/workflows/release.yml`, `README.md`

## Summary

Let users **without a Rust/Cargo toolchain** install and run the `mycelium`
CLI via `npm install` / `bun install` (and `npx` / `bunx`). We ship the
**prebuilt CLI binary** through npm using the **optionalDependencies
per-platform package** model (as used by esbuild, biome, swc, turbo): a thin
universal launcher package plus one tiny package per platform containing the
matching native binary, gated by npm/bun's `os`/`cpu` fields.

## Motivation

Today the only install path is `cargo install mycelium-rcig-cli` — it requires
a Rust toolchain, which most JS/TS users (a primary audience for an
AI-agent code-intelligence CLI) do not have. GitHub Releases currently attach
**no binaries**, and there is no npm package (the release workflow's
`publish-npm` job looks for a non-existent `bindings/node` and skips).

## Scope & relationship to Charter §3 (napi-rs)

Charter §3 lists **napi-rs** for npm "Bindings". That is for embedding Mycelium
as a **Node library** (a `.node` addon callable from JS). **This RFC is a
different, complementary concern**: distributing the **CLI executable** so the
`mycelium` command works without cargo. The two can coexist (CLI binary now;
napi-rs library later). No Charter §3 amendment is needed — this adds a
distribution channel for the existing CLI, it does not change the bindings
strategy.

## Decision: optionalDependencies prebuilt-binary model

```
npm/
  mycelium/                       # the package users install
    package.json                 # bin: mycelium; optionalDependencies: all platform pkgs
    bin/mycelium.cjs             # launcher: resolve platform binary, exec with argv
    README.md
  platform-template/             # template for the per-platform packages
    package.json                 # { os:[..], cpu:[..] }  (binary injected at build)
  scripts/
    build-npm.mjs                # fills platform packages from prebuilt binaries
```

**Target platforms (v1):** `darwin-arm64`, `darwin-x64`, `linux-x64-gnu`,
`linux-arm64-gnu`, `win32-x64`. (Matches CI's ubuntu/macos/windows matrix; more
can be added later without breaking changes.)

**Package names:** scoped under the founder's namespace to avoid the
short-name collisions that forced the `mycelium-rcig-*` crate prefix:
- main: `@aimasteracc/mycelium`
- platform: `@aimasteracc/mycelium-<platform>` (e.g. `@aimasteracc/mycelium-darwin-arm64`)

*(Final scope is the founder's call; the implementation parameterizes it.)*

**Launcher** (`bin/mycelium.cjs`): map `${process.platform}-${process.arch}` to
the platform package, `require.resolve` its binary, and `spawnSync(binary,
argv.slice(2), {stdio:'inherit'})`, forwarding the exit code. On an
unsupported/missing platform it prints a clear error and exits non-zero.

### Why this model (vs alternatives)

- **vs postinstall-download (curl from GH Releases):** rejected. Requires
  network at install, breaks in offline/sandboxed CI, postinstall scripts are
  frequently disabled for security, and integrity is harder. optionalDeps is
  install-time-deterministic and provenance-friendly (`npm publish
  --provenance`, already in release.yml).
- **vs napi-rs:** that ships a library, not the `mycelium` command; different
  use case (see Scope above).

### bun compatibility

bun honors `optionalDependencies` + `os`/`cpu` gating and runs `bin` entries, so
the **same packages work for `bun install` / `bunx`** with no extra work. The
launcher uses only Node-compatible `child_process`, which bun supports. CI will
add a bun smoke test alongside the npm one.

## CI / release integration

Rewire `release.yml`:
1. **New `build-cli-binaries` matrix job** — cross-compile `mycelium` for each
   target (`cargo build --release -p mycelium-rcig-cli`, using
   `Swatinem/rust-cache`; linux-arm64 via `cross` or the `aarch64` GNU
   toolchain). Upload each binary as a workflow artifact **and** attach it to
   the GitHub Release (so a download path also exists).
2. **Replace the `publish-npm` stub** — download the per-platform artifacts, run
   `scripts/build-npm.mjs` to assemble the platform packages + main package at
   the release version, then `npm publish --access public --provenance` each
   (idempotent: skip versions already on npm, mirroring the crates.io guard).
3. Keep the napi-rs `bindings/node` path available for the future library.

Charter §5.12 still governs: npm publish runs only after green
quality-recheck, exactly like crates.io.

## Acceptance criteria

- [x] `npm/` package scaffolding: main package + launcher + platform template +
      `build-npm.mjs`, with the launcher's platform-resolution logic unit-tested.
      *(Done in increment 1: `npm/mycelium` launcher with 8 passing `node:test`
      unit tests, main `package.json` with 5-platform `optionalDependencies`,
      and `npm/scripts/build-npm.mjs` verified end-to-end with fixture binaries.)*
- [~] `release.yml`: per-platform binary build matrix; binaries attached to the
      GitHub Release; `publish-npm` assembles + publishes the packages
      (idempotent). **Increment 2 done:** `build-cli-binaries` matrix (5
      targets, native + `cross` for linux-arm64) builds + uploads each binary
      and `finalize` attaches them to the GitHub Release. **Pending:**
      `publish-npm` rewire (increment 3).
- [ ] On a cargo-less machine: `npm i -g @aimasteracc/mycelium && mycelium
      --version` works; `bunx @aimasteracc/mycelium --version` works.
- [ ] README "Install" section documents the npm/bun path alongside cargo.
- [ ] CHANGELOG `[Unreleased]` notes the new install channel.

## Rollout

Incremental, each behind green CI:
1. ✅ RFC + `npm/` scaffolding + launcher unit test (#517).
2. ✅ `release.yml` build matrix + GH Release binary upload (this PR).
3. `publish-npm` rewire (assemble + publish) + bun/npm install smoke test.
4. README + CHANGELOG (README + initial CHANGELOG done in #517).
