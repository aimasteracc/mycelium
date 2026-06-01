#!/usr/bin/env bash
# release-ceremony.sh — complete release ceremony for Mycelium
#
# Handles the full release flow from develop → release branch → main → crates.io → develop back-merge.
# Designed to fix the drift described in issue #375.
#
# Prerequisites:
#   - CRATES_IO_TOKEN set in environment (or GitHub secrets)
#   - Clean working tree on develop
#   - Owner privileges on GitHub + crates.io
#
# Usage: scripts/release-ceremony.sh <version>
# Example: scripts/release-ceremony.sh 0.1.16

set -euo pipefail

VERSION="${1:?version required, e.g. 0.1.16}"
BRANCH="release/v${VERSION}"
TAG="v${VERSION}"

echo "=== Mycelium Release Ceremony v${VERSION} ==="

# Step 0: Validate prerequisites
if ! git diff --quiet; then
    echo "ERROR: Working tree has uncommitted changes. Commit or stash first."
    exit 1
fi

if [ -z "${CRATES_IO_TOKEN:-}" ]; then
    echo "ERROR: CRATES_IO_TOKEN is not set. Export it before running this script."
    exit 1
fi

CURRENT_BRANCH=$(git branch --show-current)
if [ "$CURRENT_BRANCH" != "develop" ]; then
    echo "ERROR: Must be on develop branch. Currently on: ${CURRENT_BRANCH}"
    exit 1
fi

echo "[1/9] Bumping workspace version to ${VERSION}..."
# Update [workspace.package].version in root Cargo.toml
if grep -q '\[workspace.package\]' Cargo.toml; then
    sed -i.bak "s/^version = \"[0-9][^\"]*\"/version = \"${VERSION}\"/" Cargo.toml
    rm -f Cargo.toml.bak
fi
# Also bump each crate's version
for cargo_toml in crates/*/Cargo.toml; do
    sed -i.bak "s/^version = \"[0-9][^\"]*\"/version = \"${VERSION}\"/" "$cargo_toml"
    rm -f "$cargo_toml.bak"
done

echo "[2/9] Running release-prep (changelog, dependency pins)..."
bash scripts/release-prep.sh "$VERSION"

echo "[3/9] Creating release branch ${BRANCH}..."
git checkout -b "$BRANCH"
git add -A
git commit -s -m "release: bump version to ${VERSION}

Signed-off-by: $(git config user.name) <$(git config user.email)>"

echo "[4/9] Running full workspace tests..."
cargo test --workspace --all-features
echo "  ✓ All tests pass"

# Publish order: dependencies first (pack → core → hyphae → mcp → cli)
echo "[5/9] Dry-run cargo publish for all crates..."
PACKAGES=(
    mycelium-rcig-pack
    mycelium-rcig-core
    mycelium-rcig-hyphae
    mycelium-rcig-mcp
    mycelium-rcig-cli
)
for pkg in "${PACKAGES[@]}"; do
    echo "  Dry-run publishing ${pkg}..."
    CARGO_REGISTRY_TOKEN="$CRATES_IO_TOKEN" cargo publish -p "$pkg" --dry-run
done
echo "  ✓ All dry-runs pass"

echo "[6/9] Pushing release branch..."
git push origin "$BRANCH"

echo "[7/9] Publishing to crates.io..."
for pkg in "${PACKAGES[@]}"; do
    echo "  Publishing ${pkg}..."
    CARGO_REGISTRY_TOKEN="$CRATES_IO_TOKEN" cargo publish -p "$pkg"
    echo "  ✓ ${pkg} published"
done

echo "[8/9] Merging to main and tagging..."
git checkout main
git pull origin main
git merge --no-ff "$BRANCH" -m "Merge ${BRANCH} into main"
git tag -a "$TAG" -m "Release ${TAG}"
git push origin main --tags

echo "[9/9] Back-merging release into develop..."
git checkout develop
git pull origin develop
git merge --no-ff "$BRANCH" -m "Merge ${BRANCH} back into develop"
git push origin develop

echo "[Cleanup] Deleting release branch..."
git push origin --delete "$BRANCH" || true
git branch -d "$BRANCH"

echo ""
echo "=== Release Ceremony Complete ==="
echo "  Version: ${TAG}"
echo "  GitHub: https://github.com/aimasteracc/mycelium/releases/tag/${TAG}"
echo "  crates.io: https://crates.io/crates/mycelium-rcig-core/${VERSION}"
echo ""
echo "Remaining manual steps:"
echo "  1. Create GitHub Release with changelog notes"
echo "  2. Verify crates.io pages for all 5 crates"
echo "  3. Record post-flight memory entry"
