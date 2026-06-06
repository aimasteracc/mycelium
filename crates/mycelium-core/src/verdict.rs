//! Pre-edit safety verdict — "safe-to-edit" before you touch (RFC-0116).
//!
//! Pure verdict logic over a plain [`EditMetrics`] struct: given a symbol an
//! agent is about to change, turn the engine's existing **blast-radius** and
//! **caller count** into a single `SAFE | CAUTION | REVIEW | UNSAFE` verdict
//! (plus the `ERROR`/`NOT_FOUND` envelope tokens) with reasons + a pre-edit
//! checklist. No `Store`, no I/O — the testable core. The thin Store adapter +
//! CLI/MCP surface, and the optional health/test-gap inputs, are later phases.
//!
//! Vocabulary is reconciled with `mycelium_context` (which already emits
//! `INFO`/`NOT_FOUND`) — this module introduces no clashing tokens.

/// A pre-edit safety verdict. The decision axis is `Safe`→`Caution`→`Review`→
/// `Unsafe`; `Error`/`NotFound` are envelope tokens (shared with `mycelium_context`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// No transitive dependents — free to change.
    Safe,
    /// A few dependents — glance before changing.
    Caution,
    /// Meaningful fan-in — review the dependents.
    Review,
    /// High fan-in — auditing all dependents is mandatory.
    Unsafe,
    /// The symbol's file does not parse — the graph is untrustworthy.
    Error,
    /// No such symbol in the graph.
    NotFound,
}

impl Verdict {
    /// The stable wire string (reconciled with `mycelium_context`'s tokens).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Safe => "SAFE",
            Self::Caution => "CAUTION",
            Self::Review => "REVIEW",
            Self::Unsafe => "UNSAFE",
            Self::Error => "ERROR",
            Self::NotFound => "NOT_FOUND",
        }
    }
}

/// Raw inputs to the verdict (filled from the engine's existing data in Phase 2).
#[derive(Debug, Clone, Copy)]
pub struct EditMetrics {
    /// Whether the target symbol exists in the graph.
    pub symbol_found: bool,
    /// Whether the symbol's file failed to parse (graph untrustworthy).
    pub parse_broken: bool,
    /// Direct callers (incoming Calls edges).
    pub direct_callers: u32,
    /// Transitive dependents (from `Store::reachable_to` / MCP `mycelium_get_reachable_to`).
    pub blast_radius: u32,
}

/// The verdict plus the human/agent-facing rationale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditVerdict {
    /// The verdict token.
    pub verdict: Verdict,
    /// Why this verdict (names the concrete counts).
    pub reasons: Vec<String>,
    /// Concrete pre-edit actions for a non-`SAFE` verdict.
    pub checklist: Vec<String>,
}

// Blast-radius bands (ported from TSA `_VERDICT_MAP`, keyed on transitive
// dependents rather than just direct callers — Mycelium has the transitive graph).
const CAUTION_MAX: u32 = 5;
const REVIEW_MAX: u32 = 20;

