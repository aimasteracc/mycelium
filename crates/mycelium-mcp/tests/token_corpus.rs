//! RFC-0120 Phase 1 — corpus snapshot test.
//!
//! Loads committed JSON fixtures from `tests/corpus/`, runs `measure_corpus` with
//! `WhitespaceTokenCounter`, and asserts sanity properties that must hold regardless
//! of tokenizer choice.
//!
//! The tiktoken-gated snapshot (BPE token counts + tight band assertion) requires the
//! `tiktoken` cargo feature (`cargo test --features tiktoken`).
//! No `Store`, no indexing, no network — purely static string/formatter arithmetic.

#[cfg(feature = "tiktoken")]
use mycelium_mcp::token_bench::BpeTokenCounter;
use mycelium_mcp::token_bench::{FixtureCase, WhitespaceTokenCounter, measure_corpus};
use std::path::Path;

fn load_corpus() -> Vec<FixtureCase> {
    let corpus_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
    let mut cases = Vec::new();
    for entry in std::fs::read_dir(&corpus_dir)
        .expect("tests/corpus/ directory must exist")
        .flatten()
    {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            let name = path
                .file_stem()
                .expect("file has stem")
                .to_str()
                .expect("valid UTF-8")
                .to_owned();
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
            let value: serde_json::Value = serde_json::from_str(&content)
                .unwrap_or_else(|e| panic!("invalid JSON in {}: {e}", path.display()));
            cases.push(FixtureCase { name, value });
        }
    }
    // Deterministic ordering so test output is stable.
    cases.sort_by(|a, b| a.name.cmp(&b.name));
    cases
}

#[test]
fn corpus_has_minimum_fixture_count() {
    let corpus = load_corpus();
    // Real ripgrep corpus has 6 fixtures; `query` and `importers_tree` captures
    // failed on this codebase (documented in REPORT.md). 6 is the honest minimum.
    assert!(
        corpus.len() >= 6,
        "corpus must have ≥6 fixtures for a meaningful aggregate; found {}",
        corpus.len()
    );
}

#[test]
fn all_fixture_names_are_non_empty() {
    let corpus = load_corpus();
    for case in &corpus {
        assert!(!case.name.is_empty(), "fixture name must not be empty");
    }
}

#[test]
fn text_format_reduces_tokens_over_corpus() {
    let corpus = load_corpus();
    let report = measure_corpus(&corpus, &WhitespaceTokenCounter);
    assert!(
        report.text_to_json_token_ratio() < 1.0,
        "TextFormatter must use fewer tokens than JsonFormatter over the corpus; ratio = {:.4}",
        report.text_to_json_token_ratio()
    );
}

#[test]
fn token_reduction_pct_is_positive() {
    let corpus = load_corpus();
    let report = measure_corpus(&corpus, &WhitespaceTokenCounter);
    assert!(
        report.token_reduction_pct() > 0.0,
        "token_reduction_pct must be positive; got {:.2}%",
        report.token_reduction_pct()
    );
}

#[test]
fn per_fixture_totals_sum_to_aggregate() {
    let corpus = load_corpus();
    let report = measure_corpus(&corpus, &WhitespaceTokenCounter);
    let sum_json: usize = report.fixtures.iter().map(|r| r.json_tokens).sum();
    let sum_text: usize = report.fixtures.iter().map(|r| r.text_tokens).sum();
    assert_eq!(
        sum_json, report.total_json_tokens,
        "per-fixture json_tokens must sum to total_json_tokens"
    );
    assert_eq!(
        sum_text, report.total_text_tokens,
        "per-fixture text_tokens must sum to total_text_tokens"
    );
}

// ---------------------------------------------------------------------------
// RFC-0120 Phase 1b — BPE (tiktoken cl100k_base) figure-of-record tests.
//
// These tests are gated behind `--features tiktoken` so normal CI stays
// hermetic. They are the binding measurement for Charter §2 "AI token
// efficiency" SLA (amended by RFC-0121 Option A to per-class thresholds).
// ---------------------------------------------------------------------------

/// Measures `TextFormatter` vs `JsonFormatter` BPE token ratio over the committed corpus.
///
/// **Current corpus is Phase 1a scaffolding** (small Mycelium self-index fixtures).
/// Phase 1b captures real ripgrep outputs via `scripts/capture_token_corpus.sh`.
/// The Charter §2 binding assertion (per-class thresholds per RFC-0121 Option A)
/// activates after real corpus lands.
///
/// This test always passes — it prints the measured ratio and fails loudly only when
/// the ratio exceeds 1.0 (`TextFormatter` makes things *larger* than `JsonFormatter`).
/// The `bpe_charter_sla_binding` test below holds the gated binding assertion.
#[cfg(feature = "tiktoken")]
#[test]
fn bpe_text_to_json_ratio_informational() {
    let corpus = load_corpus();
    let counter = BpeTokenCounter::cl100k_base();
    let report = measure_corpus(&corpus, &counter);
    let ratio = report.text_to_json_token_ratio();
    // Print for visibility in CI output.
    println!(
        "RFC-0120 BPE measurement (cl100k_base, corpus v1-synthetic):\n  \
         ratio = {ratio:.4} ({pct:.1}% of JSON tokens)  reduction = {red:.1}%\n  \
         json_tokens={jt}  text_tokens={tt}\n  \
         NOTE: corpus is Phase 1a synthetic fixtures. \
         Regenerate with scripts/capture_token_corpus.sh for the binding figure.",
        pct = ratio * 100.0,
        red = report.token_reduction_pct(),
        jt = report.total_json_tokens,
        tt = report.total_text_tokens,
    );
    // Infrastructure sanity: TextFormatter must never inflate token count.
    assert!(
        ratio < 1.0,
        "TextFormatter must not increase BPE token count vs JsonFormatter; ratio = {ratio:.4}"
    );
}

