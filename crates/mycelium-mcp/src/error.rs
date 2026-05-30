//! MCP application-level error helpers (RFC-0093).
//!
//! Wraps `rmcp::model::CallToolResult` so every tool can distinguish
//! protocol-level tool errors from application-level not-found / not-indexed
//! / invalid-input conditions using the `is_error` flag per the MCP spec.

use rmcp::model::{CallToolResult, Content};
use serde_json::Value;

/// Return a successful `CallToolResult` whose JSON payload has `is_error: false`.
#[must_use]
pub fn success_json(value: &Value) -> CallToolResult {
    CallToolResult::success(vec![Content::text(value.to_string())])
}

/// Return a `CallToolResult` that signals an application-level error
/// (`is_error: true`) so MCP clients can branch without string-matching.
#[must_use]
pub fn application_error(value: &Value) -> CallToolResult {
    CallToolResult::error(vec![Content::text(value.to_string())])
}

/// Canonical not-found error: symbol present in a valid index but the
/// requested path does not exist.
#[must_use]
pub fn not_found(path: &str) -> CallToolResult {
    application_error(&serde_json::json!({
        "found": false,
        "reason": "symbol not found",
        "path": path,
    }))
}

/// Canonical not-indexed error: the store is empty / was never indexed.
#[must_use]
pub fn not_indexed() -> CallToolResult {
    application_error(&serde_json::json!({
        "found": false,
        "reason": "index not loaded — run `mycelium index <root>` first",
    }))
}

/// Canonical invalid-path error: the caller supplied a path that
/// `TrunkPath::parse` rejected.
#[must_use]
pub fn invalid_path(path: &str, detail: &str) -> CallToolResult {
    application_error(&serde_json::json!({
        "found": false,
        "reason": "invalid path syntax",
        "path": path,
        "detail": detail,
    }))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn payload_text(result: &CallToolResult) -> &str {
        result.content[0]
            .as_text()
            .expect("content should be RawTextContent")
            .text
            .as_str()
    }

    // ── success_json ──────────────────────────────────────────────────────────

    #[test]
    fn success_json_sets_is_error_false() {
        let result = success_json(&json!({"answer": 42}));
        assert_eq!(result.is_error, Some(false));
    }

    #[test]
    fn success_json_encodes_payload_as_text_content() {
        let result = success_json(&json!({"k": "v"}));
        assert_eq!(result.content.len(), 1);
        let parsed: Value =
            serde_json::from_str(payload_text(&result)).expect("content should be valid JSON");
        assert_eq!(parsed["k"], "v");
    }

    #[test]
    fn success_json_structured_content_is_none() {
        let result = success_json(&json!({}));
        assert!(result.structured_content.is_none());
    }

    // ── application_error ─────────────────────────────────────────────────────

    #[test]
    fn application_error_sets_is_error_true() {
        let result = application_error(&json!({"reason": "boom"}));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn application_error_encodes_payload_as_text_content() {
        let result = application_error(&json!({"found": false}));
        let parsed: Value = serde_json::from_str(payload_text(&result)).unwrap();
        assert_eq!(parsed["found"], false);
    }

    // ── not_found ─────────────────────────────────────────────────────────────

    #[test]
    fn not_found_is_error_true() {
        assert_eq!(not_found("mod>foo").is_error, Some(true));
    }

    #[test]
    fn not_found_payload_has_canonical_keys() {
        let result = not_found("mod>foo");
        let v: Value = serde_json::from_str(payload_text(&result)).unwrap();
        assert_eq!(v["found"], false);
        assert_eq!(v["reason"], "symbol not found");
        assert_eq!(v["path"], "mod>foo");
    }

    // ── not_indexed ───────────────────────────────────────────────────────────

    #[test]
    fn not_indexed_is_error_true() {
        assert_eq!(not_indexed().is_error, Some(true));
    }

    #[test]
    fn not_indexed_payload_has_canonical_keys() {
        let result = not_indexed();
        let v: Value = serde_json::from_str(payload_text(&result)).unwrap();
        assert_eq!(v["found"], false);
        assert!(v["reason"].as_str().unwrap().contains("index not loaded"));
    }

    // ── invalid_path ──────────────────────────────────────────────────────────

    #[test]
    fn invalid_path_is_error_true() {
        assert_eq!(
            invalid_path("bad::path", "double colon").is_error,
            Some(true)
        );
    }

    #[test]
    fn invalid_path_payload_has_canonical_keys() {
        let result = invalid_path("bad::path", "double colon not allowed");
        let v: Value = serde_json::from_str(payload_text(&result)).unwrap();
        assert_eq!(v["found"], false);
        assert_eq!(v["reason"], "invalid path syntax");
        assert_eq!(v["path"], "bad::path");
        assert_eq!(v["detail"], "double colon not allowed");
    }

    // ── cross-type distinguishability ─────────────────────────────────────────

    #[test]
    fn success_and_error_have_opposite_is_error_flags() {
        let ok = success_json(&json!({"data": 1}));
        let err = application_error(&json!({"data": 1}));
        assert_ne!(ok.is_error, err.is_error);
    }

    #[test]
    fn not_found_differs_from_not_indexed_in_reason() {
        let found_result = not_found("x");
        let indexed_result = not_indexed();
        let found_payload: Value = serde_json::from_str(payload_text(&found_result)).unwrap();
        let indexed_payload: Value = serde_json::from_str(payload_text(&indexed_result)).unwrap();
        assert_ne!(found_payload["reason"], indexed_payload["reason"]);
    }
}
