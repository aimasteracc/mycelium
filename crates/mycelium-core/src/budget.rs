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

use serde_json::Value;

/// Per-project caps on the array sizes a tool response may return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputBudget {
    /// Cap for node-shaped arrays (`nodes`, `paths`, `results`, `symbols`).
    pub max_nodes: usize,
    /// Cap for edge-shaped arrays (`edges`, `callees`, `callers`, `reachable`).
    pub max_edges: usize,
}

impl OutputBudget {
    /// The budget tier for a project of `node_count` nodes.
    #[must_use]
    pub const fn for_project(node_count: usize) -> Self {
        if node_count < 500 {
            Self {
                max_nodes: 15,
                max_edges: 30,
            }
        } else if node_count < 5_000 {
            Self {
                max_nodes: 30,
                max_edges: 60,
            }
        } else {
            Self {
                max_nodes: 50,
                max_edges: 100,
            }
        }
    }
}

/// Truncate the budgeted arrays of a JSON tool response in place.
///
/// Node-shaped arrays are capped at `max_nodes`, edge-shaped arrays at
/// `max_edges`. When anything is truncated, `truncated: true` and
/// `total_available: <pre-truncation count>` are written so the caller can
/// detect it and ask for more. Absent keys are ignored.
pub fn apply_budget(value: &mut Value, budget: &OutputBudget) {
    let mut truncated = false;
    let mut total_available: Option<usize> = None;

    let mut cap = |key: &str, limit: usize| {
        if let Some(arr) = value.get_mut(key).and_then(Value::as_array_mut) {
            let count = arr.len();
            if count > limit {
                arr.truncate(limit);
                truncated = true;
                if total_available.is_none() {
                    total_available = Some(count);
                }
            }
        }
    };

    for key in ["nodes", "paths", "results", "symbols"] {
        cap(key, budget.max_nodes);
    }
    for key in ["edges", "callees", "callers", "reachable"] {
        cap(key, budget.max_edges);
    }

    if truncated {
        value["truncated"] = Value::Bool(true);
        if let Some(avail) = total_available {
            value["total_available"] = Value::Number(avail.into());
        }
    }
}

#[cfg(test)]
mod tests;
