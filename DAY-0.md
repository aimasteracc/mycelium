# Day-0 Bootstrap Guide

> What's in this folder, why, and what to do next. Read this once. Then
> you can ignore it forever.

## What just got built

A complete, working day-0 project skeleton at `/Users/aisheng.yu/wiki/mycelium/`.
Everything in the [Charter](CHARTER.md) §1–§5.12 is realized as concrete files.
The Hive is defined, not yet running. The Rust code is not yet written — the
RFC (`rfcs/0001-trunk-and-synapse.md`) is the contract that the Test Author
agent will translate into failing tests, and the Rust Implementer will turn
green.

## File map

```
mycelium/
├── README.md, LICENSE, CHARTER.md, CLAUDE.md, DAY-0.md     ← face + constitution
├── GITFLOW.md, CONTRIBUTING.md, GOVERNANCE.md              ← how to work together
├── CODE_OF_CONDUCT.md, SECURITY.md, SUPPORT.md             ← community norms
├── CHANGELOG.md, PRIVACY.md, SPONSORS.md                   ← operational docs
│
├── Cargo.toml, rust-toolchain.toml, rustfmt.toml,          ← Rust workspace
│   clippy.toml, deny.toml, .commitlintrc.yml
├── .gitignore, .editorconfig                                ← VCS / editor hygiene
│
├── .github/
│   ├── FUNDING.yml, CODEOWNERS, dependabot.yml
│   ├── PULL_REQUEST_TEMPLATE.md
│   ├── ISSUE_TEMPLATE/{config,bug_report,feature_request,
│   │                   language_pack_request,rfc_proposal}.yml
│   └── workflows/{ci,nightly,release,triage,hive}.yml      ← CI = single Quality Gate
│
├── .hive/                                                   ← THE AI TEAM
│   ├── _orchestrator.md                                     ← coordination protocol
│   ├── pm.agent.md                                          ← daily standup
│   ├── architect.agent.md                                   ← RFC review, ADRs
│   ├── spec-author.agent.md                                 ← drafts RFCs
│   ├── test-author.agent.md                                 ← TDD: tests first
│   ├── rust-implementer.agent.md                            ← writes Rust
│   ├── pack-author.agent.md                                 ← new language packs
│   ├── reviewer.agent.md                                    ← PR review
│   ├── doc-sync.agent.md                                    ← docs↔code contract
│   ├── bench.agent.md                                       ← perf SLA guardian
│   ├── security.agent.md                                    ← vuln / dep / secrets
│   ├── release.agent.md                                     ← cuts releases
│   ├── triage.agent.md                                      ← issue/PR labeling
│   ├── memory/
│   │   ├── INDEX.md                                         ← curated map
│   │   ├── decisions.jsonl                                  ← append-only
│   │   ├── anti-patterns.jsonl                              ← "don't do this again"
│   │   ├── lessons.jsonl                                    ← consolidated learnings
│   │   └── glossary.jsonl                                   ← canonical terms
│   ├── launchd/                                             ← macOS schedulers
│   │   ├── com.mycelium.hive.daily.plist                    ← 09:00 PM standup
│   │   ├── com.mycelium.hive.hourly.plist                   ← triage sweep
│   │   └── com.mycelium.hive.nightly.plist                  ← 02:00 bench + security
│   └── audit/.gitkeep                                       ← daily JSONL audit logs
│
├── scripts/
│   ├── hive-run.sh                                          ← agent entry point
│   ├── install-hive-launchd.sh                              ← install/uninstall jobs
│   ├── install-hooks.sh                                     ← git pre-commit hooks
│   ├── release-prep.sh                                      ← changelog + release notes
│   └── README.md
│
├── rfcs/
│   ├── README.md                                            ← RFC process
│   ├── 0000-template.md                                     ← the template
│   └── 0001-trunk-and-synapse.md                            ← FIRST REAL RFC
│
└── docs/
    └── adr/
        ├── 0001-rust-as-engine-language.md
        ├── 0002-tree-sitter-as-parser.md
        └── 0003-mit-license-and-dco.md
```

