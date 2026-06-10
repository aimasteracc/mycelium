//! RFC-0120 Phase 1 — token-accounting module.
//!
//! Measures `JsonFormatter` vs `TextFormatter` token counts over a committed corpus,
//! making the Charter §2 token-efficiency claim machine-verified rather than
//! hand-asserted.
//!
//! `WhitespaceTokenCounter` is hermetic (no external deps) and used in unit tests.
//! `BpeTokenCounter` (`cl100k_base`) is gated behind the `tiktoken` Cargo feature
//! and is the figure-of-record for README / Charter §2 claims.

use crate::formatter::{Formatter, JsonFormatter, TextFormatter};
use serde_json::Value;

/// One captured tool output used as a measurement input.
pub struct FixtureCase {
    /// Human-readable name, typically the tool name (e.g. `mycelium_context`).
    pub name: String,
    /// The JSON payload that both formatters receive.
    pub value: Value,
}

/// Per-fixture measurement result from [`measure_case`].
pub struct FixtureReport {
    /// Name from the originating [`FixtureCase`].
    pub name: String,
    /// Token count of the `JsonFormatter` output (pretty-printed; the JSON baseline
    /// agents receive on `--format json`).
    pub json_tokens: usize,
    /// Token count of the `TextFormatter` output (TOON; the primary MCP output format).
    pub text_tokens: usize,
    /// Byte count of the `JsonFormatter` output.
    pub json_bytes: usize,
    /// Byte count of the `TextFormatter` output.
    pub text_bytes: usize,
}

/// Aggregated results over an entire corpus, produced by [`measure_corpus`].
pub struct CorpusReport {
    /// Per-fixture reports, in the same order as the input corpus slice.
    pub fixtures: Vec<FixtureReport>,
    /// Sum of [`FixtureReport::json_tokens`] across all fixtures.
    pub total_json_tokens: usize,
    /// Sum of [`FixtureReport::text_tokens`] across all fixtures.
    pub total_text_tokens: usize,
    /// Sum of [`FixtureReport::json_bytes`] across all fixtures.
    pub total_json_bytes: usize,
    /// Sum of [`FixtureReport::text_bytes`] across all fixtures.
    pub total_text_bytes: usize,
}

