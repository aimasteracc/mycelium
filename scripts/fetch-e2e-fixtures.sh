#!/usr/bin/env bash
# fetch-e2e-fixtures.sh — download real open-source project fixtures for e2e tests
#
# Usage:
#   bash scripts/fetch-e2e-fixtures.sh
#
# Downloads into tests/e2e/fixtures/:
#   - requests/   (Python — psf/requests, ~30 .py files)
#   - ripgrep/    (Rust  — BurntSushi/ripgrep, ~40 .rs files)
#   - typescript-sample/  (TypeScript — microsoft/TypeScript-Node-Starter, small)
#
# These are used by tests/e2e/real_projects.rs.
# The fixtures/ directory is .gitignored — run this script once after cloning
# to enable the real-project validation tests locally and in CI.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURES="$SCRIPT_DIR/../tests/e2e/fixtures"
mkdir -p "$FIXTURES"

clone_shallow() {
    local name="$1"
    local url="$2"
    local dest="$FIXTURES/$name"
    if [ -d "$dest/.git" ]; then
        echo "✓ $name already present"
        return
    fi
    echo "↓ cloning $name (shallow)…"
    git clone --depth=1 --single-branch "$url" "$dest"
    echo "✓ $name"
}

clone_shallow "requests"           "https://github.com/psf/requests.git"
clone_shallow "ripgrep"            "https://github.com/BurntSushi/ripgrep.git"
clone_shallow "typescript-sample"  "https://github.com/microsoft/TypeScript-Node-Starter.git"

echo ""
echo "Fixtures ready in $FIXTURES"
echo "Run: cargo test --test e2e"
