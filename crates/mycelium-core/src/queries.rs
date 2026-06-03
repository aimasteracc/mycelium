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

/// Build the `{ "caller_paths": [...] }` payload for `get_callers`.
///
/// The sorted, deduplicated trunk paths that reach `id` via one incoming `kind`
/// edge. When `include_virtual` and `kind == Calls`, virtual-dispatch callers of
/// `path` (callers of an ancestor method of the same name) are merged in.
#[must_use]
pub fn callers_payload(
    store: &Store,
    id: NodeId,
    path: &str,
    kind: EdgeKind,
    include_virtual: bool,
) -> Value {
    let mut paths: Vec<String> = store
        .incoming(id, kind)
        .iter()
        .filter_map(|&src| store.path_of(src).map(str::to_owned))
        .collect();
    if kind == EdgeKind::Calls && include_virtual {
        paths.extend(
            store
                .virtual_dispatch_callers_of_path(path)
                .unwrap_or_default(),
        );
    }
    paths.sort();
    paths.dedup();
    json!({ "caller_paths": paths })
}

/// Build the `{ "dead_symbols": [...], "count": N }` payload for
/// `get_dead_symbols` from an already-computed list of dead symbols.
///
/// `count` is the full pre-budget total, so a caller still learns the true size
/// when [`apply_budget`](crate::budget::apply_budget) later truncates the array.
#[must_use]
pub fn dead_symbols_payload(dead: &[String]) -> Value {
    json!({ "dead_symbols": dead, "count": dead.len() })
}

/// Build the `{ "isolated_symbols": [...], "count": N }` payload for
/// `get_isolated_symbols` from an already-computed list.
///
/// `count` is the full pre-budget total (see [`dead_symbols_payload`]).
#[must_use]
pub fn isolated_symbols_payload(isolated: &[String]) -> Value {
    json!({ "isolated_symbols": isolated, "count": isolated.len() })
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

    #[test]
    fn callers_payload_is_a_sorted_deduped_object() {
        let mut store = Store::new();
        let target = store.upsert_node(p("src/t.rs>T>target"));
        let z = store.upsert_node(p("src/z.rs>Z>zeta"));
        let b = store.upsert_node(p("src/b.rs>B>beta"));
        store.upsert_edge(EdgeKind::Calls, z, target);
        store.upsert_edge(EdgeKind::Calls, b, target);

        let v = callers_payload(&store, target, "src/t.rs>T>target", EdgeKind::Calls, false);
        let arr = v["caller_paths"]
            .as_array()
            .expect("caller_paths must be an array");
        assert_eq!(arr[0], "src/b.rs>B>beta");
        assert_eq!(arr[1], "src/z.rs>Z>zeta");
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn dead_symbols_payload_has_array_and_count() {
        let v = dead_symbols_payload(&["a".to_owned(), "b".to_owned()]);
        assert_eq!(v["dead_symbols"], serde_json::json!(["a", "b"]));
        assert_eq!(v["count"], 2);
    }

    #[test]
    fn isolated_symbols_payload_has_array_and_count() {
        let v = isolated_symbols_payload(&["x".to_owned()]);
        assert_eq!(v["isolated_symbols"], serde_json::json!(["x"]));
        assert_eq!(v["count"], 1);
    }
}
