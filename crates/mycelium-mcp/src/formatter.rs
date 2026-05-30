//! Output formatters for MCP tool responses (RFC-0094 Phase 1).
//!
//! This module ships the [`Formatter`] trait and three implementations:
//! [`JsonFormatter`], [`TextFormatter`], and [`MsgpackHexFormatter`]. The
//! [`OutputFormat`] enum + [`formatter_for`] factory let callers select a
//! representation per call.
//!
//! ## Why three formats?
//!
//! See [RFC-0094](../../../rfcs/0094-token-efficient-output.md) for the full
//! analysis. Short version:
//!
//! - **JSON**: human-readable, machine-readable, but `{`, `"`, `:`, `,`, `}`
//!   are all tokens with no semantic value. ~73% of structural punctuation
//!   on tree-shaped responses is pure noise.
//! - **Text** (TOON-inspired): indented `key: value` and `- item` layout.
//!   Drops quotes and brackets entirely; reserved characters are escaped
//!   per the grammar below. Round-trips through a tiny parser.
//! - **`MessagePack`-hex**: binary `MessagePack` encoded as ASCII hex.
//!   Smallest byte count of the three, but opaque to humans and almost
//!   tokeniser-pessimal because every byte becomes two ASCII chars.
//!
//! ## Text format grammar (RFC-0094 §"Format grammar")
//!
//! - Top-level object: `key:` then indented body.
//! - Scalar key/value: `key: value` (no quotes; bare strings).
//! - Lists: `- item` per line, indented under the parent key.
//! - Empty list: `[]` (disambiguates from "missing").
//! - Empty object: `{}` (disambiguates from "missing").
//! - Reserved characters that force the value to be JSON-string-quoted:
//!   leading `[`, leading `{`, leading `-`, leading `"`, or any occurrence
//!   of `: ` (colon-space) inside the string.
//! - Null becomes the bare word `null`. Booleans become `true` / `false`.
//! - Numbers are emitted via [`serde_json::Number`]'s `Display`.
//!
//! Quoting strategy: when a string needs escaping, it is re-emitted as a
//! standard JSON string literal (`serde_json::to_string`) — that way the
//! reference parser only needs to recognise one escape convention.

use serde_json::Value;

/// Indent width (spaces) used by [`TextFormatter`] for each nesting level.
const TEXT_INDENT_WIDTH: usize = 2;

/// Common interface for every output format Mycelium MCP supports.
///
/// Implementations MUST be pure (no I/O, no panics on well-formed JSON
/// input). They MAY allocate.
pub trait Formatter {
    /// Render `value` as a `String` in this formatter's representation.
    fn format(&self, value: &Value) -> String;
}

/// Selects which [`Formatter`] a tool response uses.
///
/// Defaults to [`OutputFormat::Text`] because the primary audience of
/// Mycelium's MCP surface is LLM agents (RFC-0094 §"Why mycelium ought to
/// lead here").
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Deserialize,
    serde::Serialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// TOON-inspired indented `key: value` text. Smallest token footprint
    /// for tree-shaped responses; round-trippable via the reference parser.
    #[default]
    Text,
    /// Pretty-printed JSON. Best for CLI / programmatic consumers that
    /// already have a JSON parser on hand.
    Json,
    /// `MessagePack` payload encoded as ASCII hex. Smallest byte count;
    /// preserved for backwards-compatibility with the `compact_mode`
    /// behaviour shipped in RFC-0090.
    Msgpack,
}

/// Returns a heap-allocated formatter for the requested [`OutputFormat`].
///
/// Boxed so callers can hold a single trait object regardless of which
/// variant was selected at runtime.
#[must_use]
pub fn formatter_for(fmt: OutputFormat) -> Box<dyn Formatter> {
    match fmt {
        OutputFormat::Json => Box::new(JsonFormatter),
        OutputFormat::Text => Box::new(TextFormatter::default()),
        OutputFormat::Msgpack => Box::new(MsgpackHexFormatter),
    }
}

// ── JsonFormatter ────────────────────────────────────────────────────────────

/// Pretty-printed JSON formatter. Wraps [`serde_json::to_string_pretty`].
///
/// Falls back to the JSON `null` literal when serialisation fails — which
/// should never happen for a `serde_json::Value` (it is always valid JSON)
/// but the `Result` is propagated by `serde_json` regardless.
#[derive(Debug, Default, Clone, Copy)]
pub struct JsonFormatter;

