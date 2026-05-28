//! Foundational types: `NodeId`, `NodeKind`, `EdgeKind`, `Language`, `SourceSpan`.
//!
//! These are the "atoms" the rest of the engine composes. Designed to be
//! `Copy`, small, and `Hash` so they slot cleanly into adjacency lists,
//! `HashMaps`, and the eventual CSR layout.

use core::fmt;

/// A globally unique identifier for a node in the graph.
///
/// `NodeId` is derived from a [`crate::trunk::TrunkPath`] by truncating the
/// first 8 bytes of its BLAKE3 hash. The 64-bit space is large enough for
/// practical codebases (collision probability < 10⁻⁶ at 10⁸ paths). When
/// collisions arise, the resolver tags both candidates and surfaces an
/// `Ambiguous` error to the caller — never silent.
///
/// `NodeId(0)` is reserved as the **null node**, useful as a sentinel.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct NodeId(pub u64);

impl NodeId {
    /// The reserved null id. Never assigned to a real node.
    pub const NULL: Self = Self(0);

    /// True if this is the [`NULL`](Self::NULL) sentinel.
    #[must_use]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    /// Returns the raw u64 representation.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Hex makes IDs comparable at a glance during debugging.
        write!(f, "n#{:016x}", self.0)
    }
}

/// The kind of code element a node represents.
///
/// Mirrors [`codegraph`'s NODE_KINDS](https://github.com/colbymchenry/codegraph/blob/main/src/types.ts)
/// for prior-art compatibility, plus a few additions specific to UML/AI-agent
/// use cases.
///
/// `as_str` and `try_from_wire` provide a stable wire/serialization form.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum NodeKind {
    /// A file in the source tree.
    File,
    /// A module or namespace.
    Module,
    /// A class declaration.
    Class,
    /// A struct (or record) declaration.
    Struct,
    /// An interface (Java/TS) or trait (Rust) or protocol (Swift) declaration.
    Interface,
    /// A free-standing function.
    Function,
    /// A method (function attached to a class/struct/trait).
    Method,
    /// A property (getter/setter or declared attribute).
    Property,
    /// A struct/class field.
    Field,
    /// A mutable binding.
    Variable,
    /// An immutable binding.
    Constant,
    /// An enum type.
    Enum,
    /// A single variant within an enum.
    EnumMember,
    /// A `type X = ...` alias.
    TypeAlias,
    /// A function/method parameter.
    Parameter,
    /// An `import` statement.
    Import,
    /// An `export` statement.
    Export,
    /// A web framework route declaration (extracted by pack framework hooks).
    Route,
    /// A UI component declaration.
    Component,
}

impl NodeKind {
    /// Stable string form for serialization and the wire protocol.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Module => "module",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Interface => "interface",
            Self::Function => "function",
            Self::Method => "method",
            Self::Property => "property",
            Self::Field => "field",
            Self::Variable => "variable",
            Self::Constant => "constant",
            Self::Enum => "enum",
            Self::EnumMember => "enum_member",
            Self::TypeAlias => "type_alias",
            Self::Parameter => "parameter",
            Self::Import => "import",
            Self::Export => "export",
            Self::Route => "route",
            Self::Component => "component",
        }
    }

    /// Inverse of [`Self::as_str`]. Returns `None` for unknown wire strings.
    #[must_use]
    pub fn try_from_wire(s: &str) -> Option<Self> {
        Some(match s {
            "file" => Self::File,
            "module" => Self::Module,
            "class" => Self::Class,
            "struct" => Self::Struct,
            "interface" => Self::Interface,
            "function" => Self::Function,
            "method" => Self::Method,
            "property" => Self::Property,
            "field" => Self::Field,
            "variable" => Self::Variable,
            "constant" => Self::Constant,
            "enum" => Self::Enum,
            "enum_member" => Self::EnumMember,
            "type_alias" => Self::TypeAlias,
            "parameter" => Self::Parameter,
            "import" => Self::Import,
            "export" => Self::Export,
            "route" => Self::Route,
            "component" => Self::Component,
            _ => return None,
        })
    }
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The kind of relationship represented by a [`Synapse`](crate::synapse) edge.
///
/// Designed to cover the full UML relationship vocabulary plus the common
/// code-intelligence ones (calls, imports). Each kind lives in its own
/// CSR-encoded adjacency list, with forward and reverse adjacency stored
/// separately so `callers` queries are O(degree) lookups.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum EdgeKind {
    /// Parent contains child (file→class, class→method).
    Contains,
    /// One function/method calls another.
    Calls,
    /// File imports from another file/module.
    Imports,
    /// File exports a symbol.
    Exports,
    /// Class/interface inheritance.
    Extends,
    /// Class implements an interface (or realizes one, in UML).
    Implements,
    /// Generic reference to another symbol.
    References,
    /// Variable/parameter has a type.
    TypeOf,
    /// Function returns a type.
    Returns,
    /// Creates an instance of a class.
    Instantiates,
    /// Method overrides a parent method.
    Overrides,
    /// Decorator/annotation applied to symbol.
    Decorates,
    /// UML: weak ownership (whole/part, lifecycles independent).
    Aggregates,
    /// UML: strong ownership (whole/part, lifecycles coupled).
    Composes,
    /// UML: generic usage dependency.
    Uses,
}

