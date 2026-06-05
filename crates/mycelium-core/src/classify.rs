//! Static stdlib/builtin/external callee classification (RFC-0113).
//!
//! Tree-sitter is syntactic: it cannot type-resolve a method call like
//! `p.write_text()` or a stdlib call like `os.getcwd()`, so after the
//! project-binding resolver passes ([`Store::resolve_bare_call_stubs`]) those
//! callees remain as unresolved **bare stubs** — the `unknown` tail that
//! inflates dead-code false positives and truncates call graphs.
//!
//! This module is the **final, static classification tier**: given a bare
//! callee name that project resolution already failed to bind, classify it
//! against curated stdlib / builtin / well-known-external allowlists. Pure
//! table lookup, **zero LSP** — exactly the precision lever [ADR-0010] endorses.
//!
//! The allowlist data is ported from the founder's `tree-sitter-analyzer`
//! project (`synapse_resolver/_constants.py`, MIT) which proved this tier lifts
//! Python callee classification 83.9% → 95.9%. This module holds the
//! **language-agnostic cascade + the Python tables**; wiring it into the
//! resolver (so only *remaining* bare stubs reach it — the project-ownership
//! shadow gate) is a separate step.
//!
//! [ADR-0010]: ../../../docs/adr/0010-no-live-lsp-prefer-scip-ingestion.md

use std::collections::HashSet;
use std::sync::LazyLock;

/// Classification of a callee that project resolution did not bind to a
/// definition. The wire strings are stable (surfaced as an additive `class`
/// field on callee output per RFC-0113).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CalleeClass {
    /// Bound to a project-defined symbol (assigned by the resolver, not here).
    Project,
    /// A Python standard-library module or method (e.g. `os`, `write_text`).
    Stdlib,
    /// A Python builtin callable (e.g. `len`, `isinstance`, `ValueError`).
    Builtin,
    /// A well-known third-party method (e.g. `pytest.raises`, mock asserts).
    External,
    /// Nothing matched — a genuinely unresolved callee.
    Unknown,
}

impl CalleeClass {
    /// The stable wire string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Stdlib => "stdlib",
            Self::Builtin => "builtin",
            Self::External => "external",
            Self::Unknown => "unknown",
        }
    }
}

/// Classify a bare Python callee name against the static allowlists.
///
/// Precedence: builtin → stdlib method → external method → stdlib module →
/// unknown. Callers MUST apply the project-ownership shadow first (only pass
/// names project resolution left unresolved), so a project method that happens
/// to share a stdlib name is never misclassified.
#[must_use]
pub fn classify_python(name: &str) -> CalleeClass {
    // Builtins first: a name like `format` is both a builtin and a str method;
    // the builtin reading is the correct one for an unqualified call.
    if PYTHON_BUILTINS.contains(name) {
        CalleeClass::Builtin
    } else if PYTHON_STDLIB_METHODS.contains(name) || PYTHON_STDLIB_MODULES.contains(name) {
        CalleeClass::Stdlib
    } else if PYTHON_EXTERNAL_METHODS.contains(name) {
        CalleeClass::External
    } else {
        CalleeClass::Unknown
    }
}

/// Classify a module-qualified Python call `receiver.method()`.
///
/// If the `receiver` is a known stdlib module (e.g. `json.dumps`, `os.getcwd`,
/// `re.compile`), the call is **stdlib** regardless of the method name —
/// `dumps`/`getcwd` are not in the bare-method tables, so the unqualified
/// [`classify_python`] would miss them. Otherwise fall back to classifying the
/// `method` name alone (`p.write_text()` → stdlib method; `mock.assert_called()`
/// → external).
///
/// Like [`classify_python`], callers must apply the project-ownership shadow
/// first (and, for the stdlib-module case, gate on actual import evidence — only
/// trust `receiver` when the file imported that module).
#[must_use]
pub fn classify_python_qualified(receiver: &str, method: &str) -> CalleeClass {
    if PYTHON_STDLIB_MODULES.contains(receiver) {
        CalleeClass::Stdlib
    } else {
        classify_python(method)
    }
}

