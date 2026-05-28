# Contributing to Mycelium

Welcome! 🍄 Mycelium grows because of people like you. This guide will get
you from "I want to help" to a merged PR in the shortest path possible.

## TL;DR

1. Read the [Charter](CHARTER.md) — it is short and important.
2. Find or open an [Issue](https://github.com/aimasteracc/mycelium/issues).
3. For non-trivial work, open or comment on an [RFC](rfcs/README.md) first.
4. Fork. Branch from `develop`. Write tests **first**. Implement. Sign every commit (`git commit -s`).
5. Open a PR against `develop` (not `main`). Wait for CI green and 2 approvals.
6. Welcome to the network. 🍄

## Required Reading (5 minutes)

- [CHARTER.md](CHARTER.md) — the constitution of the project.
- [GITFLOW.md](GITFLOW.md) — how branches and releases work.
- [GOVERNANCE.md](GOVERNANCE.md) — who decides what.
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) — how we treat each other.

## Ways to Contribute

### 🐛 Report a bug

Open a **Bug Report** issue. The template will ask for:

- Mycelium version (`mycelium --version`)
- OS and arch
- Reproducer (smallest input that triggers the issue)
- Expected vs actual behavior

### 💡 Propose a feature

Open a **Feature Request** issue first. Discuss before coding. If
the feature is non-trivial (changes public API, storage, or performance
profile), it becomes an RFC.

### 📜 Submit an RFC

Copy `rfcs/0000-template.md` to `rfcs/XXXX-your-title.md` and PR to
`develop`. Discussion happens in the PR. Once merged, the RFC is the
contract for whoever implements it.

### 🌐 Add a language pack

The single most welcome contribution. The hard constraint: **3 files max**
under `packs/<language>/`. Use [Language Pack Request](.github/ISSUE_TEMPLATE/language_pack_request.yml)
to coordinate, then submit:

```
packs/<your-lang>/
├── pack.toml       # required
├── queries.scm     # required
└── hooks.wasm      # optional, only for true language quirks
```

See `packs/python/` for the reference example.

### 📝 Improve documentation

Doc PRs are first-class. Tag your PR `docs:` and you can skip the RFC
process entirely.

### 🧪 Write tests

If you find untested code, write a test. Bonus points if you find a bug
along the way.

## Development Setup

### Prerequisites

- Rust ≥ 1.79 (auto-installed via `rust-toolchain.toml`)
- Git
- (optional) Node ≥ 20 if working on napi-rs bindings
- (optional) Python ≥ 3.10 if working on pyo3 bindings

### One-time setup

```bash
git clone https://github.com/<your-fork>/mycelium.git
cd mycelium
git remote add upstream https://github.com/aimasteracc/mycelium.git

# Install dev tools
cargo install cargo-llvm-cov cargo-deny cargo-audit cargo-mutants

# Install commit hooks
scripts/install-hooks.sh
```

### Daily flow

```bash
# Sync
git fetch upstream
git checkout develop
git rebase upstream/develop

# Branch
git checkout -b feature/RFC-XXXX-short-desc

# TDD cycle
cargo test <target>   # see red
# ... write impl ...
cargo test <target>   # see green

# Pre-push checks (must all pass)
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo llvm-cov --workspace --fail-under-lines 90
cargo deny check
cargo audit

# Push
git push -u origin feature/RFC-XXXX-short-desc
gh pr create --base develop --fill
```

## TDD is Mandatory

Per Charter §5.1, **tests are written before implementation**. CI checks
the PR diff: if you added implementation lines without adding corresponding
test lines, CI fails. Exceptions need a `// coverage:skip <reason>`
annotation and reviewer sign-off.

This is not a style preference. It is the contract.

## Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/).

```
feat(core): introduce trunk radix trie

Implements RFC-0001 §3.1. The trunk replaces the previous flat node
table with a path-encoded trie supporting O(prefix) ancestor and
descendant queries.

Signed-off-by: Jane Doe <jane@example.com>
```

**Every commit must end with `Signed-off-by:` (DCO).** Use `git commit -s`.

## DCO, not CLA

We use the [Developer Certificate of Origin](https://developercertificate.org/).
By signing off, you certify the standard four points (your contribution is
original, you have rights to submit it, etc.). No paperwork, no central
agreement. Your contribution remains yours; the project retains the right to
distribute it under MIT.

## PR Review Process

1. Open PR targeting `develop`.
2. CI runs the **Quality Gate** (single required check).
3. Reviewer agent (Hive) does first pass within 24h.
4. At least 1 human reviewer (BDFL or maintainer) does second pass.
5. Conversations resolved → 2 approvals → squash-merge.

**Expected response times:**

- First triage: 24h
- First review: 48h
- Merge or actionable feedback: 7 days for small PRs, 14 days for large

If we miss these, ping `@aimasteracc` or post in [Discussions](https://github.com/aimasteracc/mycelium/discussions).

## What Gets a PR Rejected

- ❌ Targets `main` instead of `develop`
- ❌ No tests (or coverage drop without justification)
- ❌ CI red on lint/format/security
- ❌ Public API change without an accepted RFC
- ❌ Adding a language by modifying core (violates Charter §4)
- ❌ Adds a forbidden dependency (SQLite, graph DBs, etc. — see [CLAUDE.md](CLAUDE.md) "Tool Preferences")
- ❌ Commits not DCO-signed
- ❌ Non-Conventional commit messages

## Getting Help

- **Quick question**: [Discussions](https://github.com/aimasteracc/mycelium/discussions)
- **Real-time chat**: (Discord/Matrix link — TBD before v0.1)
- **Security report**: see [SECURITY.md](SECURITY.md). Do not open a public issue.

## Recognition

Every contributor is listed in CHANGELOG.md under their PR. Sponsors are
recognized in [SPONSORS.md](SPONSORS.md). Top contributors get write
access to relevant areas (decided by BDFL).

---

*Thank you for growing the network.* 🍄
