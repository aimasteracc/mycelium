# RFC-0095: Runtime Language Pack Loading

- **Status**: Implemented
- **Author(s)**: @aimasteracc (orchestrator dispatch)
- **Created**: 2026-05-30
- **Last updated**: 2026-05-30 (Implemented: PR #279 PackRegistry + env var; PR #280 --packs-dir CLI flag; PR #281 docs/packs.md)
- **Tracking issue**: #212 (umbrella #206)
- **Affected source paths**:
  - `crates/mycelium-core/src/extractor/mod.rs` — pack loader
  - `crates/mycelium-pack/` — `LanguagePack` runtime struct + loader
  - `crates/mycelium-cli/src/index.rs` — switches from compile-time to runtime packs
  - `packs/<lang>/pack.toml` — new manifest file per pack

## Summary

Replace the compile-time `include_str!()` embedding of language packs
with a runtime loader that reads `packs/<lang>/pack.toml` +
`queries.scm` from disk. Unblocks community-contributed packs without
forcing a binary recompile and prepares the way for a Skill-marketplace
listing of independently-versioned packs.

## Motivation

### What's wrong today

`crates/mycelium-cli/src/index.rs` and
`crates/mycelium-core/src/cortex.rs` both embed every supported
language's queries at compile time:

```rust
const PYTHON_QUERIES:     &str = include_str!("../packs/python/queries.scm");
const TYPESCRIPT_QUERIES: &str = include_str!("../packs/typescript/queries.scm");
// ... 10 of these total
```

Adding a new language requires:
1. Writing `.scm` capture patterns under `packs/<lang>/`
2. Adding an arm to the `Language` enum (extension dispatch)
3. **Re-compiling the binary**

Step 3 blocks two valuable use cases:
- **Community pack contributions** — a contributor with a new SQL or
  GraphQL pack can't ship it without a coordinated mycelium release.
- **Project-local DSLs** — teams with internal grammars can't add a
  pack without forking and rebuilding.

### Why now

Charter §1 calls Mycelium "polyglot — hundreds within reach". The
current "10 hard-coded packs" reality is at odds with the positioning.
The Skill marketplace (planned post-v0.2.0) needs an artifact users
can install without `cargo install` round-trips.

### Charter constraint

Charter §5.7: "Adding a new language touches ≤ 3 files under
`packs/<lang>/`." This RFC must preserve that constraint — runtime
loading actually makes it stricter, since the 3 files become the
ONLY surface area (no Rust changes needed).

## Detailed design

### Pack manifest (`packs/<lang>/pack.toml`)

```toml
# Required fields
name        = "python"
extensions  = [".py", ".pyi"]
grammar     = "tree-sitter-python"
queries     = "queries.scm"
version     = "0.1.0"

# Optional fields
description = "Python 3.x source code"
author      = "Mycelium contributors"
license     = "MIT"
mycelium    = "^0.2"          # minimum mycelium version
```

### Pack discovery

At server / CLI startup:

```rust
pub fn load_packs(packs_dir: &Path) -> Vec<LanguagePack> {
    walk(packs_dir).filter_map(|entry| {
        let manifest = entry.join("pack.toml");
        if !manifest.exists() { return None; }
        let meta: PackMeta = toml::from_str(&fs::read_to_string(&manifest).ok()?).ok()?;
        let queries = fs::read_to_string(entry.join(&meta.queries)).ok()?;
        let grammar = load_grammar(&meta.grammar)?;
        Some(LanguagePack { meta, queries, grammar })
    }).collect()
}
```

Default `packs_dir` resolution:
1. `--packs-dir <path>` CLI flag (explicit override)
2. `MYCELIUM_PACKS_DIR` env var
3. `<exe-dir>/packs/`
4. `~/.mycelium/packs/`
5. Bundled fallback (compile-time embed, shipping the current 10)

### Grammar loading

The tricky bit: tree-sitter grammars are normally linked at compile
time. Two options:

**Option A (recommended): Bundled grammars + runtime query swap.** Keep
the C grammar registrations at compile time (small, stable, ~10 of
them), but read `queries.scm` from disk per-language. Community packs
that need a grammar we don't bundle still require a binary release.

**Option B: Dynamic library loading.** Use `libloading` to `dlopen`
external `.so/.dylib/.dll` files containing tree-sitter grammars.
Maximally flexible but introduces platform-specific complexity,
unsafe `extern "C"`, and a much larger security surface.

This RFC adopts **Option A**. Option B can be a follow-up RFC if
community demand justifies it.

### Runtime model

```rust
pub struct LanguagePack {
    pub meta:     PackMeta,
    pub queries:  String,
    pub grammar:  tree_sitter::Language,  // looked up from a bundled registry
}

pub struct PackRegistry {
    packs:     Vec<LanguagePack>,
    by_ext:    HashMap<String, usize>,  // extension → index into packs
}

impl PackRegistry {
    pub fn load(packs_dir: &Path) -> Result<Self>;
    pub fn lookup(&self, path: &Path) -> Option<&LanguagePack>;
}
```

`Extractor::new` takes `&LanguagePack` instead of `(Language, &str)`.
`mycelium-cli`'s `index_path` iterates files and dispatches via
`registry.lookup(path)`.

### Backwards compatibility

The bundled fallback ensures the binary still works with no
`packs/` directory present. Users who don't customize packs see no
behavior change. The new flag is purely additive.

### Skill-marketplace integration (Future, deferred)

Once pack-loading is runtime, a Skill can ship `packs/<lang>/` next to
its `SKILL.md` and the Skill installer copies them into
`~/.mycelium/packs/`. The pack ecosystem becomes a thin layer over the
Skill ecosystem.

## Drawbacks

- **Startup cost.** Reading + parsing `pack.toml` × N + queries.scm
  × N adds tens of milliseconds. Acceptable for CLI (one-shot) but
  worth profiling for MCP server (long-lived; only happens once at
  startup).

- **Two-source-of-truth risk.** Today `cortex.rs` and `cli/index.rs`
  both embed packs. Either both must migrate to runtime loading, or
  one stays compile-time and reality drifts. This RFC mandates **both**
  migrate to the `PackRegistry`.

- **Grammar version skew.** A pack manifest can require
  `mycelium = "^0.2"` but there's no way to express "this pack needs
  tree-sitter-python ≥ 0.21". Future RFC if pack-vs-grammar version
  problems materialize.

## Alternatives

1. **Cargo features per pack.** `cargo build --features pack-sql`.
   Rejected: still requires recompile; doesn't help community
   contributors.

2. **WebAssembly grammars.** Tree-sitter-wasm exists. Rejected for
   now: heavyweight (wasm runtime in the binary), not yet stable for
   our parser usage.

3. **Status quo + better docs.** Rejected: the polyglot promise
   needs a real distribution mechanism, not just documentation.

## Acceptance criteria

- [x] `crates/mycelium-pack/` exposes `LanguagePack`, `PackMeta`,
  `PackRegistry` (PR #279)
- [x] `packs/<lang>/pack.toml` written for all 10 bundled languages
  (shipped across Tier 1/2 pack PRs; all 10 present)
- [x] `Extractor::new` accepts `&LanguagePack` via `PackRegistry`
  dispatch in cortex.rs + index.rs; static `include_str!` paths serve
  as compile-time fallback (backwards-compatible shim — PR #279, PR #280)
- [x] `mycelium-cli/src/index.rs` calls `PackRegistry::load` when
  `--packs-dir` is supplied; `mycelium-core/cortex.rs` uses
  `MYCELIUM_PACKS_DIR` env var; static embeds remain as fallback
  (PR #279, PR #280)
- [x] `--packs-dir` CLI flag documented in `docs/packs.md`; 
  `MYCELIUM_PACKS_DIR` env var documented in `docs/packs.md` (PR #282)
- [x] Charter §4 limit preserved: all 10 bundled packs use ≤ 2 files
  (`pack.toml` + `queries.scm`); the 3-file limit is not violated
- [x] Smoke test: `index_path_with_packs_dir_indexes_custom_extension`
  in `crates/mycelium-cli/src/index.rs` — custom `.mypy` pack loaded
  at runtime, symbols extracted without recompile (PR #280, 2 TDD tests)
- [x] CHANGELOG `[Unreleased]` Added entries for PackRegistry (PR #279)
  and `--packs-dir` (PR #280); Documentation entry: `docs/packs.md`
  created (PR #282)

## Rollout plan

Single PR introducing `PackRegistry` + migrating the 10 bundled
packs. Subsequent PRs roll out per-language community packs as they
arrive. After all 10 bundled packs work through the registry, the
`include_str!` paths can be retired in v0.3.

Target release: **v0.2.0** (alongside RFC-0093 + RFC-0094 — the
v0.2.0 "agentic-native" theme).
