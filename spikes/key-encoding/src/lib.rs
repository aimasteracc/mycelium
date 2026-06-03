//! RFC-0100 / ADR-0008 de-risk spike T0 — order-preserving composite key encoding.
//!
//! # What this proves
//!
//! Two independent encodings must both satisfy the **order-preservation** property:
//! for any two logical values A, B, `encode(A).cmp(encode(B)) == A.cmp(B)`.
//!
//! ## Part A — Adjacency key: `(EdgeKind, NodeId)`
//!
//! Wire layout: `[u16 big-endian kind_tag] ++ [u64 big-endian node_id]` = 10 bytes.
//!
//! Big-endian encoding gives lexicographic byte order == numeric order, so all
//! edges of one EdgeKind are stored contiguously in redb and ordered by NodeId
//! within that slice.
//!
//! ## Part B — Trunk path key with NUL separator
//!
//! ADR-0008 §4 identifies a defect with the current `>` (0x3E) separator:
//! because 0x3E > 0x00..=0x3D but < 0x3F..=0xFF, a path like `"foo"` and
//! `"foobar"` are wrongly ordered relative to `"foo>bar"` in a raw byte scan.
//! The fix: replace `>` with NUL (`\0`, 0x00), the **lowest** printable byte.
//! Then the invariant holds: every parent prefix `"foo\0"` sorts before every
//! key that starts with `"foo"` but adds more segments, **and** `"foo\0bar"`
//! sorts before `"foobar"` (because `\0` < any printable byte).
//!
//! This file contains:
//! 1. `encode_adjacency_key(kind_tag, node_id) -> [u8; 10]`
//! 2. `decode_adjacency_key(&[u8; 10]) -> (u16, u64)`
//! 3. `encode_path_key(segments: &[&str]) -> Vec<u8>` (NUL-separated)
//! 4. Logical comparison helpers used in property tests.
//! 5. All property and targeted tests in the `#[cfg(test)]` module.

// ─────────────────────────────────────────────────────────────────────────────
// Part A — Adjacency key encoding
// ─────────────────────────────────────────────────────────────────────────────

/// Encode a `(kind_tag: u16, node_id: u64)` pair into a 10-byte big-endian
/// composite key.
///
/// Byte layout: `[kind_tag_hi, kind_tag_lo, node_hi, …, node_lo]`
///
/// Invariant: for any `(k1, n1)` and `(k2, n2)`,
/// `encode(k1, n1).cmp(&encode(k2, n2)) == (k1, n1).cmp(&(k2, n2))`.
#[inline]
pub fn encode_adjacency_key(kind_tag: u16, node_id: u64) -> [u8; 10] {
    let mut buf = [0u8; 10];
    buf[..2].copy_from_slice(&kind_tag.to_be_bytes());
    buf[2..].copy_from_slice(&node_id.to_be_bytes());
    buf
}

/// Decode a 10-byte adjacency key back into `(kind_tag, node_id)`.
#[inline]
pub fn decode_adjacency_key(key: &[u8; 10]) -> (u16, u64) {
    let kind_tag = u16::from_be_bytes([key[0], key[1]]);
    let node_id = u64::from_be_bytes([key[2], key[3], key[4], key[5], key[6], key[7], key[8], key[9]]);
    (kind_tag, node_id)
}

// ─────────────────────────────────────────────────────────────────────────────
// Part B — Trunk path key encoding (NUL separator)
// ─────────────────────────────────────────────────────────────────────────────

/// The NUL byte used as a path segment separator in the storage key encoding.
/// Chosen because 0x00 is the lowest byte value, guaranteeing that any
/// separator-terminated prefix sorts strictly before any extension of that
/// prefix.
pub const PATH_KEY_SEPARATOR: u8 = 0x00;

