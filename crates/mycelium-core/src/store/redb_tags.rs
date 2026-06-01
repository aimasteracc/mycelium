//! Stable wire-format tags for `EdgeKind` used as redb key prefixes.
//!
//! ## APPEND-ONLY — DO NOT REORDER OR REASSIGN
//!
//! These constants are written into redb tables. Changing them would make
//! existing databases unreadable. To add a new `EdgeKind` variant, append
//! a new constant at the bottom and add it to `edge_kind_tag` / `tag_to_edge_kind`.
//!
//! RFC-0100 / P1-T03 (`EdgeKind` wire-format table).

use crate::types::EdgeKind;

/// Wire tag for `EdgeKind::Contains`.
pub const TAG_CONTAINS: u16 = 0;
/// Wire tag for `EdgeKind::Calls`.
pub const TAG_CALLS: u16 = 1;
/// Wire tag for `EdgeKind::Imports`.
pub const TAG_IMPORTS: u16 = 2;
/// Wire tag for `EdgeKind::TypeImports`.
pub const TAG_TYPE_IMPORTS: u16 = 3;
/// Wire tag for `EdgeKind::Exports`.
pub const TAG_EXPORTS: u16 = 4;
/// Wire tag for `EdgeKind::Extends`.
pub const TAG_EXTENDS: u16 = 5;
/// Wire tag for `EdgeKind::Implements`.
pub const TAG_IMPLEMENTS: u16 = 6;
/// Wire tag for `EdgeKind::References`.
pub const TAG_REFERENCES: u16 = 7;
/// Wire tag for `EdgeKind::TypeOf`.
pub const TAG_TYPE_OF: u16 = 8;
/// Wire tag for `EdgeKind::Returns`.
pub const TAG_RETURNS: u16 = 9;
/// Wire tag for `EdgeKind::Instantiates`.
pub const TAG_INSTANTIATES: u16 = 10;
/// Wire tag for `EdgeKind::Overrides`.
pub const TAG_OVERRIDES: u16 = 11;
/// Wire tag for `EdgeKind::Decorates`.
pub const TAG_DECORATES: u16 = 12;
/// Wire tag for `EdgeKind::Aggregates`.
pub const TAG_AGGREGATES: u16 = 13;
/// Wire tag for `EdgeKind::Composes`.
pub const TAG_COMPOSES: u16 = 14;
/// Wire tag for `EdgeKind::Uses`.
pub const TAG_USES: u16 = 15;

/// Map an `EdgeKind` to its stable `u16` wire tag.
///
/// # Panics
///
/// Panics if the variant has no assigned tag (e.g. a new `#[non_exhaustive]`
/// variant was added without updating this table). Add a new constant above
/// and a new arm in the `match` before shipping a new variant.
#[must_use]
pub fn edge_kind_tag(kind: EdgeKind) -> u16 {
    match kind {
        EdgeKind::Contains => TAG_CONTAINS,
        EdgeKind::Calls => TAG_CALLS,
        EdgeKind::Imports => TAG_IMPORTS,
        EdgeKind::TypeImports => TAG_TYPE_IMPORTS,
        EdgeKind::Exports => TAG_EXPORTS,
        EdgeKind::Extends => TAG_EXTENDS,
        EdgeKind::Implements => TAG_IMPLEMENTS,
        EdgeKind::References => TAG_REFERENCES,
        EdgeKind::TypeOf => TAG_TYPE_OF,
        EdgeKind::Returns => TAG_RETURNS,
        EdgeKind::Instantiates => TAG_INSTANTIATES,
        EdgeKind::Overrides => TAG_OVERRIDES,
        EdgeKind::Decorates => TAG_DECORATES,
        EdgeKind::Aggregates => TAG_AGGREGATES,
        EdgeKind::Composes => TAG_COMPOSES,
        EdgeKind::Uses => TAG_USES,
        // EdgeKind is #[non_exhaustive]; new variants must get a tag before this
        // match compiles on nightly. The wildcard arm guards against silent silent
        // data corruption in release builds where a variant was added without
        // assigning a tag.
        #[allow(unreachable_patterns)]
        _ => panic!("EdgeKind variant has no stable redb tag — add one to redb_tags.rs"),
    }
}

/// Map a `u16` wire tag back to an `EdgeKind`. Returns `None` for unknown tags.
#[must_use]
pub const fn tag_to_edge_kind(tag: u16) -> Option<EdgeKind> {
    Some(match tag {
        TAG_CONTAINS => EdgeKind::Contains,
        TAG_CALLS => EdgeKind::Calls,
        TAG_IMPORTS => EdgeKind::Imports,
        TAG_TYPE_IMPORTS => EdgeKind::TypeImports,
        TAG_EXPORTS => EdgeKind::Exports,
        TAG_EXTENDS => EdgeKind::Extends,
        TAG_IMPLEMENTS => EdgeKind::Implements,
        TAG_REFERENCES => EdgeKind::References,
        TAG_TYPE_OF => EdgeKind::TypeOf,
        TAG_RETURNS => EdgeKind::Returns,
        TAG_INSTANTIATES => EdgeKind::Instantiates,
        TAG_OVERRIDES => EdgeKind::Overrides,
        TAG_DECORATES => EdgeKind::Decorates,
        TAG_AGGREGATES => EdgeKind::Aggregates,
        TAG_COMPOSES => EdgeKind::Composes,
        TAG_USES => EdgeKind::Uses,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_edge_kinds_have_stable_tags() {
        let all = [
            EdgeKind::Contains,
            EdgeKind::Calls,
            EdgeKind::Imports,
            EdgeKind::TypeImports,
            EdgeKind::Exports,
            EdgeKind::Extends,
            EdgeKind::Implements,
            EdgeKind::References,
            EdgeKind::TypeOf,
            EdgeKind::Returns,
            EdgeKind::Instantiates,
            EdgeKind::Overrides,
            EdgeKind::Decorates,
            EdgeKind::Aggregates,
            EdgeKind::Composes,
            EdgeKind::Uses,
        ];
        for kind in all {
            let tag = edge_kind_tag(kind);
            assert_eq!(
                tag_to_edge_kind(tag),
                Some(kind),
                "roundtrip failed for {kind:?}"
            );
        }
    }

    #[test]
    fn tags_are_unique() {
        let all = [
            EdgeKind::Contains,
            EdgeKind::Calls,
            EdgeKind::Imports,
            EdgeKind::TypeImports,
            EdgeKind::Exports,
            EdgeKind::Extends,
            EdgeKind::Implements,
            EdgeKind::References,
            EdgeKind::TypeOf,
            EdgeKind::Returns,
            EdgeKind::Instantiates,
            EdgeKind::Overrides,
            EdgeKind::Decorates,
            EdgeKind::Aggregates,
            EdgeKind::Composes,
            EdgeKind::Uses,
        ];
        let mut seen = std::collections::HashSet::new();
        for kind in all {
            assert!(
                seen.insert(edge_kind_tag(kind)),
                "duplicate tag for {kind:?}"
            );
        }
    }
}