impl Formatter for JsonFormatter {
    fn format(&self, value: &Value) -> String {
        serde_json::to_string_pretty(value).unwrap_or_else(|_| "null".to_owned())
    }
}

// ── TextFormatter ────────────────────────────────────────────────────────────

/// TOON-inspired indented text formatter (RFC-0094 §"Format grammar").
///
/// Holds the per-level indent width so callers can tune visual density
/// without changing the grammar (the parser is whitespace-tolerant).
#[derive(Debug, Clone, Copy)]
pub struct TextFormatter {
    /// Spaces emitted per nesting level. Default `2`.
    pub indent: usize,
}

impl Default for TextFormatter {
    fn default() -> Self {
        Self {
            indent: TEXT_INDENT_WIDTH,
        }
    }
}

impl Formatter for TextFormatter {
    fn format(&self, value: &Value) -> String {
        let mut out = String::new();
        self.write_value(&mut out, value, 0);
        // Strip the trailing newline so the result composes cleanly inside
        // MCP `text` content blocks.
        if out.ends_with('\n') {
            out.pop();
        }
        out
    }
}

impl TextFormatter {
    fn write_value(self, out: &mut String, value: &Value, depth: usize) {
        match value {
            Value::Object(map) => {
                if map.is_empty() {
                    out.push_str("{}\n");
                    return;
                }
                // First key inherits the indent already emitted by the
                // caller (for the list-item `- ` prefix case); subsequent
                // keys get the full indent for `depth`.
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 {
                        push_indent(out, depth, self.indent);
                    }
                    self.write_kv(out, k, v, depth);
                }
            }
            // Top-level non-object: render as a single bare scalar/list.
            _ => self.write_scalar_or_list(out, value, depth),
        }
    }

    fn write_kv(self, out: &mut String, key: &str, value: &Value, depth: usize) {
        out.push_str(&escape_key(key));
        match value {
            Value::Object(map) if map.is_empty() => {
                out.push_str(": {}\n");
            }
            Value::Object(_) => {
                out.push_str(":\n");
                push_indent(out, depth + 1, self.indent);
                self.write_value(out, value, depth + 1);
            }
            Value::Array(items) if items.is_empty() => {
                out.push_str(": []\n");
            }
            Value::Array(items) => {
                out.push_str(":\n");
                self.write_list(out, items, depth + 1);
            }
            _ => {
                out.push_str(": ");
                out.push_str(&format_scalar(value));
                out.push('\n');
            }
        }
    }

    fn write_list(self, out: &mut String, items: &[Value], depth: usize) {
        for item in items {
            push_indent(out, depth, self.indent);
            out.push_str("- ");
            match item {
                Value::Object(map) if map.is_empty() => {
                    out.push_str("{}\n");
                }
                Value::Object(_) => {
                    // Object directly under `- `: first key sits on the
                    // same line as the bullet, remaining keys are indented
                    // one level deeper than the bullet.
                    self.write_value(out, item, depth + 1);
                }
                Value::Array(inner) if inner.is_empty() => {
                    out.push_str("[]\n");
                }
                Value::Array(inner) => {
                    out.push('\n');
                    self.write_list(out, inner, depth + 1);
                }
                _ => {
                    out.push_str(&format_scalar(item));
                    out.push('\n');
                }
            }
        }
    }

    fn write_scalar_or_list(self, out: &mut String, value: &Value, depth: usize) {
        match value {
            Value::Array(items) if items.is_empty() => out.push_str("[]\n"),
            Value::Array(items) => self.write_list(out, items, depth),
            _ => {
                out.push_str(&format_scalar(value));
                out.push('\n');
            }
        }
    }
}

fn push_indent(out: &mut String, depth: usize, indent: usize) {
    for _ in 0..(depth * indent) {
        out.push(' ');
    }
}

/// Format a scalar (non-object, non-array) JSON value as a bare token.
///
/// Strings are quoted only when they collide with the grammar (see
/// [`needs_quoting`]); everything else uses its native textual form.
fn format_scalar(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            if needs_quoting(s) {
                // Re-emit through serde_json so escape handling (\n, \", \\,
                // \uXXXX) follows a single, well-defined convention.
                serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\""))
            } else {
                s.clone()
            }
        }
        // Objects/arrays are handled by the caller; reach this branch only
        // if the caller misuses the helper.
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned())
        }
    }
}

