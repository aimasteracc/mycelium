//! Shared graph-list query payload builders (RFC-0109 Option A).
//!
//! Each function returns the canonical JSON object a graph-list tool emits.
//! **Both** the MCP tool and its CLI twin call the same builder, so their JSON
//! is byte-identical by construction (Charter §5.13 / RFC-0090 Three-Surface
//! Rule) — there is no per-surface payload code to drift. Budgeting
//! ([`crate::budget::apply_budget`]) is applied by the caller *after* building,
//! so the budget knob (RFC-0102) layers on uniformly.

use serde_json::{Value, json};

use crate::store::Store;
use crate::types::{EdgeKind, NodeId};

/// Build the `{ "callee_paths": [...] }` payload for `get_callees`: the sorted,
/// deduplicated trunk paths reachable from `id` via one outgoing `kind` edge.
#[must_use]
pub fn callees_payload(store: &Store, id: NodeId, kind: EdgeKind) -> Value {
    let mut paths: Vec<String> = store
        .outgoing(id, kind)
        .iter()
        .filter_map(|&dst| store.path_of(dst).map(str::to_owned))
        .collect();
    paths.sort();
    paths.dedup();
    json!({ "callee_paths": paths })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trunk::TrunkPath;

    fn p(s: &str) -> TrunkPath {
        TrunkPath::parse(s).unwrap()
    }

    #[test]
    fn callees_payload_is_a_sorted_deduped_object() {
        let mut store = Store::new();
        let a = store.upsert_node(p("src/a.rs>A>run"));
        let z = store.upsert_node(p("src/z.rs>Z>zeta"));
        let b = store.upsert_node(p("src/b.rs>B>beta"));
        store.upsert_edge(EdgeKind::Calls, a, z);
        store.upsert_edge(EdgeKind::Calls, a, b);

        let v = callees_payload(&store, a, EdgeKind::Calls);

        // Object shape with the `callee_paths` key (RFC-0109 Option A) ...
        let arr = v["callee_paths"]
            .as_array()
            .expect("callee_paths must be an array");
        // ... sorted lexicographically.
        assert_eq!(arr[0], "src/b.rs>B>beta");
        assert_eq!(arr[1], "src/z.rs>Z>zeta");
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn callees_payload_empty_for_leaf() {
        let mut store = Store::new();
        let leaf = store.upsert_node(p("src/a.rs>A>leaf"));
        let v = callees_payload(&store, leaf, EdgeKind::Calls);
        assert_eq!(v["callee_paths"].as_array().unwrap().len(), 0);
    }
}