/// Compute a pre-edit safety verdict from the metrics. Pure.
#[must_use]
pub fn edit_verdict(m: &EditMetrics) -> EditVerdict {
    // Envelope short-circuits. Check `parse_broken` FIRST: a file that does not
    // parse is the *root cause* and usually also makes the symbol un-findable, so
    // reporting NOT_FOUND would hide the real diagnostic. Broken parse wins.
    if m.parse_broken {
        return EditVerdict {
            verdict: Verdict::Error,
            reasons: vec![
                "the symbol's file does not parse — the graph is untrustworthy".to_owned(),
            ],
            checklist: vec!["fix the parse error and re-index before editing".to_owned()],
        };
    }
    if !m.symbol_found {
        return EditVerdict {
            verdict: Verdict::NotFound,
            reasons: vec!["symbol not found in the graph".to_owned()],
            checklist: Vec::new(),
        };
    }

    // Decision axis: transitive blast radius (TSA `_VERDICT_MAP`, re-keyed).
    let verdict = if m.blast_radius == 0 {
        Verdict::Safe
    } else if m.blast_radius <= CAUTION_MAX {
        Verdict::Caution
    } else if m.blast_radius <= REVIEW_MAX {
        Verdict::Review
    } else {
        Verdict::Unsafe
    };

    let mut reasons = Vec::new();
    let mut checklist = Vec::new();
    if verdict != Verdict::Safe {
        reasons.push(format!(
            "{} symbol(s) transitively depend on this",
            m.blast_radius
        ));
        if m.direct_callers > 0 {
            reasons.push(format!("{} direct caller(s)", m.direct_callers));
        }
        checklist.push(format!(
            "audit the {} dependent(s) before changing the signature or behavior",
            m.blast_radius
        ));
        if matches!(verdict, Verdict::Review | Verdict::Unsafe) {
            checklist.push("run the full test suite after the change".to_owned());
        }
        if verdict == Verdict::Unsafe {
            checklist.push("prefer an additive/deprecation path over a breaking change".to_owned());
        }
    }

    EditVerdict {
        verdict,
        reasons,
        checklist,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics(blast: u32, callers: u32) -> EditMetrics {
        EditMetrics {
            symbol_found: true,
            parse_broken: false,
            direct_callers: callers,
            blast_radius: blast,
        }
    }

    #[test]
    fn wire_strings_match_context_vocabulary() {
        assert_eq!(Verdict::Safe.as_str(), "SAFE");
        assert_eq!(Verdict::Caution.as_str(), "CAUTION");
        assert_eq!(Verdict::Review.as_str(), "REVIEW");
        assert_eq!(Verdict::Unsafe.as_str(), "UNSAFE");
        assert_eq!(Verdict::Error.as_str(), "ERROR");
        assert_eq!(Verdict::NotFound.as_str(), "NOT_FOUND");
    }

    #[test]
    fn blast_radius_bands() {
        assert_eq!(edit_verdict(&metrics(0, 0)).verdict, Verdict::Safe);
        assert_eq!(edit_verdict(&metrics(3, 2)).verdict, Verdict::Caution);
        assert_eq!(edit_verdict(&metrics(12, 4)).verdict, Verdict::Review);
        assert_eq!(edit_verdict(&metrics(40, 9)).verdict, Verdict::Unsafe);
    }

    #[test]
    fn band_boundaries() {
        assert_eq!(edit_verdict(&metrics(5, 0)).verdict, Verdict::Caution);
        assert_eq!(edit_verdict(&metrics(6, 0)).verdict, Verdict::Review);
        assert_eq!(edit_verdict(&metrics(20, 0)).verdict, Verdict::Review);
        assert_eq!(edit_verdict(&metrics(21, 0)).verdict, Verdict::Unsafe);
    }

    #[test]
    fn broken_parse_short_circuits_to_error() {
        let m = EditMetrics {
            symbol_found: true,
            parse_broken: true,
            direct_callers: 0,
            blast_radius: 0, // would be SAFE, but parse is broken
        };
        assert_eq!(edit_verdict(&m).verdict, Verdict::Error);
    }

    #[test]
    fn missing_symbol_is_not_found() {
        let m = EditMetrics {
            symbol_found: false,
            parse_broken: false,
            direct_callers: 0,
            blast_radius: 0,
        };
        assert_eq!(edit_verdict(&m).verdict, Verdict::NotFound);
    }

    #[test]
    fn broken_parse_wins_over_not_found_and_over_bands() {
        // Both flags set: a broken parse is the root cause — report ERROR, not NOT_FOUND.
        let both = EditMetrics {
            symbol_found: false,
            parse_broken: true,
            direct_callers: 0,
            blast_radius: 0,
        };
        assert_eq!(edit_verdict(&both).verdict, Verdict::Error);
        // And it wins over a non-Safe blast-radius band too.
        let broken_high = EditMetrics {
            symbol_found: true,
            parse_broken: true,
            direct_callers: 9,
            blast_radius: 40,
        };
        assert_eq!(edit_verdict(&broken_high).verdict, Verdict::Error);
    }

    #[test]
    fn non_safe_verdicts_explain_with_concrete_counts() {
        let r = edit_verdict(&metrics(40, 9));
        assert_eq!(r.verdict, Verdict::Unsafe);
        assert!(!r.reasons.is_empty(), "unsafe must have reasons");
        assert!(!r.checklist.is_empty(), "unsafe must have a checklist");
        // The concrete blast-radius count appears in the rationale.
        assert!(
            r.reasons
                .iter()
                .chain(r.checklist.iter())
                .any(|s| s.contains("40")),
            "rationale must name the 40 dependents: {r:?}"
        );
    }

    #[test]
    fn safe_has_no_checklist() {
        let r = edit_verdict(&metrics(0, 0));
        assert_eq!(r.verdict, Verdict::Safe);
        assert!(r.checklist.is_empty());
    }
}
