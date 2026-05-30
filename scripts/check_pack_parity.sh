#!/usr/bin/env bash
# Check that embedded pack query copies match the canonical packs/ directory.
# Canonical: packs/<lang>/queries.scm
# Embedded:  crates/mycelium-mcp/packs/<lang>/queries.scm
#            crates/mycelium-cli/packs/<lang>/queries.scm
#
# Fails if any embedded copy diverges from canonical.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
CANON="$ROOT/packs"
COPIES=("crates/mycelium-mcp/packs" "crates/mycelium-cli/packs")
FAIL=0

for lang in "$CANON"/*/; do
    lang_name="$(basename "$lang")"
    canon_file="$CANON/$lang_name/queries.scm"
    [ -f "$canon_file" ] || continue

    for copy_dir in "${COPIES[@]}"; do
        embedded="$ROOT/$copy_dir/$lang_name/queries.scm"
        if [ ! -f "$embedded" ]; then
            echo "MISSING: $copy_dir/$lang_name/queries.scm (canonical exists)" >&2
            FAIL=1
            continue
        fi
        if ! diff -q "$canon_file" "$embedded" > /dev/null; then
            echo "DIVERGED: $copy_dir/$lang_name/queries.scm differs from packs/$lang_name/queries.scm" >&2
            diff "$canon_file" "$embedded" >&2 || true
            FAIL=1
        fi
    done
done

if [ "$FAIL" -eq 0 ]; then
    echo "pack-parity: all embedded copies match canonical packs/ ✓"
else
    echo "" >&2
    echo "Fix: copy packs/<lang>/queries.scm → crates/mycelium-{mcp,cli}/packs/<lang>/queries.scm" >&2
    exit 1
fi
