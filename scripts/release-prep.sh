#!/usr/bin/env bash
# release-prep.sh — generate release notes and update version metadata
#
# Usage: scripts/release-prep.sh <version>
# Example: scripts/release-prep.sh 0.1.0

set -euo pipefail

VERSION="${1:?version required, e.g. 0.1.0}"
TODAY="$(date +%Y-%m-%d)"

# Validate semver
if ! echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.-]+)?$'; then
    echo "ERROR: '$VERSION' is not a valid semver string."
    exit 1
fi

echo "Preparing release v$VERSION ($TODAY)..."

# 1. Update CHANGELOG.md: move [Unreleased] to [vX.Y.Z]
if [ -f CHANGELOG.md ]; then
    # Insert a new release header above the Unreleased section
    awk -v v="$VERSION" -v d="$TODAY" '
        /^## \[Unreleased\]/ {
            print "## [Unreleased]"
            print ""
            print "## [" v "] - " d
            next
        }
        { print }
    ' CHANGELOG.md > CHANGELOG.md.new
    mv CHANGELOG.md.new CHANGELOG.md
    echo "  ✓ CHANGELOG.md updated"
fi

# 2. Extract just this release's notes for the GitHub Release body
if [ -f CHANGELOG.md ]; then
    awk -v v="$VERSION" '
        $0 ~ "^## \\[" v "\\]" { in_section=1; next }
        in_section && /^## \[/ { exit }
        in_section { print }
    ' CHANGELOG.md > .release-notes.md
    echo "  ✓ .release-notes.md generated"
fi

# 3. Verify Cargo.toml has the version
if [ -f Cargo.toml ]; then
    CARGO_VERSION=$(grep -m1 '^version =' Cargo.toml | sed -E 's/version = "(.*)"/\1/' || true)
    if [ "$CARGO_VERSION" != "$VERSION" ]; then
        echo "  ⚠ Cargo.toml version ($CARGO_VERSION) ≠ $VERSION"
        echo "    Run: cargo set-version --workspace $VERSION"
    else
        echo "  ✓ Cargo.toml version matches"
    fi
fi

echo
echo "Next steps:"
echo "  1. Review CHANGELOG.md and .release-notes.md"
echo "  2. Commit: git commit -s -m 'chore(release): v$VERSION'"
echo "  3. Push: git push origin HEAD"
echo "  4. CI will publish to crates.io / npm / PyPI, then merge to main"
