//! Architectural-constraint DSL — forbid-rule layering checks (RFC-0117).
//!
//! A pure evaluator: given frozen forbid-`Constraint`s and a plain view of the
//! synapse edges ([`EdgeRef`]), return the [`Violation`]s — which edge broke
//! which rule. No `Store`, no YAML, no I/O — the testable core. The YAML loader
//! and the Store edge adapter + CLI/MCP surface are later phases.
//!
//! Ported from tree-sitter-analyzer `constraints/{schema,parser,evaluator}.py`,
//! re-grounded on Mycelium's import-resolved Calls/Imports edges.

/// Rule severity. `Error`-severity violations fail CI (non-zero exit) in Phase 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Hard failure.
    Error,
    /// Surfaced but non-fatal.
    Warn,
    /// Informational.
    Info,
}

/// Which synapse edge kinds a rule inspects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKindFilter {
    /// Only `Calls` edges.
    Calls,
    /// Only `Imports` edges.
    Imports,
    /// Both `Calls` and `Imports`.
    Any,
}

impl EdgeKindFilter {
    /// Does this filter accept an edge of `kind`?
    #[must_use]
    pub fn accepts(self, kind: Self) -> bool {
        matches!(self, Self::Any) || self == kind
    }
}

/// One parsed forbid rule (frozen — constructed once, never mutated).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    /// Stable rule id (appears in violations).
    pub id: String,
    /// Violation severity.
    pub severity: Severity,
    /// Which edge kinds to inspect (`Any` by default at the YAML layer).
    pub applies_to: EdgeKindFilter,
    /// Caller/importer-side glob.
    pub from_glob: String,
    /// Callee/imported-side glob.
    pub to_glob: String,
    /// Human reason (echoed in violations).
    pub reason: String,
    /// Caller-side allow-list globs (a matching `from_path` suppresses the rule).
    pub exceptions: Vec<String>,
}

/// A plain, `Store`-free view of one synapse edge.
#[derive(Debug, Clone, Copy)]
pub struct EdgeRef<'a> {
    /// `Calls` or `Imports` (never `Any`).
    pub kind: EdgeKindFilter,
    /// Resolved source path, e.g. `src/ui/page.rs>Page>render`.
    pub from_path: &'a str,
    /// Resolved target path (import-aware), e.g. `src/db/pool.rs>Pool>get`.
    pub to_path: &'a str,
    /// Source line of the edge.
    pub from_line: u32,
}

/// One offending edge × rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The rule that was broken.
    pub rule_id: String,
    /// The rule's severity.
    pub severity: Severity,
    /// The edge kind that broke it.
    pub kind: EdgeKindFilter,
    /// Source path.
    pub from_path: String,
    /// Target path.
    pub to_path: String,
    /// Source line.
    pub from_line: u32,
}

/// A target path is "resolved" if it names a file or a nested symbol — a bare
/// stub (a lone identifier, no `/` and no `>`) is an unresolved/dynamic callee
/// and is skipped, to avoid false positives (TSA's "no callee file → skip").
///
/// Invariant on `path` format: this assumes the project's canonical edge-path
/// convention — `/`-separated file path, with nested symbols appended after
/// `>` (`src/db/pool.rs>Pool>get`). A path using a different separator (e.g. a
/// Windows `\` or a `::` module path) would be misclassified as an unresolved
/// stub and silently skipped. Callers MUST normalise to this format upstream.
fn is_resolved(path: &str) -> bool {
    path.contains('/') || path.contains('>')
}

/// Glob match supporting `**` (any chars, including `/`) and `*` (any chars
/// except `/`). Self-contained — no regex dependency.
#[must_use]
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    glob_rec(&p, &t)
}

/// Recursive backtracking glob matcher.
///
/// Complexity: each `**` branches into `t.len()+1` recursive calls and each `*`
/// branches in two, so a pattern with `k` wildcards over text of length `n` is
/// worst-case O(n^k) — pathological only for adversarial patterns like
/// `**a**a**a…` against a long non-matching text. Real constraint globs carry a
/// handful of wildcards over short edge paths, so this stays effectively linear;
/// we accept the worst case rather than pull in a regex engine. If untrusted
/// patterns are ever admitted, switch to an iterative two-pointer matcher (which
/// makes single-`*` linear) or cap wildcard count at parse time.
fn glob_rec(p: &[char], t: &[char]) -> bool {
    let Some(&first) = p.first() else {
        return t.is_empty();
    };
    if first == '*' {
        if p.get(1) == Some(&'*') {
            // `**` — match zero or more of anything (including '/').
            let rest = &p[2..];
            (0..=t.len()).any(|i| glob_rec(rest, &t[i..]))
        } else {
            // `*` — match zero or more chars except '/'.
            let rest = &p[1..];
            if glob_rec(rest, t) {
                return true;
            }
            matches!(t.first(), Some(&c) if c != '/') && glob_rec(p, &t[1..])
        }
    } else {
        matches!(t.first(), Some(&c) if c == first) && glob_rec(&p[1..], &t[1..])
    }
}

