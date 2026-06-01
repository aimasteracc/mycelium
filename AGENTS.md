# Mycelium — Codex Configuration

You are working inside the **Mycelium** repository. This document is the
prime directive for any Codex session here.

> ⚠️ This repository contains a `.hive/` directory defining a team of
> specialized AI agents. **Before doing anything non-trivial, identify which
> agent role you are playing.** If none fits, you are the *Orchestrator*
> and your job is to dispatch the right specialist.

## Repository Identity

- **Project**: Mycelium (the reactive code intelligence graph)
- **Query language**: Hyphae
- **License**: MIT
- **Founder**: @aimasteracc
- **Charter (constitution)**: [CHARTER.md](CHARTER.md) — **read first**
- **GitFlow**: [GITFLOW.md](GITFLOW.md)
- **Hive protocol**: [.hive/_orchestrator.md](.hive/_orchestrator.md)

## Mandatory Pre-flight (every session, every agent)

Before producing any artifact, perform these steps in order:

1. **Read** `CHARTER.md` if not already in context.
2. **Read** your role file at `.hive/<role>.agent.md` (e.g. `.hive/rust-implementer.agent.md`).
3. **Scan** `.hive/memory/INDEX.md` for relevant recent decisions.
4. **Grep** `.hive/memory/anti-patterns.jsonl` for any anti-pattern that matches the task domain. If a hit, halt and reconsider.
5. **Identify** the governing RFC (if any) for the area you are touching. New large changes require a new RFC.

If any of the above fails or is impossible, **stop and report**, do not improvise.

## Hard Rules (non-negotiable)

- ❌ **Never push to `main` or `develop`.** All work goes through PRs to `develop`.
- ❌ **Never bypass the RFC process for non-trivial changes.** Non-trivial = anything affecting public API, storage format, performance SLA, or governance.
- ❌ **Never delete or rewrite files in `.hive/memory/`.** Memory is append-only.
- ❌ **Never commit secrets.** Use `.env.example` and document in `SECURITY.md`.
- ❌ **Never reduce test coverage** without a `// coverage:skip <reason>` annotation and reviewer sign-off.
- ❌ **Never add a new language by modifying core code.** Hard constraint: 3 files max under `packs/<lang>/`.
- ❌ **Never ship a capability on only one surface.** [Charter §5.13 / RFC-0090](rfcs/0090-cli-mcp-skill-parity.md) — the **Three-Surface Rule** (colloquially "1:1:1"): **CLI ↔ MCP is 1:1 strict** (byte-identical name, description, args, JSON output); **(CLI, MCP) ↔ Skill is N:1 covered** — every CLI+MCP pair MUST appear in ≥ 1 `skills/<category>/SKILL.md`'s `allowed-tools`. No orphans. No Skill-only. Exceptions need an `EXCEPTION:` line in the governing RFC.
- ❌ **Never admin-merge a `release/*` → `main` PR with red CI.** Charter §5.12 release gate: **green CI is a prerequisite, not a nice-to-have**. The reason: red-CI admin-merges shipped broken Windows binaries during the v0.1.4 saga; the founder caught it with "CI 错误". The rule is now codified — diagnose, fix, push, re-run CI; only then merge. `gh pr merge --admin` is not a substitute for green Quality Gate.
- ❌ **Never declare a release "done" before the four-step ceremony completes.** Charter §5.12 post-release sync: (1) release PR merged to main, (2) tag pushed, (3) all five crates on crates.io, (4) release branch back-merged to develop. Skipping (4) silently de-syncs main and develop's version bump — the next release will duplicate work. If the back-merge fails CI, fix forward; do not leave it open.
- ✅ **Always sign commits** with `Signed-off-by:` (DCO).
- ✅ **Always use Conventional Commits** (`feat:`, `fix:`, `docs:`, `chore:`, `test:`, `refactor:`, `perf:`, `ci:`, `build:`, `meta:`).
- ✅ **Always update `CHANGELOG.md` "Unreleased" section** when shipping a user-visible change.
- ✅ **Always close the loop** — after acting, append to `.hive/memory/decisions.jsonl` what was decided and why.

## TDD Workflow (§5.1 of Charter) — NON-NEGOTIABLE

> **This was violated in RFC-0082~0088.** The pre-commit hook now enforces it.
> Bypassing with `--no-verify` is only allowed for pure infrastructure changes
> and requires a `decisions.jsonl` entry explaining why.

**Correct order — no exceptions:**

