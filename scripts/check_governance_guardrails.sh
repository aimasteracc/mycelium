#!/usr/bin/env bash
# Static checks for the human-facing governance rules that protect releases.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

require_literal() {
    local file="$1"
    local needle="$2"
    local why="$3"

    if ! grep -Fq "$needle" "$file"; then
        echo "Missing governance guardrail in ${file#$ROOT/}: $needle"
        echo "Why it matters: $why"
        exit 1
    fi
}

require_literal \
    "$ROOT/GITFLOW.md" \
    "### Admin Merge Override Protocol" \
    "Admin merge must be a documented BDFL override of the review gate, never a CI bypass."

require_literal \
    "$ROOT/GITFLOW.md" \
    "Admin merge may bypass a review-count gate only after Quality Gate is green." \
    "The v0.1.x release history already recorded red-CI admin merges as an anti-pattern."

require_literal \
    "$ROOT/GITFLOW.md" \
    "### Incomplete Release Incident Response" \
    "A tag or GitHub Release can exist even when registry publish or branch sync failed."

require_literal \
    "$ROOT/GITFLOW.md" \
    "A public tag or GitHub Release is not proof that a release completed." \
    "Release completion is the four-step ceremony, not a single GitHub artifact."

require_literal \
    "$ROOT/.github/PULL_REQUEST_TEMPLATE.md" \
    "## BDFL Override / Admin Merge" \
    "PR authors and reviewers need an explicit place to record override evidence."

require_literal \
    "$ROOT/.github/PULL_REQUEST_TEMPLATE.md" \
    "Quality Gate is green before override." \
    "Override permission must not normalize merging red CI."

require_literal \
    "$ROOT/.hive/release.agent.md" \
    "## Release Completion Invariant" \
    "The release agent must treat the four-step ceremony as one atomic obligation."

require_literal \
    "$ROOT/scripts/README.md" \
    "## Repairing an incomplete release" \
    "Operators need a local runbook for partial public releases."

echo "Governance guardrails documented."
