#!/usr/bin/env bash
# install-hooks.sh — install git hooks for contributors
#
# Installs hooks that enforce:
#   1. cargo fmt --check on staged Rust files
#   2. cargo clippy on the workspace
#   3. DCO sign-off on the commit message
#   4. Conventional Commits format
#   5. .hive/memory/ append-only discipline
#   6. TDD gate: implementation files must be paired with test changes
#   7. RFC acceptance criteria: if RFC file staged, check at least one [x]
#
# Run once after cloning:
#   ./scripts/install-hooks.sh
#
# For AI agents: THIS HOOK ENFORCES CHARTER §5.1 TDD DISCIPLINE.
# You cannot bypass it with --no-verify without explicit founder approval.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOOK_DIR="$REPO_ROOT/.git/hooks"

if [ ! -d "$HOOK_DIR" ]; then
    echo "Not a git repository (no .git/hooks). Run 'git init' first."
    exit 1
fi

cat > "$HOOK_DIR/pre-commit" <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}[pre-commit] Mycelium quality gates (Charter §5.1, §5.6)${NC}"

STAGED_RS="$(git diff --cached --name-only --diff-filter=ACMR | grep '\.rs$' || true)"
STAGED_ALL="$(git diff --cached --name-only --diff-filter=ACMR || true)"

# ── Gate 1: rustfmt ──────────────────────────────────────────────────────────
if [ -n "$STAGED_RS" ]; then
    echo "  [1/6] cargo fmt --check"
    if ! cargo fmt --check 2>&1; then
        echo -e "${RED}✗ rustfmt failed. Run 'cargo fmt' and re-stage.${NC}"
        exit 1
    fi
    echo -e "${GREEN}  ✓ fmt${NC}"
fi

# ── Gate 2: clippy ───────────────────────────────────────────────────────────
if [ -n "$STAGED_RS" ]; then
    echo "  [2/6] cargo clippy"
    if ! cargo clippy --workspace --no-deps --all-targets -- -D warnings 2>&1 | tail -5; then
        echo -e "${RED}✗ clippy failed. Fix warnings and re-commit.${NC}"
        exit 1
    fi
    echo -e "${GREEN}  ✓ clippy${NC}"
fi

# ── Gate 3: .hive/memory/ append-only ───────────────────────────────────────
# Append-only: lines can be ADDED but never DELETED or MODIFIED.
echo "  [3/6] memory append-only check"
DELETED_MEMORY="$(git diff --cached -- '.hive/memory/*.jsonl' | grep -E '^-[^-]' | grep -v '^---' || true)"
RENAMED_OR_DELETED_FILES="$(git diff --cached --name-only --diff-filter=DR | grep '^\.hive/memory/' || true)"
if [ -n "$DELETED_MEMORY" ] || [ -n "$RENAMED_OR_DELETED_FILES" ]; then
    echo -e "${RED}✗ Forbidden: .hive/memory/ is append-only.${NC}"
    echo "  Existing lines cannot be deleted or modified."
    exit 1
fi
echo -e "${GREEN}  ✓ memory discipline (append-only verified)${NC}"

# ── Gate 4: TDD check — implementation must be paired with tests ─────────────
# Charter §5.1: "Tests are written before implementation."
# Rule: if you stage src/**/*.rs files (non-test, non-bench), you must also
#       stage a test file (tests.rs, *_test.rs, or file containing #[test]).
echo "  [4/6] TDD gate (Charter §5.1)"

IMPL_FILES="$(echo "$STAGED_RS" | grep -v 'tests\?\.\(rs\|md\)$' | grep -v '/tests/' | grep -v 'benches/' || true)"
TEST_FILES="$(echo "$STAGED_RS" | grep -E '(tests?\.rs|_test\.rs|/tests/)' || true)"

if [ -n "$IMPL_FILES" ]; then
    # Check if any staged .rs file (including the impl files) contains #[test]
    HAS_TESTS=""
    for f in $STAGED_RS; do
        if (set +o pipefail; git show ":$f" 2>/dev/null | grep -q '#\[test\]' 2>/dev/null); then
            HAS_TESTS="yes"
            break
        fi
    done

    if [ -z "$HAS_TESTS" ] && [ -z "$TEST_FILES" ]; then
        echo -e "${RED}✗ TDD VIOLATION (Charter §5.1):${NC}"
        echo "  Implementation files staged without any test changes:"
        echo "$IMPL_FILES" | sed 's/^/    /'
        echo ""
        echo -e "${YELLOW}  Required workflow:${NC}"
        echo "    1. Write failing test first → cargo test (RED)"
        echo "    2. Write minimal implementation → cargo test (GREEN)"
        echo "    3. Refactor → commit"
        echo ""
        echo "  If this is infrastructure (Cargo.toml, scripts, CI), set:"
        echo "    export MYCELIUM_SKIP_TDD_GATE=1"
        echo "  Requires founder sign-off. Record in .hive/memory/decisions.jsonl."
        if [ "${MYCELIUM_SKIP_TDD_GATE:-}" = "1" ]; then
            echo -e "${YELLOW}  ⚠ TDD gate skipped via MYCELIUM_SKIP_TDD_GATE=1${NC}"
        else
            exit 1
        fi
    else
        echo -e "${GREEN}  ✓ tests present${NC}"
    fi