/// Curated top-level Python stdlib module names. Ported from
/// `tree-sitter-analyzer` `_FALLBACK_STDLIB` (the portable subset of
/// `sys.stdlib_module_names`).
static PYTHON_STDLIB_MODULES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "abc",
        "argparse",
        "array",
        "ast",
        "asyncio",
        "base64",
        "bisect",
        "builtins",
        "calendar",
        "collections",
        "concurrent",
        "configparser",
        "contextlib",
        "copy",
        "csv",
        "dataclasses",
        "datetime",
        "decimal",
        "difflib",
        "enum",
        "errno",
        "fnmatch",
        "functools",
        "gc",
        "glob",
        "gzip",
        "hashlib",
        "heapq",
        "hmac",
        "html",
        "http",
        "importlib",
        "inspect",
        "io",
        "ipaddress",
        "itertools",
        "json",
        "logging",
        "math",
        "mimetypes",
        "multiprocessing",
        "operator",
        "os",
        "pathlib",
        "pickle",
        "platform",
        "posixpath",
        "pprint",
        "queue",
        "random",
        "re",
        "shelve",
        "shutil",
        "signal",
        "socket",
        "sqlite3",
        "ssl",
        "stat",
        "statistics",
        "string",
        "struct",
        "subprocess",
        "sys",
        "tempfile",
        "textwrap",
        "threading",
        "time",
        "tomllib",
        "traceback",
        "types",
        "typing",
        "unicodedata",
        "unittest",
        "urllib",
        "uuid",
        "warnings",
        "weakref",
        "xml",
        "zipfile",
        "zlib",
    ]
    .into_iter()
    .collect()
});

/// Python builtin callables — never a project symbol. Ported from `BUILTINS_PY`.
static PYTHON_BUILTINS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "abs",
        "all",
        "any",
        "ascii",
        "bin",
        "bool",
        "bytearray",
        "bytes",
        "callable",
        "chr",
        "classmethod",
        "compile",
        "complex",
        "delattr",
        "dict",
        "dir",
        "divmod",
        "enumerate",
        "eval",
        "exec",
        "filter",
        "float",
        "format",
        "frozenset",
        "getattr",
        "globals",
        "hasattr",
        "hash",
        "help",
        "hex",
        "id",
        "input",
        "int",
        "isinstance",
        "issubclass",
        "iter",
        "len",
        "list",
        "locals",
        "map",
        "max",
        "memoryview",
        "min",
        "next",
        "object",
        "oct",
        "open",
        "ord",
        "pow",
        "print",
        "property",
        "range",
        "repr",
        "reversed",
        "round",
        "set",
        "setattr",
        "slice",
        "sorted",
        "staticmethod",
        "str",
        "sum",
        "super",
        "tuple",
        "type",
        "vars",
        "zip",
        "__import__",
        "Exception",
        "ValueError",
        "TypeError",
        "KeyError",
        "IndexError",
        "RuntimeError",
        "StopIteration",
        "AttributeError",
        "NotImplementedError",
        "FileNotFoundError",
        "OSError",
        "IOError",
        "ImportError",
    ]
    .into_iter()
    .collect()
});

/// Well-known stdlib *method* names (str/path/dict/regex/argparse/datetime/io).
/// Ported from `STDLIB_METHODS_PY` (RFC-0004 in tree-sitter-analyzer).
static PYTHON_STDLIB_METHODS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        // str
        "strip",
        "lstrip",
        "rstrip",
        "lower",
        "upper",
        "title",
        "capitalize",
        "casefold",
        "swapcase",
        "split",
        "rsplit",
        "splitlines",
        "join",
        "startswith",
        "endswith",
        "replace",
        "find",
        "rfind",
        "index",
        "rindex",
        "count",
        "format",
        "format_map",
        "encode",
        "decode",
        "zfill",
        "ljust",
        "rjust",
        "center",
        "partition",
        "rpartition",
        "expandtabs",
        "translate",
        "maketrans",
        "removeprefix",
        "removesuffix",
        "isdigit",
        "isalpha",
        "isalnum",
        "isspace",
        "isupper",
        "islower",
        "istitle",
        "isnumeric",
        "isdecimal",
        "isidentifier",
        "isprintable",
        "isascii",
        // pathlib.Path
        "write_text",
        "read_text",
        "write_bytes",
        "read_bytes",
        "mkdir",
        "exists",
        "is_file",
        "is_dir",
        "is_symlink",
        "is_absolute",
        "glob",
        "rglob",
        "resolve",
        "absolute",
        "relative_to",
        "with_suffix",
        "with_name",
        "with_stem",
        "iterdir",
        "unlink",
        "rmdir",
        "touch",
        "rename",
        "samefile",
        "expanduser",
        "as_posix",
        "as_uri",
        "joinpath",
        "stat",
        "lstat",
        "chmod",
        "lchmod",
        "symlink_to",
        "hardlink_to",
        "owner",
        "group",
        "readlink",
        // dict / list / set
        "items",
        "keys",
        "values",
        "get",
        "setdefault",
        "update",
        "pop",
        "popitem",
        "fromkeys",
        "append",
        "extend",
        "insert",
        "remove",
        "sort",
        "reverse",
        "add",
        "discard",
        "union",
        "intersection",
        "difference",
        "symmetric_difference",
        "issubset",
        "issuperset",
        "isdisjoint",
        "copy",
        "clear",
        // re
        "group",
        "groups",
        "groupdict",
        "fullmatch",
        "search",
        "findall",
        "finditer",
        "sub",
        "subn",
        "span",
        "start",
        "end",
        "expand",
        "match",
        // argparse
        "add_argument",
        "add_subparsers",
        "add_parser",
        "parse_args",
        "parse_known_args",
        "set_defaults",
        "get_default",
        "add_argument_group",
        "add_mutually_exclusive_group",
        "print_help",
        "print_usage",
        "error",
        "format_help",
        // datetime
        "isoformat",
        "strftime",
        "strptime",
        "total_seconds",
        "timestamp",
        "astimezone",
        "date",
        "time",
        "weekday",
        "isoweekday",
        "fromtimestamp",
        "fromisoformat",
        "utcnow",
        "now",
        "today",
        // io / contextlib protocol
        "write",
        "writelines",
        "read",
        "readline",
        "readlines",
        "seek",
        "tell",
        "flush",
        "close",
        "fileno",
        "getvalue",
    ]
    .into_iter()
    .collect()
});