## Numbers as of Day-0

| What | Count |
|---|---|
| Charter clauses formalized | 12 / 12 |
| Hive agents defined | 12 + 1 orchestrator |
| Memory files seeded | 5 (1 index + 4 JSONL) |
| GitHub workflows | 5 (ci, nightly, release, triage, hive) |
| Issue templates | 4 |
| ADRs | 3 |
| RFCs | 1 (drafted, ready for review) |
| ADRs locked at Day-0 | Rust as engine; tree-sitter; MIT + DCO |
| Performance SLA targets | 4 (Charter §2) |
| Language tier-1 packs | 0 (planned in v0.1, written under packs/) |

## What's NOT yet in the folder (because the Hive will write it)

- `crates/` — Rust source. Empty until Test Author writes failing tests for RFC-0001 §Testing strategy.
- `packs/` — language packs. Empty until Pack Author drafts Python and TS.
- `tests/`, `benches/`, `fuzz/` — test artifacts.
- `bindings/node/`, `bindings/python/` — language bindings.
- `docs/book/` — built mdbook output.

This is deliberate. The skeleton governs *how* code lands; the code lands
via Hive iterations as RFC-0001 (and successors) are implemented.

## Step-by-step: turn this folder into a live GitHub repo

### 1. Create the repo on GitHub (5 minutes)

Either via web UI or `gh`:

```bash
gh repo create aimasteracc/mycelium \
  --public \
  --description "Reactive code intelligence graph for AI agents. The wood-wide-web of your codebase." \
  --homepage "https://github.com/aimasteracc/mycelium" \
  --license MIT \
  --confirm
```

**Do NOT add a README/LICENSE through the UI** — we have our own.

### 2. Initialize and push from the local skeleton

```bash
cd ~/wiki/mycelium

# Sanity: confirm contents
ls -la

# Initialize
git init -b main
git add .
git commit -s -m "chore: day-0 project skeleton

Initial scaffold: charter, governance, GitFlow, Hive agent system,
RFCs/0000+0001, ADRs/0001-0003, GitHub workflows, launchd schedulers."

# Add remote
git remote add origin git@github.com:aimasteracc/mycelium.git

# Push main
git push -u origin main

# Create and push develop
git checkout -b develop
git push -u origin develop
```

### 3. Set branch protection (5 minutes via GitHub UI)

Go to **Settings → Branches**:

**For `main`:**
- ✅ Require pull request before merging
- ✅ Require approvals: **2** (1 must be a maintainer)
- ✅ Dismiss stale approvals when new commits pushed
- ✅ Require status checks: **Quality Gate**
- ✅ Require branches to be up to date
- ✅ Require signed commits
- ✅ Require linear history
- ✅ Restrict who can push: **Only @aimasteracc + release bot**
- ✅ Do not allow force pushes

**For `develop`:**
- ✅ Require pull request before merging
- ✅ Require approvals: **2**
- ✅ Require status checks: **Quality Gate**
- ✅ Require signed commits
- ✅ Do not allow force pushes

### 4. Create the kill switch issue (2 minutes)

This is **issue #1**. Mandatory. The Hive checks it before every run.

Title: `🍄 Hive kill switch — do not close unless you mean it`

Body (paste into issue):

```markdown
This issue controls the entire Hive's autonomous activity on this repository.

- **OPEN**: Hive is active. Scheduled agents run normally.
- **CLOSED**: Hive halts within 60 seconds. All scheduled invocations exit immediately.

To halt the Hive in an emergency, close this issue.
To resume, reopen it.

See [Charter §5.12](../blob/develop/CHARTER.md#512--247-autonomous-development) and [.hive/_orchestrator.md](../blob/develop/.hive/_orchestrator.md) for the kill-switch protocol.

⚠️ Do not close on a whim. Closing this halts the entire autonomous workflow.
```

