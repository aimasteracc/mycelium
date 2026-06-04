# @aimasteracc/mycelium

**The `mycelium` CLI — no Rust toolchain required.**

[Mycelium](https://github.com/aimasteracc/mycelium) is a reactive, AI-native
code-intelligence graph. This package ships the prebuilt `mycelium` binary so
you can install it with **npm** or **bun** — no `cargo` needed.

## Install

```bash
# npm
npm install -g @aimasteracc/mycelium
mycelium --version

# bun
bun add -g @aimasteracc/mycelium
# or run without installing:
bunx @aimasteracc/mycelium --version
npx  @aimasteracc/mycelium --version
```

## How it works

This is a thin launcher. The actual binary lives in a per-platform package
(`@aimasteracc/mycelium-<platform>`) that your package manager installs
automatically based on your OS and CPU (via npm/bun `optionalDependencies` +
`os`/`cpu` gating). No postinstall download, no network at runtime.

Supported platforms: macOS (arm64, x64), Linux (x64, arm64 / glibc),
Windows (x64). On an unsupported platform, install from source instead:

```bash
cargo install mycelium-rcig-cli
```

## Usage

Same CLI as the cargo-installed binary:

```bash
mycelium index .
mycelium serve --mcp
mycelium context --task "how does request routing work"
```

See the [main README](https://github.com/aimasteracc/mycelium) for the full
command reference. MIT licensed.
