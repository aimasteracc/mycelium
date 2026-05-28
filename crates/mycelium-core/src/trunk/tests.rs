//! Trunk tests — written *before* the Trunk implementation per Charter §5.1.
//!
//! Each test maps to a specific acceptance criterion in RFC-0001 §Testing strategy.
//! When all of these pass, Trunk satisfies the spike contract.

use super::{Trunk, TrunkPath, path};
use crate::error::Error;

// ──────────────────────────────────────────────────────────────────────
// TrunkPath validation
// ──────────────────────────────────────────────────────────────────────

#[test]
fn trunk_path_parse_accepts_simple_path() {
    // RFC-0001 §3.1: valid paths are addressable.
    let p = TrunkPath::parse("src/auth.rs").unwrap();
    assert_eq!(p.as_str(), "src/auth.rs");
    assert_eq!(p.depth(), 1);
}

#[test]
fn trunk_path_parse_accepts_deep_path() {
    let p = TrunkPath::parse("src/auth.rs>AuthService>login(email, password)>validate").unwrap();
    assert_eq!(p.depth(), 4);
    let segs: Vec<&str> = p.segments().collect();
    assert_eq!(
        segs,
        vec![
            "src/auth.rs",
            "AuthService",
            "login(email, password)",
            "validate"
        ]
    );
}

#[test]
fn trunk_path_parse_rejects_empty_string() {
    assert!(matches!(
        TrunkPath::parse(""),
        Err(Error::InvalidPath { .. })
    ));
}

#[test]
fn trunk_path_parse_rejects_double_separator() {
    // RFC-0001 §3.1: empty segments not allowed.
    assert!(matches!(
        TrunkPath::parse("a>>b"),
        Err(Error::InvalidPath { .. })
    ));
}

#[test]
fn trunk_path_parse_rejects_leading_or_trailing_separator() {
    assert!(TrunkPath::parse(">a").is_err());
    assert!(TrunkPath::parse("a>").is_err());
}

#[test]
fn trunk_path_parse_rejects_control_character() {
    assert!(TrunkPath::parse("a\x07b").is_err());
}

#[test]
fn trunk_path_join_appends_segment() {
    let p = TrunkPath::parse("src/lib.rs>Foo").unwrap();
    let q = p.join("bar").unwrap();
    assert_eq!(q.as_str(), "src/lib.rs>Foo>bar");
}

#[test]
fn trunk_path_join_rejects_segment_with_separator() {
    let p = TrunkPath::parse("a").unwrap();
    assert!(p.join("b>c").is_err());
}

#[test]
fn trunk_path_parent_returns_immediate_ancestor() {
    let p = TrunkPath::parse("a>b>c").unwrap();
    assert_eq!(p.parent().unwrap().as_str(), "a>b");
}

#[test]
fn trunk_path_parent_returns_none_for_root() {
    let p = TrunkPath::parse("solo").unwrap();
    assert!(p.parent().is_none());
}

// ──────────────────────────────────────────────────────────────────────
// Trunk insertion + lookup
// ──────────────────────────────────────────────────────────────────────

#[test]
fn trunk_inserts_create_addressable_paths() {
    // RFC-0001 §3.1: insertions must make the path queryable by exact match.
    let mut trunk = Trunk::new();
    let id = trunk.upsert(TrunkPath::parse("src/auth.rs>AuthService>login").unwrap());
    assert_eq!(trunk.lookup_path("src/auth.rs>AuthService>login"), Some(id));
}

#[test]
fn trunk_upsert_is_idempotent() {
    let mut trunk = Trunk::new();
    let a = trunk.upsert(TrunkPath::parse("a>b").unwrap());
    let b = trunk.upsert(TrunkPath::parse("a>b").unwrap());
    assert_eq!(a, b, "same path must yield same NodeId");
    assert_eq!(trunk.len(), 1, "duplicate upsert must not grow state");
}

#[test]
fn trunk_lookup_distinguishes_exact_match_from_prefix() {
    // RFC-0001 §3.1: a non-materialized ancestor is NOT a node.
    let mut trunk = Trunk::new();
    trunk.upsert(TrunkPath::parse("src/auth.rs>AuthService>login").unwrap());

    // The materialized leaf is addressable…
    assert!(trunk.lookup_path("src/auth.rs>AuthService>login").is_some());
    // …but an unmaterialized intermediate is NOT.
    assert_eq!(trunk.lookup_path("src/auth.rs>AuthService"), None);
    assert_eq!(trunk.lookup_path("src/auth.rs"), None);
}

#[test]
fn trunk_path_of_returns_the_path_for_an_id() {
    let mut trunk = Trunk::new();
    let id = trunk.upsert(TrunkPath::parse("x>y").unwrap());
    assert_eq!(trunk.path_of(id), Some("x>y"));
}

// ──────────────────────────────────────────────────────────────────────
// Trunk ancestors
// ──────────────────────────────────────────────────────────────────────

