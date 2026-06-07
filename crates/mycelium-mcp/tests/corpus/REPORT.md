# RFC-0120 Token-Density Corpus Report

**Corpus version**: v1-synthetic (Phase 1a scaffolding — small Mycelium self-index fixtures)  
**Tokenizer**: tiktoken-rs `cl100k_base` (GPT-4o / Claude-adjacent BPE family)  
**Measured**: 2026-06-07  
**Regenerate with**: `scripts/capture_token_corpus.sh` (Phase 1b: replaces with real ripgrep output)

> ⚠️ **This corpus is Phase 1a synthetic scaffolding.** The Charter §2 binding
> assertion (`TextFormatter` ≤ 30% of `JsonFormatter` tokens) requires the real
> ripgrep corpus captured via `scripts/capture_token_corpus.sh`. The numbers below
> are informational; they show the BPE infrastructure works but are NOT the
> figure-of-record for marketing claims or Charter §2.

---

## Per-fixture measurement

| Fixture            | JsonFormatter tokens | TextFormatter tokens | Ratio (text/json) |
|--------------------|---------------------:|---------------------:|------------------:|
| callee_tree        |                  419 |                  339 |             0.809 |
| caller_tree        |                  331 |                  260 |             0.785 |
| context            |                  375 |                  301 |             0.803 |
| importers_tree     |                  247 |                  186 |             0.753 |
| query              |                  270 |                  205 |             0.759 |
| search_symbol      |                  482 |                  360 |             0.747 |
| subclasses_tree    |                  206 |                  154 |             0.748 |
| symbol_info        |                  203 |                  154 |             0.759 |
| **TOTAL**          |             **2533** |             **1959** |         **0.773** |

## Aggregate summary

| Metric                              | Value   |
|-------------------------------------|---------|
| Total JSON tokens (cl100k_base)     |  2 533  |
| Total Text tokens (cl100k_base)     |  1 959  |
| **text/json token ratio**           | **0.773** |
| **Token reduction %**               | **22.7%** |
| Byte reduction %                    |  ~25%   |

## Interpretation (Phase 1a)

- `TextFormatter` uses **22.7% fewer BPE tokens** than `JsonFormatter` over this corpus.
- Charter §2 claims "≤ 30% of JSON token count" (i.e. ratio ≤ 0.30 = 70% reduction).
- **The current synthetic corpus does NOT validate this claim** (ratio 0.773 ≫ 0.30).
- The synthetic fixtures are small (200–480 JSON tokens each); real large tool outputs
  have proportionally more structural JSON overhead, so the real ratio is expected to be
  significantly lower.

## Next steps (Phase 1b)

1. Build the `mycelium` release binary:
   ```
   cargo build --release
   export PATH="$PWD/target/release:$PATH"
   ```
2. Run the capture script against the ripgrep fixture:
   ```
   ./scripts/capture_token_corpus.sh tests/e2e/fixtures/ripgrep/
   ```
3. Re-run measurement and update this file:
   ```
   MYCELIUM_REAL_CORPUS=1 cargo test --package mycelium-rcig-mcp \
     --test token_corpus --features tiktoken -- bpe --nocapture
   ```
4. If `bpe_charter_sla_binding` passes (ratio ≤ 0.30), commit the updated corpus
   and this REPORT.md and update `README.md` + Charter §2 footnotes with
   "measured with cl100k_base over corpus v2 (ripgrep)".
5. If `bpe_charter_sla_binding` fails (ratio > 0.30), the claim must be retracted
   per RFC-0120 §Decision and founder notified (Charter §2 amendment = governance event).
