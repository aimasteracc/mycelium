#!/usr/bin/env bash
# capture_token_corpus.sh — RFC-0120 Phase 1b/1c
#
# Re-captures the committed token-bench corpus from a real indexed fixture repo.
#
# Usage:
#   ./scripts/capture_token_corpus.sh [FIXTURE_ROOT]
#
# Default FIXTURE_ROOT: tests/e2e/fixtures/ripgrep/
#   Populate it first: ./scripts/fetch-e2e-fixtures.sh
#
# Output: crates/mycelium-mcp/tests/corpus/<tool>.json  (overwrites existing files)
#         Exports MYCELIUM_REAL_CORPUS=1 so the Charter §2 binding test activates.
#
# Prerequisites:
#   - `mycelium` binary in PATH (build with: cargo build --release && export PATH="$PWD/target/release:$PATH")
#   - jq (for extracting fields from JSON output)
#
# Regenerate when:
#   - A tiktoken-rs version bump changes BPE counts
#   - A TextFormatter or JsonFormatter change alters output shape
#   After regeneration: commit the updated corpus + updated REPORT.md,
#   then push a PR with MYCELIUM_REAL_CORPUS=1 set in CI (nightly or explicit job).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURE_ROOT="${1:-$REPO_ROOT/tests/e2e/fixtures/ripgrep}"
CORPUS_DIR="$REPO_ROOT/crates/mycelium-mcp/tests/corpus"

echo "=== capture_token_corpus.sh ==="
echo "Fixture root : $FIXTURE_ROOT"
echo "Corpus output: $CORPUS_DIR"
echo ""

# Verify prerequisites
if ! command -v mycelium &>/dev/null; then
    echo "ERROR: mycelium binary not in PATH."
    echo "Build it first:"
    echo "  cargo build --release"
    echo "  export PATH=\"\$PWD/target/release:\$PATH\""
    exit 1
fi
if ! command -v jq &>/dev/null; then
    echo "ERROR: jq not found. Install it (e.g. apt-get install jq / brew install jq)."
    exit 1
fi
if [[ ! -d "$FIXTURE_ROOT" ]]; then
    echo "ERROR: fixture root '$FIXTURE_ROOT' does not exist."
    echo "Run: ./scripts/fetch-e2e-fixtures.sh  (or git lfs pull)"
    exit 1
fi

# Index the fixture repo.
# The index is written to $FIXTURE_ROOT/.mycelium/index.rmp automatically.
echo "Indexing $FIXTURE_ROOT ..."
mycelium index "$FIXTURE_ROOT"
echo "Index complete. (written to $FIXTURE_ROOT/.mycelium/index.rmp)"
echo ""

# Helper: run a CLI command with --root <FIXTURE_ROOT> and --format json, save output.
# Usage: capture <output_name> <subcommand> [subcommand-args...]
# Note: --root and --format json are always appended by this helper; do not pass them
# in the subcommand args. Every mycelium subcommand accepts --root <project-dir>.
capture() {
    local name="$1"; shift
    local out="$CORPUS_DIR/$name.json"
    echo -n "Capturing $name ... "
    if mycelium "$@" --root "$FIXTURE_ROOT" --format json 2>/dev/null | jq '.' > "$out"; then
        local lines
        lines=$(wc -l < "$out")
        echo "ok ($lines lines)"
    else
        echo "FAILED (skipping)"
        rm -f "$out"
    fi
}

# --- Discover a reference symbol for tree queries ---
# get-all-symbols returns {symbols:[...], count, total_count} (RFC-0109 object shape).
REF_SYM="$(mycelium get-all-symbols --root "$FIXTURE_ROOT" --format json 2>/dev/null \
    | jq -r '.symbols[0] // empty' | head -1 || true)"

# --- Capture each representative tool (RFC-0120 §Design "Corpus definition") ---

# mycelium_context: composite entry-points + call-graph response
capture "context" context --task "explore main entry points and their callee graph"

# mycelium_get_callee_tree / mycelium_get_caller_tree: recursive trees
if [[ -n "$REF_SYM" ]]; then
    capture "callee_tree" get-callee-tree "$REF_SYM" --max-depth 3
    capture "caller_tree" get-caller-tree "$REF_SYM" --max-depth 3
fi

# mycelium_search_symbol: flat list
capture "search_symbol" search-symbol "grep" --limit 20

# mycelium_get_symbol_info: single record
if [[ -n "$REF_SYM" ]]; then
    capture "symbol_info" get-symbol-info "$REF_SYM"
fi

# mycelium_query: Hyphae result
capture "query" query ':defined()'

# mycelium_get_importers_tree: "who imports me" tree rooted at a file/module.
# src/main.rs is the representative entry point for Rust projects like ripgrep;
# the capture helper silently skips if the file is absent in the fixture.
capture "importers_tree" get-importers-tree "src/main.rs" --max-depth 2

# mycelium_subclasses_tree: recursive subclasses tree (children of children)
if [[ -n "$REF_SYM" ]]; then
    capture "subclasses_tree" subclasses-tree "$REF_SYM" --max-depth 2
fi

echo ""
echo "=== Corpus captured to $CORPUS_DIR ==="
ls -la "$CORPUS_DIR"/*.json 2>/dev/null || true

echo ""
echo "Next step: run the binding test:"
echo "  MYCELIUM_REAL_CORPUS=1 cargo test --package mycelium-rcig-mcp --test token_corpus --features tiktoken -- bpe_charter_sla_binding --nocapture"
echo ""
echo "Then commit the updated corpus + REPORT.md."

# Signal to the binding test that this corpus is real.
export MYCELIUM_REAL_CORPUS=1
