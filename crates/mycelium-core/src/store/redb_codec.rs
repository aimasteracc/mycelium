//! Pure value codecs for the redb backend (RFC-0100 / ADR-0008).
//!
//! Extracted from `redb_backend.rs` to keep that file focused on the storage
//! engine. These are dependency-free byte ↔ value conversions: span packing,
//! the stable `NodeKind` wire tags, and adjacency-id packing. Nothing here
//! touches a redb table or a transaction.

use crate::types::{NodeKind, SourceSpan};

// ── span ↔ 24-byte little-endian ─────────────────────────────────────────────

pub(super) fn encode_span(span: SourceSpan) -> [u8; 24] {
    let mut out = [0u8; 24];
    out[0..4].copy_from_slice(&span.start_line.to_le_bytes());
    out[4..8].copy_from_slice(&span.start_col.to_le_bytes());
    out[8..12].copy_from_slice(&span.end_line.to_le_bytes());
    out[12..16].copy_from_slice(&span.end_col.to_le_bytes());
    out[16..20].copy_from_slice(&span.start_byte.to_le_bytes());
    out[20..24].copy_from_slice(&span.end_byte.to_le_bytes());
    out
}

pub(super) fn decode_span(bytes: &[u8]) -> SourceSpan {
    if bytes.len() < 24 {
        return SourceSpan::default();
    }
    SourceSpan {
        start_line: u32::from_le_bytes(bytes[0..4].try_into().unwrap_or([0; 4])),
        start_col: u32::from_le_bytes(bytes[4..8].try_into().unwrap_or([0; 4])),
        end_line: u32::from_le_bytes(bytes[8..12].try_into().unwrap_or([0; 4])),
        end_col: u32::from_le_bytes(bytes[12..16].try_into().unwrap_or([0; 4])),
        start_byte: u32::from_le_bytes(bytes[16..20].try_into().unwrap_or([0; 4])),
        end_byte: u32::from_le_bytes(bytes[20..24].try_into().unwrap_or([0; 4])),
    }
}

// ── NodeKind ↔ stable u8 wire tag ─────────────────────────────────────────────

#[must_use]
pub(super) const fn node_kind_tag(kind: NodeKind) -> u8 {
    match kind {
        NodeKind::File => 0,
        NodeKind::Module => 1,
        NodeKind::Class => 2,
        NodeKind::Struct => 3,
        NodeKind::Interface => 4,
        NodeKind::Function => 5,
        NodeKind::Method => 6,
        NodeKind::Property => 7,
        NodeKind::Field => 8,
        NodeKind::Variable => 9,
        NodeKind::Constant => 10,
        NodeKind::Enum => 11,
        NodeKind::EnumMember => 12,
        NodeKind::TypeAlias => 13,
        NodeKind::Parameter => 14,
        NodeKind::Import => 15,
        NodeKind::Export => 16,
        NodeKind::Route => 17,
        NodeKind::Component => 18,
        // NodeKind is #[non_exhaustive]; a new variant must get a stable tag
        // here before it can be persisted. Fail loud (like `edge_kind_tag` in
        // redb_tags.rs) instead of writing a 255 sentinel that `tag_to_node_kind`
        // would silently drop on read — that path is silent data corruption.
        #[allow(unreachable_patterns)]
        _ => panic!("NodeKind variant has no stable redb tag — add one to redb_codec.rs"),
    }
}

#[must_use]
pub(super) const fn tag_to_node_kind(tag: u8) -> Option<NodeKind> {
    Some(match tag {
        0 => NodeKind::File,
        1 => NodeKind::Module,
        2 => NodeKind::Class,
        3 => NodeKind::Struct,
        4 => NodeKind::Interface,
        5 => NodeKind::Function,
        6 => NodeKind::Method,
        7 => NodeKind::Property,
        8 => NodeKind::Field,
        9 => NodeKind::Variable,
        10 => NodeKind::Constant,
        11 => NodeKind::Enum,
        12 => NodeKind::EnumMember,
        13 => NodeKind::TypeAlias,
        14 => NodeKind::Parameter,
        15 => NodeKind::Import,
        16 => NodeKind::Export,
        17 => NodeKind::Route,
        18 => NodeKind::Component,
        _ => return None,
    })
}

// ── adjacency id list ↔ big-endian bytes (sorted, deduped) ────────────────────

pub(super) fn pack_ids(ids: &[u64]) -> Vec<u8> {
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    ids.iter().flat_map(|id| id.to_be_bytes()).collect()
}

pub(super) fn unpack_ids(bytes: &[u8]) -> Vec<u64> {
    bytes
        .chunks_exact(8)
        .map(|c| u64::from_be_bytes(c.try_into().unwrap_or([0; 8])))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{node_kind_tag, tag_to_node_kind};
    use crate::types::NodeKind;

    /// Every persistable `NodeKind` must roundtrip through its stable redb tag.
    /// Guards the fail-loud contract: `node_kind_tag` `panic!`s on an unmapped
    /// variant (instead of writing a 255 sentinel the reader would silently
    /// drop), and a new `#[non_exhaustive]` variant added without a tag would
    /// fail this completeness check first.
    #[test]
    fn all_known_node_kinds_roundtrip() {
        let all = [
            NodeKind::File,
            NodeKind::Module,
            NodeKind::Class,
            NodeKind::Struct,
            NodeKind::Interface,
            NodeKind::Function,
            NodeKind::Method,
            NodeKind::Property,
            NodeKind::Field,
            NodeKind::Variable,
            NodeKind::Constant,
            NodeKind::Enum,
            NodeKind::EnumMember,
            NodeKind::TypeAlias,
            NodeKind::Parameter,
            NodeKind::Import,
            NodeKind::Export,
            NodeKind::Route,
            NodeKind::Component,
        ];
        let mut seen_tags = Vec::new();
        for kind in all {
            let tag = node_kind_tag(kind);
            assert_ne!(tag, 255, "{kind:?} must not map to the sentinel tag");
            assert!(!seen_tags.contains(&tag), "tag {tag} is assigned twice");
            seen_tags.push(tag);
            assert_eq!(
                tag_to_node_kind(tag),
                Some(kind),
                "{kind:?} (tag {tag}) must roundtrip"
            );
        }
    }
}
