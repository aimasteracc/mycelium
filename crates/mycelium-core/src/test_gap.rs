//! Coverage-aware test-gap ranking — pure core (RFC-0115).
//!
//! Two pure functions over plain structs: [`is_covered`] decides whether a
//! symbol is exercised by an external coverage artifact (measured on **body
//! lines only** — the `def`/decorator line executes on mere import, so counting
//! it would hide an untested body), and [`rank`] orders the untested symbols by
//! graph **reach** (blast-radius dominant) into a "what to test next" worklist.
//! No `Store`, no filesystem, no coverage parsing — the testable core. The
//! artifact parser + Store adapter + CLI/MCP surface are later phases.

use std::collections::{BTreeMap, BTreeSet};

/// One production symbol's line span, derived from the graph.
#[derive(Debug, Clone)]
pub struct SymbolSpan {
    /// Symbol path/name (for output).
    pub name: String,
    /// Project-root-relative file path (the coverage-artifact key).
    pub file: String,
    /// First **body** line (after the signature/decorator span) — coverage is
    /// measured from here so the bare `def` line never counts as "covered".
    pub body_start: u32,
    /// Last line of the symbol body (inclusive).
    pub end_line: u32,
}

/// Executed lines per file, parsed+normalised from coverage.json/lcov.
#[derive(Debug, Clone, Default)]
pub struct CoverageFacts {
    /// `file` → set of 1-based executed line numbers.
    pub executed_lines: BTreeMap<String, BTreeSet<u32>>,
}

/// A symbol's graph reach, pulled from the Store in Phase 2.
#[derive(Debug, Clone, Copy)]
pub struct GraphReach {
    /// Transitive dependents (`mycelium_impact`).
    pub blast_radius: u32,
    /// Direct callers.
    pub in_degree: u32,
    /// Degree centrality ∈ [0, 1].
    pub degree_centrality: f64,
}

/// An untested symbol with its reach-based rank score.
#[derive(Debug, Clone, PartialEq)]
pub struct TestGap {
    /// Symbol name/path.
    pub name: String,
    /// File.
    pub file: String,
    /// Higher = more worth testing (more reach).
    pub rank_score: f64,
}

/// Is `span` exercised by the coverage artifact?
///
/// True iff **≥1 body line** (`body_start..=end_line`) is recorded executed for
/// the symbol's file. The declaration/decorator lines (before `body_start`) are
/// deliberately excluded.
#[must_use]
pub fn is_covered(span: &SymbolSpan, facts: &CoverageFacts) -> bool {
    facts
        .executed_lines
        .get(&span.file)
        .is_some_and(|lines| (span.body_start..=span.end_line).any(|l| lines.contains(&l)))
}

/// Rank the **untested** symbols by graph reach, highest first.
///
/// Blast-radius dominant, in-degree secondary, centrality tiebreak. Tested
/// symbols are dropped. Deterministic and total (ties broken by name).
#[must_use]
pub fn rank(symbols: &[(SymbolSpan, GraphReach)], facts: &CoverageFacts) -> Vec<TestGap> {
    let mut gaps: Vec<TestGap> = symbols
        .iter()
        .filter(|(span, _)| !is_covered(span, facts))
        .map(|(span, reach)| TestGap {
            name: span.name.clone(),
            file: span.file.clone(),
            rank_score: reach_score(reach),
        })
        .collect();
    // Highest reach first; ties broken by name for a total, deterministic order.
    gaps.sort_by(|a, b| {
        b.rank_score
            .total_cmp(&a.rank_score)
            .then_with(|| a.name.cmp(&b.name))
    });
    gaps
}

fn reach_score(r: &GraphReach) -> f64 {
    // Blast radius dominates; direct callers next; centrality is the fine tiebreak.
    f64::from(r.blast_radius).mul_add(1000.0, f64::from(r.in_degree) * 10.0) + r.degree_centrality
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(file: &str, lines: &[u32]) -> CoverageFacts {
        let mut m = BTreeMap::new();
        m.insert(file.to_owned(), lines.iter().copied().collect());
        CoverageFacts { executed_lines: m }
    }

    fn span(name: &str, file: &str, body_start: u32, end_line: u32) -> SymbolSpan {
        SymbolSpan {
            name: name.to_owned(),
            file: file.to_owned(),
            body_start,
            end_line,
        }
    }

    #[test]
    fn body_line_executed_is_tested() {
        let s = span("foo", "a.py", 5, 10);
        assert!(is_covered(&s, &facts("a.py", &[6])));
    }

    #[test]
    fn only_declaration_line_executed_is_a_gap() {
        // Line 4 (the `def`/decorator, before body_start=5) executes on import.
        // That must NOT count as covered — the body (5..=10) ran nothing.
        let s = span("foo", "a.py", 5, 10);
        assert!(!is_covered(&s, &facts("a.py", &[4])));
    }

    #[test]
    fn no_executed_line_is_a_gap() {
        let s = span("foo", "a.py", 5, 10);
        assert!(!is_covered(&s, &facts("a.py", &[99])));
        assert!(!is_covered(&s, &CoverageFacts::default())); // file absent
    }

    #[test]
    fn coverage_beats_naming_heuristics() {
        // No test named `foo`; the artifact simply says a body line ran.
        // A naming heuristic would call this a gap; line coverage says tested.
        let s = span("obscure_internal", "a.py", 5, 10);
        assert!(is_covered(&s, &facts("a.py", &[8])));
    }

    #[test]
    fn rank_orders_untested_by_reach_and_drops_tested() {
        let leaf = (
            span("leaf", "a.py", 5, 6),
            GraphReach {
                blast_radius: 0,
                in_degree: 0,
                degree_centrality: 0.0,
            },
        );
        let hub = (
            span("hub", "b.py", 5, 6),
            GraphReach {
                blast_radius: 40,
                in_degree: 9,
                degree_centrality: 0.8,
            },
        );
        let tested = (
            span("tested", "c.py", 5, 6),
            GraphReach {
                blast_radius: 100,
                in_degree: 50,
                degree_centrality: 1.0,
            },
        );
        // `tested` has a body line executed → excluded despite huge reach.
        let mut f = facts("c.py", &[5]);
        f.executed_lines.insert("a.py".to_owned(), BTreeSet::new());
        f.executed_lines.insert("b.py".to_owned(), BTreeSet::new());

        let gaps = rank(&[leaf, hub, tested], &f);
        let names: Vec<_> = gaps.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["hub", "leaf"]); // tested excluded; hub outranks leaf
    }

    #[test]
    fn rank_is_deterministic_on_ties() {
        let mk = |n: &str| {
            (
                span(n, "z.py", 5, 6),
                GraphReach {
                    blast_radius: 1,
                    in_degree: 0,
                    degree_centrality: 0.0,
                },
            )
        };
        let f = facts("z.py", &[]); // nothing executed → all gaps
        let gaps = rank(&[mk("b"), mk("a"), mk("c")], &f);
        let names: Vec<_> = gaps.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b", "c"]); // equal score → name order
    }
}