/// Keys go through the same escape rules as scalar strings, but
/// additionally must not contain `: ` (which would confuse the parser
/// about where the key ends and the value begins).
fn escape_key(key: &str) -> String {
    if needs_quoting(key) || key.is_empty() {
        serde_json::to_string(key).unwrap_or_else(|_| format!("\"{key}\""))
    } else {
        key.to_owned()
    }
}

/// True iff the string would be ambiguous when emitted bare in TOON.
///
/// Triggers (RFC-0094 §"Format grammar"):
/// - leading `[`, `{`, `-`, `"`, or whitespace
/// - any occurrence of `: ` (colon-space) — would collide with `key: value`
/// - any occurrence of `\n`, `\r`, or `\t` — would break the line-oriented grammar
/// - exact match of a reserved literal: `null`, `true`, `false`, `[]`, `{}`
fn needs_quoting(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    if matches!(s, "null" | "true" | "false" | "[]" | "{}") {
        return true;
    }
    let first = s.as_bytes()[0];
    if matches!(first, b'[' | b'{' | b'-' | b'"' | b' ' | b'\t') {
        return true;
    }
    if s.contains(": ") || s.contains('\n') || s.contains('\r') || s.contains('\t') {
        return true;
    }
    false
}

// ── MsgpackHexFormatter ──────────────────────────────────────────────────────

/// `MessagePack` formatter that returns ASCII hex of the encoded bytes.
///
/// Preserved for compatibility with the `compact_mode` switch introduced
/// in RFC-0090. Not human-friendly, not particularly tokeniser-friendly
/// (every byte costs two ASCII chars), but useful for clients that already
/// know how to consume it.
#[derive(Debug, Default, Clone, Copy)]
pub struct MsgpackHexFormatter;

impl Formatter for MsgpackHexFormatter {
    fn format(&self, value: &Value) -> String {
        // Encoding a `serde_json::Value` to `MessagePack` should be infallible
        // in practice; fall back to empty rather than panicking inside an MCP
        // tool body.
        rmp_serde::to_vec(value).map_or_else(|_| String::new(), hex::encode)
    }
}

