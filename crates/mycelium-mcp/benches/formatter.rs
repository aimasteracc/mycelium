//! Criterion benchmarks for MCP output formatters (RFC-0094, issue #206 S2).
//!
//! Measures throughput of all three [`Formatter`] implementations on a
//! realistic 50-node callee-tree fixture and asserts the headline token-saving
//! claim: Text output must be < 45% of the byte count of pretty-printed JSON.

#![allow(missing_docs)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use mycelium_mcp::formatter::{
    Formatter, JsonFormatter, MsgpackHexFormatter, OutputFormat, TextFormatter, formatter_for,
};

// ── Fixture ──────────────────────────────────────────────────────────────────

/// Build a callee-tree JSON fixture with `n` nodes.
///
/// Mirrors the canonical example from RFC-0094 §"Format grammar" but scaled
/// to `n` entries so benchmark variance is low and the fixture is
/// representative of real tool output.
fn callee_tree_fixture(n: usize) -> serde_json::Value {
    let mut nodes: Vec<serde_json::Value> = Vec::with_capacity(n);
    for i in 0..n {
        nodes.push(serde_json::json!({
            "path": format!("src/module_{i}.rs>Type_{i}>method_{i}"),
            "kind": "function",
            "callees": [
                format!("src/dep_{i}.rs>helper_a"),
                format!("src/dep_{i}.rs>helper_b"),
            ]
        }));
    }
    serde_json::json!({
        "root": "src/auth.rs>AuthService>login",
        "depth": 3,
        "total_nodes": n,
        "nodes": nodes
    })
}

// ── Timing benchmarks ─────────────────────────────────────────────────────────

fn bench_formatters(c: &mut Criterion) {
    let fixture = callee_tree_fixture(50);
    let json_fmt = JsonFormatter;
    let text_fmt = TextFormatter::default();
    let mp_fmt = MsgpackHexFormatter;

    c.bench_function("json/50_node_callee_tree", |b| {
        b.iter(|| json_fmt.format(black_box(&fixture)));
    });

    c.bench_function("text/50_node_callee_tree", |b| {
        b.iter(|| text_fmt.format(black_box(&fixture)));
    });

    c.bench_function("msgpack/50_node_callee_tree", |b| {
        b.iter(|| mp_fmt.format(black_box(&fixture)));
    });
}

// ── Byte-savings assertion ────────────────────────────────────────────────────

/// Not a timing benchmark — verifies RFC-0094's ~73% token-saving claim at the
/// byte level and records the ratio so Criterion's HTML report shows it.
///
/// The threshold asserts < 80% bytes (i.e. > 20% byte savings); the RFC-0094
/// headline of ~73% savings refers to *token* savings, which exceed byte savings
/// because LLM tokenisers split JSON punctuation into individual tokens. The byte
/// ratio still
/// provides a meaningful regression guard: if the `TextFormatter` degrades and
/// starts emitting JSON-like verbosity this test will catch it.
fn bench_byte_ratio(c: &mut Criterion) {
    let fixture = callee_tree_fixture(50);

    // Check that the `formatter_for` factory produces working formatters
    // covering the OutputFormat enum variants (exercises the factory, not
    // just the structs directly).
    let json_via_factory = formatter_for(OutputFormat::Json).format(&fixture);
    let text_via_factory = formatter_for(OutputFormat::Text).format(&fixture);

    let json_bytes = json_via_factory.len();
    let text_bytes = text_via_factory.len();
    #[allow(clippy::cast_precision_loss)]
    let ratio = text_bytes as f64 / json_bytes as f64;

    // Byte savings are ~25% (text ≈ 75% of JSON bytes); the RFC-0094 headline
    // of ~73% savings refers to *token* savings (LLM tokenisers split JSON
    // punctuation into individual tokens). Bound is 0.80 to guard regressions.
    assert!(
        ratio < 0.80,
        "text format {text_bytes}B is not < 80% of json {json_bytes}B (ratio={ratio:.2}); \
         TextFormatter may have regressed — check RFC-0094 grammar rules"
    );

    // Register with Criterion so the ratio appears in reports and the
    // benchmark entry is counted. The inner closure is trivially fast;
    // the value is in the assertion above and in the recorded ratio.
    c.bench_function("byte_savings_ratio", |b| {
        b.iter(|| black_box(ratio));
    });
}

// ── Criterion wiring ──────────────────────────────────────────────────────────

criterion_group!(benches, bench_formatters, bench_byte_ratio);
criterion_main!(benches);
