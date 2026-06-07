# ADR-0011: tiktoken-rs as the BPE Figure-of-Record Tokenizer

**Status**: Accepted  
**Date**: 2026-06-07  
**RFC**: [RFC-0120](../../rfcs/0120-token-density-measurement-honesty.md)  
**Deciders**: orchestrator (PM), founder (implicit via RFC-0120 ratification)

---

## Context

Mycelium's `README.md` and Charter §2 assert "≤ 30% of JSON token count for the same
payload" for `TextFormatter` output. RFC-0120 found these claims were:

1. **Unsubstantiated** — never machine-verified against a real tokenizer.
2. **Wrong-axis** — `mycelium_get_token_stats` measured JSON-vs-MessagePack *bytes*,
   not Text-vs-JSON *tokens*.

A specific, stated BPE tokenizer is needed to make the claim reproducible: "X tokens
per `cl100k_base` over corpus vN" is falsifiable; "X tokens" alone is not.

---

## Decision

Use **tiktoken-rs** (`cl100k_base` encoding) as the figure-of-record BPE tokenizer
for RFC-0120 token-efficiency measurements.

- Crate: [`tiktoken-rs`](https://crates.io/crates/tiktoken-rs) (MIT)
- Encoding: `cl100k_base` — the GPT-4o / Claude-adjacent BPE family that RFC-0094
  implicitly referenced when it first asserted the ~70% reduction claim.
- Feature-gated: `tiktoken` cargo feature on `mycelium-rcig-mcp` so ordinary CI
  stays hermetic (no network, no heavy deps for standard test runs).

---

## Rationale

**Why tiktoken-rs?**
- Same BPE family RFC-0094 referenced ("gpt-4o tokeniser").
- MIT license (passes `cargo deny`).
- Pure-Rust, no Python, no network (all BPE data embedded in the crate).
- `cl100k_base()` + `encode_ordinary()` is a stable, well-documented API.

**Why cl100k_base specifically?**
- It is the tokenizer most AI agents consuming Mycelium output are likely using.
- It was the implicit assumption behind the original 70% claim in RFC-0094.
- Changing to a different tokenizer would require re-baselining the claim; this ADR
  pins the assumption so any such change is explicit and auditable.

**Why optional feature, not always-on?**
- tiktoken-rs adds a non-trivial dependency tree (fancy-regex etc.).
- The hermetic `WhitespaceTokenCounter` is sufficient for CI correctness checks
  (direction of reduction, per-fixture aggregation).
- The BPE measurement is a figure-of-record check, not a fast feedback check;
  it belongs in a deliberate "measure + commit" step, not every PR.

---

## Alternatives Considered

| Alternative | Rejected because |
|---|---|
| `tokenizers` (HuggingFace) | Heavier dep; different BPE family than RFC-0094 assumed. |
| `bpe` crate | Doesn't support cl100k_base natively. |
| Hand-count / bytes | Already done (RFC-0094); proved insufficient to back the claim. |
| OpenAI tiktoken (Python) | Python dependency breaks the pure-Rust embedded model. |

---

## Consequences

**Positive:**
- Charter §2 token-efficiency claim becomes machine-verifiable and CI-gated
  (under `MYCELIUM_REAL_CORPUS=1 cargo test --features tiktoken`).
- Any future change to `TextFormatter` output that degrades token efficiency
  is caught by the binding test.
- The stated tokenizer assumption is documented, so a consumer using a
  different tokenizer knows to re-measure.

**Negative / Risks:**
- tiktoken-rs version bumps may shift BPE counts slightly, requiring
  `REPORT.md` regeneration and a re-commit. Mitigated by pinning the
  tiktoken-rs version in `Cargo.lock`.
- The `cl100k_base` family may not be the tokenizer all consumers use
  (e.g., future models may use o200k_base). The claim is explicitly
  "per cl100k_base", so this is acknowledged, not hidden.

---

## Implementation

- `crates/mycelium-mcp/src/token_bench.rs`: `BpeTokenCounter` behind `#[cfg(feature = "tiktoken")]`
- `crates/mycelium-mcp/Cargo.toml`: `tiktoken = ["dep:tiktoken-rs"]` feature
- `Cargo.toml` (workspace): `tiktoken-rs = "0.6"` dependency
- `crates/mycelium-mcp/tests/token_corpus.rs`: 5 BPE tests (4 sanity + 1 Charter §2 binding)
- `crates/mycelium-mcp/tests/corpus/REPORT.md`: pinned per-fixture measurements
- `scripts/capture_token_corpus.sh`: reproducible corpus regeneration script