/// Well-known third-party (test-framework) method names. Ported from
/// `EXTERNAL_METHODS_PY` (RFC-0005 in tree-sitter-analyzer): pytest, hypothesis,
/// unittest.mock. Conservative — only overwhelmingly-test-framework names.
static PYTHON_EXTERNAL_METHODS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        // pytest
        "raises",
        "skip",
        "skipif",
        "parametrize",
        "fixture",
        "mark",
        "approx",
        "warns",
        "deprecated_call",
        "readouterr",
        "monkeypatch",
        // hypothesis
        "given",
        "integers",
        "sampled_from",
        "characters",
        "text",
        "floats",
        "lists",
        "dictionaries",
        "tuples",
        "booleans",
        "composite",
        "assume",
        "note",
        "target",
        "event",
        "reproduce_failure",
        // unittest.mock
        "assert_called_once_with",
        "assert_called_once",
        "assert_called_with",
        "assert_called",
        "assert_not_called",
        "assert_any_call",
        "assert_has_calls",
        "call_args_list",
        "mock_calls",
        "reset_mock",
        "configure_mock",
        "MagicMock",
        "patch",
        "call",
    ]
    .into_iter()
    .collect()
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_strings_are_stable() {
        assert_eq!(CalleeClass::Project.as_str(), "project");
        assert_eq!(CalleeClass::Stdlib.as_str(), "stdlib");
        assert_eq!(CalleeClass::Builtin.as_str(), "builtin");
        assert_eq!(CalleeClass::External.as_str(), "external");
        assert_eq!(CalleeClass::Unknown.as_str(), "unknown");
    }

    #[test]
    fn classifies_builtins() {
        assert_eq!(classify_python("len"), CalleeClass::Builtin);
        assert_eq!(classify_python("isinstance"), CalleeClass::Builtin);
        assert_eq!(classify_python("ValueError"), CalleeClass::Builtin);
    }

    #[test]
    fn classifies_stdlib_methods() {
        assert_eq!(classify_python("write_text"), CalleeClass::Stdlib);
        assert_eq!(classify_python("strip"), CalleeClass::Stdlib);
        assert_eq!(classify_python("isoformat"), CalleeClass::Stdlib);
    }

    #[test]
    fn classifies_stdlib_modules() {
        assert_eq!(classify_python("os"), CalleeClass::Stdlib);
        assert_eq!(classify_python("pathlib"), CalleeClass::Stdlib);
    }

    #[test]
    fn classifies_external_test_methods() {
        assert_eq!(classify_python("raises"), CalleeClass::External);
        assert_eq!(classify_python("parametrize"), CalleeClass::External);
        assert_eq!(classify_python("assert_called_once"), CalleeClass::External);
    }

    #[test]
    fn unknown_names_stay_unknown() {
        assert_eq!(classify_python("frobnicate"), CalleeClass::Unknown);
        assert_eq!(classify_python("my_project_helper"), CalleeClass::Unknown);
    }

    #[test]
    fn builtin_takes_precedence_over_method_tables() {
        // `format` is both a builtin and a str method; builtin wins (checked first).
        assert_eq!(classify_python("format"), CalleeClass::Builtin);
    }

    #[test]
    fn qualified_stdlib_module_call_is_stdlib() {
        // `dumps`/`getcwd` are not in the bare-method tables, but the receiver is.
        assert_eq!(
            classify_python_qualified("json", "dumps"),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_python_qualified("os", "getcwd"),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_python_qualified("re", "compile"),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn qualified_falls_back_to_method_name() {
        // Unknown receiver → classify the method alone.
        assert_eq!(
            classify_python_qualified("p", "write_text"),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_python_qualified("mock", "assert_called_once"),
            CalleeClass::External
        );
        assert_eq!(
            classify_python_qualified("obj", "frobnicate"),
            CalleeClass::Unknown
        );
    }
}
