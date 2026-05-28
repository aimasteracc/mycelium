# 0001. Rust as the engine language

- **Status**: accepted
- **Date**: 2026-05-28
- **RFC**: founders' decision, predates RFC process

## Context

Mycelium's engine must:

- Embed cleanly in CLI binaries, MCP servers, npm packages (via napi-rs), and PyPI wheels (via pyo3).
- Run at C-level performance — Charter §2 requires < 5 ms cold queries and < 1 ms 3-hop traversal.
- Match tree-sitter's native bindings without WASM overhead.
- Compose well with Salsa, which is the reactivity framework chosen for RFC-0002.

## Decision

Implement the engine in **Rust** (2024 edition, MSRV 1.79).

## Consequences

### Positive

- Native performance with no FFI cost in the embedding language.
- Memory safety without GC; predictable latency.
- First-class integration with tree-sitter, Salsa, Apache Arrow Rust.
- Single binary distributable everywhere (musl static for Linux, codesigned macOS, MSVC Windows).
- Strong tooling: rustfmt, clippy, cargo-deny, cargo-audit, cargo-mutants, cargo-llvm-cov.

### Negative

- Higher contributor barrier than Python or TypeScript.
- Compile times longer than dynamic languages.
- Some platforms (e.g., older BSDs) have less Rust stdlib coverage.

### Neutral / Trade-offs

- Choosing Rust commits us to ahead-of-time compilation; we lose dynamic plugin loading at runtime (we accept this by using declarative pack.toml + queries.scm).

## Alternatives considered

### Alternative A: Go
- Pros: simpler concurrency, faster compile, easy cross-compile.
- Cons: GC pause violates < 1 ms traversal SLA; no Salsa equivalent; CSR-style tight memory layout harder.
- **Rejected**: GC + lack of Salsa is disqualifying.

### Alternative B: C++
- Pros: maximum performance, mature tree-sitter integration.
- Cons: no memory safety; Hive agents would need a strong C++ specialist; package distribution is painful across platforms.
- **Rejected**: safety, tooling, and contributor onboarding all worse than Rust.

### Alternative C: TypeScript / Node (like codegraph)
- Pros: lowest barrier, fastest prototyping, rich ecosystem.
- Cons: cannot meet Charter §2 SLAs; tree-sitter via WASM has 5-10× overhead vs native.
- **Rejected**: performance disqualifying.
