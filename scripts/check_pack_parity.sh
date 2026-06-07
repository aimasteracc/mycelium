#!/usr/bin/env bash
# Check that embedded pack query copies match the canonical packs/ directory.
#
# Canonical: packs/<lang>/queries.scm  (the single source of truth)
# Embedded copies:
#   - crates/mycelium-mcp/packs/<lang>/queries.scm   (FULL: all canonical langs)
#   - crates/mycelium-cli/packs/<lang>/queries.scm   (FULL: all canonical langs)
#   - crates/mycelium-core/packs/<lang>/queries.scm  (SUBSET: only the Tier-1
#       langs that crates/mycelium-core/src/cortex.rs `include_str!`s INTO THE
#       BINARY — go/javascript/python/rust/typescript). This copy is what the
#       shipped engine actually parses with, so a stale core copy means the
#       binary runs old queries even when CI + tests (which load the ROOT copy)
#       are green. It was previously UNCHECKED — a latent footgun that shipped
#       stale python/typescript/javascript queries. Now verified.
#
# FULL copies: every canonical lang must be present AND identical.
# SUBSET copies: whatever langs the copy contains must be identical to canonical
#   (missing langs are not an error — it is a subset by design — but every lang
#   cortex.rs embeds MUST exist here).
#
# Fails if any embedded copy diverges from canonical.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
CANON="$ROOT/packs"
FULL_COPIES=("crates/mycelium-mcp/packs" "crates/mycelium-cli/packs")
SUBSET_COPIES=("crates/mycelium-core/packs")
# Langs the binary embeds via cortex.rs include_str! — these MUST exist in the
# core subset copy. DERIVED from cortex.rs itself (not hardcoded) so the list
# can never silently drift from what the binary actually compiles in — the same
# class of footgun this script guards against. Each cortex.rs line reads:
#   const <X>_QUERIES: &str = include_str!("../packs/<lang>/queries.scm");
CORTEX_RS="$ROOT/crates/mycelium-core/src/cortex.rs"
CORE_EMBEDDED=()
while IFS= read -r lang; do
    [ -n "$lang" ] && CORE_EMBEDDED+=("$lang")
done < <(grep -oE 'include_str!\("\.\./packs/[a-z_]+/queries\.scm"\)' "$CORTEX_RS" \
    | sed -E 's#.*/packs/([a-z_]+)/.*#\1#')
if [ "${#CORE_EMBEDDED[@]}" -eq 0 ]; then
    echo "ERROR: could not derive cortex-embedded langs from $CORTEX_RS" >&2
    echo "       (expected lines like: include_str!(\"../packs/rust/queries.scm\"))" >&2
    exit 1
fi
FAIL=0

diff_or_fail() {
    # $1 canonical file, $2 embedded file, $3 human label
    if ! diff -q "$1" "$2" > /dev/null; then
        echo "DIVERGED: $3 differs from canonical packs/" >&2
        diff "$1" "$2" >&2 || true
        FAIL=1
    fi
}

# FULL copies — every canonical lang must be present and identical.
for lang in "$CANON"/*/; do
    lang_name="$(basename "$lang")"
    canon_file="$CANON/$lang_name/queries.scm"
    [ -f "$canon_file" ] || continue
    for copy_dir in "${FULL_COPIES[@]}"; do
        embedded="$ROOT/$copy_dir/$lang_name/queries.scm"
        if [ ! -f "$embedded" ]; then
            echo "MISSING: $copy_dir/$lang_name/queries.scm (canonical exists)" >&2
            FAIL=1
            continue
        fi
        diff_or_fail "$canon_file" "$embedded" "$copy_dir/$lang_name/queries.scm"
    done
done

# SUBSET copies — whatever langs are present must match canonical.
for copy_dir in "${SUBSET_COPIES[@]}"; do
    for lang in "$ROOT/$copy_dir"/*/; do
        [ -d "$lang" ] || continue
        lang_name="$(basename "$lang")"
        embedded="$ROOT/$copy_dir/$lang_name/queries.scm"
        canon_file="$CANON/$lang_name/queries.scm"
        [ -f "$embedded" ] || continue
        if [ ! -f "$canon_file" ]; then
            echo "ORPHAN: $copy_dir/$lang_name/queries.scm has no canonical packs/$lang_name" >&2
            FAIL=1
            continue
        fi
        diff_or_fail "$canon_file" "$embedded" "$copy_dir/$lang_name/queries.scm"
    done
    # Every cortex-embedded lang MUST exist in the core subset copy.
    for lang_name in "${CORE_EMBEDDED[@]}"; do
        if [ ! -f "$ROOT/$copy_dir/$lang_name/queries.scm" ]; then
            echo "MISSING: $copy_dir/$lang_name/queries.scm (embedded by cortex.rs)" >&2
            FAIL=1
        fi
    done
done

if [ "$FAIL" -eq 0 ]; then
    echo "pack-parity: all embedded copies (mcp+cli full, core subset) match canonical packs/ ✓"
else
    echo "" >&2
    echo "Fix: copy packs/<lang>/queries.scm → crates/mycelium-{mcp,cli,core}/packs/<lang>/queries.scm" >&2
    echo "     (core embeds only: ${CORE_EMBEDDED[*]})" >&2
    exit 1
fi
