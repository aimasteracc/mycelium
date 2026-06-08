//! Adaptive output budget for agent-facing results (RFC-0102, issue #380).
//!
//! Lives in `mycelium_core` so **both** the MCP tool surface and the CLI apply
//! the *same* truncation to the *same* payload — a budget applied on only one
//! surface would break the Three-Surface byte-identical contract (Charter
//! §5.13). The budget caps the large arrays a single call can return so one
//! response cannot flood an agent's context window.
//!
//! Tiers (keyed on `Store::node_count`) match `CodeGraph`'s proven sizing:
//!
//! | Project size | `max_nodes` | `max_edges` |
//! |--------------|-------------|-------------|
//! | `< 500`      | 15          | 30          |
//! | `500..5_000` | 30          | 60          |
//! | `>= 5_000`   | 50          | 100         |

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The project-size tier a budget was derived from (RFC-0102 §"Response
/// metadata"). Reported back to the caller in the nested `budget.mode` field so
/// an agent can reason about *why* a response was capped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BudgetMode {
    /// `< 500` nodes.
    Small,
    /// `500..5_000` nodes.
    Medium,
    /// `>= 5_000` nodes.
    Large,
    /// No caps — the caller opted out of budgeting (`BudgetOverride::Disabled`).
    Disabled,
}

impl BudgetMode {
    /// The lowercase wire token (`"small"`, `"medium"`, `"large"`,
    /// `"disabled"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
            Self::Disabled => "disabled",
        }
    }
}

/// A caller-supplied per-call budget override (RFC-0102 §"Request knobs").
///
/// Parsed from the MCP `budget` field / CLI `--budget` flag. `Auto` (the
/// default) defers to the project-size tier; the explicit tiers pin the caps;
/// `Disabled` opts out of truncation entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BudgetOverride {
    /// Defer to the project-size tier (`OutputBudget::for_project`).
    Auto,
    /// Pin the `Small` tier regardless of project size.
    Small,
    /// Pin the `Medium` tier regardless of project size.
    Medium,
    /// Pin the `Large` tier regardless of project size.
    Large,
    /// Opt out of truncation — return the full payload.
    Disabled,
}

impl std::str::FromStr for BudgetOverride {
    type Err = String;

    /// Parse a wire token case-insensitively. Unknown values are rejected with
    /// a message that names the offending value (boundary validation).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "small" => Ok(Self::Small),
            "medium" => Ok(Self::Medium),
            "large" => Ok(Self::Large),
            "disabled" => Ok(Self::Disabled),
            other => Err(format!(
                "unknown budget value {other:?}; expected one of: auto, small, medium, large, disabled"
            )),
        }
    }
}

/// Per-project caps on the array sizes a tool response may return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputBudget {
    /// The tier this budget was derived from.
    pub mode: BudgetMode,
    /// Cap for node-shaped arrays (`nodes`, `paths`, `results`, `symbols`).
    pub max_nodes: usize,
    /// Cap for edge-shaped arrays (`edges`, `callees`, `callers`, `reachable`).
    pub max_edges: usize,
}

impl OutputBudget {
    /// Resolve an effective budget from an optional per-call override and the
    /// live project size (RFC-0102 §"Request knobs").
    ///
    /// * `None` / `Auto` → the project-size tier ([`Self::for_project`]).
    /// * `Small` / `Medium` / `Large` → that tier's caps, *ignoring* size.
    /// * `Disabled` → uncapped (no truncation).
    ///
    /// Both the MCP tool and its CLI twin call this with the same arguments, so
    /// the resolved budget — and therefore the truncated payload — stays
    /// byte-identical across surfaces (Three-Surface Rule).
    #[must_use]
    pub const fn resolve(over: Option<BudgetOverride>, node_count: usize) -> Self {
        match over {
            None | Some(BudgetOverride::Auto) => Self::for_project(node_count),
            // Pin each tier to a representative size so the caps stay in one
            // place (`for_project`) rather than duplicated here.
            Some(BudgetOverride::Small) => Self::for_project(0),
            Some(BudgetOverride::Medium) => Self::for_project(500),
            Some(BudgetOverride::Large) => Self::for_project(5_000),
            Some(BudgetOverride::Disabled) => Self {
                mode: BudgetMode::Disabled,
                max_nodes: usize::MAX,
                max_edges: usize::MAX,
            },
        }
    }