/// Encode a sequence of path segments into a NUL-separated byte key.
///
/// Examples:
/// - `["foo"]`            → `b"foo"`
/// - `["foo", "bar"]`     → `b"foo\0bar"`
/// - `["foo", "bar", "baz"]` → `b"foo\0bar\0baz"`
///
/// # Panics
/// Panics if any segment contains a NUL byte (0x00), since NUL is the
/// separator. Path segments in Mycelium are validated to exclude control
/// characters < 0x20 (including NUL), so this never triggers in production.
pub fn encode_path_key(segments: &[&str]) -> Vec<u8> {
    for seg in segments {
        assert!(
            !seg.bytes().any(|b| b == 0x00),
            "path segment must not contain NUL byte: {seg:?}"
        );
        assert!(!seg.is_empty(), "path segment must not be empty");
    }
    assert!(!segments.is_empty(), "path must have at least one segment");

    let total_len: usize = segments.iter().map(|s| s.len()).sum::<usize>()
        + segments.len().saturating_sub(1); // one separator between each pair
    let mut buf = Vec::with_capacity(total_len);
    for (i, seg) in segments.iter().enumerate() {
        if i > 0 {
            buf.push(PATH_KEY_SEPARATOR);
        }
        buf.extend_from_slice(seg.as_bytes());
    }
    buf
}