/// Evaluate forbid `rules` over `edges`, returning every violation. Pure: no
/// I/O, no clock, no `Store`. O(rules × edges) glob matching.
#[must_use]
pub fn evaluate(rules: &[Constraint], edges: &[EdgeRef<'_>]) -> Vec<Violation> {
    let mut violations = Vec::new();
    for edge in edges {
        // Skip unresolved/dynamic targets (bare stubs) — TSA "no callee file → skip".
        if !is_resolved(edge.to_path) {
            continue;
        }
        for rule in rules {
            if !rule.applies_to.accepts(edge.kind) {
                continue;
            }
            if !glob_match(&rule.from_glob, edge.from_path)
                || !glob_match(&rule.to_glob, edge.to_path)
            {
                continue;
            }
            // A whitelisted seam on the caller side suppresses the rule.
            if rule
                .exceptions
                .iter()
                .any(|ex| glob_match(ex, edge.from_path))
            {
                continue;
            }
            violations.push(Violation {
                rule_id: rule.id.clone(),
                severity: rule.severity,
                kind: edge.kind,
                from_path: edge.from_path.to_owned(),
                to_path: edge.to_path.to_owned(),
                from_line: edge.from_line,
            });
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: &str, from: &str, to: &str, applies_to: EdgeKindFilter) -> Constraint {
        Constraint {
            id: id.to_owned(),
            severity: Severity::Error,
            applies_to,
            from_glob: from.to_owned(),
            to_glob: to.to_owned(),
            reason: "test rule".to_owned(),
            exceptions: Vec::new(),
        }
    }

    fn edge<'a>(kind: EdgeKindFilter, from: &'a str, to: &'a str) -> EdgeRef<'a> {
        EdgeRef {
            kind,
            from_path: from,
            to_path: to,
            from_line: 1,
        }
    }

    #[test]
    fn glob_matches_recursive_and_segment() {
        assert!(glob_match("src/ui/**", "src/ui/page.rs>Page>render"));
        assert!(glob_match("src/db/**", "src/db/pool.rs>Pool>get"));
        assert!(!glob_match("src/ui/**", "src/api/page.rs"));
        assert!(glob_match("*.rs", "main.rs"));
        assert!(!glob_match("*.rs", "src/main.rs")); // '*' does not cross '/'
        assert!(glob_match("src/*/page.rs", "src/ui/page.rs"));
    }

    #[test]
    fn forbidden_edge_is_a_violation() {
        let rules = [rule(
            "ui-no-db",
            "src/ui/**",
            "src/db/**",
            EdgeKindFilter::Any,
        )];
        let edges = [edge(
            EdgeKindFilter::Imports,
            "src/ui/page.rs>Page>render",
            "src/db/pool.rs>Pool>get",
        )];
        let v = evaluate(&rules, &edges);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule_id, "ui-no-db");
        assert_eq!(v[0].kind, EdgeKindFilter::Imports);
    }

    #[test]
    fn exception_suppresses_the_violation() {
        let mut r = rule("ui-no-db", "src/ui/**", "src/db/**", EdgeKindFilter::Any);
        r.exceptions.push("src/ui/admin/**".to_owned());
        let edges = [edge(
            EdgeKindFilter::Imports,
            "src/ui/admin/migrate.rs>run",
            "src/db/pool.rs>Pool>get",
        )];
        assert!(evaluate(&[r], &edges).is_empty());
    }

    #[test]
    fn applies_to_filters_edge_kind() {
        let rules = [rule(
            "ui-no-db-imports",
            "src/ui/**",
            "src/db/**",
            EdgeKindFilter::Imports,
        )];
        // A Calls edge must be ignored by an Imports-only rule.
        let edges = [edge(
            EdgeKindFilter::Calls,
            "src/ui/page.rs>render",
            "src/db/pool.rs>get",
        )];
        assert!(evaluate(&rules, &edges).is_empty());
    }

    #[test]
    fn unresolved_bare_stub_target_is_skipped() {
        let rules = [rule("any-no-foo", "**", "**", EdgeKindFilter::Any)];
        // `getcwd` is a bare stub (no '/' no '>') — unresolved, must be skipped.
        let edges = [edge(EdgeKindFilter::Calls, "src/app.rs>main", "getcwd")];
        assert!(evaluate(&rules, &edges).is_empty());
    }

    #[test]
    fn non_matching_layer_yields_nothing() {
        let rules = [rule(
            "ui-no-db",
            "src/ui/**",
            "src/db/**",
            EdgeKindFilter::Any,
        )];
        let edges = [edge(
            EdgeKindFilter::Imports,
            "src/service/x.rs>f",
            "src/db/pool.rs>get",
        )];
        assert!(evaluate(&rules, &edges).is_empty());
    }
}
