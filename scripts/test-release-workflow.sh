#!/usr/bin/env bash
# Static release workflow checks that protect the registry-first contract.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKFLOW="$ROOT/.github/workflows/release.yml"

if grep -Eq 'cargo publish.*\|\| true' "$WORKFLOW"; then
    echo "release.yml must not ignore cargo publish failures"
    exit 1
fi

grep -Fq 'Verify crates.io token' "$WORKFLOW"
grep -Fq 'Publish crates in dependency order' "$WORKFLOW"

line_for() {
    pattern="$1"
    grep -nF "$pattern" "$WORKFLOW" | head -n1 | cut -d: -f1
}

require_line() {
    pattern="$1"
    line="$(line_for "$pattern")"
    if [ -z "$line" ]; then
        echo "release.yml missing expected line: $pattern"
        exit 1
    fi
    echo "$line"
}

pack_line="$(require_line '"mycelium-pack:mycelium-rcig-pack"')"
core_line="$(require_line '"mycelium-core:mycelium-rcig-core"')"
hyphae_line="$(require_line '"mycelium-hyphae:mycelium-rcig-hyphae"')"
mcp_line="$(require_line '"mycelium-mcp:mycelium-rcig-mcp"')"
cli_line="$(require_line '"mycelium-cli:mycelium-rcig-cli"')"

if ! [ "$pack_line" -lt "$core_line" ] ||
   ! [ "$core_line" -lt "$hyphae_line" ] ||
   ! [ "$hyphae_line" -lt "$mcp_line" ] ||
   ! [ "$mcp_line" -lt "$cli_line" ]; then
    echo "crates must publish in dependency order: pack -> core -> hyphae -> mcp -> cli"
    exit 1
fi

merge_main_line="$(require_line 'name: Merge release branch to main')"
merge_develop_line="$(require_line 'name: Merge release branch to develop')"
tag_line="$(require_line 'name: Create and push release tag')"

if ! [ "$merge_main_line" -lt "$tag_line" ] ||
   ! [ "$merge_develop_line" -lt "$tag_line" ]; then
    echo "release tag must be created only after main and develop merges succeed"
    exit 1
fi
