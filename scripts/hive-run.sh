#!/usr/bin/env bash
# hive-run.sh — entry point for autonomous Hive invocations
#
# Usage: hive-run.sh <role> <trigger>
#   role:    pm | architect | spec-author | test-author | rust-implementer
#            | pack-author | reviewer | doc-sync | bench | security
#            | release | triage | orchestrator
#   trigger: daily-standup | hourly-sweep | nightly | webhook | manual | ...
#
# Invoked by launchd plists in .hive/launchd/, or manually for testing.
#
# Mandatory gates:
#   1. Check Hive kill switch (issue #1 on GitHub). If closed, exit 0.
#   2. Check per-day token budget (currently not enforced; Claude Code Max).
#   3. Pre-flight: cd to repo root, fetch latest, sync develop.
#   4. Invoke Claude Code with the role's system prompt loaded.
#   5. Post-flight: append audit log entry.

set -euo pipefail

ROLE="${1:?role required}"
TRIGGER="${2:?trigger required}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

NOW_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
TODAY_UTC="$(date -u +%Y-%m-%d)"
AUDIT_FILE=".hive/audit/${TODAY_UTC}.jsonl"
LOCK_DIR=".hive/run/.locks"
RUN_LOG_DIR=".hive/run"

mkdir -p "$LOCK_DIR" "$RUN_LOG_DIR" .hive/audit .hive/scratch

# ─────────────────────────────────────────────────────────────
# 1. Kill-switch check
# ─────────────────────────────────────────────────────────────

KILL_STATE="unknown"
if command -v gh >/dev/null 2>&1; then
    KILL_STATE="$(gh issue view 1 --json state --jq .state 2>/dev/null || echo "missing")"
fi

if [ "$KILL_STATE" = "CLOSED" ]; then
    echo "{\"ts\":\"$NOW_UTC\",\"role\":\"$ROLE\",\"trigger\":\"$TRIGGER\",\"event\":\"halted_by_kill_switch\"}" >> "$AUDIT_FILE"
    echo "Hive kill switch (issue #1) is CLOSED. Exiting."
    exit 0
fi

# ─────────────────────────────────────────────────────────────
# 2. Single-instance lock per (role, trigger)
# ─────────────────────────────────────────────────────────────

LOCK_FILE="$LOCK_DIR/${ROLE}.${TRIGGER}.lock"
exec 200>"$LOCK_FILE"
if ! flock -n 200; then
    echo "{\"ts\":\"$NOW_UTC\",\"role\":\"$ROLE\",\"trigger\":\"$TRIGGER\",\"event\":\"skipped_lock_held\"}" >> "$AUDIT_FILE"
    echo "Another instance of $ROLE:$TRIGGER is running. Exiting."
    exit 0
fi

# ─────────────────────────────────────────────────────────────
# 3. Pre-flight: sync develop
# ─────────────────────────────────────────────────────────────

if [ -d .git ]; then
    git fetch origin >/dev/null 2>&1 || true
    # Stay on develop for non-destructive read operations; do not switch
    # branches if we are mid-feature work.
    CURRENT_BRANCH="$(git branch --show-current)"
    if [ "$CURRENT_BRANCH" = "develop" ]; then
        git reset --hard origin/develop >/dev/null 2>&1 || true
    fi
fi

# ─────────────────────────────────────────────────────────────
# 4. Compose the agent prompt and invoke Claude Code
# ─────────────────────────────────────────────────────────────

AGENT_BRIEF=".hive/${ROLE}.agent.md"
if [ ! -f "$AGENT_BRIEF" ]; then
    echo "{\"ts\":\"$NOW_UTC\",\"role\":\"$ROLE\",\"trigger\":\"$TRIGGER\",\"event\":\"missing_brief\",\"path\":\"$AGENT_BRIEF\"}" >> "$AUDIT_FILE"
    echo "Missing agent brief at $AGENT_BRIEF — aborting."
    exit 1
fi

PROMPT_FILE=".hive/scratch/${ROLE}.${TRIGGER}.${TODAY_UTC}.prompt.md"
cat > "$PROMPT_FILE" <<EOF
You are the **${ROLE}** agent of the Mycelium Hive, triggered by **${TRIGGER}** at ${NOW_UTC}.

Your role brief is at \`${AGENT_BRIEF}\`. Read it now.
Your project charter is at \`CHARTER.md\`. Read it if not in context.
Your orchestration protocol is at \`.hive/_orchestrator.md\`. Read it.
Your shared memory is at \`.hive/memory/\`. Index at \`.hive/memory/INDEX.md\`.

Perform the mandatory pre-flight:
1. Read CHARTER.md and your role brief.
2. Read \`.hive/memory/INDEX.md\`.
3. Grep \`.hive/memory/anti-patterns.jsonl\` for the domain you are about to touch.
4. Identify the governing RFC, if any.
5. Confirm branch policy (never push to main/develop).

Then execute the trigger: **${TRIGGER}**.

After your work, perform the mandatory post-flight:
1. Append to \`.hive/memory/decisions.jsonl\` (if applicable).
2. Append to \`.hive/memory/anti-patterns.jsonl\` (if applicable).
3. Append to \`.hive/memory/lessons.jsonl\` (if applicable).
4. Append to \`.hive/audit/${TODAY_UTC}.jsonl\`:
   {"ts":"...","agent":"${ROLE}","trigger":"${TRIGGER}","actions":[...],"outcomes":[...]}

Wall-clock limit: 30 minutes. If you cannot finish, commit what works,
write a continuation file, and log the pause to the audit log.

Begin.
EOF

# Record start in audit
echo "{\"ts\":\"$NOW_UTC\",\"role\":\"$ROLE\",\"trigger\":\"$TRIGGER\",\"event\":\"started\",\"prompt\":\"$PROMPT_FILE\"}" >> "$AUDIT_FILE"

# Invoke Claude Code in headless mode with a 30-minute timeout
if command -v claude >/dev/null 2>&1; then
    timeout 1800 claude \
        --print \
        --dangerously-skip-permissions \
        --permission-mode acceptEdits \
        < "$PROMPT_FILE" \
        > "$RUN_LOG_DIR/${ROLE}.${TRIGGER}.${TODAY_UTC}.out.log" 2>&1
    RC=$?
elif command -v claude-code >/dev/null 2>&1; then
    timeout 1800 claude-code \
        --print \
        < "$PROMPT_FILE" \
        > "$RUN_LOG_DIR/${ROLE}.${TRIGGER}.${TODAY_UTC}.out.log" 2>&1
    RC=$?
else
    echo "{\"ts\":\"$NOW_UTC\",\"role\":\"$ROLE\",\"trigger\":\"$TRIGGER\",\"event\":\"missing_claude_code\"}" >> "$AUDIT_FILE"
    echo "claude (Claude Code) not in PATH — cannot invoke agent."
    exit 1
fi

END_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# Record completion in audit
echo "{\"ts\":\"$END_UTC\",\"role\":\"$ROLE\",\"trigger\":\"$TRIGGER\",\"event\":\"finished\",\"exit_code\":$RC}" >> "$AUDIT_FILE"

# Clean up scratch prompt (keep the run log)
rm -f "$PROMPT_FILE"

exit "$RC"