/// Charter §2 per-class binding assertion (RFC-0121 Option A).
///
/// Replaces the single `≤ 30%` aggregate with three per-response-class targets:
///
/// | Class  | Threshold | Matched by                                   |
/// |--------|-----------|----------------------------------------------|
/// | tree   | ≤ 35%     | fixture names ending in `_tree`              |
/// | list   | ≤ 70%     | all other fixtures (default)                 |
/// | scalar | ≤ 90%     | names ending in `_info`, `_status`, `_count` |
///
/// Classification uses fixture name conventions so no JSON metadata changes are needed.
/// Gated on `MYCELIUM_REAL_CORPUS=1` (unchanged) — skip on the Phase 1a synthetic corpus
/// (which produces ~77% ratio on small fixtures — not representative of large real outputs).
///
/// See [RFC-0121](../../rfcs/0121-charter-hyphae-token-sla-amendment.md) for rationale,
/// measured production averages (75.3% overall), and Option A vs B vs C analysis.
#[cfg(feature = "tiktoken")]
#[test]
#[allow(clippy::cast_precision_loss)]
fn bpe_charter_sla_binding() {
    if std::env::var("MYCELIUM_REAL_CORPUS").is_err() {
        println!(
            "SKIP: bpe_charter_sla_binding — set MYCELIUM_REAL_CORPUS=1 after running \
             scripts/capture_token_corpus.sh to activate the Charter §2 binding assertion."
        );
        return;
    }
    let corpus = load_corpus();
    let counter = BpeTokenCounter::cl100k_base();
    let report = measure_corpus(&corpus, &counter);

    // Accumulate per-class token totals using fixture name conventions.
    let mut tree = (0usize, 0usize); // (json_tokens, text_tokens)
    let mut list = (0usize, 0usize);
    let mut scalar = (0usize, 0usize);

    for r in &report.fixtures {
        let bucket = if r.name.ends_with("_tree") || r.name.contains("_tree_") {
            &mut tree
        } else if r.name.ends_with("_info")
            || r.name.ends_with("_status")
            || r.name.ends_with("_count")
        {
            &mut scalar
        } else {
            &mut list
        };
        bucket.0 += r.json_tokens;
        bucket.1 += r.text_tokens;
    }

    // Assert RFC-0121 Option A per-class thresholds.
    for (class, (json, text), limit) in [
        ("tree", tree, 0.35_f64),
        ("list", list, 0.70_f64),
        ("scalar", scalar, 0.90_f64),
    ] {
        if json == 0 {
            continue;
        }
        let ratio = text as f64 / json as f64;
        assert!(
            ratio <= limit,
            "Charter §2 SLA (RFC-0121 Option A): {class} responses must use \
             ≤{limit_pct:.0}% of JSON tokens; measured {ratio:.4} ({measured_pct:.1}%). \
             Regenerate corpus: scripts/capture_token_corpus.sh",
            limit_pct = limit * 100.0,
            measured_pct = ratio * 100.0,
        );
    }
}

/// Sanity: BPE also reduces tokens vs JSON (same direction as whitespace counter).
#[cfg(feature = "tiktoken")]
#[test]
fn bpe_text_format_reduces_tokens_over_corpus() {
    let corpus = load_corpus();
    let counter = BpeTokenCounter::cl100k_base();
    let report = measure_corpus(&corpus, &counter);
    assert!(
        report.text_to_json_token_ratio() < 1.0,
        "TextFormatter must use fewer BPE tokens than JsonFormatter; ratio = {:.4}",
        report.text_to_json_token_ratio()
    );
}

/// Sanity: BPE counter returns non-zero for a non-empty input.
#[cfg(feature = "tiktoken")]
#[test]
fn bpe_counter_non_zero_for_nonempty_input() {
    use mycelium_mcp::token_bench::TokenCounter;
    let counter = BpeTokenCounter::cl100k_base();
    assert!(
        counter.count("hello world") > 0,
        "BpeTokenCounter must return a positive count for non-empty input"
    );
}

/// Prints per-fixture BPE breakdown for REPORT.md generation.
#[cfg(feature = "tiktoken")]
#[test]
#[allow(clippy::cast_precision_loss)]
fn bpe_per_fixture_breakdown() {
    use mycelium_mcp::token_bench::measure_case;
    let corpus = load_corpus();
    let counter = BpeTokenCounter::cl100k_base();
    println!(
        "\n{:<24} {:>10} {:>10} {:>8}",
        "fixture", "json_tok", "text_tok", "ratio"
    );
    println!("{}", "-".repeat(56));
    for case in &corpus {
        let r = measure_case(case, &counter);
        let ratio = if r.json_tokens > 0 {
            r.text_tokens as f64 / r.json_tokens as f64
        } else {
            1.0
        };
        println!(
            "{:<24} {:>10} {:>10} {:>7.3}",
            r.name, r.json_tokens, r.text_tokens, ratio,
        );
    }
    let report = measure_corpus(&corpus, &counter);
    println!("{}", "-".repeat(56));
    println!(
        "{:<24} {:>10} {:>10} {:>7.3}",
        "TOTAL",
        report.total_json_tokens,
        report.total_text_tokens,
        report.text_to_json_token_ratio(),
    );
}

/// Sanity: BPE counter returns zero for empty string.
#[cfg(feature = "tiktoken")]
#[test]
fn bpe_counter_zero_for_empty_input() {
    use mycelium_mcp::token_bench::TokenCounter;
    let counter = BpeTokenCounter::cl100k_base();
    assert_eq!(
        counter.count(""),
        0,
        "BpeTokenCounter must return 0 for empty string"
    );
}