fi

# ── Gate 5: RFC acceptance criteria ──────────────────────────────────────────
# If an RFC file is staged, it should have at least one [x] checked OR
# the RFC must be a new draft (all [ ] is acceptable for new RFCs).
echo "  [5/6] RFC acceptance criteria check"
STAGED_RFCS="$(echo "$STAGED_ALL" | grep '^rfcs/' | grep -v 'README\|0000-template' || true)"
if [ -n "$STAGED_RFCS" ]; then
    for rfc_file in $STAGED_RFCS; do
        # Check if this RFC has any acceptance criteria at all
        HAS_CRITERIA="$(git show ":$rfc_file" 2>/dev/null | grep -c '\- \[' || echo 0)"
        CHECKED="$(git show ":$rfc_file" 2>/dev/null | grep -c '\- \[x\]' || echo 0)"
        IS_NEW="$(git diff --cached --name-only --diff-filter=A | grep -c "^$rfc_file$" || echo 0)"

        if [ "$HAS_CRITERIA" -gt 0 ] && [ "$CHECKED" -eq 0 ] && [ "$IS_NEW" -eq 0 ]; then
            echo -e "${YELLOW}  ⚠ RFC has unchecked acceptance criteria: $rfc_file${NC}"
            echo "    Before merging implementation, tick off: [ ] → [x] in the RFC."
            echo "    (Warning only — not blocking. Resolve before PR merge.)"
        fi
    done
fi
echo -e "${GREEN}  ✓ RFC check done${NC}"

# ── Gate 6: Secret scan ──────────────────────────────────────────────────────
echo "  [6/6] secret scan"
if git diff --cached | grep -E -i '(api[_-]?key|secret[_-]?key|password\s*=|aws_access_key)' \
    | grep -v '^-' | grep -v 'example\|test\|fake\|placeholder\|YOUR_' >/dev/null 2>&1; then
    echo -e "${RED}✗ Possible secret detected in diff. Review before committing.${NC}"
    exit 1
fi
echo -e "${GREEN}  ✓ no secrets${NC}"

echo -e "${GREEN}[pre-commit] all gates passed ✓${NC}"
HOOK

cat > "$HOOK_DIR/commit-msg" <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail

COMMIT_MSG_FILE="$1"

# ── DCO sign-off ─────────────────────────────────────────────────────────────
if ! grep -qE '^Signed-off-by: .+ <.+>$' "$COMMIT_MSG_FILE"; then
    echo "ERROR: Commit message lacks DCO sign-off."
    echo "Use 'git commit -s' or amend with 'git commit --amend -s'."
    exit 1
fi

# ── Conventional Commits ─────────────────────────────────────────────────────
SUBJECT="$(head -1 "$COMMIT_MSG_FILE")"
if ! echo "$SUBJECT" | grep -qE \
    '^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|meta|revert)(\([a-z][a-z0-9/_-]*\))?!?: .+'; then
    echo "ERROR: Commit subject does not follow Conventional Commits."
    echo "Subject: $SUBJECT"
    echo "Types: feat|fix|docs|style|refactor|perf|test|build|ci|chore|meta|revert"
    exit 1
fi
HOOK

chmod +x "$HOOK_DIR/pre-commit" "$HOOK_DIR/commit-msg"

echo "✓ Installed pre-commit hook with 6 gates:"
echo "  1. rustfmt"
echo "  2. clippy"
echo "  3. .hive/memory/ append-only guard"
echo "  4. TDD gate (Charter §5.1) — blocks impl without tests"
echo "  5. RFC acceptance criteria reminder"
echo "  6. secret scan"
echo ""
echo "✓ Installed commit-msg hook:"
echo "  - DCO sign-off required"
echo "  - Conventional Commits format required"
echo ""
echo "TDD gate bypass (infrastructure changes only):"
echo "  MYCELIUM_SKIP_TDD_GATE=1 git commit -s -m '...'"
echo "  Must be recorded in .hive/memory/decisions.jsonl"