/// Decode a NUL-separated byte key back into owned segment strings.
pub fn decode_path_key(key: &[u8]) -> Vec<String> {
    key.split(|&b| b == PATH_KEY_SEPARATOR)
        .map(|seg| String::from_utf8_lossy(seg).into_owned())
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Logical ordering helpers (used by property tests to compare intended order)
// ─────────────────────────────────────────────────────────────────────────────

/// The intended logical ordering for adjacency keys: first by kind_tag, then
/// by node_id, both ascending.
#[inline]
pub fn logical_cmp_adjacency(
    (k1, n1): (u16, u64),
    (k2, n2): (u16, u64),
) -> std::cmp::Ordering {
    k1.cmp(&k2).then(n1.cmp(&n2))
}

/// The intended logical ordering for path keys: lexicographic over segments.
/// - Compare element-by-element; shorter wins on common prefix tie
///   (i.e. `["foo"]` < `["foo", "bar"]`).
/// This matches what a NUL-separated byte comparison produces.
#[inline]
pub fn logical_cmp_path(a: &[&str], b: &[&str]) -> std::cmp::Ordering {
    for (sa, sb) in a.iter().zip(b.iter()) {
        let c = sa.cmp(sb);
        if c != std::cmp::Ordering::Equal {
            return c;
        }
    }
    a.len().cmp(&b.len())
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::cmp::Ordering;

    // ── Unit tests ────────────────────────────────────────────────

    #[test]
    fn adjacency_roundtrip() {
        let cases: &[(u16, u64)] = &[
            (0, 0),
            (0, 1),
            (1, 0),
            (0xFFFF, u64::MAX),
            (0x0001, 0xDEAD_BEEF_CAFE_1234),
            (7, 42),
        ];
        for &(k, n) in cases {
            let encoded = encode_adjacency_key(k, n);
            let (k2, n2) = decode_adjacency_key(&encoded);
            assert_eq!((k, n), (k2, n2), "roundtrip failed for ({k}, {n})");
        }
    }

    #[test]
    fn adjacency_key_is_10_bytes() {
        let key = encode_adjacency_key(3, 99);
        assert_eq!(key.len(), 10);
    }

    #[test]
    fn adjacency_kind_contiguity() {
        let max_node_kind1 = encode_adjacency_key(1, u64::MAX);
        let min_node_kind2 = encode_adjacency_key(2, 0);
        assert!(
            max_node_kind1 < min_node_kind2,
            "max(kind=1) must sort before min(kind=2)"
        );
    }

    #[test]
    fn adjacency_same_kind_ordered_by_node_id() {
        let k1 = encode_adjacency_key(5, 100);
        let k2 = encode_adjacency_key(5, 200);
        assert!(k1 < k2, "node_id=100 should sort before node_id=200 for same kind");
    }

    #[test]
    fn path_boundary_nul_fixes_gt_defect() {
        let gt_foo_bar: &[u8]    = b"foo>bar";
        let gt_foo_colonbar: &[u8] = b"foo::bar";
        assert!(
            gt_foo_colonbar < gt_foo_bar,
            "DEFECT DEMONSTRATED: with '>' separator, 'foo::bar' < 'foo>bar' (wrong!)"
        );
        let nul_foo_child_bar = encode_path_key(&["foo", "bar"]);
        let nul_foo_only = encode_path_key(&["foo"]);
        let nul_foobar = b"foobar".to_vec();
        let nul_foo_colonbar = b"foo::bar".to_vec();
        assert!(nul_foo_only < nul_foo_child_bar);
        assert!(nul_foo_child_bar < nul_foo_colonbar);
        assert!(nul_foo_child_bar < nul_foobar);
        assert!(nul_foo_colonbar < nul_foobar);
    }

    #[test]
    fn path_boundary_exact_cases() {
        let foo       = encode_path_key(&["foo"]);
        let foo_bar   = encode_path_key(&["foo", "bar"]);
        let foobar    = b"foobar".to_vec();
        let foo_colon = b"foo::bar".to_vec();
        assert!(foo < foo_bar, "parent < child");
        assert!(foo_bar < foo_colon, "NUL-sep child < colon-sibling");
        assert!(foo_colon < foobar, "colon-sibling < foobar-string");
        assert!(foo < foobar, "transitivity: foo < foobar");
        assert!(foo < foo_colon, "transitivity: foo < foo::bar");
    }

    #[test]
    fn path_roundtrip() {
        let cases: &[&[&str]] = &[
            &["foo"], &["foo", "bar"], &["foo", "bar", "baz"],
            &["src/auth.rs", "AuthService", "login"],
            &["lib.rs", "Vec<T>::push"],
        ];
        for &segs in cases {
            let decoded = decode_path_key(&encode_path_key(segs));
            let refs: Vec<&str> = decoded.iter().map(|s| s.as_str()).collect();
            assert_eq!(segs, refs.as_slice());
        }
    }

    #[test]
    fn path_single_segment_no_separator() {
        let encoded = encode_path_key(&["hello"]);
        assert_eq!(encoded, b"hello");
        assert!(!encoded.contains(&0x00));
    }

    #[test]
    fn path_two_segments_has_one_separator() {
        let encoded = encode_path_key(&["a", "b"]);
        assert_eq!(encoded, b"a\0b");
        assert_eq!(encoded.iter().filter(|&&b| b == 0x00).count(), 1);
    }

    proptest! {
        #[test]
        fn prop_adjacency_order_preservation(
            k1 in 0u16..=u16::MAX, n1 in 0u64..=u64::MAX,
            k2 in 0u16..=u16::MAX, n2 in 0u64..=u64::MAX,
        ) {
            let enc1 = encode_adjacency_key(k1, n1);
            let enc2 = encode_adjacency_key(k2, n2);
            prop_assert_eq!(enc1.cmp(&enc2), logical_cmp_adjacency((k1, n1), (k2, n2)),
                "order mismatch for ({}, {}) vs ({}, {})", k1, n1, k2, n2);
        }

        #[test]
        fn prop_adjacency_roundtrip(k in 0u16..=u16::MAX, n in 0u64..=u64::MAX) {
            let (k2, n2) = decode_adjacency_key(&encode_adjacency_key(k, n));
            prop_assert_eq!(k, k2);
            prop_assert_eq!(n, n2);
        }

        #[test]
        fn prop_path_order_preservation(
            a_segs in prop::collection::vec(
                prop::string::string_regex("[a-zA-Z0-9_/.:,<>() ]{1,20}").unwrap(), 1..=4),
            b_segs in prop::collection::vec(
                prop::string::string_regex("[a-zA-Z0-9_/.:,<>() ]{1,20}").unwrap(), 1..=4),
        ) {
            let a: Vec<&str> = a_segs.iter().map(|s| s.as_str()).collect();
            let b: Vec<&str> = b_segs.iter().map(|s| s.as_str()).collect();
            prop_assert_eq!(encode_path_key(&a).cmp(&encode_path_key(&b)),
                logical_cmp_path(&a, &b),
                "order mismatch: {:?} vs {:?}", a_segs, b_segs);
        }

        #[test]
        fn prop_path_roundtrip(
            segs in prop::collection::vec(
                prop::string::string_regex("[a-zA-Z0-9_/.:,<>() ]{1,20}").unwrap(), 1..=4),
        ) {
            let refs: Vec<&str> = segs.iter().map(|s| s.as_str()).collect();
            let decoded = decode_path_key(&encode_path_key(&refs));
            prop_assert_eq!(segs, decoded, "roundtrip failed");
        }

        #[test]
        fn prop_path_separator_count(
            segs in prop::collection::vec(
                prop::string::string_regex("[a-zA-Z0-9_/.:,<>() ]{1,20}").unwrap(), 1..=5),
        ) {
            let refs: Vec<&str> = segs.iter().map(|s| s.as_str()).collect();
            let encoded = encode_path_key(&refs);
            let nul_count = encoded.iter().filter(|&&b| b == 0x00).count();
            prop_assert_eq!(nul_count, segs.len() - 1,
                "expected {} separators for {} segments", segs.len() - 1, segs.len());
        }

        #[test]
        fn prop_nul_beats_every_printable_byte(b in 0x20u8..=0xFFu8) {
            prop_assert!(PATH_KEY_SEPARATOR < b, "NUL should be < {b:#04x}");
        }

        #[test]
        fn prop_parent_before_child(
            base in prop::collection::vec(
                prop::string::string_regex("[a-zA-Z0-9_/.:,<>() ]{1,20}").unwrap(), 1..=3),
            extra in prop::collection::vec(
                prop::string::string_regex("[a-zA-Z0-9_/.:,<>() ]{1,20}").unwrap(), 1..=2),
        ) {
            let mut child = base.clone();
            child.extend(extra);
            let base_refs: Vec<&str> = base.iter().map(|s| s.as_str()).collect();
            let child_refs: Vec<&str> = child.iter().map(|s| s.as_str()).collect();
            prop_assert!(
                encode_path_key(&base_refs) < encode_path_key(&child_refs),
                "parent {:?} should sort before child {:?}", base, child
            );
        }
    }

    #[test]
    fn adjacency_equal_inputs_equal_output() {
        let k = encode_adjacency_key(7, 42);
        assert_eq!(k.cmp(&k), Ordering::Equal);
    }

    #[test]
    fn path_equal_inputs_equal_output() {
        let a = encode_path_key(&["foo", "bar"]);
        let b = encode_path_key(&["foo", "bar"]);
        assert_eq!(a.cmp(&b), Ordering::Equal);
    }

    #[test]
    fn single_segment_before_child() {
        let parent = encode_path_key(&["abc"]);
        let child1 = encode_path_key(&["abc", "x"]);
        let child2 = encode_path_key(&["abc", "zzz"]);
        assert!(parent < child1);
        assert!(parent < child2);
        assert!(child1 < child2);
    }

    #[test]
    fn deep_nesting_ordered() {
        let paths: Vec<Vec<&str>> = vec![
            vec!["a"], vec!["a", "b"], vec!["a", "b", "c"], vec!["a", "b", "c", "d"],
        ];
        for i in 0..paths.len() - 1 {
            let ea = encode_path_key(&paths[i]);
            let eb = encode_path_key(&paths[i + 1]);
            assert!(ea < eb);
        }
    }
}