    /// The budget tier for a project of `node_count` nodes.
    #[must_use]
    pub const fn for_project(node_count: usize) -> Self {
        if node_count < 500 {
            Self {
                mode: BudgetMode::Small,
                max_nodes: 15,
                max_edges: 30,
            }
        } else if node_count < 5_000 {
            Self {
                mode: BudgetMode::Medium,
                max_nodes: 30,
                max_edges: 60,
            }
        } else {
            Self {
                mode: BudgetMode::Large,
                max_nodes: 50,
                max_edges: 100,
            }
        }
    }
}

/// Truncate the budgeted arrays of a JSON tool response in place.
///
/// Node-shaped arrays are capped at `max_nodes`, edge-shaped arrays at
/// `max_edges`. When anything is truncated:
///
/// * the flat `truncated: true` + `total_available: <first capped count>`
///   fields are written (kept for backward compatibility), and
/// * a nested `budget` object (RFC-0102 §"Response metadata") is attached,
///   carrying `mode`, `truncated`, the `truncated_fields` list, a per-field
///   `total_available` map, and the `limits` that were applied.
///
/// The nested object is added without removing existing keys, per RFC-0102.
/// Absent keys are ignored; when nothing is truncated, no metadata is written.
pub fn apply_budget(value: &mut Value, budget: &OutputBudget) {
    // (field-key, pre-truncation count), in the deterministic order capping ran.
    let mut capped: Vec<(&'static str, usize)> = Vec::new();

    let mut cap = |key: &'static str, limit: usize| {
        if let Some(arr) = value.get_mut(key).and_then(Value::as_array_mut) {
            let count = arr.len();
            if count > limit {
                arr.truncate(limit);
                capped.push((key, count));
            }
        }
    };

    // Node-shaped arrays. `dead_symbols` / `isolated_symbols` are the keys the
    // get_dead_symbols / get_isolated_symbols tools actually emit (they are not
    // `symbols`), so they must be listed explicitly or their budget no-ops.
    for key in [
        "nodes",
        "paths",
        "results",
        "symbols",
        "dead_symbols",
        "isolated_symbols",
        "entry_points",
    ] {
        cap(key, budget.max_nodes);
    }
    // Edge-shaped arrays. `callee_paths` / `caller_paths` are the keys the
    // get_callees / get_callers tools actually emit (not `callees`/`callers`).
    for key in [
        "edges",
        "callees",
        "callers",
        "reachable",
        "callee_paths",
        "caller_paths",
    ] {
        cap(key, budget.max_edges);
    }

    if capped.is_empty() {
        return;
    }

    // Flat fields (backward compatible): first capped field's pre-trunc count.
    value["truncated"] = Value::Bool(true);
    value["total_available"] = Value::Number(capped[0].1.into());

    // Nested object (RFC-0102 §"Response metadata").
    let truncated_fields: Vec<Value> = capped
        .iter()
        .map(|(key, _)| Value::String((*key).to_string()))
        .collect();
    let mut total_available = serde_json::Map::new();
    for (key, count) in &capped {
        total_available.insert((*key).to_string(), Value::Number((*count).into()));
    }
    value["budget"] = serde_json::json!({
        "mode": budget.mode.as_str(),
        "truncated": true,
        "truncated_fields": truncated_fields,
        "total_available": total_available,
        "limits": {
            "max_nodes": budget.max_nodes,
            "max_edges": budget.max_edges,
        },
    });
}

#[cfg(test)]
mod tests;