Pin the issue.

### 5. Configure secrets (10 minutes)

In **Settings → Secrets and variables → Actions**:

- `CRATES_IO_TOKEN` — from https://crates.io/me (API token, scope: publish-new + publish-update)
- `NPM_TOKEN` — from https://www.npmjs.com (Automation type)
- (PyPI uses Trusted Publishers; configure on PyPI side instead)
- `RELEASE_BOT_TOKEN` — a fine-grained PAT for the release workflow to push to `main`/`develop`. Scope: **only this repo**, write contents.
- `CODECOV_TOKEN` (optional) — from https://codecov.io

In **Settings → Environments**, create:

- `crates-io` — protection rule: required reviewer @aimasteracc
- `npm` — protection rule: required reviewer @aimasteracc
- `pypi` — protection rule: required reviewer @aimasteracc

### 6. Install the Hive on your Mac Pro (5 minutes)

```bash
cd ~/wiki/mycelium
scripts/install-hooks.sh
scripts/install-hive-launchd.sh install
```

Verify:

```bash
scripts/install-hive-launchd.sh status
```

Expected output:

```
Hive launchd jobs status:
  ✓ com.mycelium.hive.daily : installed and loaded
  ✓ com.mycelium.hive.hourly : installed and loaded
  ✓ com.mycelium.hive.nightly : installed and loaded
```

The Hive is now scheduled. PM agent will trigger at 09:00, Triage hourly,
Bench nightly at 02:00. All triggers check the kill switch first.

### 7. First Hive activity (next morning, automatic)

09:00 the next morning, PM agent will:

1. Pre-flight (read Charter, role brief, memory).
2. Pull repo state (open issues, PRs, CI runs).
3. Write the first standup entry to `.hive/audit/<date>.jsonl`.
4. Comment on the rolling standup issue (which PM creates on first run).
5. Post-flight.

Review the audit log in the morning. Adjust the agent briefs as you learn.

## What you (the founder) do next

| When | What |
|---|---|
| Today | Run steps 1–6 above. ~30 minutes total. |
| Tomorrow | Check `.hive/audit/<today>.jsonl` and the rolling standup issue. |
| This week | Review RFC-0001, sign off (or request changes). Once accepted, the Test Author agent (you spawn it via `scripts/hive-run.sh test-author rfc-0001`) writes the failing tests. |
| Each morning (5 min) | Read audit log. |
| Each Monday (15 min) | Read weekly synthesis the PM agent posts. |
| First of month (30 min) | Direction calibration. |

That is **~10 hours/month of your time** for what is otherwise 24/7 autonomous progress.

## Reading order for anyone new (human or AI)

1. [README.md](README.md) — what is this?
2. [CHARTER.md](CHARTER.md) — what are the rules?
3. [CLAUDE.md](CLAUDE.md) — how does Claude Code behave here?
4. Your role in [.hive/](.hive/) — what is your job?
5. [.hive/_orchestrator.md](.hive/_orchestrator.md) — how do we coordinate?
6. [.hive/memory/INDEX.md](.hive/memory/INDEX.md) — what do we already know?

## When something goes wrong

| Symptom | First action |
|---|---|
| Hive seems stuck | Check `.hive/run/*.log` for the most recent agent invocation. |
| Bad PR from a Hive agent | Close it. Append an entry to `.hive/memory/anti-patterns.jsonl` describing the failure mode. |
| Runaway loop | Close issue #1 (kill switch). Investigate. |
| CI broken | Triage agent will surface; if not, check `.github/workflows/`. |
| Founder unavailable for > 7 days | Hive continues operating within safety rails. No new RFCs reach `accepted` without your sign-off. |

## Final note

> *This skeleton is a living thing. The Hive's first job is to grow it.*
>
> *Welcome to the network.* 🍄
