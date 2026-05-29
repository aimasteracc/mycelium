<div align="center">

# 🍄 Mycelium

**The wood-wide-web of your codebase.**

A reactive, AI-native code intelligence graph.
Your AI agent perceives your code like a nervous system perceives a body —
every change, instantly felt, instantly understood.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Status](https://img.shields.io/badge/status-alpha-blue.svg)](#)
[![Version](https://img.shields.io/badge/version-v0.1.0-green.svg)](CHANGELOG.md)
[![Rust](https://img.shields.io/badge/built_with-Rust-dea584.svg)](https://www.rust-lang.org/)
[![Sponsor](https://img.shields.io/badge/sponsor-aimasteracc-ea4aaa.svg?logo=github-sponsors)](https://github.com/sponsors/aimasteracc)

[Charter](CHARTER.md) · [GitFlow](GITFLOW.md) · [Contributing](CONTRIBUTING.md) · [Governance](GOVERNANCE.md) · [RFCs](rfcs/) · [Sponsors](SPONSORS.md)

</div>

---

## Why Mycelium

In a forest, trees do not communicate through their leaves. They communicate
underground through **mycelium** — a fungal network that exchanges signals,
nutrients, and warnings across the entire ecosystem. Scientists call it the
**Wood Wide Web**.

Your codebase is a forest. Today, AI agents read it leaf by leaf — `grep`,
`find`, `read`, repeat. They burn tokens, miss connections, and forget
everything between sessions.

**Mycelium is the network that lives beneath your code.** It is:

- **Reactive** — every edit propagates through the graph in milliseconds. No reindex. No staleness.
- **AI-native** — query results are serialized in a compact symbolic DSL, not JSON. AI gets 3–4× more meaning per token.
- **Polyglot** — 20+ languages on day one, hundreds within reach. Adding a new language touches ≤3 files. No core changes.
- **Local, embeddable, fast** — a single Rust binary. Sub-millisecond multi-hop queries. No server, no cloud.

## Status

**v0.1.0 — Alpha.** All Charter §2 performance SLAs satisfied. 835 tests passing. 96% coverage.

| Component | Status |
|---|---|
| Core engine (Trunk + Synapse + Cortex) | ✅ Shipped |
| Language packs: Python, TS, JS, Rust, Go | ✅ Tier 1 complete |
| Language packs: Java, C, C++, C#, Ruby | ✅ Tier 2 complete |
| MCP server (88 tools) | ✅ Shipped |
| Hyphae DSL (lexer + parser + evaluator) | ✅ RFC-0004 complete |
| CLI (`mycelium index`, `mycelium serve --mcp`) | ✅ Shipped |
| Persistence (MessagePack snapshot) | ✅ Shipped |
| Watch mode (reactive FSE re-index) | ✅ Shipped |
| npm / PyPI bindings | 🔜 v0.2 |

**Public roadmap:** [GitHub Projects](https://github.com/aimasteracc/mycelium/projects).
**Changelog:** [CHANGELOG.md](CHANGELOG.md).
**Charter:** [CHARTER.md](CHARTER.md) — read this before any contribution.

## Design Pillars

| Pillar | What it means |
|---|---|
| **Trunk** | Containment as a Materialized Path Radix Trie. Ancestor and descendant queries are string operations, not graph walks. |
| **Synapse** | Cross-cutting relationships (calls, extends, implements, the full UML set) as CSR-encoded adjacency lists, indexed per edge kind. |
| **Hyphae** | A CSS-selector-inspired query language with edge pseudo-classes. `class.AuthService > method:async:calls(UserRepo > method)`. |
| **Reactive Layer** | Salsa-style dependency tracking. File changes invalidate only the affected query subscriptions, not the whole graph. |
| **AI Serialization** | Emmet-like compact DSL output. ~70% fewer tokens than JSON for the same information. |
| **Batch API** | Multi-step query plans in a single MCP call. Computation pushed down, AI round-trips cut 5–20× to 1–3×. |

## Three Faces, One Engine

Mycelium ships as a single Rust binary exposing three faces, sharing one core:

```
┌─────────────────────────────────────────┐
│      mycelium-core (Rust crate)         │  ← the engine
└────────────────┬────────────────────────┘
                 │
   ┌─────────────┼─────────────┐
   ▼             ▼             ▼
 CLI          MCP Server     Claude Skill
 mycelium     mycelium       SKILL.md + bin
 query …      serve --mcp    + auto-config
```

Install once. Use from terminal, from your AI agent, or as a skill bundle.

## Quick Start

```bash
# Install from crates.io (the `mycelium-rcig-*` prefix is because the short names
# `mycelium-core` and `mycelium-cli` were taken by unrelated 2019/2025 projects):
cargo install mycelium-rcig-cli

# Or install latest from source:
cargo install --git https://github.com/aimasteracc/mycelium mycelium-rcig-cli

# Index a project (Python, TS, JS, Rust, Go, Java, C, C++, C#, Ruby)
mycelium index ./my-project

# Serve as MCP for your AI agent (Claude, GPT-4, etc.)
mycelium serve --mcp --root ./my-project
```

### Hyphae DSL — query the graph like CSS selects DOM

```bash
# via mycelium_query MCP tool:
# Find all functions that call AuthService
{ "query": "function:calls(#AuthService)" }

# Find all methods inside Foo class
{ "query": "#Foo > method" }

# Find callers of login, up to depth 3
{ "query": "*:callers(#login)" }
```

## Performance SLA (the bar we ship against)

| Metric | Target | Compared to |
|---|---|---|
| Cold small query | < 5 ms | SQLite-based code graphs ≈ 50 ms |
| 3-hop traversal | < 1 ms | Neo4j ≈ 10–30 ms |
| Reactive re-query after file change | < 10 ms | Full reindex on most tools |
| AI token efficiency | 25–30% of JSON | — |
| New language onboarding | ≤ 3 files, 0 core changes | codegraph-style: 5–10 files + core |

CI gates regressions on these numbers. If a release does not meet them, it does not ship.

## The Hive — Our Autonomous Development Team

Mycelium is developed by a team of AI agents under a single human founder.
The team is defined in [`.hive/`](.hive/):

| Agent | Role |
|---|---|
| PM | Roadmap, prioritization, daily standup |
| Architect | RFC review, ADRs |
| Spec Author | Drafts RFCs from issues |
| Test Author | TDD — tests precede implementation |
| Rust Implementer | Engine and CLI code |
| Pack Author | New language packs |
| Reviewer | PR review, lint, style, security |
| Doc Sync | Docs ↔ code consistency |
| Bench | Performance regression detection |
| Security | Vulnerabilities, dependencies, secrets |
| Release | Versioning, changelog, publishing |
| Triage | Issue labeling, duplicate detection |

The Hive shares persistent memory ([`.hive/memory/`](.hive/memory/)) so
lessons learned and decisions made stay across sessions. See
[`.hive/_orchestrator.md`](.hive/_orchestrator.md) for the protocol.

## Contributing

Pull requests are welcome but **must target `develop`**, never `main`.
Large changes go through the [RFC process](rfcs/README.md) first.

Start here: **[CONTRIBUTING.md](CONTRIBUTING.md)** · **[GITFLOW.md](GITFLOW.md)** · **[GOVERNANCE.md](GOVERNANCE.md)**

We use **DCO** (Developer Certificate of Origin), not a CLA. Add `Signed-off-by:`
to every commit and you are good.

## Sponsorship

Mycelium is MIT-licensed and developed by a small team backed by Anthropic's Claude.
If it saves your AI agent tokens, your engineers time, or your team frustration,
please consider [sponsoring](https://github.com/sponsors/aimasteracc).
See [SPONSORS.md](SPONSORS.md) for the current honor roll.

## License

[MIT](LICENSE) © aimasteracc and the Mycelium contributors.

---

<div align="center">
<sub>
Built with 🍄 by humans and AI, together.<br/>
<em>"Every neuron that fires, every leaf that trembles, every line of code that changes — all felt instantly."</em>
</sub>
</div>