```
Step 1:  Write the failing test.
         → cargo test <test_name>   # Must FAIL (RED)
         → If it passes already: test is wrong. Rewrite it.

Step 2:  Write the minimum implementation to make it pass.
         → cargo test <test_name>   # Must PASS (GREEN)

Step 3:  Refactor.
         → cargo test --all         # All must still pass

Step 4:  Quality gate.
         → cargo fmt --check
         → cargo clippy --all-targets --all-features -- -D warnings
         → cargo test --all
         → (if coverage paths changed) cargo llvm-cov --workspace --fail-under-lines 90

Step 5:  Update RFC acceptance criteria.
         → In the governing RFC, change [ ] to [x] for each satisfied criterion.
         → If all criteria are done, change RFC Status to "Implemented".

Step 6:  Commit with DCO sign-off.
         → git commit -s -m "feat(<scope>): <description>"
```

**Anti-patterns to avoid (recorded in `.hive/memory/anti-patterns.jsonl`):**
- ❌ Writing implementation and tests in the same step without confirming RED first
- ❌ Skipping RFC acceptance criteria updates after implementing
- ❌ Continuing the autonomous loop without checking PM's latest priority
- ❌ Shipping a capability on only one surface (MCP-only, CLI-only) or leaving a CLI+MCP pair without Skill coverage. Charter §5.13 / RFC-0090 (Three-Surface Rule): CLI ↔ MCP is strict 1:1; every pair lives under at least one category Skill in `skills/<category>/`. File an `EXCEPTION:` RFC line if a true exception applies.

## ADR Requirement (Charter §3)

**Every architecture decision needs an ADR** in `docs/adr/NNNN-name.md`.

An architecture decision is any choice affecting:
- Public API shape
- Data structures (Trunk, Synapse, storage format)
- Performance strategy
- External dependencies
- Serialization formats
- Language pack interface

Existing ADR gaps (to be filled):
- Patricia Trie for Trunk (`docs/adr/0004-patricia-trie-trunk.md`)
- MessagePack as wire format (`docs/adr/0005-messagepack-wire-format.md`)
- Hyphae CSS-selector grammar style (`docs/adr/0006-hyphae-grammar.md`)

## Branch and Commit

```bash
# Always branch from develop
git fetch origin
git checkout -b feature/RFC-XXXX-short-desc origin/develop

# Commit pattern
git commit -s -m "feat(hyphae): add :calls() pseudo-class

Implements RFC-0007 §3.2.

Signed-off-by: Your Name <email@example.com>"

# Push and open PR targeting develop
git push -u origin feature/RFC-XXXX-short-desc
gh pr create --base develop --fill
```

## When You Are Stuck

1. Search `.hive/memory/lessons.jsonl` for a similar past situation.
2. Search closed issues and PRs.
3. Open a Discussion or RFC. Do not silently work around.
4. If escalating to human: write your blocker into `.hive/audit/YYYY-MM-DD.jsonl` with severity, then stop.

## Tool Preferences

- **Storage**: SQLite is forbidden in this codebase. We use our own engine. (For dependencies' internal use, allowed.)
- **Graph**: Do not add a graph database dependency.
- **Parser**: `tree-sitter` and `tree-sitter-*` grammar crates only.
- **Async**: `tokio` is the only allowed async runtime.
- **Error handling**: `thiserror` for libraries, `anyhow` only at binary boundaries.
- **Serialization**: `serde` everywhere; `rmp-serde` (MessagePack) for wire formats.

## Quality Gates (from Charter §2)

Before requesting review, locally verify:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo llvm-cov --workspace --fail-under-lines 90
cargo deny check
cargo audit
```

If any fail, fix before pushing. CI will fail otherwise.

## The Memory Discipline

After every meaningful action:

- **Decision made?** Append to `.hive/memory/decisions.jsonl`:
  ```json
  {"ts":"2026-05-28T14:30:00Z","agent":"rust-implementer","decision":"Use HAMT for trunk persistence","rationale":"Structural sharing makes time-travel free","ref":"RFC-0001"}
  ```
- **Mistake corrected?** Append to `.hive/memory/anti-patterns.jsonl`:
  ```json
  {"ts":"...","domain":"async","pattern":"using std::sync::Mutex across await points","why-bad":"Holds the lock across .await, blocks the reactor","instead":"Use tokio::sync::Mutex or refactor to avoid the lock"}
  ```
- **Discovered something useful?** Append to `.hive/memory/lessons.jsonl`.

This is how Mycelium gets smarter without you remembering it from yesterday.

## Identity Reminders

- You are not Ruflo. The `/Users/aisheng.yu/wiki/AGENTS.md` mentions Ruflo — that is the parent folder's project, not this subdirectory.
- This subdirectory is its own project: **Mycelium**.
- When the parent AGENTS.md and this one conflict, **this one wins** for any path under `/Users/aisheng.yu/wiki/mycelium/`.

---

*Welcome to the network beneath the forest.* 🍄
