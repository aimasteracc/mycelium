#!/usr/bin/env bash
# Regression tests for scripts/release-prep.sh.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

mkdir -p "$TMP/crates/mycelium-cli"

cat > "$TMP/Cargo.toml" <<'TOML'
[workspace.package]
version       = "0.1.16"

[workspace.dependencies]
mycelium-core      = { path = "crates/mycelium-core",   version = "0.1.10", package = "mycelium-rcig-core" }
mycelium-hyphae    = { path = "crates/mycelium-hyphae", version = "0.1.10", package = "mycelium-rcig-hyphae" }
mycelium-pack      = { path = "crates/mycelium-pack",   version = "0.1.10", package = "mycelium-rcig-pack" }
TOML

cat > "$TMP/crates/mycelium-cli/Cargo.toml" <<'TOML'
[package]
name = "mycelium-rcig-cli"

[dependencies]
mycelium-core      = { workspace = true }
mycelium-hyphae    = { workspace = true }
mycelium-pack      = { workspace = true }
mycelium-mcp       = { path = "../mycelium-mcp", version = "0.1.14", package = "mycelium-rcig-mcp" }
TOML

cat > "$TMP/CHANGELOG.md" <<'MD'
# Changelog

## [Unreleased]

### Fixed

- Harden the release pipeline.

## [0.1.15] - 2026-05-31

### Added

- Previous release.
MD

(
    cd "$TMP"
    output="$("$ROOT/scripts/release-prep.sh" 0.1.16)"
    grep -q "Cargo.toml version matches" <<<"$output"
)

grep -Eq '^mycelium-core[[:space:]]*=.*version = "0.1.16"' "$TMP/Cargo.toml"
grep -Eq '^mycelium-hyphae[[:space:]]*=.*version = "0.1.16"' "$TMP/Cargo.toml"
grep -Eq '^mycelium-pack[[:space:]]*=.*version = "0.1.16"' "$TMP/Cargo.toml"
grep -Eq '^mycelium-mcp[[:space:]]*=.*version = "0.1.16"' "$TMP/crates/mycelium-cli/Cargo.toml"
grep -q '^## \[0.1.16\] - ' "$TMP/CHANGELOG.md"
grep -q 'Harden the release pipeline.' "$TMP/.release-notes.md"
