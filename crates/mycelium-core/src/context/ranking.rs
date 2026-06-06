/// Static classification of a trunk path / symbol w.r.t. test code.
///
/// Returned by [`classify_test_path`]. `TestFile` takes precedence over `TestSymbol`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestKind {
    /// Not test code.
    None,
    /// The path's file part is a recognised test file.
    TestFile,
    /// The file is not a test file, but the symbol leaf looks like a test function.
    TestSymbol,
}

/// A candidate entry point with precomputed match-quality, importance, and path.
///
/// `importance` is injected by the caller so the pure core never touches `Store`.
/// Phase 2 fills it with in-degree; Phase 3 can swap to `PageRank` with no signature change.
#[derive(Debug, Clone)]
pub struct ScoredCandidate {
    /// Full trunk path, e.g. `"crates/foo/src/lib.rs>MyType>method"`.
    pub path: String,
    /// `true` when the candidate string equals the leaf segment (case-insensitive) — RFC-0101 §3.
    pub exact_match: bool,
    /// Higher = more central (in-degree as `f64`); secondary tiebreak after `exact_match`.
    pub importance: f64,
    /// Original discovery index; stable final tiebreak so ranking is deterministic.
    pub order: usize,
}

/// Options for [`rank_entry_points`].
#[derive(Debug, Clone, Copy)]
pub struct RankOpts {
    /// Maximum number of paths to return.
    pub max_nodes: usize,
    /// When `true` and at least one non-test candidate exists, test candidates are dropped.
    /// When every candidate is test code, they are returned anyway (never-empty guarantee).
    pub exclude_tests: bool,
}

/// Pure, language-aware, allocation-light test path classifier (RFC-0119 §Design).
///
/// Inspects trunk-path segments and the symbol leaf using only cheap string operations —
/// no regex, no I/O, no parsing. `TestFile` takes precedence over `TestSymbol`.
///
/// See the RFC for the complete rule table and per-language rationale.
#[must_use]
pub fn classify_test_path(trunk_path: &str) -> TestKind {
    // Split on the first `>` to separate the file part from the symbol chain.
    let (file_part, leaf) = trunk_path.find('>').map_or((trunk_path, ""), |pos| {
        let lf = trunk_path.rfind('>').map_or("", |i| &trunk_path[i + 1..]);
        (&trunk_path[..pos], lf)
    });

    // ── FILE-LEVEL SIGNALS ───────────────────────────────────────────────────
    let segments: Vec<&str> = file_part.split('/').collect();
    let filename = segments.last().copied().unwrap_or("");

    // Directory segments (all except the trailing filename component).
    if segments.len() > 1 {
        for &seg in &segments[..segments.len() - 1] {
            if matches!(seg, "tests" | "test" | "__tests__" | "testing") {
                return TestKind::TestFile;
            }
        }
    }

    // Filename stem (the part before the first `.`).
    let stem = filename.split('.').next().unwrap_or("");
    if stem == "tests" || stem == "test" {
        return TestKind::TestFile;
    }

    // Filename suffix / infix patterns.
    if let Some(dot_pos) = filename.find('.') {
        let name_part = &filename[..dot_pos]; // before first `.`
        let rest = &filename[dot_pos..]; // from first `.` onward (includes the dot)

        // *_test.<ext>  (Go, generic)
        if name_part.ends_with("_test") {
            return TestKind::TestFile;
        }

        // *.test.<ext>  or  *.spec.<ext>  (JS / TS)
        if rest.starts_with(".test.") || rest.starts_with(".spec.") {
            return TestKind::TestFile;
        }

        // *_tests.rs  (Rust multi-test-file convention)
        let is_rs = std::path::Path::new(filename)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("rs"));
        if name_part.ends_with("_tests") && is_rs {
            return TestKind::TestFile;
        }

        // conftest.py  (pytest fixtures)
        if filename == "conftest.py" {
            return TestKind::TestFile;
        }

        // test_<word>.py  — pytest discovery convention; **Python-only** (avoids
        // the `test_gap.rs` false-positive: `test_*` is NOT a Rust file convention).
        let is_py = std::path::Path::new(filename)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("py"));
        if is_py && name_part.starts_with("test_") && name_part.len() > "test_".len() {
            return TestKind::TestFile;
        }
    }

    // ── SYMBOL-LEVEL SIGNALS ─────────────────────────────────────────────────
    if leaf.is_empty() {
        return TestKind::None;
    }

    // `test_` prefix — Rust `#[test] fn test_*` and Python pytest discovery.
    if leaf.starts_with("test_") {
        return TestKind::TestSymbol;
    }

    // `test` immediately followed by an uppercase letter — JS/Go camelCase.
    // Word-boundary: `testbed` / `testimony` / `testableConfig` must NOT match.
    if leaf.starts_with("test") && leaf.len() > 4 && leaf.as_bytes()[4].is_ascii_uppercase() {
        return TestKind::TestSymbol;
    }

    // xUnit lifecycle methods.
    if leaf == "setUp" || leaf == "tearDown" {
        return TestKind::TestSymbol;
    }

    TestKind::None
}

/// Rank candidates by `(exact_match desc, non_test desc, importance desc, order asc)`.
///
/// Algorithm (RFC-0119 §Design):
/// 1. Dedup by path (first-seen) — a later duplicate with higher importance must not
///    steal the rank position of the first-seen entry (AC-7 contract).
/// 2. Classify each surviving candidate via [`classify_test_path`].
/// 3. Partition into `non_test` and `test` buckets.
/// 4. Sort each bucket by the ordering key.
/// 5. Concatenate: `non_test` first, `test` appended (dropped when `exclude_tests` and
///    `non_test` is non-empty).
/// 6. Truncate to `max_nodes`.
///
/// **Never-empty guarantee:** if every candidate is test code, they are returned
/// importance-ranked regardless of `exclude_tests`.
#[must_use]
pub fn rank_entry_points(candidates: &[ScoredCandidate], opts: RankOpts) -> Vec<String> {
    // Dedup by path (first-seen) before partitioning so a later duplicate with
    // higher importance cannot silently promote an entry point (AC-7).
    let mut seen_paths = std::collections::HashSet::new();
    let mut non_test: Vec<&ScoredCandidate> = Vec::new();
    let mut test: Vec<&ScoredCandidate> = Vec::new();

    for c in candidates {
        if !seen_paths.insert(c.path.as_str()) {
            continue;
        }
        match classify_test_path(&c.path) {
            TestKind::None => non_test.push(c),
            TestKind::TestFile | TestKind::TestSymbol => test.push(c),
        }
    }

    // Deterministic comparator: exact_match desc, importance desc, order asc.
    let sort_key = |a: &&ScoredCandidate, b: &&ScoredCandidate| {
        b.exact_match
            .cmp(&a.exact_match)
            .then_with(|| {
                b.importance
                    .partial_cmp(&a.importance)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then(a.order.cmp(&b.order))
    };
    non_test.sort_by(sort_key);
    test.sort_by(sort_key);

    // Build ordered result: non-test first, then (optionally) demoted test.
    let ordered: Vec<&ScoredCandidate> = if non_test.is_empty() {
        // All-test corpus — never return empty; fall back to importance-ranked tests.
        test
    } else if opts.exclude_tests {
        non_test
    } else {
        let mut v = non_test;
        v.extend(test);
        v
    };

    ordered
        .into_iter()
        .take(opts.max_nodes)
        .map(|c| c.path.clone())
        .collect()
}
