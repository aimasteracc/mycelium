#!/usr/bin/env bash
# capture_token_corpus.sh — RFC-0120 Phase 1b
#
# Re-captures the committed token-bench corpus from a real indexed fixture repo.
#
# Usage:
#   ./scripts/capture_token_corpus.sh [FIXTURE_ROOT]
#
# Default FIXTURE_ROOT: tests/e2e/fixtures/ripgrep/
#
# Output: crates/mycelium-mcp/tests/corpus/<tool>.json  (overwrites existing files)
#         Export MYCELIUM_REAL_CORPUS=1 so the Charter §2 binding test activates.
#
# Prerequisites:
#   - `mycelium` binary in PATH (build with: cargo build --release && export PATH="$PWD/target/release:$PATH")
#   - jq (for extracting the success payload from MCP JSON-RPC responses)
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

# Index the fixture repo into a temp directory.
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

INDEX_PATH="$TMPDIR/index.myc"
echo "Indexing $FIXTURE_ROOT → $INDEX_PATH ..."
mycelium index "$FIXTURE_ROOT" --output "$INDEX_PATH"
echo "Index complete."
echo ""

# Helper: run a CLI command in JSON mode and save the payload.
# Args: <output_name> <mycelium subcommand and args...>
capture() {
    local name="$1"; shift
    local out="$CORPUS_DIR/$name.json"
    echo -n "Capturing $name ... "
    # `mycelium <cmd> --format json` emits a JSON object on stdout.
    # We strip the outer MCP envelope (if any) by piping through jq identity.
    if mycelium --index "$INDEX_PATH" "$@" --format json 2>/dev/null | jq '.' > "$out"; then
        local lines
        lines=$(wc -l < "$out")
        echo "ok ($lines lines)"
    else
        echo "FAILED (skipping)"
        rm -f "$out"
    fi
}

# --- Capture each representative tool (RFC-0120 §Design "Corpus definition") ---

# mycelium_context: composite response (entry_points + callers + callees + symbols)
capture "context" context --entry "src/main.rs" --depth 2

# mycelium_get_callee_tree: deep tree
# Find a function with known callees in the fixture.
ROOT_SYM="$(mycelium --index "$INDEX_PATH" get-callees --format json 2>/dev/null \
    | jq -r '.callee_paths[0] // empty' | head -1 || true)"
if [[ -n "$ROOT_SYM" ]]; then
    capture "callee_tree" get-callees "$ROOT_SYM" --depth 3
fi

# mycelium_get_caller_tree
if [[ -n "$ROOT_SYM" ]]; then
    capture "caller_tree" get-callers "$ROOT_SYM" --depth 3
fi

# mycelium_search_symbol: flat list
capture "search_symbol" search "grep" --limit 20

# mycelium_get_symbol_info: record
FIRST_SYM="$(mycelium --index "$INDEX_PATH" get-all-symbols --format json 2>/dev/null \
    | jq -r '.symbols[0] // empty' | head -1 || true)"
if [[ -n "$FIRST_SYM" ]]; then
    capture "symbol_info" symbol-info "$FIRST_SYM"
fi

# mycelium_query: Hyphae result
capture "query" query ':file("src/main.rs"):defined()'

# mycelium_get_importers_tree
capture "importers_tree" get-reachable-to --root "src/lib.rs" --depth 2

# mycelium_get_subclasses_tree (struct hierarchy if any)
capture "subclasses_tree" get-reachable --root "src/main.rs" --depth 2

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
