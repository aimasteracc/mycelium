# Dogfood Report — v0.1.14

> Mycelium indexing **itself** end-to-end. This validates the v0.2 PRD success
> metric "*orchestrator runs `mycelium *` end-to-end against this repo*" and the
> 8/8 core-command dogfood target.
>
> Run: 2026-05-31 · binary: `target/release/mycelium` (develop @ c59aab6) ·
> target repo: this workspace.

## Method

Built the release binary, indexed the repo, then exercised the 8 core CLI
commands from the v0.2 PRD dogfood metric. Each command's exit status and a
sample of its output were captured.

```bash
cargo build --release -p mycelium-rcig-cli
target/release/mycelium index .
target/release/mycelium get-stats --root .
target/release/mycelium query ".function" --root .
target/release/mycelium search-symbol Store --root .
target/release/mycelium get-symbol-info "<path>" --root .
target/release/mycelium get-callers "<path>" --root .
target/release/mycelium get-callees "<path>" --root .
target/release/mycelium get-imports "<file>" --root .
```

## Result: 8 / 8 ✅

| # | Command | Status | Notes |
|---|---|---|---|
| 1 | `index .` | ✅ | 195 files, 14 523 nodes, 9 871 edges, ~0.4 s, wrote `.mycelium/index.rmp` |
| 2 | `get-stats` | ✅ | Returns node/edge/file counts + detected languages (`rust`) |
| 3 | `query ".function"` | ✅ | Hyphae selector returns function symbol paths |
| 4 | `search-symbol Store` | ✅ | Finds `mycelium-core/src/store/mod.rs>Store` |
| 5 | `get-symbol-info` | ✅ | Returns kind + span for a symbol path |
| 6 | `get-callers` | ✅ | Non-empty caller set for `Trunk>lookup` |
| 7 | `get-callees` | ✅ | Returns callee set |
| 8 | `get-imports` | ✅ | Returns file import edges |

## Findings

### F1 — Indexed path prefix omits the workspace `crates/` directory (UX note)

Symbol paths in the index start at the **crate directory**, not the workspace
root. The `Store` struct indexes as:

```
mycelium-core/src/store/mod.rs>Store        ✅ found
crates/mycelium-core/src/store/mod.rs>Store ❌ not found (wrong prefix)
```

This is internally consistent (every command uses the same prefix), but a user
who *guesses* the path with the `crates/` prefix gets an empty result with no
hint. **Recommendation**: always run `search-symbol <name>` first to learn the
exact path prefix, then feed that path to `get-symbol-info` / `get-callers` /
etc. A future ergonomics improvement could be a fuzzy-path fallback that
suggests the correct prefix when an exact lookup misses (candidate v0.2 issue).

### F2 — Language detection limited to `rust` on this repo (expected)

`get-stats` reports only `rust`. Correct: the workspace is Rust source; the
`packs/<lang>/*.scm` query files are pack definitions, not indexable source in
the supported extensions (`.rs/.py/.ts/.js/.go/...`). No action needed.

## Verdict

All 8 core commands work end-to-end against the project's own source. The
v0.2 PRD dogfood metric (8/8 CLI commands green) is **met** as of develop
@ c59aab6. The single UX rough edge (F1, path prefix discoverability) is a
candidate enhancement, not a blocker.
