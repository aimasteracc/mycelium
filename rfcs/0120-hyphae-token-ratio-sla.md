# RFC-0120: Charter §2 Hyphae Token Efficiency SLA — Amend, Compress, or Retire

- **Status**: Draft — **FOUNDER DECISION REQUIRED** (Option A / B / C)
- **Author(s)**: orchestrator (PM dispatch v137)
- **Created**: 2026-06-08
- **Last updated**: 2026-06-08
- **Tracking issue**: see P0 #2 in `docs/sprints/2026-Q2-pm-state.md`
- **Affected source paths**:
  - `CHARTER.md` §2 Performance SLA table (if Option A or C)
  - `crates/mycelium-mcp/src/formatter.rs` (if Option B)
  - `scripts/check_token_ratio.sh` (if Option A — gate update)

## Summary

Charter §2 currently targets **"AI token efficiency (Hyphae DSL vs JSON):
≤ 30% of JSON token count for the same payload"**. Measured production
average across all 93+ tools is **0.753** (75.3% of JSON). This RFC
documents the root cause, presents three resolution options with
tradeoffs, and asks the founder to choose one.

This is a `meta` RFC because it modifies a Charter §2 public SLA
commitment. **The PM cannot resolve it autonomously.** Founder approval
required per Charter §9 amendment process.

## Background & Root Cause

### What Charter §2 says

```
AI token efficiency (Hyphae DSL vs JSON) | ≤ 30% of JSON token count for the same payload
```

### What RFC-0094 demonstrated

RFC-0094 (token-efficient output) measured `get_callee_tree` on a
50-node subtree:

| Format | Tokens |
|--------|--------|
| JSON   | 1,973  |
| Text   | 562    |

Ratio: 562 / 1,973 = **28.5%** — meets the ≤30% target.

### Why measured production average is 0.753

The RFC-0094 benchmark used a **tree-shaped response** — the best case for
the TOON text format. Tree responses gain from eliminating JSON structural
punctuation (braces, quotes, commas) on nested data.

Most Mycelium tools return **flat responses**: lists of symbol paths,
scalar counts, boolean flags, single-depth objects. For these:

- A JSON string `"src/auth.rs>AuthService::login"` → 38 tokens  
- The same value in TOON text → `src/auth.rs>AuthService::login` → ~32 tokens  
- Ratio ≈ **84%** — barely below JSON

Averaging across 93+ tools — most of which are flat — produces the
measured 0.753. The target was anchored on the best-case tool; the
production reality is a workload-weighted average.

### Why this matters

