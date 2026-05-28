//! `TrunkPath` — the materialized-path encoding used by [`super::Trunk`].
//!
//! ## Encoding
//!
//! - Segments are separated by [`SEPARATOR`] (`>`).
//! - Segments must be non-empty.
//! - The forbidden characters in a segment are `>` (the separator) and any
//!   control character `< 0x20`. Everything else, including parentheses,
//!   commas, spaces, unicode, is allowed — this matters because function
//!   signatures and generics appear in qualified names.
//!
//! Examples of valid paths:
//!
//! - `src/auth.rs`
//! - `src/auth.rs>AuthService`
//! - `src/auth.rs>AuthService>login(email, password)`
//! - `src/auth.rs>AuthService>login(email, password)>validate`
//! - `lib.rs>Vec<T>::push`
//!
//! ## API
//!
//! Construct with [`TrunkPath::parse`], [`TrunkPath::from_segments`], or
//! join existing paths with [`TrunkPath::join`]. Inspect with iterators
//! over segments, or extract the parent.

use core::fmt;

use smol_str::SmolStr;

use crate::error::{Error, Result};

/// The materialized-path segment separator.
pub(super) const SEPARATOR: char = '>';

/// A validated containment path.
///
/// Cheap to clone (inner `String`). Always non-empty, always free of empty
/// segments, always free of forbidden characters.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TrunkPath(SmolStr);

impl TrunkPath {
    /// Parse a path string.
    ///
    /// ```
    /// use mycelium_core::trunk::TrunkPath;
    /// let p = TrunkPath::parse("src/lib.rs>Foo>bar").unwrap();
    /// assert_eq!(p.as_str(), "src/lib.rs>Foo>bar");
    ///
    /// assert!(TrunkPath::parse("").is_err());
    /// assert!(TrunkPath::parse("a>>b").is_err());
    /// assert!(TrunkPath::parse(">leading-sep").is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidPath`] if the string is empty, contains an
    /// empty segment, or contains a forbidden character (separator or `< 0x20`).
    pub fn parse(s: &str) -> Result<Self> {
        if s.is_empty() {
            return Err(Error::InvalidPath {
                reason: "path is empty".to_owned(),
            });
        }

        // Reject leading or trailing separator, and any empty segment.
        if s.starts_with(SEPARATOR) {
            return Err(Error::InvalidPath {
                reason: "path starts with separator".to_owned(),
            });
        }
        if s.ends_with(SEPARATOR) {
            return Err(Error::InvalidPath {
                reason: "path ends with separator".to_owned(),
            });
        }

        for (i, seg) in s.split(SEPARATOR).enumerate() {
            if seg.is_empty() {
                return Err(Error::InvalidPath {
                    reason: format!("empty segment at position {i}"),
                });
            }
            for ch in seg.chars() {
                if (ch as u32) < 0x20 {
                    return Err(Error::InvalidPath {
                        reason: format!(
                            "segment at position {i} contains control character U+{:04X}",
                            ch as u32
                        ),
                    });
                }
            }
        }

        Ok(Self(SmolStr::new(s)))
    }

    /// Construct from a slice of segments. Each segment is validated the
    /// same way [`parse`](Self::parse) validates a parsed segment.
    ///
    /// ```
    /// use mycelium_core::trunk::TrunkPath;
    /// let p = TrunkPath::from_segments(&["src/lib.rs", "Foo", "bar"]).unwrap();
    /// assert_eq!(p.as_str(), "src/lib.rs>Foo>bar");
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidPath`] if `segments` is empty, any segment is
    /// empty, any segment contains the separator `>`, or any segment contains
    /// a control character `< 0x20`.
    pub fn from_segments<S: AsRef<str>>(segments: &[S]) -> Result<Self> {
        if segments.is_empty() {
            return Err(Error::InvalidPath {
                reason: "no segments provided".to_owned(),
            });
        }
        let mut out = String::new();
        for (i, seg) in segments.iter().enumerate() {
            let seg = seg.as_ref();
            if seg.is_empty() {
                return Err(Error::InvalidPath {
                    reason: format!("segment at position {i} is empty"),
                });
            }
            if seg.contains(SEPARATOR) {
                return Err(Error::InvalidPath {
                    reason: format!("segment at position {i} contains separator '>'"),
                });
            }
            for ch in seg.chars() {
                if (ch as u32) < 0x20 {
                    return Err(Error::InvalidPath {
                        reason: format!(
                            "segment at position {i} contains control character U+{:04X}",
                            ch as u32
                        ),
                    });
                }
            }
            if i > 0 {
                out.push(SEPARATOR);
            }
            out.push_str(seg);
        }
        Ok(Self(SmolStr::new(out)))
    }

    /// Append `child` segment to this path, returning a new path.
    ///
    /// ```
    /// use mycelium_core::trunk::TrunkPath;
    /// let p = TrunkPath::parse("src/lib.rs>Foo").unwrap();
    /// let q = p.join("bar").unwrap();
    /// assert_eq!(q.as_str(), "src/lib.rs>Foo>bar");
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidPath`] if `child` is empty, contains the
    /// separator `>`, or contains a control character `< 0x20`.
    pub fn join(&self, child: &str) -> Result<Self> {
        if child.is_empty() {
            return Err(Error::InvalidPath {
                reason: "child segment is empty".to_owned(),
            });
        }
        if child.contains(SEPARATOR) {
            return Err(Error::InvalidPath {
                reason: "child segment contains separator '>'".to_owned(),
            });
        }
        for ch in child.chars() {
            if (ch as u32) < 0x20 {
                return Err(Error::InvalidPath {
                    reason: format!(
                        "child segment contains control character U+{:04X}",
                        ch as u32
                    ),
                });
            }
        }
        let mut out = String::with_capacity(self.0.len() + 1 + child.len());
        out.push_str(self.0.as_str());
        out.push(SEPARATOR);
        out.push_str(child);
        Ok(Self(SmolStr::new(out)))
    }

    /// Borrow as `&str`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Consume into the owned `String`.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0.into()
    }

    /// Iterate segments in root-to-leaf order.
    pub fn segments(&self) -> impl Iterator<Item = &str> + '_ {
        self.0.split(SEPARATOR)
    }

    /// Number of segments. Always ≥ 1 for a valid path.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.0.matches(SEPARATOR).count() + 1
    }

    /// Return the immediate parent path, if any.
    ///
    /// ```
    /// use mycelium_core::trunk::TrunkPath;
    /// let p = TrunkPath::parse("a>b>c").unwrap();
    /// assert_eq!(p.parent().unwrap().as_str(), "a>b");
    ///
    /// let root = TrunkPath::parse("solo").unwrap();
    /// assert!(root.parent().is_none());
    /// ```
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        parent(self.0.as_str()).map(|p| Self(SmolStr::new(p)))
    }
}

impl fmt::Display for TrunkPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl AsRef<str> for TrunkPath {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

/// Return the parent string of a path, if any.
///
/// Module-internal helper; not part of the public API.
pub(super) fn parent(path: &str) -> Option<&str> {
    path.rfind(SEPARATOR).map(|idx| &path[..idx])
}
