# scripts/

Local utilities for contributors and the Hive runtime.

| Script | Purpose |
|---|---|
| `hive-run.sh` | Entry point for autonomous Hive agent invocations. Called by `launchd` plists. |
| `install-hive-launchd.sh` | Install / uninstall / status of the macOS launchd jobs that run the Hive. |
| `install-hooks.sh` | Install local git pre-commit and commit-msg hooks. Run once after cloning. |
| `release-prep.sh` | Bump CHANGELOG.md and generate `.release-notes.md` before a release branch push. |
| `check_governance_guardrails.sh` | Verify release/admin-merge guardrails remain documented in GitFlow, PR, and agent runbooks. |

## First-time contributor setup

```bash
git clone https://github.com/<your-fork>/mycelium.git
cd mycelium
scripts/install-hooks.sh
cargo build
```

## First-time founder setup (Mac Pro)

```bash
git clone git@github.com:aimasteracc/mycelium.git
cd mycelium
scripts/install-hooks.sh
scripts/install-hive-launchd.sh install
```

After this, the Hive runs on schedule (PM at 09:00, Triage hourly, Bench at 02:00).
Close GitHub issue `#1 — Hive kill switch` to halt within 60 seconds.

## Releasing

```bash
git checkout -b release/v0.1.0 origin/develop
cargo set-version --workspace 0.1.0
scripts/release-prep.sh 0.1.0
git add -A
git commit -s -m "chore(release): v0.1.0"
git push origin release/v0.1.0
# CI handles the rest (publish registries → main/develop merge → tag → GitHub Release)
```

## Repairing an incomplete release

Treat an incomplete public release as an incident, not as normal release work.
A GitHub tag or Release page is only one signal; completion means the full
four-step ceremony in `GITFLOW.md` and Charter §5.12 has landed.

Checklist:

```bash
git fetch origin --prune
git merge-base --is-ancestor vX.Y.Z origin/main
git merge-base --is-ancestor vX.Y.Z origin/develop

for crate in \
  mycelium-rcig-pack \
  mycelium-rcig-core \
  mycelium-rcig-hyphae \
  mycelium-rcig-mcp \
  mycelium-rcig-cli; do
  cargo search "$crate" --limit 1 | head -n 1
done
```

If any branch or registry check is missing, stop and update the release repair
issue. Do not delete or retarget public tags/releases without explicit founder
approval. Fix automation and credentials first, then prefer a superseding
patch version when a public artifact already exists.
