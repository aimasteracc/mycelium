//! Graph-native project health grade (RFC-0114).
//!
//! A one-call A–F grade for an indexed project, computed **purely from the RCIG
//! graph** — dead-code ratio, isolation ratio, and connectivity — with a
//! weighted 0–100 score and a per-dimension breakdown. No cyclomatic-complexity
//! parser, no coverage file: cross-language by construction.
//!
//! This module is the **pure scorer core** ([`score`] over [`HealthMetrics`]).
//! The thin `Store::health` adapter (filling the metrics from the public API)
//! and the `project-health` CLI+MCP surface are a separate phase, so this core
//! is testable with no `Store` and collides with nothing.

/// Letter grade for a project's structural health.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthGrade {
    /// `score >= 90`.
    A,
    /// `score 80..=89`.
    B,
    /// `score 70..=79`.
    C,
    /// `score 60..=69`.
    D,
    /// `score < 60`.
    F,
}

impl HealthGrade {
    /// The stable wire string (`"A".."F"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::F => "F",
        }
    }

    /// Map a 0–100 score to a letter grade (bands A≥90, B≥80, C≥70, D≥60, F<60).
    #[must_use]
    pub const fn from_score(score: u8) -> Self {
        match score {
            90..=u8::MAX => Self::A,
            80..=89 => Self::B,
            70..=79 => Self::C,
            60..=69 => Self::D,
            _ => Self::F,
        }
    }
}

/// Raw graph inputs to the health score (filled from `Store`'s public API).
///
/// Invariant: `dead_count` and `isolated_count` are each ≤ `total_symbols` (they
/// are subsets of the symbol set). [`score`] clamps if violated, but a violation
/// indicates corrupt/partial metrics from the adapter.
#[derive(Debug, Clone, Copy)]
pub struct HealthMetrics {
    /// Total indexed symbols (definitions).
    pub total_symbols: usize,
    /// Symbols with no incoming Calls/Imports (dead). ≤ `total_symbols`.
    pub dead_count: usize,
    /// Symbols with no edges of any kind (isolated). ≤ `total_symbols`.
    pub isolated_count: usize,
    /// Total edges of all kinds.
    pub edge_count: usize,
}

/// The graded report: overall grade + score + per-dimension 0–100 sub-scores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthReport {
    /// Overall letter grade.
    pub grade: HealthGrade,
    /// Overall 0–100 score (weighted mean of the dimensions).
    pub score: u8,
    /// Per-dimension sub-scores: `(name, 0..=100)`.
    pub dimensions: Vec<(&'static str, u8)>,
}

/// Healthy graphs wire together; below this edges-per-node, connectivity is
/// penalized linearly. Above it, connectivity is full marks.
const TARGET_DENSITY: f64 = 2.0;
const W_DEAD: f64 = 0.45;
const W_ISOLATION: f64 = 0.35;
const W_CONNECTIVITY: f64 = 0.20;

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "ratios of small counts, clamped to 0..=100 before any cast"
)]
fn pct(numerator: usize, denominator: usize) -> f64 {
    // `numerator` (dead/isolated count) must be ≤ `denominator` (total). If the
    // Store ever feeds an inconsistent count, `.min` clamps it so the score
    // stays in range rather than going negative — but the debug-assert flags the
    // corrupt input in tests/dev instead of silently reporting it as "healthy".
    debug_assert!(
        numerator <= denominator,
        "subset count {numerator} exceeds total {denominator}"
    );
    100.0 * (1.0 - (numerator.min(denominator) as f64 / denominator as f64))
}

/// Build the JSON response object for the `project-health` CLI+MCP surface.
///
/// Shared builder — both surfaces call this function so the JSON shape is
/// byte-identical by construction (Three-Surface Rule / RFC-0109 pattern).
#[must_use]
pub fn project_health_payload(report: &HealthReport) -> serde_json::Value {
    let dims: Vec<serde_json::Value> = report
        .dimensions
        .iter()
        .map(|(name, s)| serde_json::json!({ "name": name, "score": s }))
        .collect();
    serde_json::json!({
        "grade": report.grade.as_str(),
        "score": report.score,
        "dimensions": dims,
    })
}

