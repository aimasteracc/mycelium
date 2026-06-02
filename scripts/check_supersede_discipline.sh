#!/usr/bin/env bash
# Supersede-discipline guardrails (RFC governance).
#
# Why this exists: during the v0.1.16 era the project shipped hand-built
# machinery (journal #343, LRU #344) for problems an already-accepted RFC
# (RFC-0100 redb) had decided to solve differently — and RFC-0100's
# "Supersedes" line pointed at a phantom rfcs/0099 file that did not exist on
# develop. Any worker (human or AI) told "deal with the open issues/PRs" would
# have implemented the superseded approach, because nothing in the tree flagged
# it as retired. These checks make the supersede chain machine-verifiable so the
# next worker's pre-flight can trust it.
#
# Check 1 — no dangling supersede link: every RFC referenced in a
#           "Supersedes" / "Superseded by" line must have a backing rfcs/ file.
# Check 2 — no un-flagged superseded-RFC module: any source file whose MODULE
#           HEADER (the leading doc block) declares it implements an RFC that is
#           Status:Superseded must carry a TRANSITIONAL / superseded note in that
#           same header, so a reader knows the module is a bridge, not the chosen
#           path. Only the header is scanned — an inline reference to a superseded
#           RFC (e.g. "edge ownership follows RFC-0098") by the *superseding*
#           implementation is legitimate and is not flagged.
#
# Pure grep/awk/sed only — no ripgrep (not guaranteed on CI runners). No
# `grep -q` inside a pipeline (SIGPIPE + pipefail false-negative footgun); all
# greps read files directly.

set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0

# ---------------------------------------------------------------------------
# Check 1: supersede references resolve to a real RFC file.
# ---------------------------------------------------------------------------
supersede_lines="$(grep -rhnE 'Supersed(es|ed by)' rfcs/*.md 2>/dev/null || true)"
referenced_rfcs="$(printf '%s\n' "$supersede_lines" | grep -oE 'RFC-[0-9]{4}' | sort -u || true)"

for rfc in $referenced_rfcs; do
    num="${rfc#RFC-}"
    # ls glob in a subshell so a no-match doesn't trip set -e.
    if ! ls "rfcs/${num}"-*.md >/dev/null 2>&1; then
        echo "GOVERNANCE FAIL (dangling supersede): ${rfc} is referenced in a supersede line but rfcs/${num}-*.md does not exist."
        echo "  Fix: restore/create the RFC file, or correct the reference. A supersede link that points at nothing breaks the audit trail."
        fail=1
    fi
done

# ---------------------------------------------------------------------------
# Check 2: source code citing a Superseded RFC must flag itself as transitional.
# ---------------------------------------------------------------------------
# Collect the numbers of RFCs whose Status says "Superseded".
superseded_nums=""
for f in rfcs/*.md; do
    [ -e "$f" ] || continue
    if grep -iE '^\**[[:space:]]*-?[[:space:]]*\**Status\**[[:space:]]*:.*[Ss]uperseded' "$f" >/dev/null 2>&1; then
        base="$(basename "$f")"
        superseded_nums="${superseded_nums} ${base%%-*}"
    fi
done

for num in $superseded_nums; do
    # Candidate files that mention this superseded RFC at all.
    while IFS= read -r src; do
        [ -n "$src" ] || continue
        # Only the module header (leading doc block) counts as "declares it
        # implements RFC-N". Here-strings (<<<) keep this off a pipeline so the
        # pipefail+SIGPIPE false-negative footgun cannot bite.
        header="$(head -n 20 "$src" 2>/dev/null || true)"
        if grep -iE "RFC-${num}" <<<"$header" >/dev/null 2>&1; then
            if ! grep -iE 'transitional|superseded' <<<"$header" >/dev/null 2>&1; then
                echo "GOVERNANCE FAIL (un-flagged superseded module): ${src} header declares it implements RFC-${num} (Status: Superseded) without a TRANSITIONAL/superseded note."
                echo "  Fix: add a header note that this module is a transitional bridge retained until the superseding RFC's migration completes, or delete it."
                fail=1
            fi
        fi
    done < <(grep -rlE "RFC-${num}" crates --include='*.rs' 2>/dev/null || true)
done

if [ "$fail" -ne 0 ]; then
    echo ""
    echo "Supersede-discipline guardrails FAILED. See messages above."
    exit 1
fi

echo "Supersede-discipline guardrails passed."