#[test]
fn trunk_ancestors_returns_materialized_ancestors_child_first() {
    let mut trunk = Trunk::new();
    let file = trunk.upsert(TrunkPath::parse("src/auth.rs").unwrap());
    let cls = trunk.upsert(TrunkPath::parse("src/auth.rs>AuthService").unwrap());
    let m = trunk.upsert(TrunkPath::parse("src/auth.rs>AuthService>login").unwrap());

    let ancestors: Vec<_> = trunk.ancestors(m).collect();
    // Child-to-root order: class first, then file.
    assert_eq!(ancestors, vec![cls, file]);
}

#[test]
fn trunk_ancestors_skips_unmaterialized_ones() {
    let mut trunk = Trunk::new();
    let file = trunk.upsert(TrunkPath::parse("src/auth.rs").unwrap());
    // Intermediate class is NOT upserted.
    let m = trunk.upsert(TrunkPath::parse("src/auth.rs>AuthService>login").unwrap());

    let ancestors: Vec<_> = trunk.ancestors(m).collect();
    assert_eq!(
        ancestors,
        vec![file],
        "non-materialized ancestor must be skipped"
    );
}

// ──────────────────────────────────────────────────────────────────────
// Trunk descendants
// ──────────────────────────────────────────────────────────────────────

#[test]
fn trunk_descendants_returns_all_below() {
    let mut trunk = Trunk::new();
    let cls = trunk.upsert(TrunkPath::parse("src/auth.rs>AuthService").unwrap());
    let m1 = trunk.upsert(TrunkPath::parse("src/auth.rs>AuthService>login").unwrap());
    let m2 = trunk.upsert(TrunkPath::parse("src/auth.rs>AuthService>logout").unwrap());
    let m1_local =
        trunk.upsert(TrunkPath::parse("src/auth.rs>AuthService>login>validate").unwrap());

    let mut descendants: Vec<_> = trunk.descendants(cls).collect();
    descendants.sort();
    let mut want = vec![m1, m2, m1_local];
    want.sort();
    assert_eq!(descendants, want);
}

#[test]
fn trunk_descendants_excludes_siblings_and_self() {
    let mut trunk = Trunk::new();
    let cls_a = trunk.upsert(TrunkPath::parse("file>A").unwrap());
    let cls_b = trunk.upsert(TrunkPath::parse("file>B").unwrap());
    let m_a = trunk.upsert(TrunkPath::parse("file>A>m").unwrap());
    let _m_b = trunk.upsert(TrunkPath::parse("file>B>m").unwrap());

    let descendants: Vec<_> = trunk.descendants(cls_a).collect();
    assert_eq!(descendants, vec![m_a]);
    assert!(!descendants.contains(&cls_a)); // not self
    assert!(!descendants.contains(&cls_b)); // not sibling
}

#[test]
fn trunk_descendants_must_not_match_prefix_of_sibling_segment_name() {
    // The classic radix-trie correctness trap: "Auth" must NOT match "AuthService".
    // We enforce this by requiring `>` after the prefix.
    let mut trunk = Trunk::new();
    let auth = trunk.upsert(TrunkPath::parse("Auth").unwrap());
    let _auth_service = trunk.upsert(TrunkPath::parse("AuthService").unwrap());
    let auth_m = trunk.upsert(TrunkPath::parse("Auth>method").unwrap());

    let descendants: Vec<_> = trunk.descendants(auth).collect();
    assert_eq!(
        descendants,
        vec![auth_m],
        "descendants of `Auth` must NOT include `AuthService`"
    );
}

// ──────────────────────────────────────────────────────────────────────
// Trunk removal
// ──────────────────────────────────────────────────────────────────────

#[test]
fn trunk_remove_drops_only_self() {
    let mut trunk = Trunk::new();
    let cls = trunk.upsert(TrunkPath::parse("file>A").unwrap());
    let m = trunk.upsert(TrunkPath::parse("file>A>m").unwrap());

    assert!(trunk.remove(cls));
    assert!(trunk.lookup_path("file>A").is_none());
    assert_eq!(trunk.lookup_path("file>A>m"), Some(m), "non-cascading");
}

#[test]
fn trunk_remove_subtree_drops_descendants_and_returns_count() {
    let mut trunk = Trunk::new();
    let cls = trunk.upsert(TrunkPath::parse("file>A").unwrap());
    trunk.upsert(TrunkPath::parse("file>A>m").unwrap());
    trunk.upsert(TrunkPath::parse("file>A>n").unwrap());
    let sibling = trunk.upsert(TrunkPath::parse("file>B").unwrap());

    let removed = trunk.remove_subtree(cls);
    assert_eq!(removed, 3, "should remove class A + 2 methods");
    assert!(trunk.lookup_path("file>A").is_none());
    assert!(trunk.lookup_path("file>A>m").is_none());
    assert!(trunk.lookup_path("file>A>n").is_none());
    assert_eq!(trunk.lookup_path("file>B"), Some(sibling), "sibling intact");
}

#[test]
fn trunk_remove_unknown_id_is_a_noop_returning_false() {
    let mut trunk = Trunk::new();
    let ghost = crate::types::NodeId(0xDEAD_BEEF_0000_0000);
    assert!(!trunk.remove(ghost));
    assert_eq!(trunk.remove_subtree(ghost), 0);
}

