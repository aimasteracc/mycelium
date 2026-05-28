#!/usr/bin/env bash
# install-hooks.sh — install git hooks for contributors
#
# Installs a pre-commit hook that:
#   - Runs cargo fmt --check on staged Rust files
#   - Runs cargo clippy on the workspace
#   - Verifies DCO sign-off on the staged commit
#
# Run once after cloning:
#   ./scripts/install-hooks.sh

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
NC='\033[0m'

echo -e "${YELLOW}[pre-commit] Mycelium quality gates${NC}"

# 1. Format check on staged Rust files
STAGED_RS="$(git diff --cached --name-only --diff-filter=ACMR | grep '\.rs$' || true)"
if [ -n "$STAGED_RS" ]; then
    echo "  → cargo fmt --check"
    if ! cargo fmt --check; then
        echo -e "${RED}rustfmt failed. Run 'cargo fmt' and re-stage.${NC}"
        exit 1
    fi
fi

# 2. Clippy on the workspace (fast subset)
if [ -n "$STAGED_RS" ]; then
    echo "  → cargo clippy"
    if ! cargo clippy --workspace --no-deps --all-targets -- -D warnings 2>&1 | tail -20; then
        echo -e "${RED}clippy failed. Fix warnings and re-commit.${NC}"
        exit 1
    fi
fi

# 3. Verify nothing in .hive/memory/ was modified (append-only discipline)
MODIFIED_MEMORY="$(git diff --cached --name-only --diff-filter=MDR | grep '^\.hive/memory/' || true)"
if [ -n "$MODIFIED_MEMORY" ]; then
    echo -e "${RED}Forbidden: .hive/memory/ is append-only. Do not modify or delete existing lines:${NC}"
    echo "$MODIFIED_MEMORY"
    exit 1
fi

# 4. Verify no secrets-looking strings
if git diff --cached | grep -E -i '(api[_-]?key|secret|token|password|aws_access)' | grep -v '^-' | grep -v 'example\|test\|fake' >/dev/null 2>&1; then
    echo -e "${YELLOW}Warning: possible secret in diff. Review before committing.${NC}"
fi

echo -e "${GREEN}[pre-commit] all checks passed${NC}"
HOOK

cat > "$HOOK_DIR/commit-msg" <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail

COMMIT_MSG_FILE="$1"

# Require DCO sign-off
if ! grep -qE '^Signed-off-by: .+ <.+>$' "$COMMIT_MSG_FILE"; then
    echo "ERROR: Commit message lacks DCO sign-off."
    echo "Use 'git commit -s' or amend with 'git commit --amend -s'."
    echo "See CONTRIBUTING.md for details."
    exit 1
fi

# Light Conventional Commits check (just the type prefix on the subject line)
SUBJECT="$(head -1 "$COMMIT_MSG_FILE")"
if ! echo "$SUBJECT" | grep -qE '^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|meta|revert)(\([a-z][a-z0-9/_-]*\))?!?: .+'; then
    echo "ERROR: Commit subject does not follow Conventional Commits."
    echo "Subject: $SUBJECT"
    echo "Expected: <type>(<scope>): <subject>"
    echo "Types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, meta, revert"
    exit 1
fi
HOOK

chmod +x "$HOOK_DIR/pre-commit" "$HOOK_DIR/commit-msg"

echo "✓ Installed pre-commit hook (fmt + clippy + .hive/memory/ guard + secret scan)"
echo "✓ Installed commit-msg hook (DCO sign-off + Conventional Commits check)"
echo
echo "Tip: use 'git commit -s' to auto-add the DCO sign-off."