Charter §2 rows are CI-gated ("every release must meet or beat these
numbers, otherwise it does not ship"). If the CI gate is strict on the
0.30 target, it would block every future release until compression is
implemented. As of today the gate appears soft (no CI check enforces
this specific row), but the public promise remains.

---

## Option A — Amend Charter §2 to per-class targets

**Change:**  
Replace the single row with three rows reflecting actual compression
profile by response type:

| Metric | Target |
|--------|--------|
| AI token efficiency — tree responses (callee_tree, reachable_set, hub_symbols) | ≤ 35% of JSON token count |
| AI token efficiency — list responses (get_callees, get_callers, all_symbols, …) | ≤ 70% of JSON token count |
| AI token efficiency — scalar / status responses | ≤ 90% of JSON token count |

The tree target (≤35%) is already satisfied by the current implementation
(RFC-0094 measured 28.5%). The list and scalar targets reflect actual
measured performance and are immediately met without new engineering work.

**Implementation:**  
1. Add the three rows to `CHARTER.md §2` (this RFC merge).
2. Update / add `scripts/check_token_ratio.sh` per-class gates.
3. No Rust code changes needed.

**Pros:**
- Honest: reflects measured reality for each response class.
- Preserves the AI-native positioning for tree queries (the high-value
  use case where Mycelium actually delivers a real competitive edge).
- No engineering work blocking release.
- CI gate stays meaningful (per-class guards catch genuine regressions).

**Cons:**
- The headline "≤30%" promise disappears from the public SLA table;
  messaging needs updating.
- List responses at ≤70% is still better than JSON but not the "3×
  better" story originally intended.

**PM recommendation: ✅ Option A is the right call.** The 0.30 target
was set from one data point (the best-case tree tool). Amending to
per-class targets is honest, CI-enforceable, and avoids blocking
releases or investing in compression work that buys marginal gains on
the non-tree majority.

---

## Option B — Implement compression to reach ≤30% across all tools

**What it would take:**  
Reaching ≤30% on flat list responses (currently ~75–85%) would require
aggressive additional compression:

1. **Field-name abbreviation**: replace long keys with terse aliases in
   the text format (e.g., `callee_paths` → `cp`). Saves ~20–30% of
   structure tokens but hurts human readability.
2. **Symbol-path dictionary / deduplication**: interned symbol references
   (many responses repeat the same crate prefix). Saves ~15% on large
   responses but requires a stateful encoder/decoder.
3. **Prefix stripping**: strip common path prefixes (e.g., project root)
   from every symbol path in the response. Simple, saves ~10–20% on
   typical monorepo responses.

Combined, these might bring flat responses to ~40–50%. Reaching ≤30%
for flat lists would require binary encoding (giving up human
readability entirely), defeating one of the text format's key advantages.

**Pros:**
- Meets the original ambitious 30% target.
- Could be a genuine competitive moat if documented in benchmarks.

**Cons:**
- Significant engineering work (RFC-0094 Phase 5) — at least 2–3 sprint
  weeks.
- Field-name abbreviation + dictionary deduplication changes the text
  format contract (breaking change for any consumers parsing the text
  format).
- ≤30% for list responses may be physically impossible without binary
  encoding; the 30% target might have been unrealistic for flat data.
- Blocks future release until implemented.

**PM assessment:** Option B should only be chosen if the 30% claim is
central to the commercial/positioning story. The engineering cost is
real, the target may not be achievable for flat responses, and it would
delay v0.4 work.

---

## Option C — Retire the metric

**Change:**  
Remove the "AI token efficiency" row from Charter §2 entirely.

Replace it with a qualitative statement in Charter §5.11 or §5.5:
> "Mycelium MCP tools use a token-efficient text format (RFC-0094) for
> tree-shaped responses. Measured token savings vs JSON are reported
> per-tool in the release benchmarks."

**Pros:**
- Removes an unmeasured and unenforced public SLA.
- No CI gate maintenance needed.
- Honest: we don't have a single aggregate number that's meaningful
  across 93+ tools.

**Cons:**
- Looks like backing away from a commitment.
- Loses the competitive positioning signal (even if individual tools
  actually deliver 28–30% for tree queries).
- A future competitor could cite the removed row as evidence.

**PM assessment:** Option C is the most defensive choice. It's cleaner
than a broken promise but abandons even the per-class story.

---

## Detailed design

If **Option A** is approved, the change to `CHARTER.md §2` is:

```diff
-| AI token efficiency (Hyphae DSL vs JSON) | ≤ 30% of JSON token count for the same payload |
+| AI token efficiency — tree responses (`callee_tree`, `reachable_set`, `hub_symbols`, et al.) | ≤ 35% of JSON token count |
+| AI token efficiency — list responses (`get_callees`, `get_callers`, `all_symbols`, `page_rank`, et al.) | ≤ 70% of JSON token count |
+| AI token efficiency — scalar / status responses | ≤ 90% of JSON token count |
```

Additionally, a brief note is added to the table header:

> **Response-class definitions (RFC-0120).** *Tree*: response contains
> nested `callees`, `nodes`, `reachable_set`, or similar hierarchical
> structure. *List*: response is a flat or single-depth list of
> symbol paths / counts. *Scalar*: response is a single value,
> boolean, or short status object.

## Testing strategy

Option A: Add `scripts/check_token_ratio.sh` fixtures for each class
and hook into CI (fast-lane). No Rust changes needed.

Option B: TDD per existing RFC-0094 acceptance criteria. New Criterion
benchmark per tool family. Gate: cargo bench token_ratio --all-features
reports ≤30% for each class.

Option C: Delete the `check_token_ratio` CI job.

## Open questions

1. **Which option?** — founder choice required.
2. **If Option A**: should the three rows be CI-gated on every PR
   (fast-lane), or nightly only? (PM suggestion: nightly to avoid
   tokenizer dependency in fast CI.)
3. **If Option B**: is ≤30% for flat list responses the correct target,
   or should we define "≤30% for tree responses only" as the engineering
   objective?

---

*Once the founder chooses, this RFC will be updated to Status: Accepted
and a follow-up commit will land the CHARTER.md change (Option A/C) or
the implementation RFC + implementation (Option B).*