// ──────────────────────────────────────────────────────────────────────
// NodeId stability
// ──────────────────────────────────────────────────────────────────────

#[test]
fn trunk_node_ids_are_stable_across_trunks_for_same_path() {
    // RFC-0001 §detailed-design: NodeId derives from a hash of the path.
    let mut a = Trunk::new();
    let mut b = Trunk::new();
    let id_a = a.upsert(TrunkPath::parse("src/lib.rs>Foo").unwrap());
    let id_b = b.upsert(TrunkPath::parse("src/lib.rs>Foo").unwrap());
    assert_eq!(id_a, id_b);
}

#[test]
fn trunk_node_ids_have_shard_byte_reserved() {
    // RFC-0001: low 8 bits reserved for shard tag (currently 0).
    let mut trunk = Trunk::new();
    for path in [
        "a",
        "b",
        "c",
        "src/auth.rs>AuthService>login(x, y)>validate",
    ] {
        let id = trunk.upsert(TrunkPath::parse(path).unwrap());
        assert_eq!(
            id.as_u64() & 0xFF,
            0,
            "low byte of NodeId must be the (currently-zero) shard tag for path {path:?}",
        );
    }
}

// ──────────────────────────────────────────────────────────────────────
// path::parent helper
// ──────────────────────────────────────────────────────────────────────

#[test]
fn path_parent_helper_handles_edges() {
    assert_eq!(path::parent("a>b>c"), Some("a>b"));
    assert_eq!(path::parent("a>b"), Some("a"));
    assert_eq!(path::parent("a"), None);
    assert_eq!(path::parent(""), None);
}

// ──────────────────────────────────────────────────────────────────────
// Proptest — property-based invariants (RFC-0001 §Testing strategy)
// ──────────────────────────────────────────────────────────────────────

mod prop {
    use crate::trunk::{Trunk, TrunkPath};
    use crate::types::NodeId;
    use proptest::prelude::*;

    fn valid_path_str() -> impl Strategy<Value = String> {
        proptest::collection::vec(
            proptest::string::string_regex("[a-z][a-z0-9]{0,7}").unwrap(),
            1..=4_usize,
        )
        .prop_map(|segs| segs.join(">"))
    }

    proptest! {
        /// After any sequence of upserts, `lookup_path` and `path_of` are
        /// mutually consistent (RFC-0001 §Testing strategy, invariant 1).
        #[test]
        fn indices_consistent_after_upserts(
            strs in proptest::collection::vec(valid_path_str(), 1..=20_usize)
        ) {
            let mut trunk = Trunk::new();
            // Track unique (path_str, id) pairs; skip hash collisions (negligible).
            let mut pairs: Vec<(String, NodeId)> = Vec::new();
            for s in &strs {
                if let Ok(p) = TrunkPath::parse(s) {
                    let id = trunk.upsert(p);
                    if !pairs.iter().any(|(_, eid)| *eid == id) {
                        pairs.push((s.clone(), id));
                    }
                }
            }
            for (s, id) in &pairs {
                prop_assert_eq!(trunk.lookup_path(s), Some(*id));
                prop_assert_eq!(trunk.path_of(*id), Some(s.as_str()));
            }
        }

        /// `upsert` is idempotent: same path yields same id and the trunk
        /// does not grow (RFC-0001 §3.1 "idempotent upsert" requirement).
        #[test]
        fn upsert_is_idempotent(s in valid_path_str()) {
            let mut trunk = Trunk::new();
            let id1 = trunk.upsert(TrunkPath::parse(&s).unwrap());
            let id2 = trunk.upsert(TrunkPath::parse(&s).unwrap());
            prop_assert_eq!(id1, id2);
            prop_assert_eq!(trunk.len(), 1);
        }

        /// After removing a prefix of nodes, removed nodes are inaccessible
        /// and the remaining nodes remain consistent.
        #[test]
        fn consistent_after_removes(
            strs in proptest::collection::vec(valid_path_str(), 2..=20_usize),
            remove_count in 0..=5_usize,
        ) {
            let mut trunk = Trunk::new();
            let mut pairs: Vec<(String, NodeId)> = Vec::new();
            for s in &strs {
                if let Ok(p) = TrunkPath::parse(s) {
                    let id = trunk.upsert(p);
                    if !pairs.iter().any(|(_, eid)| *eid == id) {
                        pairs.push((s.clone(), id));
                    }
                }
            }
            let cut = remove_count.min(pairs.len());
            for (_, id) in &pairs[..cut] {
                trunk.remove(*id);
            }
            // Removed: gone from both directions.
            for (s, id) in &pairs[..cut] {
                prop_assert!(trunk.lookup_path(s).is_none());
                prop_assert!(trunk.path_of(*id).is_none());
            }
            // Remaining: still accessible.
            for (s, id) in &pairs[cut..] {
                prop_assert_eq!(trunk.lookup_path(s), Some(*id));
                prop_assert_eq!(trunk.path_of(*id), Some(s.as_str()));
            }
        }
    }
}
