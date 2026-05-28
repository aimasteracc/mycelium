# Mycelium · Project Charter

> This is the constitution of the Mycelium project. Every contributor, human
> or AI, is expected to have read it. Every decision must trace back to a
> clause here, or to an RFC that explicitly amends one.

**Version:** 1.0  
**Status:** Active  
**Last amended:** 2026-05-28  
**Amendment process:** Open a `meta` RFC.

---

## 1. Identity

| | |
|---|---|
| Project name | Mycelium |
| Query language | Hyphae |
| Repository | https://github.com/aimasteracc/mycelium |
| License | MIT |
| Founder / BDFL | [@aimasteracc](https://github.com/aimasteracc) |
| AI Hive operator | Claude (Anthropic) — Claude Code Max 20× |
| Sponsorship | https://github.com/sponsors/aimasteracc |
| One-line mission | A reactive, AI-native symbol graph that perceives code like a nervous system. |

## 2. Performance SLA (the contract)

Every release must meet or beat these numbers on a 100k-node graph, otherwise
it does not ship. CI gates them.

| Metric | Target |
|---|---|
| Cold small query (single symbol lookup) | < 5 ms |
| 3-hop graph traversal (callers, depth 3) | < 1 ms |
| Reactive re-query after file change | < 10 ms |
| AI token efficiency (Hyphae DSL vs JSON) | ≤ 30% of JSON token count for the same payload |
| New language onboarding | ≤ 3 files, 0 core-code lines changed |
| Public API documentation coverage | 100% of pub items have rustdoc |
| Test coverage (line) | ≥ 90% |
| Test coverage (branch) | ≥ 80% |
| Mutation testing kill rate | ≥ 70% |
| Fast-lane CI duration | < 5 min |
| Full-lane CI duration | < 20 min |

## 3. Tech Stack (locked)

| Layer | Choice | Why |
|---|---|---|
| Engine | Rust 2024 edition | Performance, embeddability, ecosystem |
| Parser | tree-sitter + declarative `.scm` queries | Language extensibility without core changes |
| Reactivity | Salsa 3 | Same proven foundation as rust-analyzer |
| Storage | Self-built: trunk (radix trie) + synapse (CSR) + Apache Arrow columnar attrs | See RFC-0001 |
| Persistence | Single-file `.myc`: WAL + periodic snapshot; HAMT structural sharing | Time-travel queries free |
| MCP / CLI | One Rust binary, multiple subcommands | Three faces, one engine |
| Bindings | napi-rs (npm) + maturin/pyo3 (PyPI) | Reach both ecosystems |
| Unit/integration test | `cargo test` + `insta` (snapshot) + `proptest` (property) | Industry default |
| Bench | `criterion` + `iai` | Statistical + instruction-level regression detection |
| Fuzz | `cargo-fuzz` (libFuzzer) | Parser robustness |
| Coverage | `cargo-llvm-cov` | Most accurate |
| Lint | `rustfmt` + `clippy::pedantic` + `cargo-deny` + `cargo-audit` | Zero warnings on default config |
| Mutation | `cargo-mutants` | Catch meaningless tests |
| Docs | `cargo doc` + `mdbook` + `markdownlint` | Code↔docs two-way contract |
| CI | GitHub Actions | Free, sufficient |
| Release | `release-plz` | Automated changelog + tag + publish |

## 4. Language Support Matrix

| Tier | Languages | Milestone |
|---|---|---|
| Tier 1 | Python, TypeScript, JavaScript, Rust, Go | v0.1 |
| Tier 2 | Java, C, C++, C#, Ruby | v0.5 |
| Tier 3 | Swift, Kotlin, PHP, Lua, Bash | v1.0 |
| Tier 4 | Dart, Scala, Elixir, OCaml, Zig, … | community-led, post-v1 |

**Hard constraint:** Each new language pack consists of at most 3 files under
`packs/<lang>/`: `pack.toml`, `queries.scm`, optional `hooks.wasm`. Core code
changes for language onboarding require a passing RFC.

## 5. The Twelve Commitments

These map 1:1 to the founder's original requirements. Each is the contract
governing one operating dimension of the project.

### §5.1 — Spec-driven, TDD-driven

- Every feature begins as an RFC in `rfcs/`.
- Tests are written before implementation. CI fails any PR adding implementation lines without a corresponding test diff.
- See `rfcs/0000-template.md` for the RFC template.

### §5.2 — AI Hive: PM + Specialist Agents

- The development team is defined in `.hive/`.
- Each agent has a markdown spec, a role, and a set of bound Claude skills.
- The orchestration protocol is in `.hive/_orchestrator.md`.

### §5.3 — Cross-session continuity, self-evolution, shared memory

- `.hive/memory/` is append-only JSONL.
- Every agent reads memory before acting, writes after acting.
- `anti-patterns.jsonl` is checked before any non-trivial decision.
- Weekly PM reflection consolidates anti-patterns into lessons.

### §5.4 — Code quality

- 100% rustfmt-clean.
- 0 clippy warnings on `--all-targets --all-features -- -D warnings`.
- 90%+ line coverage, 80%+ branch coverage.
- ≥ 70% mutation kill rate. Tests must change behavior, not just exercise code.
- Coverage exemptions require an inline `// coverage:skip <reason>` and reviewer sign-off.
- E2E tests in `tests/e2e/` run against real repositories nightly.
- 0 high-severity security findings on `cargo-audit` and `cargo-deny`.

### §5.5 — Docs ↔ code two-way contract

- Public items must have rustdoc with at least one doctest where applicable.
- Each RFC pins the source paths it governs. CI warns on drift.
- mdbook at `docs/` is the canonical user-facing documentation site.
- ADRs live at `docs/adr/`, MADR template.

### §5.6 — Efficient CI

- Fast lane: rustfmt + clippy + unit tests, under 5 minutes.
- Full lane: matrix (linux/macos/windows × stable/nightly) + coverage + e2e, under 20 minutes.
- Nightly: 1h fuzz + full benchmark + cross-platform e2e.
- Release lane: `release-plz` + cross-ecosystem publishing.
- Single quality gate aggregation as the only required branch-protection check.

### §5.7 — GitFlow

- `main` is protected; only release-plz tag commits land there.
- `develop` is protected; all PRs target it.
- `feature/RFC-XXXX-*`, `release/vX.Y.Z`, `hotfix/issue-XXXX-*` follow tree-sitter-analyzer's pattern.
- Conventional Commits enforced.
- See [GITFLOW.md](GITFLOW.md) for the full procedure.

### §5.8 — MIT + sponsorship

- License is MIT, forever. No re-licensing.
- Sponsors are recognized in `SPONSORS.md` and on the docs site.
- Gold-tier+ sponsors get logo placement; all sponsors get name credit.

### §5.9 — Tech stack

- See §3 above. Locked. Amendments require RFC.

### §5.10 — Contributor flow

- Direct push to `main` and `develop` is forbidden for every actor, human or AI.
- PRs target `develop`.
- Two approving reviews required; at least one human or BDFL override.
- PR template at `.github/PULL_REQUEST_TEMPLATE.md` is mandatory.
- We use **DCO**, not CLA. `Signed-off-by:` on every commit.

### §5.11 — Additional commitments the founder may not have specified

These are added by the AI architect with the founder's blessing:

- Semantic versioning, `CHANGELOG.md` in Keep-a-Changelog format.
- Reproducible builds: `Cargo.lock` in repo, pinned toolchain via `rust-toolchain.toml`.
- Dev container (`.devcontainer/`) for one-command onboarding.
- Privacy: telemetry is **off by default**, opt-in only. Code content is never transmitted. See [PRIVACY.md](PRIVACY.md).
- Trademark policy: "Mycelium" name and logo usage governed by `assets/TRADEMARK.md`.
- Performance SLA is publicly tracked and CI-gated (see §2).
- Dogfooding: Mycelium indexes itself; CI runs Hyphae queries against the Mycelium codebase as part of e2e.
- Code signing: releases signed via Sigstore; npm packages have provenance.
- Snapshot tests for query results via `insta`.
- Public roadmap on GitHub Projects.
- Multi-arch releases: Linux x86_64/aarch64, macOS universal, Windows x86_64.
- Triage SLA: new issues auto-triaged within 24 hours by the triage agent, human review within 48 hours.
- Bus-factor mitigation: at least two maintainer accounts hold continuous credentials.

### §5.12 — 24/7 autonomous development

- The Hive runs on the founder's local Mac Pro via `launchd` timers.
- Backed by Claude Code Max 20× (no per-call API cost; subject to session windows).
- Safety rails are non-negotiable:
  - Per-PR autonomous iteration cap: **3 rounds**, then escalate to human.
  - Per-agent wall-clock limit: **30 minutes**, then SIGTERM and log.
  - Any operation touching `main` requires a GPG-signed founder approval.
  - High-risk classes of change (schema migration, public API breakage, license, security model) are **always** escalated to founder.
  - Full audit log: `.hive/audit/YYYY-MM-DD.jsonl`, public.
  - Kill switch: closing issue `#1 — Hive kill switch` halts all autonomous activity within 60 seconds.
- The founder's role is **auditor + decision-maker**, not coder. Daily review of audit log, weekly review of anti-patterns, monthly direction calibration.

## 6. Governance Model

- **BDFL**: aimasteracc. Final authority. Can veto any RFC, block any merge.
- **Maintainers**: humans or AI agents with merge rights. Initially: BDFL + Reviewer agent. Expanded by BDFL decree.
- **Hive Agents**: defined in `.hive/`. Each has constrained authority; the Orchestrator enforces.
- **Contributors**: anyone who opens an issue, PR, RFC, or discussion. Bound by [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## 7. Decision Records

- Anything irreversible or strategic gets an **ADR** under `docs/adr/`.
- Anything affecting public API or storage format gets an **RFC** under `rfcs/`.
- Anything procedural (release, hotfix, post-mortem) gets a numbered entry in `.hive/memory/decisions.jsonl`.

## 8. Honest Limitations the Charter Acknowledges

- "Global #1" is a benchmark goal, not a starting state. Each release must close gaps; v1.0 must satisfy §2 in full.
- "All languages" is conditional on a maintained tree-sitter grammar existing for that language. Architecture can host it; quality depends on upstream.
- Autonomous development is 90% leverage, not 100% replacement. The founder remains the gatekeeper for trust-critical decisions.
- MIT-with-DCO means we cannot sell the project to a single company. If that path is desired later, this charter must be amended **before** non-trivial contributions are accepted under DCO, not after.

## 9. Amendment

This charter is amended via a `meta` RFC. Founder must approve. Amendments take effect on merge to `main`.

---

*This document is the soil from which the mycelium grows.*
