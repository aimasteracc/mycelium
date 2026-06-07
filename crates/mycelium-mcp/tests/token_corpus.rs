//! RFC-0120 Phase 1 — corpus snapshot test.
//!
//! Loads committed JSON fixtures from `tests/corpus/`, runs `measure_corpus` with
//! `WhitespaceTokenCounter`, and asserts sanity properties that must hold regardless
//! of tokenizer choice.
//!
//! The tiktoken-gated snapshot (BPE token counts + tight band assertion) is Phase 2.
//! No `Store`, no indexing, no network — purely static string/formatter arithmetic.

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
    assert!(
        corpus.len() >= 8,
        "corpus must have ≥8 fixtures for a meaningful aggregate; found {}",
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