impl EdgeKind {
    /// Stable string form for serialization.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::Calls => "calls",
            Self::Imports => "imports",
            Self::Exports => "exports",
            Self::Extends => "extends",
            Self::Implements => "implements",
            Self::References => "references",
            Self::TypeOf => "type_of",
            Self::Returns => "returns",
            Self::Instantiates => "instantiates",
            Self::Overrides => "overrides",
            Self::Decorates => "decorates",
            Self::Aggregates => "aggregates",
            Self::Composes => "composes",
            Self::Uses => "uses",
        }
    }
}

impl fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The programming language a node was extracted from.
///
/// Determined per-file by the language-pack engine, persisted on every node
/// for fast filtering ("all `async` Python methods").
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum Language {
    /// Indicates the language could not be detected, or the source is a non-source file.
    Unknown,
    /// Python source file.
    Python,
    /// TypeScript source file.
    TypeScript,
    /// JavaScript source file.
    JavaScript,
    /// Go source file.
    Go,
    /// Rust source file.
    Rust,
    /// Java source file.
    Java,
    /// C source file.
    C,
    /// C++ source file.
    Cpp,
    /// C# source file.
    CSharp,
    /// PHP source file.
    Php,
    /// Ruby source file.
    Ruby,
    /// Swift source file.
    Swift,
    /// Kotlin source file.
    Kotlin,
    /// Dart source file.
    Dart,
    /// Lua source file.
    Lua,
    /// Bash/shell script.
    Bash,
    /// SQL source file.
    Sql,
    /// YAML configuration file.
    Yaml,
    /// TOML configuration file.
    Toml,
    /// JSON data file.
    Json,
}

impl Language {
    /// Stable string form for serialization.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Python => "python",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Go => "go",
            Self::Rust => "rust",
            Self::Java => "java",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Php => "php",
            Self::Ruby => "ruby",
            Self::Swift => "swift",
            Self::Kotlin => "kotlin",
            Self::Dart => "dart",
            Self::Lua => "lua",
            Self::Bash => "bash",
            Self::Sql => "sql",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Json => "json",
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A half-open source range: `[start, end)`.
///
/// Lines are 1-indexed (matching editor convention); columns are 0-indexed
/// (matching tree-sitter and LSP). The byte offset is included for
/// O(1) slice extraction.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct SourceSpan {
    /// 1-indexed starting line.
    pub start_line: u32,
    /// 0-indexed starting column.
    pub start_col: u32,
    /// 1-indexed ending line (inclusive of the line, exclusive of past-end column).
    pub end_line: u32,
    /// 0-indexed ending column (exclusive).
    pub end_col: u32,
    /// Byte offset of `start_line`/`start_col` in the source file.
    pub start_byte: u32,
    /// Byte offset of `end_line`/`end_col`.
    pub end_byte: u32,
}

impl SourceSpan {
    /// True if this span has zero length.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start_byte == self.end_byte
    }

    /// Byte length of the span.
    #[must_use]
    pub const fn len_bytes(&self) -> u32 {
        self.end_byte.saturating_sub(self.start_byte)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_null_is_null() {
        assert!(NodeId::NULL.is_null());
        assert!(!NodeId(1).is_null());
    }

    #[test]
    fn node_id_display_is_hex() {
        assert_eq!(NodeId(0x1234_abcd).to_string(), "n#000000001234abcd");
    }

    #[test]
    fn node_kind_roundtrips_through_str() {
        for kind in [
            NodeKind::File,
            NodeKind::Class,
            NodeKind::Method,
            NodeKind::EnumMember,
            NodeKind::TypeAlias,
            NodeKind::Component,
        ] {
            assert_eq!(NodeKind::try_from_wire(kind.as_str()), Some(kind));
        }
    }

    #[test]
    fn node_kind_from_str_rejects_garbage() {
        assert_eq!(NodeKind::try_from_wire(""), None);
        assert_eq!(NodeKind::try_from_wire("not-a-kind"), None);
    }

    #[test]
    fn source_span_emptiness() {
        let empty = SourceSpan::default();
        assert!(empty.is_empty());
        assert_eq!(empty.len_bytes(), 0);

        let nonempty = SourceSpan {
            start_byte: 10,
            end_byte: 25,
            ..SourceSpan::default()
        };
        assert!(!nonempty.is_empty());
        assert_eq!(nonempty.len_bytes(), 15);
    }
}