impl CorpusReport {
    /// `100 * (1 - text_tokens / json_tokens)`.
    ///
    /// Positive values mean `TextFormatter` uses fewer tokens than `JsonFormatter`.
    /// Returns `0.0` when `total_json_tokens == 0` to avoid divide-by-zero.
    #[must_use]
    pub fn token_reduction_pct(&self) -> f64 {
        if self.total_json_tokens == 0 {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        let ratio = self.total_text_tokens as f64 / self.total_json_tokens as f64;
        100.0 * (1.0 - ratio)
    }

    /// `total_text_tokens / total_json_tokens`.
    ///
    /// Primary metric for Charter §2 (≤ 30 % of JSON token count target).
    /// Returns `1.0` when `total_json_tokens == 0` to avoid divide-by-zero.
    #[must_use]
    pub fn text_to_json_token_ratio(&self) -> f64 {
        if self.total_json_tokens == 0 {
            return 1.0;
        }
        #[allow(clippy::cast_precision_loss)]
        {
            self.total_text_tokens as f64 / self.total_json_tokens as f64
        }
    }

    /// `100 * (1 - text_bytes / json_bytes)`. Byte-level reduction (secondary metric).
    ///
    /// Returns `0.0` when `total_json_bytes == 0`.
    #[must_use]
    pub fn byte_reduction_pct(&self) -> f64 {
        if self.total_json_bytes == 0 {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        let ratio = self.total_text_bytes as f64 / self.total_json_bytes as f64;
        100.0 * (1.0 - ratio)
    }
}

/// Abstraction over token-counting strategies.
///
/// Allows [`measure_case`] / [`measure_corpus`] to run hermetically in unit tests
/// (`WhitespaceTokenCounter`) and with the figure-of-record BPE counter in CI.
pub trait TokenCounter {
    /// Return the number of tokens in `s`.
    fn count(&self, s: &str) -> usize;
}

/// Hermetic, dependency-free token counter.
///
/// Each run of alphanumeric + underscore chars is one token; each non-whitespace,
/// non-alphanumeric character is its own pseudo-token (approximating BPE behaviour
/// where JSON structural characters are individually tokenised).
///
/// Used in unit tests only — NOT the figure-of-record (`BpeTokenCounter` is).
#[derive(Debug, Default, Clone, Copy)]
pub struct WhitespaceTokenCounter;

/// BPE token counter using tiktoken-rs `cl100k_base` encoding.
///
/// This is the **figure-of-record** tokenizer for Charter §2 "AI token efficiency
/// (Hyphae DSL vs JSON) ≤ 30% of JSON token count" SLA (RFC-0120 Phase 1b).
///
/// Gated behind the `tiktoken` cargo feature so ordinary CI remains hermetic.
/// Run `cargo test --features tiktoken` to execute the binding corpus assertion.
///
/// The tokenizer family (`cl100k_base`) is the GPT-4o / Claude-adjacent BPE used
/// by RFC-0094 when it first asserted the ~70% claim.  The stated assumption is
/// committed in `tests/corpus/REPORT.md` so the measurement is reproducible.
#[cfg(feature = "tiktoken")]
pub struct BpeTokenCounter {
    bpe: tiktoken_rs::CoreBPE,
}

#[cfg(feature = "tiktoken")]
impl BpeTokenCounter {
    /// Construct a counter backed by the `cl100k_base` BPE encoding.
    ///
    /// # Panics
    ///
    /// Panics if the tiktoken-rs encoding data cannot be loaded (should only
    /// happen if the tiktoken-rs crate is misconfigured).
    #[must_use]
    pub fn cl100k_base() -> Self {
        Self {
            bpe: tiktoken_rs::cl100k_base().expect("tiktoken-rs cl100k_base load failed"),
        }
    }
}

#[cfg(feature = "tiktoken")]
impl TokenCounter for BpeTokenCounter {
    fn count(&self, s: &str) -> usize {
        self.bpe.encode_ordinary(s).len()
    }
}

impl TokenCounter for WhitespaceTokenCounter {
    fn count(&self, s: &str) -> usize {
        let mut count = 0usize;
        let mut in_alnum = false;
        for ch in s.chars() {
            if ch.is_alphanumeric() || ch == '_' {
                if !in_alnum {
                    count += 1;
                    in_alnum = true;
                }
            } else if !ch.is_whitespace() {
                // Each punctuation/structural character is its own pseudo-token,
                // matching BPE behaviour for JSON brackets, quotes, and colons.
                count += 1;
                in_alnum = false;
            } else {
                in_alnum = false;
            }
        }
        count
    }
}

/// Measure one fixture case, formatting through both `JsonFormatter` and
/// `TextFormatter` and counting tokens + bytes via the given counter.
///
/// The JSON baseline is `JsonFormatter` (pretty-printed, `to_string_pretty`) — the
/// same output agents receive on `--format json`. This is NOT compact JSON.
pub fn measure_case<C: TokenCounter>(case: &FixtureCase, counter: &C) -> FixtureReport {
    let json_str = JsonFormatter.format(&case.value);
    let text_str = TextFormatter::default().format(&case.value);
    FixtureReport {
        name: case.name.clone(),
        json_tokens: counter.count(&json_str),
        text_tokens: counter.count(&text_str),
        json_bytes: json_str.len(),
        text_bytes: text_str.len(),
    }
}

/// Measure an entire corpus. Returns per-fixture reports plus aggregated totals.
pub fn measure_corpus<C: TokenCounter>(corpus: &[FixtureCase], counter: &C) -> CorpusReport {
    let fixtures: Vec<FixtureReport> = corpus.iter().map(|c| measure_case(c, counter)).collect();
    let total_json_tokens = fixtures.iter().map(|r| r.json_tokens).sum();
    let total_text_tokens = fixtures.iter().map(|r| r.text_tokens).sum();
    let total_json_bytes = fixtures.iter().map(|r| r.json_bytes).sum();
    let total_text_bytes = fixtures.iter().map(|r| r.text_bytes).sum();
    CorpusReport {
        fixtures,
        total_json_tokens,
        total_text_tokens,
        total_json_bytes,
        total_text_bytes,
    }
}

/// Build the `mycelium_get_token_stats` / `mycelium get-token-stats` payload.
///
/// Shared by the MCP tool handler and the CLI subcommand so both surfaces emit
/// structurally identical JSON (RFC-0120 Phase 3B byte-identity contract).
///
/// The corpus is embedded at compile time via `include_str!` from
/// `crates/mycelium-mcp/tests/corpus/`.
///
/// # Panics
///
/// Panics if the embedded corpus JSON files are invalid (would indicate a broken
/// build) or if `tiktoken-rs` fails to load `cl100k_base` when the `tiktoken`
/// feature is enabled.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn token_stats_payload() -> serde_json::Value {
    let corpus = vec![
        FixtureCase {
            name: "callee_tree".to_owned(),
            value: serde_json::from_str(include_str!("../tests/corpus/callee_tree.json"))
                .expect("corpus callee_tree.json is valid JSON"),
        },
        FixtureCase {
            name: "caller_tree".to_owned(),
            value: serde_json::from_str(include_str!("../tests/corpus/caller_tree.json"))
                .expect("corpus caller_tree.json is valid JSON"),
        },
        FixtureCase {
            name: "context".to_owned(),
            value: serde_json::from_str(include_str!("../tests/corpus/context.json"))
                .expect("corpus context.json is valid JSON"),
        },
        FixtureCase {
            name: "search_symbol".to_owned(),
            value: serde_json::from_str(include_str!("../tests/corpus/search_symbol.json"))
                .expect("corpus search_symbol.json is valid JSON"),
        },
        FixtureCase {
            name: "subclasses_tree".to_owned(),
            value: serde_json::from_str(include_str!("../tests/corpus/subclasses_tree.json"))
                .expect("corpus subclasses_tree.json is valid JSON"),
        },
        FixtureCase {
            name: "symbol_info".to_owned(),
            value: serde_json::from_str(include_str!("../tests/corpus/symbol_info.json"))
                .expect("corpus symbol_info.json is valid JSON"),
        },
    ];

    #[cfg(feature = "tiktoken")]
    let (report, tokenizer) = {
        use crate::token_bench::BpeTokenCounter;
        (
            measure_corpus(&corpus, &BpeTokenCounter::cl100k_base()),
            "cl100k_base",
        )
    };
    #[cfg(not(feature = "tiktoken"))]
    let (report, tokenizer) = {
        (
            measure_corpus(&corpus, &WhitespaceTokenCounter),
            "whitespace-approximate",
        )
    };

    // Secondary metric: JSON-vs-MessagePack byte ratio (wire-format, NOT the token ratio).
    let wire_sample = serde_json::json!({"matches": ["a", "b", "c"]});
    let wire_json_bytes = wire_sample.to_string().len();
    let wire_msgpack_bytes = rmp_serde::to_vec_named(&wire_sample)
        .unwrap_or_default()
        .len();
    let wire_format_byte_ratio = wire_msgpack_bytes as f64 / wire_json_bytes as f64;

    let fixtures: Vec<serde_json::Value> = report
        .fixtures
        .iter()
        .map(|f| {
            let ratio = if f.json_tokens > 0 {
                f.text_tokens as f64 / f.json_tokens as f64
            } else {
                1.0
            };
            serde_json::json!({
                "name": f.name,
                "json_tokens": f.json_tokens,
                "text_tokens": f.text_tokens,
                "ratio": ratio,
            })
        })
        .collect();

    serde_json::json!({
        "tokenizer": tokenizer,
        "corpus_version": "v2-ripgrep",
        "fixtures": fixtures,
        "aggregate_json_tokens": report.total_json_tokens,
        "aggregate_text_tokens": report.total_text_tokens,
        "text_to_json_token_ratio": report.text_to_json_token_ratio(),
        "token_reduction_pct": report.token_reduction_pct(),
        "wire_format_byte_ratio": wire_format_byte_ratio,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_counter_empty() {
        assert_eq!(WhitespaceTokenCounter.count(""), 0);
    }

    #[test]
    fn whitespace_counter_simple_words() {
        assert_eq!(WhitespaceTokenCounter.count("hello world"), 2);
    }

    #[test]
    fn whitespace_counter_json_structure() {
        // JSON structural chars ("{", "}", "\"", ":") each count as separate tokens.
        let json = r#"{"key": "value"}"#;
        // At minimum: `{`, `"`, key, `"`, `:`, `"`, value, `"`, `}` = 9
        assert!(
            WhitespaceTokenCounter.count(json) >= 7,
            "json structural chars should produce multiple tokens"
        );
    }

    #[test]
    fn measure_case_produces_correct_name() {
        let case = FixtureCase {
            name: "test_fixture".to_owned(),
            value: serde_json::json!({"symbols": ["a", "b", "c"]}),
        };
        let report = measure_case(&case, &WhitespaceTokenCounter);
        assert_eq!(report.name, "test_fixture");
    }

    #[test]
    fn measure_case_text_uses_fewer_tokens_than_json() {
        // TextFormatter drops "{", "}", "\"", "[", "]", "," structural tokens.
        let case = FixtureCase {
            name: "multi_field".to_owned(),
            value: serde_json::json!({
                "symbols": ["src/main.rs>main", "src/lib.rs>process", "src/store.rs>Store"],
                "count": 3,
                "truncated": false
            }),
        };
        let report = measure_case(&case, &WhitespaceTokenCounter);
        assert!(
            report.text_tokens < report.json_tokens,
            "text tokens ({}) should be less than json tokens ({})",
            report.text_tokens,
            report.json_tokens
        );
    }

    #[test]
    fn measure_corpus_empty_returns_zeros() {
        let report = measure_corpus(&[], &WhitespaceTokenCounter);
        assert_eq!(report.total_json_tokens, 0);
        assert_eq!(report.total_text_tokens, 0);
        assert_eq!(report.total_json_bytes, 0);
        assert_eq!(report.total_text_bytes, 0);
        assert_eq!(report.fixtures.len(), 0);
    }

    #[test]
    fn token_reduction_pct_formula() {
        // 100 * (1 - 30/100) = 70.0
        let report = CorpusReport {
            fixtures: vec![],
            total_json_tokens: 100,
            total_text_tokens: 30,
            total_json_bytes: 200,
            total_text_bytes: 60,
        };
        let pct = report.token_reduction_pct();
        assert!((pct - 70.0).abs() < 0.001, "expected ~70.0, got {pct}");
    }

    #[test]
    fn text_to_json_token_ratio_formula() {
        // 30 / 100 = 0.30
        let report = CorpusReport {
            fixtures: vec![],
            total_json_tokens: 100,
            total_text_tokens: 30,
            total_json_bytes: 200,
            total_text_bytes: 60,
        };
        let ratio = report.text_to_json_token_ratio();
        assert!((ratio - 0.30).abs() < 0.001, "expected ~0.30, got {ratio}");
    }

    #[test]
    fn zero_json_tokens_does_not_divide_by_zero() {
        let report = CorpusReport {
            fixtures: vec![],
            total_json_tokens: 0,
            total_text_tokens: 0,
            total_json_bytes: 0,
            total_text_bytes: 0,
        };
        assert!(report.token_reduction_pct().abs() < 0.001, "expected 0.0");
        assert!(
            (report.text_to_json_token_ratio() - 1.0).abs() < 0.001,
            "expected 1.0"
        );
        assert!(report.byte_reduction_pct().abs() < 0.001, "expected 0.0");
    }
}