/// Grade a project's structural health from its graph metrics. Pure.
///
/// An empty project (`total_symbols == 0`) fails closed: grade `F`, score `0`.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "edge/node ratio; scores clamped to 0..=100 before any cast"
)]
pub fn score(m: &HealthMetrics) -> HealthReport {
    if m.total_symbols == 0 {
        return HealthReport {
            grade: HealthGrade::F,
            score: 0,
            dimensions: vec![("dead_code", 0), ("isolation", 0), ("connectivity", 0)],
        };
    }

    let dead = pct(m.dead_count, m.total_symbols);
    let isolation = pct(m.isolated_count, m.total_symbols);
    let density = m.edge_count as f64 / m.total_symbols as f64;
    let connectivity = (density / TARGET_DENSITY).clamp(0.0, 1.0) * 100.0;

    let weighted = W_DEAD.mul_add(
        dead,
        W_ISOLATION.mul_add(isolation, W_CONNECTIVITY * connectivity),
    );
    let overall = weighted.round().clamp(0.0, 100.0) as u8;

    HealthReport {
        grade: HealthGrade::from_score(overall),
        score: overall,
        dimensions: vec![
            ("dead_code", dead.round().clamp(0.0, 100.0) as u8),
            ("isolation", isolation.round().clamp(0.0, 100.0) as u8),
            ("connectivity", connectivity.round().clamp(0.0, 100.0) as u8),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grade_bands() {
        assert_eq!(HealthGrade::from_score(100), HealthGrade::A);
        assert_eq!(HealthGrade::from_score(90), HealthGrade::A);
        assert_eq!(HealthGrade::from_score(89), HealthGrade::B);
        assert_eq!(HealthGrade::from_score(80), HealthGrade::B);
        assert_eq!(HealthGrade::from_score(70), HealthGrade::C);
        assert_eq!(HealthGrade::from_score(60), HealthGrade::D);
        assert_eq!(HealthGrade::from_score(59), HealthGrade::F);
        assert_eq!(HealthGrade::from_score(0), HealthGrade::F);
    }

    #[test]
    fn grade_wire_strings() {
        assert_eq!(HealthGrade::A.as_str(), "A");
        assert_eq!(HealthGrade::F.as_str(), "F");
    }

    #[test]
    fn empty_project_fails_closed() {
        let r = score(&HealthMetrics {
            total_symbols: 0,
            dead_count: 0,
            isolated_count: 0,
            edge_count: 0,
        });
        assert_eq!(r.grade, HealthGrade::F);
        assert_eq!(r.score, 0);
    }

    #[test]
    fn healthy_project_grades_high() {
        // 2% dead, 1% isolated, density 2.5 (>= target) → near-100.
        let r = score(&HealthMetrics {
            total_symbols: 100,
            dead_count: 2,
            isolated_count: 1,
            edge_count: 250,
        });
        assert!(
            r.score >= 95,
            "expected A, got {} ({})",
            r.score,
            r.grade.as_str()
        );
        assert_eq!(r.grade, HealthGrade::A);
    }

    #[test]
    fn high_dead_code_drops_the_grade() {
        let healthy = score(&HealthMetrics {
            total_symbols: 100,
            dead_count: 2,
            isolated_count: 1,
            edge_count: 250,
        });
        let rotten = score(&HealthMetrics {
            total_symbols: 100,
            dead_count: 50, // half the codebase is dead
            isolated_count: 1,
            edge_count: 250,
        });
        assert!(rotten.score < healthy.score);
        // 0.45 weight × 50pt drop ≈ −22 → from ~99 to ~77 → grade falls.
        assert!(matches!(rotten.grade, HealthGrade::C | HealthGrade::D));
    }

    #[test]
    fn fully_isolated_project_scores_low() {
        let r = score(&HealthMetrics {
            total_symbols: 100,
            dead_count: 100,
            isolated_count: 100,
            edge_count: 0,
        });
        assert_eq!(r.score, 0);
        assert_eq!(r.grade, HealthGrade::F);
    }

    #[test]
    fn payload_shape_is_correct() {
        let r = score(&HealthMetrics {
            total_symbols: 100,
            dead_count: 2,
            isolated_count: 1,
            edge_count: 200,
        });
        let v = project_health_payload(&r);
        // Grade should be "A" for healthy project.
        assert_eq!(v["grade"], r.grade.as_str());
        assert!(v["score"].is_u64());
        let dims = v["dimensions"].as_array().unwrap();
        assert_eq!(dims.len(), 3);
        assert_eq!(dims[0]["name"], "dead_code");
        assert_eq!(dims[1]["name"], "isolation");
        assert_eq!(dims[2]["name"], "connectivity");
    }

    #[test]
    fn payload_empty_project() {
        let r = score(&HealthMetrics {
            total_symbols: 0,
            dead_count: 0,
            isolated_count: 0,
            edge_count: 0,
        });
        let v = project_health_payload(&r);
        assert_eq!(v["grade"], "F");
        assert_eq!(v["score"], 0);
    }

    #[test]
    fn dimensions_are_reported() {
        let r = score(&HealthMetrics {
            total_symbols: 100,
            dead_count: 10,
            isolated_count: 0,
            edge_count: 200,
        });
        let names: Vec<_> = r.dimensions.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["dead_code", "isolation", "connectivity"]);
        // dead_code dim = 100*(1 - 10/100) = 90.
        assert_eq!(r.dimensions[0], ("dead_code", 90));
        assert_eq!(r.dimensions[1], ("isolation", 100));
    }
}
