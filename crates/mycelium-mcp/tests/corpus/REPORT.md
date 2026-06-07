# RFC-0120 Token-Density Corpus Report

**Corpus version**: v2-ripgrep (Phase 1c — real BurntSushi/ripgrep shallow-clone fixture)  
**Tokenizer**: tiktoken-rs `cl100k_base` (GPT-4o / Claude-adjacent BPE family)  
**Measured**: 2026-06-07 (PM dispatch v107)  
**Regenerate with**: `scripts/capture_token_corpus.sh tests/e2e/fixtures/ripgrep/`  
**Ripgrep fixture**: `git clone --depth=1 https://github.com/BurntSushi/ripgrep.git` (101 files indexed)

---

## Per-fixture measurement

| Fixture            | JsonFormatter tokens | TextFormatter tokens | Ratio (text/json) |
|--------------------|---------------------:|---------------------:|------------------:|
| callee_tree        |                  308 |                  199 |             0.646 |
| caller_tree        |                   18 |                   11 |             0.611 |
| context            |                2 603 |                1 984 |             0.762 |
| search_symbol      |                  155 |                  137 |             0.884 |
| subclasses_tree    |                   25 |                   14 |             0.560 |
| symbol_info        |                   67 |                   47 |             0.701 |
| **TOTAL**          |             **3 176** |             **2 392** |         **0.753** |

> **Note:** `query` (Hyphae) and `importers_tree` corpus captures failed at runtime
> (unknown error on the ripgrep fixture — these tools need separate investigation).
> The six captured fixtures represent the primary high-traffic tools.

---

## Aggregate summary

| Metric                              | Value   |
|-------------------------------------|---------|
| Total JSON tokens (cl100k_base)     |  3 176  |
| Total Text tokens (cl100k_base)     |  2 392  |
| **text/json token ratio**           | **0.753** |
| **Token reduction %**               | **24.7%** |
| Ripgrep index size                  | 101 files, ~850 nodes |

---

## Charter §2 binding result

> ⚠️ **BINDING TEST FAILS.** `bpe_charter_sla_binding` asserts ratio ≤ 0.30; measured 0.753.

Charter §2 claims:
> "AI token efficiency (Hyphae DSL vs JSON) | ≤ 30% of JSON token count for the same payload"

This means `TextFormatter` output should use ≤ 30% as many BPE tokens as `JsonFormatter` output for the same data — a **70%+ reduction**. The real measurement shows only **24.7% reduction**.

### Why the gap is so large

`TextFormatter` emits the same symbol paths as `JsonFormatter`, minus JSON structural tokens (`{`, `}`, `"key":`, `,`). JSON structural overhead is approximately 25% of total tokens for these tool outputs — not the 70% that would be needed to meet the ≤30% ratio.

For the ratio to reach 0.30, the text format would need to either:
1. **Omit ~70% of the JSON content entirely** (e.g. by eliding paths and emitting only counts / summaries), OR
2. **Use a fundamentally different representation** (e.g. Hyphae query results vs raw JSON dumps of multi-thousand-symbol graphs).

### Decision required (founder, Charter §2 governance event)

Per RFC-0120 §Decision, one of the following is required:

| Option | Action |
|--------|--------|
| **A — Retract claim** | Amend Charter §2: replace "≤ 30% of JSON token count" with the honest figure (≈ 25% reduction / ratio ≈ 0.75). Update README accordingly. |
| **B — Redesign formatter** | Redesign `TextFormatter` to emit abbreviated output that genuinely achieves ≥70% token reduction (e.g. short symbol IDs, count-only summaries, Hyphae-native output). |
| **C — Reframe the claim** | Clarify that the "≤30%" claim applies to *Hyphae query syntax vs JSON API calls* (a different comparison), and measure that instead. |

**Recommendation**: Option A (retract/correct) is the honest and fastest path. The `TextFormatter` is a real improvement (~25% reduction) — just not the improvement the charter claimed. Options B/C are higher-value but require more design work.

**Blocking**: Charter §2 amendment requires BDFL (founder) approval per Charter §9.

---

## Next steps

1. **Founder decision**: choose Option A, B, or C above.
2. If Option A: open `meta` RFC to amend Charter §2; update README + skills/; close RFC-0120 as "implemented (corrected)".
3. If Option B: design new TextFormatter (new RFC); this report's corpus is the benchmark baseline.
4. If Option C: define the Hyphae-vs-JSON comparison, capture the corpus, re-measure.