// ── Tests (Charter §5.1: written before implementation) ─────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ─ OutputFormat / factory ──────────────────────────────────────────────

    #[test]
    fn output_format_default_is_text() {
        assert_eq!(OutputFormat::default(), OutputFormat::Text);
    }

    #[test]
    fn factory_returns_each_variant() {
        // Smoke test: each variant produces a usable formatter. We assert
        // observable differences in their outputs rather than reflection.
        let value = json!({ "k": "v" });
        let json_out = formatter_for(OutputFormat::Json).format(&value);
        let text_out = formatter_for(OutputFormat::Text).format(&value);
        let hex_out = formatter_for(OutputFormat::Msgpack).format(&value);

        assert!(json_out.contains('"'), "JSON output should contain quotes");
        assert!(
            !text_out.contains('"'),
            "simple text output should be unquoted, got: {text_out}"
        );
        assert!(
            hex_out.chars().all(|c| c.is_ascii_hexdigit()),
            "msgpack output should be pure hex, got: {hex_out}"
        );
    }

    #[test]
    fn output_format_serde_roundtrip_lowercase() {
        // Wire-format sanity: the enum must deserialise from lowercase
        // strings the way the RFC promises (`"text"`, `"json"`, `"msgpack"`).
        let text: OutputFormat = serde_json::from_str("\"text\"").unwrap();
        let json_: OutputFormat = serde_json::from_str("\"json\"").unwrap();
        let mp: OutputFormat = serde_json::from_str("\"msgpack\"").unwrap();
        assert_eq!(text, OutputFormat::Text);
        assert_eq!(json_, OutputFormat::Json);
        assert_eq!(mp, OutputFormat::Msgpack);
    }

    // ─ JsonFormatter ───────────────────────────────────────────────────────

    #[test]
    fn json_formatter_pretty_prints() {
        let value = json!({ "callee_tree": { "root": "a", "depth": 1 } });
        let out = JsonFormatter.format(&value);
        // Pretty print uses newlines + 2-space indent.
        assert!(out.contains('\n'));
        assert!(out.contains("  \"root\""), "expected indented key: {out}");
    }

    #[test]
    fn json_formatter_roundtrip() {
        let value = json!({ "a": 1, "b": ["x", "y"], "c": null });
        let out = JsonFormatter.format(&value);
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed, value);
    }

    // ─ TextFormatter ───────────────────────────────────────────────────────

    #[test]
    fn text_formatter_renders_callee_tree_example() {
        // Mirrors the example from RFC-0094 §"Format grammar".
        let value = json!({
            "callee_tree": {
                "root": "src/auth.rs>AuthService>login",
                "depth": 3,
                "nodes": [
                    {
                        "path": "src/auth.rs>AuthService>login",
                        "kind": "function",
                        "callees": [
                            "src/db.rs>Pool>acquire",
                            "src/crypto.rs>verify_token"
                        ]
                    },
                    {
                        "path": "src/db.rs>Pool>acquire",
                        "kind": "method",
                        "callees": []
                    }
                ]
            }
        });
        let out = TextFormatter::default().format(&value);
        // The exact byte layout is part of the RFC's contract.
        let expected = "\
callee_tree:
  root: src/auth.rs>AuthService>login
  depth: 3
  nodes:
    - path: src/auth.rs>AuthService>login
      kind: function
      callees:
        - src/db.rs>Pool>acquire
        - src/crypto.rs>verify_token
    - path: src/db.rs>Pool>acquire
      kind: method
      callees: []";
        assert_eq!(out, expected, "got:\n{out}\n\nwanted:\n{expected}");
    }

    #[test]
    fn text_formatter_quotes_leading_dash_path() {
        // A path that starts with `-` would be parsed as a list bullet.
        let value = json!({ "flag": "-rf" });
        let out = TextFormatter::default().format(&value);
        assert_eq!(out, "flag: \"-rf\"", "got: {out}");
    }

    #[test]
    fn text_formatter_quotes_leading_bracket_or_brace() {
        let value = json!({ "tpl": "[T]", "blk": "{x}" });
        let out = TextFormatter::default().format(&value);
        // Order is preserved by serde_json::Map insertion order with the
        // default `preserve_order` disabled, so we check substring match.
        assert!(out.contains("tpl: \"[T]\""), "got: {out}");
        assert!(out.contains("blk: \"{x}\""), "got: {out}");
    }

    #[test]
    fn text_formatter_quotes_colon_space() {
        // `key: value` inside a string would let an attacker (or accident)
        // inject a fake key.
        let value = json!({ "title": "fix: panic in parser" });
        let out = TextFormatter::default().format(&value);
        assert_eq!(out, "title: \"fix: panic in parser\"", "got: {out}");
    }

    #[test]
    fn text_formatter_quotes_reserved_literals() {
        // `null`, `true`, `false`, `[]`, `{}` would round-trip into the
        // wrong type if emitted bare.
        let value = json!({
            "a": "null",
            "b": "true",
            "c": "false",
            "d": "[]",
            "e": "{}"
        });
        let out = TextFormatter::default().format(&value);
        assert!(out.contains("a: \"null\""), "got: {out}");
        assert!(out.contains("b: \"true\""), "got: {out}");
        assert!(out.contains("c: \"false\""), "got: {out}");
        assert!(out.contains("d: \"[]\""), "got: {out}");
        assert!(out.contains("e: \"{}\""), "got: {out}");
    }

    #[test]
    fn text_formatter_empty_list_and_object() {
        let value = json!({ "list": [], "obj": {} });
        let out = TextFormatter::default().format(&value);
        assert!(out.contains("list: []"), "got: {out}");
        assert!(out.contains("obj: {}"), "got: {out}");
    }

    #[test]
    fn text_formatter_renders_scalars() {
        let value = json!({ "n": 42, "f": 3.5, "b": true, "z": null });
        let out = TextFormatter::default().format(&value);
        assert!(out.contains("n: 42"), "got: {out}");
        assert!(out.contains("f: 3.5"), "got: {out}");
        assert!(out.contains("b: true"), "got: {out}");
        assert!(out.contains("z: null"), "got: {out}");
    }

    #[test]
    fn text_formatter_quotes_newline_in_string() {
        // Multi-line strings would break the line-oriented grammar.
        let value = json!({ "msg": "line1\nline2" });
        let out = TextFormatter::default().format(&value);
        // serde_json emits the escape sequence inside a quoted string.
        assert_eq!(out, "msg: \"line1\\nline2\"", "got: {out}");
    }

    #[test]
    fn text_formatter_nested_objects_indent_correctly() {
        let value = json!({
            "outer": {
                "inner": {
                    "leaf": "v"
                }
            }
        });
        let out = TextFormatter::default().format(&value);
        let expected = "\
outer:
  inner:
    leaf: v";
        assert_eq!(out, expected, "got:\n{out}\nwanted:\n{expected}");
    }

    #[test]
    fn text_formatter_list_of_scalars() {
        let value = json!({ "tags": ["a", "b", "c"] });
        let out = TextFormatter::default().format(&value);
        let expected = "\
tags:
  - a
  - b
  - c";
        assert_eq!(out, expected, "got:\n{out}\nwanted:\n{expected}");
    }

    // ─ MsgpackHexFormatter ─────────────────────────────────────────────────

    #[test]
    fn msgpack_hex_formatter_roundtrip() {
        let value = json!({ "k": "v", "n": 7, "list": [1, 2, 3] });
        let hex_str = MsgpackHexFormatter.format(&value);
        // Pure hex.
        assert!(hex_str.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!hex_str.is_empty());
        // Decode and verify the payload survives a round-trip.
        let bytes = hex::decode(&hex_str).expect("valid hex");
        let parsed: Value = rmp_serde::from_slice(&bytes).expect("valid msgpack");
        assert_eq!(parsed, value);
    }

    // ─ Token-saving spot check (RFC-0094 §"Numbers") ───────────────────────

    #[test]
    fn text_format_eliminates_json_structural_punctuation() {
        // RFC-0094's headline claim is ~73% **token** savings vs JSON on
        // real fixtures (1,973 → 562 gpt-4o tokens for a 50-node
        // callee_tree). Token savings exceed byte savings dramatically
        // because LLM tokenisers group whitespace runs into a single
        // token while exploding JSON punctuation (`{"`, `":`, `,"`, `"}`)
        // into multiple tokens.
        //
        // We can't link a real tokeniser into a unit test without pulling
        // in `tiktoken-rs` for one assertion. Instead we count the
        // **structural punctuation** that JSON inflicts on the tokeniser:
        // `{ } [ ] " ,` are the chars that consistently cause token
        // splits and contribute zero semantic value the model needs.
        //
        // Bound: the text format should emit `<= 5%` of the structural
        // punctuation that compact JSON emits, on a tree-shaped fixture.
        let mut nodes = Vec::new();
        for i in 0..30 {
            nodes.push(json!({
                "path": format!("src/module_{i}.rs>Type>method_{i}"),
                "kind": "function",
                "callees": [
                    format!("src/dep_{i}.rs>fn_a"),
                    format!("src/dep_{i}.rs>fn_b"),
                ]
            }));
        }
        let value = json!({
            "callee_tree": {
                "root": "src/auth.rs>AuthService>login",
                "depth": 3,
                "nodes": nodes
            }
        });

        let json_compact = serde_json::to_string(&value).unwrap();
        let text_out = TextFormatter::default().format(&value);

        let count_punct = |s: &str| -> usize {
            s.chars()
                .filter(|c| matches!(c, '{' | '}' | '[' | ']' | '"' | ','))
                .count()
        };
        let json_punct = count_punct(&json_compact);
        let text_punct = count_punct(&text_out);

        // Sanity: JSON must have meaningful structural punctuation.
        assert!(json_punct > 100, "fixture too small: {json_punct} chars");

        // The point: text format emits effectively zero of the
        // tokeniser-pessimal characters (the fixture has no values that
        // need escape-quoting, so the only `{`/`[`/etc. should be the
        // empty-list/empty-object literals, of which the fixture has
        // none).
        //
        // Compare via cross-multiplication to avoid `as f64` precision-loss
        // casts: `text/json <= 0.05` iff `text * 20 <= json`.
        assert!(
            text_punct.saturating_mul(20) <= json_punct,
            "expected text structural punctuation to be <= 5% of JSON \
             (json={json_punct}, text={text_punct})"
        );
    }
}
