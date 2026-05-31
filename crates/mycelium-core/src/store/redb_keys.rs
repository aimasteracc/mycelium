//! Key codec functions for the redb storage backend.
//!
//! ## Wire format (APPEND-ONLY — never change without a schema migration)
//!
//! | Key space | Encoding |
//! |---|---|
//! | Node path → id | NUL (0x00) separated path segments; `>` prefix stripped per segment |
//! | Node id → path | u64 big-endian (8 bytes) |
//! | Adjacency (kind, src) | u16 BE kind-tag ++ u64 BE node-id = 10 bytes |
//! | Kind/span map | u64 BE node-id (8 bytes) |
//!
//! NUL separator was chosen over `>` (0x3E) because NUL < any printable ASCII,
//! ensuring lexicographic order of encoded keys matches prefix-scan semantics
//! for "all symbols under file F". See ADR-0007 §Key encoding.
//!
//! RFC-0100 / P1-T05.

/// Byte used to separate path segments in the `trunk_by_path` key.
pub const PATH_SEP: u8 = 0x00;

/// Encode a node path as a redb key.
///
/// Splits on `>` and joins with NUL (`0x00`) bytes so that lexicographic
/// ordering of byte-keys matches prefix-scan semantics (all children of a
/// file appear in contiguous key range).
#[must_use]
pub fn encode_path_key(path: &str) -> Vec<u8> {
    let segments: Vec<&str> = path.split('>').collect();
    let len: usize =
        segments.iter().map(|s| s.len()).sum::<usize>() + segments.len().saturating_sub(1);
    let mut out = Vec::with_capacity(len);
    for (i, seg) in segments.iter().enumerate() {
        if i > 0 {
            out.push(PATH_SEP);
        }
        out.extend_from_slice(seg.as_bytes());
    }
    out
}

/// Decode a key produced by [`encode_path_key`] back to a path string.
#[must_use]
pub fn decode_path_key(key: &[u8]) -> String {
    let parts: Vec<&[u8]> = key.split(|&b| b == PATH_SEP).collect();
    parts
        .iter()
        .enumerate()
        .map(|(i, seg)| {
            let s = std::str::from_utf8(seg).unwrap_or("");
            if i == 0 {
                s.to_owned()
            } else {
                format!(">{s}")
            }
        })
        .collect()
}

/// Encode a `u64` node-id as an 8-byte big-endian key.
#[inline]
#[must_use]
pub const fn encode_id_key(id: u64) -> [u8; 8] {
    id.to_be_bytes()
}

/// Decode an 8-byte big-endian key to a `u64` node-id.
#[inline]
#[must_use]
#[allow(clippy::missing_const_for_fn)]
pub fn decode_id_key(bytes: &[u8]) -> u64 {
    let arr: [u8; 8] = bytes.try_into().unwrap_or([0u8; 8]);
    u64::from_be_bytes(arr)
}

/// Encode an adjacency-list key: `u16 BE kind-tag ++ u64 BE node-id` = 10 bytes.
///
/// Storing the kind tag first groups all edges of the same kind together, so a
/// prefix scan for `kind_tag ++ *` enumerates every edge of that kind.
#[inline]
#[must_use]
pub fn encode_adj_key(kind_tag: u16, node_id: u64) -> [u8; 10] {
    let mut out = [0u8; 10];
    out[..2].copy_from_slice(&kind_tag.to_be_bytes());
    out[2..].copy_from_slice(&node_id.to_be_bytes());
    out
}

/// Decode an adjacency-list key produced by [`encode_adj_key`].
#[inline]
#[must_use]
pub fn decode_adj_key(bytes: &[u8]) -> (u16, u64) {
    if bytes.len() < 10 {
        return (0, 0);
    }
    let kind = u16::from_be_bytes([bytes[0], bytes[1]]);
    let id = u64::from_be_bytes(bytes[2..10].try_into().unwrap_or([0u8; 8]));
    (kind, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_key_roundtrip() {
        for path in &[
            "src/lib.rs>main",
            "a>b>c",
            "simple",
            "src/x.rs>Class>method",
        ] {
            let encoded = encode_path_key(path);
            let decoded = decode_path_key(&encoded);
            assert_eq!(decoded, *path, "roundtrip failed for {path}");
        }
    }

    #[test]
    fn path_key_nul_prefix_ordering() {
        // "a>b" < "aZ>c" under NUL-encoding because NUL(0x00) < 'Z'(0x5A)
        let ab = encode_path_key("a>b");
        let az_c = encode_path_key("aZ>c");
        assert!(ab < az_c, "NUL-encoded a>b must sort before aZ>c");
    }

    #[test]
    fn id_key_roundtrip() {
        for id in &[0u64, 1, u64::MAX, 0xDEAD_BEEF_CAFE_1234] {
            let enc = encode_id_key(*id);
            assert_eq!(decode_id_key(&enc), *id);
        }
    }

    #[test]
    fn adj_key_roundtrip() {
        let cases = [(0u16, 0u64), (1, 42), (0xFFFF, u64::MAX)];
        for (kind, id) in cases {
            let enc = encode_adj_key(kind, id);
            assert_eq!(decode_adj_key(&enc), (kind, id));
        }
    }

    #[test]
    fn adj_key_groups_by_kind() {
        // All keys with the same kind tag sort contiguously.
        let k0_a = encode_adj_key(0, 100);
        let k0_b = encode_adj_key(0, 200);
        let k1_a = encode_adj_key(1, 50);
        assert!(k0_a < k0_b);
        assert!(k0_b < k1_a);
    }
}
