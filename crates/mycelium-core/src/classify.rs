//! Static stdlib/builtin/external callee classification (RFC-0113).
//!
//! Tree-sitter is syntactic: it cannot type-resolve a method call like
//! `p.write_text()` or a stdlib call like `os.getcwd()`, so after the
//! project-binding resolver passes (`Store::resolve_bare_call_stubs`) those
//! callees remain as unresolved **bare stubs** — the `unknown` tail that
//! inflates dead-code false positives and truncates call graphs.
//!
//! This module is the **final, static classification tier**: given a bare
//! callee name that project resolution already failed to bind, classify it
//! against curated stdlib / builtin / well-known-external allowlists. Pure
//! table lookup, **zero LSP** — exactly the precision lever ADR-0010 endorses.
//!
//! The allowlist data is ported from the founder's `tree-sitter-analyzer`
//! project (`synapse_resolver/_constants.py`, MIT) which proved this tier lifts
//! Python callee classification 83.9% → 95.9%. This module holds the
//! **language-agnostic cascade + the Python tables**; wiring it into the
//! resolver (so only *remaining* bare stubs reach it — the project-ownership
//! shadow gate) is a separate step.

use std::collections::{HashMap, HashSet};
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
/// Precedence: builtin → stdlib (method / module / module-function) → external
/// method → unknown. Callers MUST apply the project-ownership shadow first (only pass
/// names project resolution left unresolved), so a project method that happens
/// to share a stdlib name is never misclassified.
#[must_use]
pub fn classify_python(name: &str) -> CalleeClass {
    // Builtins first: a name like `format` is both a builtin and a str method;
    // the builtin reading is the correct one for an unqualified call.
    if PYTHON_BUILTINS.contains(name) {
        CalleeClass::Builtin
    } else if PYTHON_STDLIB_METHODS.contains(name)
        || PYTHON_STDLIB_MODULES.contains(name)
        || PYTHON_STDLIB_FUNCTIONS.contains(name)
    {
        CalleeClass::Stdlib
    } else if PYTHON_EXTERNAL_METHODS.contains(name) {
        CalleeClass::External
    } else {
        CalleeClass::Unknown
    }
}

/// Classify a bare Python callee name with **import-context gating** (RFC-0113 Phase 3).
///
/// Unlike [`classify_python`] (name-only), this function requires import evidence
/// before firing the stdlib/external tiers. The gate is module-specific:
///
/// - **Builtins** never need an import — they fire unconditionally.
/// - **Stdlib module names** (e.g. `json`, `os`): fire only when `caller_imports`
///   contains that exact module name.
/// - **Stdlib module-level functions** (e.g. `getcwd` → `os`, `dumps` → `json`/`pickle`):
///   fire only when `caller_imports` contains one of the function's owning modules.
///   See [`STDLIB_FUNCTION_MODULES`] for the ownership map.
/// - **Stdlib methods** (str/pathlib/datetime/re/argparse/io): conservative gate —
///   any stdlib import enables classification. String methods are always available
///   but in practice every file using stdlib methods imports at least one stdlib
///   module. A per-method module map is deferred to a future phase.
/// - **External** names (pytest/hypothesis/mock): fire only when `caller_imports`
///   contains `pytest`, `hypothesis`, `mock`, or `unittest`.
/// - All other names return `Unknown`.
///
/// `caller_imports` is the set of module-name stems from the caller file's
/// `Imports` + `TypeImports` edges — e.g. `{"pathlib", "json"}` from
/// `import pathlib; from json import dumps`.
#[must_use]
pub fn classify_python_import_gated<S: std::hash::BuildHasher>(
    name: &str,
    caller_imports: &std::collections::HashSet<String, S>,
) -> CalleeClass {
    // Builtins are always available without an import.
    if PYTHON_BUILTINS.contains(name) {
        return CalleeClass::Builtin;
    }

    // Stdlib module name (e.g. `json`, `os`): require that exact module imported.
    if PYTHON_STDLIB_MODULES.contains(name) {
        return if caller_imports.contains(name) {
            CalleeClass::Stdlib
        } else {
            CalleeClass::Unknown
        };
    }

    // Stdlib module-level function (e.g. `getcwd` → os, `dumps` → json/pickle):
    // require that one of the owning modules is imported.
    if let Some(owners) = STDLIB_FUNCTION_MODULES.get(name) {
        let imported = owners.iter().any(|&m| caller_imports.contains(m));
        return if imported {
            CalleeClass::Stdlib
        } else {
            CalleeClass::Unknown
        };
    }

    // Stdlib method (str/pathlib/datetime/re/argparse/io): conservative gate.
    // Any stdlib import enables method classification (string methods are always
    // available; the gate mainly keeps completely import-free files clean).
    if PYTHON_STDLIB_METHODS.contains(name) {
        let has_stdlib = caller_imports
            .iter()
            .any(|m| PYTHON_STDLIB_MODULES.contains(m.as_str()));
        return if has_stdlib {
            CalleeClass::Stdlib
        } else {
            CalleeClass::Unknown
        };
    }

    if PYTHON_EXTERNAL_METHODS.contains(name) {
        const EXTERNAL_ROOTS: &[&str] = &["pytest", "hypothesis", "mock", "unittest"];
        let has_external_import = caller_imports
            .iter()
            .any(|m| EXTERNAL_ROOTS.contains(&m.as_str()));
        return if has_external_import {
            CalleeClass::External
        } else {
            CalleeClass::Unknown
        };
    }

    CalleeClass::Unknown
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

/// Distinctive stdlib **module-level function** names. The Python pack
/// materializes `os.getcwd()` as the bare attribute `getcwd` (the receiver is
/// dropped — see `packs/python/queries.scm` "Method calls"), so module functions
/// reach the classifier as bare names that are neither module names nor in the
/// method table. This curated set classifies the common ones as `stdlib`.
///
/// Conservative by design: only names that are overwhelmingly stdlib. Ambiguous
/// names a project routinely defines (`run`, `get`, `call`, `info`, `error`,
/// `seed`, `time`, `date`) are deliberately EXCLUDED — the resolver's
/// project-ownership shadow + import evidence are the right gate for those.
static PYTHON_STDLIB_FUNCTIONS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        // os
        "getcwd",
        "getcwdb",
        "getenv",
        "listdir",
        "makedirs",
        "getpid",
        "getppid",
        "urandom",
        "fspath",
        "scandir",
        // json / pickle
        "dumps",
        "loads",
        // subprocess
        "check_output",
        "check_call",
        "getoutput",
        "getstatusoutput",
        // base64
        "b64encode",
        "b64decode",
        "urlsafe_b64encode",
        "urlsafe_b64decode",
        // logging
        "getLogger",
        "basicConfig",
        // random
        "randint",
        "randrange",
        "getrandbits",
        "shuffle",
        "uniform",
        "gauss",
        // hashlib
        "sha256",
        "sha1",
        "sha512",
        "sha224",
        "sha384",
        "md5",
        "blake2b",
        "blake2s",
        // uuid
        "uuid1",
        "uuid3",
        "uuid4",
        "uuid5",
        // itertools
        "islice",
        "permutations",
        "combinations",
        "groupby",
        "accumulate",
        "starmap",
        "zip_longest",
        "dropwhile",
        "takewhile",
        // functools
        "lru_cache",
        "cmp_to_key",
        "reduce",
        "partial",
        "wraps",
        // shutil
        "copytree",
        "rmtree",
        "copyfileobj",
        "copyfile",
        "make_archive",
        // collections
        "defaultdict",
        "namedtuple",
        "OrderedDict",
        "deque",
        // textwrap
        "dedent",
        "shorten",
        // glob
        "iglob",
        // math
        "isclose",
        "factorial",
        "hypot",
        "isnan",
        "isinf",
        "copysign",
        // time
        "monotonic",
        "perf_counter",
        "process_time",
        "gmtime",
        "localtime",
        // tempfile
        "mkdtemp",
        "mkstemp",
        "gettempdir",
        // importlib / inspect
        "import_module",
        "getsource",
        "getmembers",
        "signature",
        "getfullargspec",
        // secrets
        "token_hex",
        "token_urlsafe",
        "token_bytes",
    ]
    .into_iter()
    .collect()
});

/// Maps each name in [`PYTHON_STDLIB_FUNCTIONS`] to the module(s) that own it.
/// Used by [`classify_python_import_gated`] to enforce module-specific import
/// evidence: `getcwd` requires `os`; `dumps`/`loads` accept either `json` or `pickle`.
static STDLIB_FUNCTION_MODULES: LazyLock<HashMap<&'static str, Vec<&'static str>>> =
    LazyLock::new(|| {
        HashMap::from([
            // os
            ("getcwd", vec!["os"]),
            ("getcwdb", vec!["os"]),
            ("getenv", vec!["os"]),
            ("listdir", vec!["os"]),
            ("makedirs", vec!["os"]),
            ("getpid", vec!["os"]),
            ("getppid", vec!["os"]),
            ("urandom", vec!["os"]),
            ("fspath", vec!["os"]),
            ("scandir", vec!["os"]),
            // json / pickle (both expose dumps/loads)
            ("dumps", vec!["json", "pickle"]),
            ("loads", vec!["json", "pickle"]),
            // subprocess
            ("check_output", vec!["subprocess"]),
            ("check_call", vec!["subprocess"]),
            ("getoutput", vec!["subprocess"]),
            ("getstatusoutput", vec!["subprocess"]),
            // base64
            ("b64encode", vec!["base64"]),
            ("b64decode", vec!["base64"]),
            ("urlsafe_b64encode", vec!["base64"]),
            ("urlsafe_b64decode", vec!["base64"]),
            // logging
            ("getLogger", vec!["logging"]),
            ("basicConfig", vec!["logging"]),
            // random
            ("randint", vec!["random"]),
            ("randrange", vec!["random"]),
            ("getrandbits", vec!["random"]),
            ("shuffle", vec!["random"]),
            ("uniform", vec!["random"]),
            ("gauss", vec!["random"]),
            // hashlib
            ("sha256", vec!["hashlib"]),
            ("sha1", vec!["hashlib"]),
            ("sha512", vec!["hashlib"]),
            ("sha224", vec!["hashlib"]),
            ("sha384", vec!["hashlib"]),
            ("md5", vec!["hashlib"]),
            ("blake2b", vec!["hashlib"]),
            ("blake2s", vec!["hashlib"]),
            // uuid
            ("uuid1", vec!["uuid"]),
            ("uuid3", vec!["uuid"]),
            ("uuid4", vec!["uuid"]),
            ("uuid5", vec!["uuid"]),
            // itertools
            ("islice", vec!["itertools"]),
            ("permutations", vec!["itertools"]),
            ("combinations", vec!["itertools"]),
            ("groupby", vec!["itertools"]),
            ("accumulate", vec!["itertools"]),
            ("starmap", vec!["itertools"]),
            ("zip_longest", vec!["itertools"]),
            ("dropwhile", vec!["itertools"]),
            ("takewhile", vec!["itertools"]),
            // functools
            ("lru_cache", vec!["functools"]),
            ("cmp_to_key", vec!["functools"]),
            ("reduce", vec!["functools"]),
            ("partial", vec!["functools"]),
            ("wraps", vec!["functools"]),
            // shutil
            ("copytree", vec!["shutil"]),
            ("rmtree", vec!["shutil"]),
            ("copyfileobj", vec!["shutil"]),
            ("copyfile", vec!["shutil"]),
            ("make_archive", vec!["shutil"]),
            // collections
            ("defaultdict", vec!["collections"]),
            ("namedtuple", vec!["collections"]),
            ("OrderedDict", vec!["collections"]),
            ("deque", vec!["collections"]),
            // textwrap
            ("dedent", vec!["textwrap"]),
            ("shorten", vec!["textwrap"]),
            // glob
            ("iglob", vec!["glob"]),
            // math
            ("isclose", vec!["math"]),
            ("factorial", vec!["math"]),
            ("hypot", vec!["math"]),
            ("isnan", vec!["math"]),
            ("isinf", vec!["math"]),
            ("copysign", vec!["math"]),
            // time
            ("monotonic", vec!["time"]),
            ("perf_counter", vec!["time"]),
            ("process_time", vec!["time"]),
            ("gmtime", vec!["time"]),
            ("localtime", vec!["time"]),
            // tempfile
            ("mkdtemp", vec!["tempfile"]),
            ("mkstemp", vec!["tempfile"]),
            ("gettempdir", vec!["tempfile"]),
            // importlib
            ("import_module", vec!["importlib"]),
            // inspect
            ("getsource", vec!["inspect"]),
            ("getmembers", vec!["inspect"]),
            ("signature", vec!["inspect"]),
            ("getfullargspec", vec!["inspect"]),
            // secrets
            ("token_hex", vec!["secrets"]),
            ("token_urlsafe", vec!["secrets"]),
            ("token_bytes", vec!["secrets"]),
        ])
    });

/// Well-known third-party (test-framework) method names. Ported from
/// `EXTERNAL_METHODS_PY` (RFC-0005 in tree-sitter-analyzer): pytest, hypothesis,
/// unittest.mock. Conservative — only overwhelmingly-test-framework names.
static PYTHON_EXTERNAL_METHODS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        // NOTE: bare names that are also common *application* method names
        // (skip, mark, text, target, event, patch, call) are deliberately
        // EXCLUDED — they collide with ordinary user code (`obj.text`,
        // `self.skip()`, `mock.call`) and would mislabel project calls as
        // external. We keep only names distinctive enough to be unambiguous.
        // pytest
        "raises",
        "skipif",
        "parametrize",
        "fixture",
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
        "floats",
        "lists",
        "dictionaries",
        "tuples",
        "booleans",
        "composite",
        "assume",
        "note",
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
    fn classifies_bare_stdlib_module_functions() {
        // The Python pack drops the receiver: `os.getcwd()` → bare stub `getcwd`.
        // These are module functions, not module names nor method-table entries.
        assert_eq!(classify_python("getcwd"), CalleeClass::Stdlib);
        assert_eq!(classify_python("dumps"), CalleeClass::Stdlib);
        assert_eq!(classify_python("loads"), CalleeClass::Stdlib);
        assert_eq!(classify_python("getLogger"), CalleeClass::Stdlib);
        assert_eq!(classify_python("check_output"), CalleeClass::Stdlib);
    }

    #[test]
    fn ambiguous_names_are_not_force_classified() {
        // Deliberately excluded so a project's own `run`/`execute` is not
        // mislabeled stdlib — the resolver's project-ownership gate handles them.
        assert_eq!(classify_python("run"), CalleeClass::Unknown);
        assert_eq!(classify_python("execute"), CalleeClass::Unknown);
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

    // ── RFC-0113 Phase 3: import-gated classification ─────────────────────────

    fn imports(modules: &[&str]) -> std::collections::HashSet<String> {
        modules.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn import_gate_builtins_always_fire_without_imports() {
        // Builtins need no import — unconditional.
        let none = imports(&[]);
        assert_eq!(
            classify_python_import_gated("len", &none),
            CalleeClass::Builtin
        );
        assert_eq!(
            classify_python_import_gated("print", &none),
            CalleeClass::Builtin
        );
        assert_eq!(
            classify_python_import_gated("ValueError", &none),
            CalleeClass::Builtin
        );
    }

    #[test]
    fn import_gate_stdlib_method_blocked_without_import() {
        // write_text is a pathlib stdlib method; no import → unknown.
        let none = imports(&[]);
        assert_eq!(
            classify_python_import_gated("write_text", &none),
            CalleeClass::Unknown
        );
        assert_eq!(
            classify_python_import_gated("isoformat", &none),
            CalleeClass::Unknown
        );
    }

    #[test]
    fn import_gate_stdlib_function_blocked_without_import() {
        // getcwd / dumps need an import (os / json) — no stdlib import → unknown.
        let none = imports(&[]);
        assert_eq!(
            classify_python_import_gated("getcwd", &none),
            CalleeClass::Unknown
        );
        assert_eq!(
            classify_python_import_gated("dumps", &none),
            CalleeClass::Unknown
        );
    }

    #[test]
    fn import_gate_stdlib_allowed_with_any_stdlib_import() {
        // Any stdlib import enables the stdlib tier (conservative gate).
        let with_os = imports(&["os"]);
        assert_eq!(
            classify_python_import_gated("getcwd", &with_os),
            CalleeClass::Stdlib
        );

        let with_json = imports(&["json"]);
        assert_eq!(
            classify_python_import_gated("dumps", &with_json),
            CalleeClass::Stdlib
        );

        let with_pathlib = imports(&["pathlib"]);
        assert_eq!(
            classify_python_import_gated("write_text", &with_pathlib),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn import_gate_external_blocked_without_test_import() {
        // pytest.raises → external, but no pytest import → unknown.
        let none = imports(&[]);
        assert_eq!(
            classify_python_import_gated("raises", &none),
            CalleeClass::Unknown
        );
        assert_eq!(
            classify_python_import_gated("parametrize", &none),
            CalleeClass::Unknown
        );
    }

    #[test]
    fn import_gate_external_allowed_with_pytest_import() {
        let with_pytest = imports(&["pytest"]);
        assert_eq!(
            classify_python_import_gated("raises", &with_pytest),
            CalleeClass::External
        );
        assert_eq!(
            classify_python_import_gated("parametrize", &with_pytest),
            CalleeClass::External
        );
    }

    #[test]
    fn import_gate_external_allowed_with_hypothesis_import() {
        let with_hypothesis = imports(&["hypothesis"]);
        assert_eq!(
            classify_python_import_gated("given", &with_hypothesis),
            CalleeClass::External
        );
    }

    #[test]
    fn import_gate_unknown_names_stay_unknown_regardless_of_imports() {
        let lots = imports(&["os", "json", "pathlib", "pytest"]);
        assert_eq!(
            classify_python_import_gated("frobnicate", &lots),
            CalleeClass::Unknown
        );
        assert_eq!(
            classify_python_import_gated("my_project_helper", &lots),
            CalleeClass::Unknown
        );
    }

    // ── RFC-0113 Phase 3 Codex fix: module-specific gate ─────────────────────

    #[test]
    fn import_gate_wrong_module_does_not_enable_stdlib_function() {
        // `json` imported but `getcwd` belongs to `os` — wrong module → unknown.
        let with_json = imports(&["json"]);
        assert_eq!(
            classify_python_import_gated("getcwd", &with_json),
            CalleeClass::Unknown
        );
        // `pathlib` imported but `dumps` belongs to `json`/`pickle` — wrong module → unknown.
        let with_pathlib = imports(&["pathlib"]);
        assert_eq!(
            classify_python_import_gated("dumps", &with_pathlib),
            CalleeClass::Unknown
        );
        // `os` imported but `uuid4` belongs to `uuid` — wrong module → unknown.
        let with_os = imports(&["os"]);
        assert_eq!(
            classify_python_import_gated("uuid4", &with_os),
            CalleeClass::Unknown
        );
    }

    #[test]
    fn import_gate_module_name_requires_exact_module_imported() {
        // `name = "json"` but only `os` imported → json not in caller_imports → unknown.
        let with_os = imports(&["os"]);
        assert_eq!(
            classify_python_import_gated("json", &with_os),
            CalleeClass::Unknown
        );
        // `name = "os"` and `os` is imported → stdlib.
        assert_eq!(
            classify_python_import_gated("os", &with_os),
            CalleeClass::Stdlib
        );
    }
}

// ── RFC-0113 Phase 2: TypeScript / JavaScript ─────────────────────────────────

/// Classify a bare TypeScript/JavaScript callee against the static allowlists.
///
/// Precedence: global builtin → stdlib method → Node.js module name → Node.js
/// module-level function → external test → unknown. Callers MUST apply the
/// project-ownership shadow first (only pass names project resolution left
/// unresolved).
#[must_use]
pub fn classify_typescript(name: &str) -> CalleeClass {
    if TS_GLOBAL_BUILTINS.contains(name) {
        CalleeClass::Builtin
    } else if TS_STDLIB_METHODS.contains(name)
        || TS_NODE_MODULES.contains(name)
        || TS_STDLIB_FUNCTIONS.contains(name)
    {
        CalleeClass::Stdlib
    } else if TS_EXTERNAL_TEST_METHODS.contains(name) {
        CalleeClass::External
    } else {
        CalleeClass::Unknown
    }
}

/// Classify a bare TypeScript/JavaScript callee name with import-context gating
/// (RFC-0113 Phase 2).
///
/// - **Global builtins** (`parseInt`, `setTimeout`, `Error`, …) fire without any import.
/// - **Node.js module names** (`fs`, `path`, `os`, …): fire only when
///   `caller_imports` contains that module name (with or without `node:` prefix).
/// - **Node.js module-level functions** (`readFileSync`, `join`, `randomUUID`, …):
///   fire only when `caller_imports` contains one of the owning modules. See
///   [`TS_FUNCTION_MODULES`] for the ownership map.
/// - **Stdlib methods** (Array/String/Promise/Object methods): conservative gate —
///   any Node.js module import enables classification.
/// - **External** names (jest/vitest/mocha/chai matchers): fire only when
///   `caller_imports` contains a test framework name.
///
/// `caller_imports` is the set of module-specifier stems from the caller
/// file's `Imports` + `TypeImports` edges — e.g. `{"path", "fs"}` from
/// `import * as path from 'path'; import { readFileSync } from 'fs'`.
#[must_use]
pub fn classify_typescript_import_gated<S: std::hash::BuildHasher>(
    name: &str,
    caller_imports: &std::collections::HashSet<String, S>,
) -> CalleeClass {
    if TS_GLOBAL_BUILTINS.contains(name) {
        return CalleeClass::Builtin;
    }

    // Node.js module name: require that exact module imported.
    if TS_NODE_MODULES.contains(name) {
        return if ts_imports_contains(caller_imports, name) {
            CalleeClass::Stdlib
        } else {
            CalleeClass::Unknown
        };
    }

    // Node.js module-level function: require owning module imported.
    if let Some(owners) = TS_FUNCTION_MODULES.get(name) {
        let imported = owners
            .iter()
            .any(|&m| ts_imports_contains(caller_imports, m));
        return if imported {
            CalleeClass::Stdlib
        } else {
            CalleeClass::Unknown
        };
    }

    // Stdlib method: conservative gate — any Node.js module import enables it.
    if TS_STDLIB_METHODS.contains(name) {
        let has_stdlib = caller_imports.iter().any(|m| {
            let canonical = m.strip_prefix("node:").unwrap_or(m.as_str());
            TS_NODE_MODULES.contains(canonical)
        });
        return if has_stdlib {
            CalleeClass::Stdlib
        } else {
            CalleeClass::Unknown
        };
    }

    if TS_STDLIB_FUNCTIONS.contains(name) && !TS_FUNCTION_MODULES.contains_key(name) {
        // A stdlib function that has no explicit module ownership map entry: keep
        // conservative and check for any Node.js import.
        let has_stdlib = caller_imports.iter().any(|m| {
            let canonical = m.strip_prefix("node:").unwrap_or(m.as_str());
            TS_NODE_MODULES.contains(canonical)
        });
        return if has_stdlib {
            CalleeClass::Stdlib
        } else {
            CalleeClass::Unknown
        };
    }

    if TS_EXTERNAL_TEST_METHODS.contains(name) {
        const TEST_ROOTS: &[&str] = &[
            "jest",
            "vitest",
            "mocha",
            "chai",
            "jasmine",
            "@jest/globals",
            "@vitest/snapshot",
        ];
        let has_test = caller_imports.iter().any(|m| {
            TEST_ROOTS
                .iter()
                .any(|root| m == root || m.starts_with(&format!("{root}/")))
        });
        return if has_test {
            CalleeClass::External
        } else {
            CalleeClass::Unknown
        };
    }

    CalleeClass::Unknown
}

/// Helper: checks whether `imports` contains `module`, tolerating a `node:`
/// prefix on either side (i.e. `"node:fs"` matches `"fs"` and vice-versa).
fn ts_imports_contains<S: std::hash::BuildHasher>(
    imports: &std::collections::HashSet<String, S>,
    module: &str,
) -> bool {
    imports.iter().any(|m| {
        let canonical = m.strip_prefix("node:").unwrap_or(m.as_str());
        canonical == module || m.as_str() == module
    })
}

/// Classify a module-qualified TypeScript call `receiver.method()`.
///
/// If the `receiver` is a known Node.js module or a well-known global namespace
/// object (`Math`, `JSON`, `console`, `process`, …), the call is **stdlib**
/// regardless of the method name — `readFileSync`/`floor`/`stringify` are not
/// always in the bare tables, but the receiver is unambiguous. Otherwise fall
/// back to classifying the `method` name alone.
///
/// Like [`classify_typescript`], callers must apply the project-ownership shadow
/// first and, for the module-receiver case, gate on actual import evidence.
#[must_use]
pub fn classify_typescript_qualified(receiver: &str, method: &str) -> CalleeClass {
    let canonical = receiver.strip_prefix("node:").unwrap_or(receiver);
    if TS_NODE_MODULES.contains(canonical) || TS_GLOBAL_NAMESPACE_OBJECTS.contains(canonical) {
        CalleeClass::Stdlib
    } else {
        classify_typescript(method)
    }
}

/// Browser-only DOM/Web API globals — available without any import in a
/// browser JS context. Used as a fallback tier for `.js` files after
/// `classify_typescript_import_gated` returns `Unknown`.
static JS_BROWSER_GLOBALS: std::sync::LazyLock<std::collections::HashSet<&'static str>> =
    std::sync::LazyLock::new(|| {
        [
            // DOM / BOM top-level objects
            "document",
            "window",
            "navigator",
            "location",
            "history",
            // Web APIs (fetch is also in TS_GLOBAL_BUILTINS for Node 18+)
            "fetch",
            "XMLHttpRequest",
            "localStorage",
            "sessionStorage",
            "indexedDB",
            "Worker",
            "WebSocket",
            // Event handling
            "addEventListener",
            "removeEventListener",
            "dispatchEvent",
            "CustomEvent",
            // UI dialogs
            "alert",
            "confirm",
            "prompt",
        ]
        .into_iter()
        .collect()
    });

/// Classify a bare callee name as a browser-global stdlib call for `.js` files.
///
/// Fires after `classify_typescript_import_gated` returns `Unknown` — covers
/// DOM and Web API globals that are always in scope in browser contexts without
/// any import statement.
#[must_use]
pub fn classify_javascript_browser_global(name: &str) -> CalleeClass {
    // Split on the first dot so that synthesized "receiver.method" names
    // (RFC-0126 Phase 3) are classified via their receiver root.
    let root = name.split('.').next().unwrap_or(name);
    if JS_BROWSER_GLOBALS.contains(root) {
        CalleeClass::Stdlib
    } else {
        CalleeClass::Unknown
    }
}

/// JavaScript / Node.js global builtin callables — available without any
/// import in both browser and Node.js contexts.
static TS_GLOBAL_BUILTINS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        // type-conversion / parsing
        "parseInt",
        "parseFloat",
        "isNaN",
        "isFinite",
        // isInteger is NOT a global — the correct form is Number.isInteger(...)
        // URI encoding
        "encodeURI",
        "encodeURIComponent",
        "decodeURI",
        "decodeURIComponent",
        // timers (browser + Node.js)
        "setTimeout",
        "clearTimeout",
        "setInterval",
        "clearInterval",
        "setImmediate",
        "clearImmediate",
        "queueMicrotask",
        "requestAnimationFrame",
        "cancelAnimationFrame",
        // Fetch API (browser + Node.js 18+)
        "fetch",
        // intrinsic error types (used as `throw new TypeError(...)` → bare callee)
        "Error",
        "TypeError",
        "RangeError",
        "SyntaxError",
        "ReferenceError",
        "URIError",
        "EvalError",
        "AggregateError",
        // typed arrays / binary data
        "ArrayBuffer",
        "SharedArrayBuffer",
        "DataView",
        "Float32Array",
        "Float64Array",
        "Int8Array",
        "Int16Array",
        "Int32Array",
        "Uint8Array",
        "Uint16Array",
        "Uint32Array",
        "BigInt64Array",
        "BigUint64Array",
        // special callables
        "eval",
        "require",
        // structural builtins
        "structuredClone",
    ]
    .into_iter()
    .collect()
});

/// Node.js built-in module names. When a bare callee stub matches one of
/// these, it is most likely an aliased module call (`const os = require('os');
/// os(...)` — rare but valid) or an `import * as fs from 'fs'` usage where
/// tree-sitter extracted the module name. The overwhelming common case is
/// the receiver-qualified path (handled by [`classify_typescript_qualified`]).
static TS_NODE_MODULES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "fs",
        "path",
        "os",
        "http",
        "https",
        "crypto",
        "stream",
        "child_process",
        "events",
        "url",
        "util",
        "buffer",
        "net",
        "tls",
        "dns",
        "readline",
        "zlib",
        "assert",
        "cluster",
        "dgram",
        "domain",
        "module",
        "perf_hooks",
        "string_decoder",
        "timers",
        "worker_threads",
        "v8",
        "process",
        "repl",
        "vm",
    ]
    .into_iter()
    .collect()
});

/// Well-known global namespace objects that appear as receivers in qualified
/// calls (`Math.floor(x)`, `JSON.parse(s)`, `console.log(...)`, …).
static TS_GLOBAL_NAMESPACE_OBJECTS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "Math",
        "JSON",
        "console",
        "process",
        "Buffer",
        "Array",
        "Object",
        "String",
        "Number",
        "Boolean",
        "Symbol",
        "BigInt",
        "Promise",
        "Map",
        "Set",
        "WeakMap",
        "WeakSet",
        "Reflect",
        "Proxy",
        "globalThis",
        "global",
        "Atomics",
        "Intl",
        "Date",
        "RegExp",
    ]
    .into_iter()
    .collect()
});

/// Well-known Array / String / Promise / Object *method* names. Ported from
/// the ECMAScript specification method tables. Conservative: only names that
/// are overwhelmingly stdlib. Ambiguous names a project routinely defines
/// (`get`, `set`, `update`, `clear`, `init`, `run`) are deliberately EXCLUDED.
static TS_STDLIB_METHODS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        // Array iteration
        "forEach",
        "map",
        "filter",
        "reduce",
        "reduceRight",
        "find",
        "findIndex",
        "findLast",
        "findLastIndex",
        "some",
        "every",
        "flatMap",
        "flat",
        // Array mutation
        "splice",
        "fill",
        "copyWithin",
        "toReversed",
        "toSorted",
        "toSpliced",
        // Array access
        "includes",
        "indexOf",
        "lastIndexOf",
        "at",
        // Array shape
        "concat",
        "join",
        "slice",
        "entries",
        "reverse",
        "sort",
        // Array stack / queue
        "pop",
        "push",
        "shift",
        "unshift",
        // String manipulation
        "trim",
        "trimStart",
        "trimEnd",
        "trimLeft",
        "trimRight",
        "padStart",
        "padEnd",
        "repeat",
        "replace",
        "replaceAll",
        "split",
        "startsWith",
        "endsWith",
        "substring",
        "substr",
        "charAt",
        "charCodeAt",
        "codePointAt",
        "fromCharCode",
        "fromCodePoint",
        "normalize",
        "toUpperCase",
        "toLowerCase",
        "toLocaleUpperCase",
        "toLocaleLowerCase",
        "localeCompare",
        "match",
        "matchAll",
        "search",
        // Promise / async
        "then",
        "catch",
        "finally",
        // Object proto
        "hasOwnProperty",
        "isPrototypeOf",
        "propertyIsEnumerable",
        "toLocaleString",
        "toJSON",
        // Map / Set
        "has",
        "delete",
        "forEach",
    ]
    .into_iter()
    .collect()
});

/// Distinctive Node.js module-level function names — the bare-stub equivalent
/// of `fs.readFileSync(...)`, `path.join(...)`, etc., after the receiver is
/// dropped by tree-sitter. Conservative: only names overwhelmingly from a
/// single Node.js module. Genuinely ambiguous names (`parse`, `resolve`,
/// `format`) are EXCLUDED — the receiver-qualified [`classify_typescript_qualified`]
/// handles them correctly.
static TS_STDLIB_FUNCTIONS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        // fs
        "readFileSync",
        "writeFileSync",
        "existsSync",
        "mkdirSync",
        "readdirSync",
        "statSync",
        "lstatSync",
        "unlinkSync",
        "renameSync",
        "copyFileSync",
        "appendFileSync",
        "createReadStream",
        "createWriteStream",
        "watchFile",
        "unwatchFile",
        // path — `join`/`resolve`/`normalize` are in TS_FUNCTION_MODULES
        "dirname",
        "basename",
        "extname",
        "isAbsolute",
        "relative",
        // os
        "platform",
        "arch",
        "cpus",
        "freemem",
        "totalmem",
        "homedir",
        "tmpdir",
        "hostname",
        "userInfo",
        "networkInterfaces",
        // crypto
        "randomBytes",
        "randomUUID",
        "createHash",
        "createHmac",
        "createCipheriv",
        "createDecipheriv",
        // util
        "promisify",
        "callbackify",
        // url
        "pathToFileURL",
        "fileURLToPath",
    ]
    .into_iter()
    .collect()
});

/// Maps each name in [`TS_STDLIB_FUNCTIONS`] (and some from
/// [`TS_STDLIB_METHODS`]) to the Node.js module(s) that own it. Used by
/// [`classify_typescript_import_gated`] to enforce module-specific import
/// evidence.
static TS_FUNCTION_MODULES: LazyLock<HashMap<&'static str, Vec<&'static str>>> =
    LazyLock::new(|| {
        HashMap::from([
            // fs
            ("readFileSync", vec!["fs"]),
            ("writeFileSync", vec!["fs"]),
            ("existsSync", vec!["fs"]),
            ("mkdirSync", vec!["fs"]),
            ("readdirSync", vec!["fs"]),
            ("statSync", vec!["fs"]),
            ("lstatSync", vec!["fs"]),
            ("unlinkSync", vec!["fs"]),
            ("renameSync", vec!["fs"]),
            ("copyFileSync", vec!["fs"]),
            ("appendFileSync", vec!["fs"]),
            ("createReadStream", vec!["fs"]),
            ("createWriteStream", vec!["fs"]),
            ("watchFile", vec!["fs"]),
            ("unwatchFile", vec!["fs"]),
            // path (also in TS_STDLIB_METHODS join/slice — ownership map takes precedence)
            ("join", vec!["path"]),
            ("dirname", vec!["path"]),
            ("basename", vec!["path"]),
            ("extname", vec!["path"]),
            ("isAbsolute", vec!["path"]),
            ("relative", vec!["path"]),
            // os
            ("platform", vec!["os"]),
            ("arch", vec!["os"]),
            ("cpus", vec!["os"]),
            ("freemem", vec!["os"]),
            ("totalmem", vec!["os"]),
            ("homedir", vec!["os"]),
            ("tmpdir", vec!["os"]),
            ("hostname", vec!["os"]),
            ("userInfo", vec!["os"]),
            ("networkInterfaces", vec!["os"]),
            // crypto
            ("randomBytes", vec!["crypto"]),
            ("randomUUID", vec!["crypto"]),
            ("createHash", vec!["crypto"]),
            ("createHmac", vec!["crypto"]),
            ("createCipheriv", vec!["crypto"]),
            ("createDecipheriv", vec!["crypto"]),
            // util
            ("promisify", vec!["util"]),
            ("callbackify", vec!["util"]),
            // url
            ("pathToFileURL", vec!["url"]),
            ("fileURLToPath", vec!["url"]),
        ])
    });

/// Well-known test-framework method names (jest, vitest, mocha, chai).
/// Conservative: only names distinctive enough to be unambiguous test API
/// calls. Names that collide with common application code (`before`, `after`,
/// `it`, `test`, `describe`) are deliberately EXCLUDED.
static TS_EXTERNAL_TEST_METHODS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        // jest / vitest expect matchers
        "toBe",
        "toEqual",
        "toStrictEqual",
        "toBeNull",
        "toBeUndefined",
        "toBeDefined",
        "toBeTruthy",
        "toBeFalsy",
        "toThrow",
        "toThrowError",
        "toHaveBeenCalled",
        "toHaveBeenCalledWith",
        "toHaveBeenCalledTimes",
        "toHaveBeenNthCalledWith",
        "toHaveBeenLastCalledWith",
        "toMatchSnapshot",
        "toMatchInlineSnapshot",
        "toMatchObject",
        "toContain",
        "toContainEqual",
        "toHaveLength",
        "toHaveProperty",
        "toBeGreaterThan",
        "toBeGreaterThanOrEqual",
        "toBeLessThan",
        "toBeLessThanOrEqual",
        "toBeCloseTo",
        // jest spy / mock
        "mockReturnValue",
        "mockReturnValueOnce",
        "mockResolvedValue",
        "mockResolvedValueOnce",
        "mockRejectedValue",
        "mockImplementation",
        "mockImplementationOnce",
        "mockClear",
        "mockReset",
        "mockRestore",
        "spyOn",
        // chai
        "deepEqual",
        "strictEqual",
        "notStrictEqual",
        "doesNotThrow",
        "isAbove",
        "isBelow",
        "isAtLeast",
        "isAtMost",
        "isOk",
        "isNotOk",
        // jasmine
        "createSpy",
        "createSpyObj",
        "jasmine",
    ]
    .into_iter()
    .collect()
});

#[cfg(test)]
mod ts_tests {
    use super::*;

    fn ts_imports(modules: &[&str]) -> std::collections::HashSet<String> {
        modules.iter().map(|s| (*s).to_owned()).collect()
    }

    // ── classify_typescript (name-only) ──────────────────────────────────────

    #[test]
    fn ts_classifies_global_builtins() {
        assert_eq!(classify_typescript("parseInt"), CalleeClass::Builtin);
        assert_eq!(classify_typescript("parseFloat"), CalleeClass::Builtin);
        assert_eq!(classify_typescript("isNaN"), CalleeClass::Builtin);
        assert_eq!(classify_typescript("setTimeout"), CalleeClass::Builtin);
        assert_eq!(classify_typescript("Error"), CalleeClass::Builtin);
        assert_eq!(classify_typescript("TypeError"), CalleeClass::Builtin);
    }

    #[test]
    fn ts_classifies_stdlib_array_methods() {
        assert_eq!(classify_typescript("map"), CalleeClass::Stdlib);
        assert_eq!(classify_typescript("filter"), CalleeClass::Stdlib);
        assert_eq!(classify_typescript("reduce"), CalleeClass::Stdlib);
        assert_eq!(classify_typescript("forEach"), CalleeClass::Stdlib);
        assert_eq!(classify_typescript("includes"), CalleeClass::Stdlib);
        assert_eq!(classify_typescript("flatMap"), CalleeClass::Stdlib);
    }

    #[test]
    fn ts_classifies_stdlib_string_methods() {
        assert_eq!(classify_typescript("trim"), CalleeClass::Stdlib);
        assert_eq!(classify_typescript("replace"), CalleeClass::Stdlib);
        assert_eq!(classify_typescript("startsWith"), CalleeClass::Stdlib);
        assert_eq!(classify_typescript("toUpperCase"), CalleeClass::Stdlib);
        assert_eq!(classify_typescript("padStart"), CalleeClass::Stdlib);
    }

    #[test]
    fn ts_classifies_node_module_names() {
        assert_eq!(classify_typescript("fs"), CalleeClass::Stdlib);
        assert_eq!(classify_typescript("path"), CalleeClass::Stdlib);
        assert_eq!(classify_typescript("os"), CalleeClass::Stdlib);
        assert_eq!(classify_typescript("crypto"), CalleeClass::Stdlib);
    }

    #[test]
    fn ts_classifies_node_module_functions() {
        assert_eq!(classify_typescript("readFileSync"), CalleeClass::Stdlib);
        assert_eq!(classify_typescript("writeFileSync"), CalleeClass::Stdlib);
        assert_eq!(classify_typescript("promisify"), CalleeClass::Stdlib);
        assert_eq!(classify_typescript("randomUUID"), CalleeClass::Stdlib);
        assert_eq!(classify_typescript("dirname"), CalleeClass::Stdlib);
        assert_eq!(classify_typescript("basename"), CalleeClass::Stdlib);
    }

    #[test]
    fn ts_classifies_test_matchers() {
        assert_eq!(classify_typescript("toBe"), CalleeClass::External);
        assert_eq!(classify_typescript("toEqual"), CalleeClass::External);
        assert_eq!(
            classify_typescript("toHaveBeenCalled"),
            CalleeClass::External
        );
        assert_eq!(
            classify_typescript("toMatchSnapshot"),
            CalleeClass::External
        );
        assert_eq!(classify_typescript("spyOn"), CalleeClass::External);
    }

    #[test]
    fn ts_unknown_names_stay_unknown() {
        assert_eq!(classify_typescript("myProjectHelper"), CalleeClass::Unknown);
        assert_eq!(classify_typescript("frobnicate"), CalleeClass::Unknown);
        // Ambiguous names deliberately excluded
        assert_eq!(classify_typescript("run"), CalleeClass::Unknown);
        assert_eq!(classify_typescript("init"), CalleeClass::Unknown);
    }

    // ── classify_typescript_import_gated ─────────────────────────────────────

    #[test]
    fn ts_import_gate_builtins_fire_without_imports() {
        let none = ts_imports(&[]);
        assert_eq!(
            classify_typescript_import_gated("parseInt", &none),
            CalleeClass::Builtin
        );
        assert_eq!(
            classify_typescript_import_gated("setTimeout", &none),
            CalleeClass::Builtin
        );
        assert_eq!(
            classify_typescript_import_gated("TypeError", &none),
            CalleeClass::Builtin
        );
    }

    #[test]
    fn ts_import_gate_stdlib_method_blocked_without_import() {
        let none = ts_imports(&[]);
        assert_eq!(
            classify_typescript_import_gated("map", &none),
            CalleeClass::Unknown
        );
        assert_eq!(
            classify_typescript_import_gated("trim", &none),
            CalleeClass::Unknown
        );
    }

    #[test]
    fn ts_import_gate_stdlib_method_allowed_with_any_node_import() {
        let with_path = ts_imports(&["path"]);
        assert_eq!(
            classify_typescript_import_gated("map", &with_path),
            CalleeClass::Stdlib
        );
        let with_fs = ts_imports(&["fs"]);
        assert_eq!(
            classify_typescript_import_gated("trim", &with_fs),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn ts_import_gate_module_function_requires_exact_module() {
        // readFileSync belongs to fs — needs fs import.
        let none = ts_imports(&[]);
        assert_eq!(
            classify_typescript_import_gated("readFileSync", &none),
            CalleeClass::Unknown
        );
        let with_fs = ts_imports(&["fs"]);
        assert_eq!(
            classify_typescript_import_gated("readFileSync", &with_fs),
            CalleeClass::Stdlib
        );
        // Wrong module: path import does NOT enable readFileSync (fs function).
        let with_path = ts_imports(&["path"]);
        assert_eq!(
            classify_typescript_import_gated("readFileSync", &with_path),
            CalleeClass::Unknown
        );
    }

    #[test]
    fn ts_import_gate_path_functions_require_path_import() {
        let with_fs = ts_imports(&["fs"]);
        assert_eq!(
            classify_typescript_import_gated("dirname", &with_fs),
            CalleeClass::Unknown
        );
        let with_path = ts_imports(&["path"]);
        assert_eq!(
            classify_typescript_import_gated("dirname", &with_path),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_typescript_import_gated("basename", &with_path),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_typescript_import_gated("join", &with_path),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn ts_import_gate_node_prefix_is_tolerated() {
        // `import * as fs from 'node:fs'` — canonical strip must match.
        let with_node_fs = ts_imports(&["node:fs"]);
        assert_eq!(
            classify_typescript_import_gated("readFileSync", &with_node_fs),
            CalleeClass::Stdlib
        );
        let with_node_path = ts_imports(&["node:path"]);
        assert_eq!(
            classify_typescript_import_gated("dirname", &with_node_path),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn ts_import_gate_test_methods_blocked_without_test_import() {
        let none = ts_imports(&[]);
        assert_eq!(
            classify_typescript_import_gated("toBe", &none),
            CalleeClass::Unknown
        );
        assert_eq!(
            classify_typescript_import_gated("toHaveBeenCalled", &none),
            CalleeClass::Unknown
        );
    }

    #[test]
    fn ts_import_gate_test_methods_allowed_with_jest_import() {
        let with_jest = ts_imports(&["jest"]);
        assert_eq!(
            classify_typescript_import_gated("toBe", &with_jest),
            CalleeClass::External
        );
        assert_eq!(
            classify_typescript_import_gated("toMatchSnapshot", &with_jest),
            CalleeClass::External
        );
    }

    #[test]
    fn ts_import_gate_test_methods_allowed_with_vitest_import() {
        let with_vitest = ts_imports(&["vitest"]);
        assert_eq!(
            classify_typescript_import_gated("toEqual", &with_vitest),
            CalleeClass::External
        );
        assert_eq!(
            classify_typescript_import_gated("spyOn", &with_vitest),
            CalleeClass::External
        );
    }

    #[test]
    fn ts_import_gate_unknown_names_stay_unknown_regardless_of_imports() {
        let lots = ts_imports(&["fs", "path", "os", "jest"]);
        assert_eq!(
            classify_typescript_import_gated("frobnicate", &lots),
            CalleeClass::Unknown
        );
        assert_eq!(
            classify_typescript_import_gated("myProjectFn", &lots),
            CalleeClass::Unknown
        );
    }

    // ── classify_typescript_qualified ────────────────────────────────────────

    #[test]
    fn ts_qualified_node_module_is_stdlib() {
        assert_eq!(
            classify_typescript_qualified("fs", "readFileSync"),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_typescript_qualified("path", "join"),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_typescript_qualified("os", "platform"),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_typescript_qualified("crypto", "randomUUID"),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn ts_qualified_global_namespace_is_stdlib() {
        assert_eq!(
            classify_typescript_qualified("Math", "floor"),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_typescript_qualified("JSON", "parse"),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_typescript_qualified("console", "log"),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_typescript_qualified("Promise", "all"),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_typescript_qualified("Object", "keys"),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn ts_qualified_node_prefix_is_stdlib() {
        // `import * as fs from 'node:fs'` → receiver is `fs`, method is `readFileSync`.
        assert_eq!(
            classify_typescript_qualified("node:fs", "readFileSync"),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_typescript_qualified("node:path", "join"),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn ts_qualified_falls_back_to_method_name() {
        // Unknown receiver → classify the method alone.
        assert_eq!(
            classify_typescript_qualified("arr", "map"),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_typescript_qualified("obj", "frobnicate"),
            CalleeClass::Unknown
        );
        assert_eq!(
            classify_typescript_qualified("suite", "toBe"),
            CalleeClass::External
        );
    }
}

// ── Go stdlib classification (RFC-0113 Phase 3) ──────────────────────────────

/// Classify a bare Go callee name without import context.
///
/// - **Builtins** (`make`, `len`, `append`, …) are classified unconditionally.
/// - **Stdlib package names** (`fmt`, `os`, `http`, `json`, …) are classified
///   as `Stdlib` even without import evidence — the import gate is [`classify_go_import_gated`].
/// - Everything else is `Unknown`.
#[must_use]
pub fn classify_go(name: &str) -> CalleeClass {
    if GO_BUILTINS.contains(name) {
        CalleeClass::Builtin
    } else if GO_STDLIB_PKG_NAMES.contains(name) {
        CalleeClass::Stdlib
    } else {
        CalleeClass::Unknown
    }
}

/// Classify a bare Go callee name with **import-context gating** (RFC-0113 Phase 3).
///
/// - **Builtins** never need an import — they fire unconditionally.
/// - **Stdlib package local names** (`fmt`, `http`, `json`, …): require that
///   `caller_imports` contains either the exact local name (e.g. `"fmt"`) or a
///   multi-segment import path whose last component equals the local name
///   (e.g. `"net/http"` → local name `"http"`). This covers both simple imports
///   (`import "fmt"`) and path imports (`import "encoding/json"` → `json`).
/// - Everything else is `Unknown`.
///
/// `caller_imports` is the set of import path strings from the caller file's
/// `Imports` edges — e.g. `{"fmt", "net/http", "encoding/json"}`.
#[must_use]
pub fn classify_go_import_gated<S: std::hash::BuildHasher>(
    name: &str,
    caller_imports: &std::collections::HashSet<String, S>,
) -> CalleeClass {
    if GO_BUILTINS.contains(name) {
        return CalleeClass::Builtin;
    }

    if GO_STDLIB_PKG_NAMES.contains(name) {
        // Accept if any import path's last segment (after the final '/') equals name,
        // or if the full path equals name (for single-segment packages like "fmt").
        let imported = caller_imports.iter().any(|imp| {
            let local = imp.rsplit('/').next().unwrap_or(imp.as_str());
            local == name
        });
        return if imported {
            CalleeClass::Stdlib
        } else {
            CalleeClass::Unknown
        };
    }

    CalleeClass::Unknown
}

/// Classify a module-qualified Go call `receiver.Method()`.
///
/// If the `receiver` is a known Go stdlib package local name (e.g. `fmt`,
/// `http`, `json`) or a full import path (e.g. `net/http`, `encoding/json`),
/// the call is **stdlib** — the method name is trusted to be whatever the
/// package exports. Otherwise returns `Unknown`.
///
/// Callers must apply the project-ownership shadow first (only unresolved stubs
/// reach here).
#[must_use]
pub fn classify_go_qualified(receiver: &str, _method: &str) -> CalleeClass {
    // Accept the local name directly (e.g. "fmt", "http", "json").
    if GO_STDLIB_PKG_NAMES.contains(receiver) {
        return CalleeClass::Stdlib;
    }
    // Accept a full import path as receiver (e.g. "net/http", "encoding/json").
    // The local name is the last path segment.
    let local = receiver.rsplit('/').next().unwrap_or(receiver);
    if local != receiver && GO_STDLIB_PKG_NAMES.contains(local) {
        return CalleeClass::Stdlib;
    }
    CalleeClass::Unknown
}

/// Go builtin functions — available without any import.
static GO_BUILTINS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "make", "len", "cap", "append", "copy", "delete", "close", "panic", "recover", "new",
        "print", "println", "real", "imag", "complex", "min", "max", "clear", // Go 1.21+
    ]
    .into_iter()
    .collect()
});

/// Go stdlib package local names — the identifiers used in code after import.
/// Covers all packages from the Go standard library used in typical projects.
/// Multi-segment packages (e.g. `net/http`) are listed by their local name (`http`).
static GO_STDLIB_PKG_NAMES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        // core I/O and OS
        "fmt",
        "os",
        "io",
        "bufio",
        "bytes",
        "strings",
        "strconv",
        "log",
        "errors",
        // data structures and algorithms
        "sort",
        "math",
        "rand",
        // concurrency
        "sync",
        "atomic",
        "context",
        "time",
        // networking
        "net",
        "http",
        "url",
        "rpc",
        "smtp",
        "mail",
        // encoding
        "json",
        "xml",
        "csv",
        "base64",
        "hex",
        "binary",
        "gob",
        "asn1",
        // filesystem
        "path",
        "filepath",
        "fs",
        "embed",
        // text/templates
        "template",
        "html",
        // reflection and runtime
        "reflect",
        "runtime",
        "unsafe",
        // testing
        "testing",
        "iotest",
        "httptest",
        // compression / archive
        "gzip",
        "zlib",
        "bzip2",
        "zip",
        "tar",
        "lzw",
        // crypto
        "tls",
        "rsa",
        "ecdsa",
        "ed25519",
        "sha256",
        "sha512",
        "md5",
        "hmac",
        "cipher",
        "aes",
        "des",
        "rc4",
        "rand_crypto",
        // Go tools
        "ast",
        "parser",
        "token",
        "types",
        "scanner",
        "format",
        // flags and CLI
        "flag",
        // misc stdlib
        "regexp",
        "unicode",
        "utf8",
        "utf16",
        "big",
        "bits",
        "cmplx",
        "ring",
        "heap",
        "list",
        "pprof",
        "trace",
        "signal",
        "exec",
        "user",
        "sql",
        "driver",
        "debug",
        "dwarf",
        "elf",
        "gosym",
        "macho",
        "pe",
        "plan9obj",
        "build",
        "constraint",
        "importer",
        "tabwriter",
        "template",
        "syslog",
        "ioutil",
    ]
    .into_iter()
    .collect()
});

#[cfg(test)]
mod go_tests {
    use super::*;

    fn go_imports(pkgs: &[&str]) -> std::collections::HashSet<String> {
        pkgs.iter().map(|s| (*s).to_owned()).collect()
    }

    // ── classify_go (name-only) ──────────────────────────────────────────────

    #[test]
    fn go_classifies_builtins() {
        assert_eq!(classify_go("make"), CalleeClass::Builtin);
        assert_eq!(classify_go("len"), CalleeClass::Builtin);
        assert_eq!(classify_go("cap"), CalleeClass::Builtin);
        assert_eq!(classify_go("append"), CalleeClass::Builtin);
        assert_eq!(classify_go("copy"), CalleeClass::Builtin);
        assert_eq!(classify_go("delete"), CalleeClass::Builtin);
        assert_eq!(classify_go("close"), CalleeClass::Builtin);
        assert_eq!(classify_go("panic"), CalleeClass::Builtin);
        assert_eq!(classify_go("recover"), CalleeClass::Builtin);
        assert_eq!(classify_go("new"), CalleeClass::Builtin);
    }

    #[test]
    fn go_classifies_stdlib_package_names() {
        assert_eq!(classify_go("fmt"), CalleeClass::Stdlib);
        assert_eq!(classify_go("os"), CalleeClass::Stdlib);
        assert_eq!(classify_go("io"), CalleeClass::Stdlib);
        assert_eq!(classify_go("strings"), CalleeClass::Stdlib);
        assert_eq!(classify_go("http"), CalleeClass::Stdlib);
        assert_eq!(classify_go("json"), CalleeClass::Stdlib);
    }

    #[test]
    fn go_unknown_names_stay_unknown() {
        assert_eq!(classify_go("myHelper"), CalleeClass::Unknown);
        assert_eq!(classify_go("frobnicate"), CalleeClass::Unknown);
        assert_eq!(classify_go("handler"), CalleeClass::Unknown);
    }

    // ── classify_go_import_gated ─────────────────────────────────────────────

    #[test]
    fn go_import_gate_builtins_fire_without_imports() {
        let none = go_imports(&[]);
        assert_eq!(
            classify_go_import_gated("make", &none),
            CalleeClass::Builtin
        );
        assert_eq!(classify_go_import_gated("len", &none), CalleeClass::Builtin);
        assert_eq!(
            classify_go_import_gated("panic", &none),
            CalleeClass::Builtin
        );
        assert_eq!(classify_go_import_gated("new", &none), CalleeClass::Builtin);
    }

    #[test]
    fn go_import_gate_stdlib_pkg_blocked_without_import() {
        let none = go_imports(&[]);
        assert_eq!(classify_go_import_gated("fmt", &none), CalleeClass::Unknown);
        assert_eq!(classify_go_import_gated("os", &none), CalleeClass::Unknown);
        assert_eq!(
            classify_go_import_gated("http", &none),
            CalleeClass::Unknown
        );
    }

    #[test]
    fn go_import_gate_simple_pkg_requires_exact_import() {
        // `import "fmt"` → local name "fmt" → stem "fmt".
        let with_fmt = go_imports(&["fmt"]);
        assert_eq!(
            classify_go_import_gated("fmt", &with_fmt),
            CalleeClass::Stdlib
        );
        // Wrong import does not enable fmt.
        let with_os = go_imports(&["os"]);
        assert_eq!(
            classify_go_import_gated("fmt", &with_os),
            CalleeClass::Unknown
        );
    }

    #[test]
    fn go_import_gate_multi_segment_pkg_matched_by_last_component() {
        // `import "net/http"` → stem stored as "net/http"; local name in code is "http".
        let with_net_http = go_imports(&["net/http"]);
        assert_eq!(
            classify_go_import_gated("http", &with_net_http),
            CalleeClass::Stdlib
        );
        // encoding/json → local name "json".
        let with_enc_json = go_imports(&["encoding/json"]);
        assert_eq!(
            classify_go_import_gated("json", &with_enc_json),
            CalleeClass::Stdlib
        );
        // path/filepath → local name "filepath".
        let with_filepath = go_imports(&["path/filepath"]);
        assert_eq!(
            classify_go_import_gated("filepath", &with_filepath),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn go_import_gate_unknown_names_stay_unknown_regardless_of_imports() {
        let lots = go_imports(&["fmt", "os", "net/http", "encoding/json"]);
        assert_eq!(
            classify_go_import_gated("frobnicate", &lots),
            CalleeClass::Unknown
        );
        assert_eq!(
            classify_go_import_gated("myHelper", &lots),
            CalleeClass::Unknown
        );
    }

    // ── classify_go_qualified ────────────────────────────────────────────────

    #[test]
    fn go_qualified_stdlib_receiver_is_stdlib() {
        assert_eq!(classify_go_qualified("fmt", "Println"), CalleeClass::Stdlib);
        assert_eq!(classify_go_qualified("os", "Open"), CalleeClass::Stdlib);
        assert_eq!(
            classify_go_qualified("strings", "Join"),
            CalleeClass::Stdlib
        );
        assert_eq!(classify_go_qualified("http", "Get"), CalleeClass::Stdlib);
        assert_eq!(
            classify_go_qualified("json", "Marshal"),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_go_qualified("filepath", "Join"),
            CalleeClass::Stdlib
        );
        assert_eq!(classify_go_qualified("errors", "New"), CalleeClass::Stdlib);
        assert_eq!(classify_go_qualified("sync", "Mutex"), CalleeClass::Stdlib);
        assert_eq!(
            classify_go_qualified("context", "Background"),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_go_qualified("regexp", "MustCompile"),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn go_qualified_full_import_path_as_receiver_is_stdlib() {
        // If the import path itself appears as receiver (rare, e.g. aliased)
        assert_eq!(
            classify_go_qualified("net/http", "Get"),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_go_qualified("encoding/json", "Marshal"),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn go_qualified_unknown_receiver_falls_back_to_method() {
        // Unknown receiver → classify the method alone.
        assert_eq!(
            classify_go_qualified("myPkg", "frobnicate"),
            CalleeClass::Unknown
        );
        // Unknown receiver but method is a builtin name → still unknown (builtins aren't methods).
        assert_eq!(classify_go_qualified("myPkg", "len"), CalleeClass::Unknown);
    }
}

// ── RFC-0113 Phase 4: Rust stdlib / builtin classification ────────────────────

/// Classify a bare Rust callee name without import context.
///
/// - **Builtins** (macros + `drop`): always `Builtin` — no import required.
/// - **Stdlib module local names** (`fs`, `io`, `env`, …): `Stdlib` when the
///   name matches a known standard-library module. Callers that need import
///   gating should use [`classify_rust_import_gated`] instead.
/// - Everything else: `Unknown`.
#[must_use]
pub fn classify_rust(name: &str) -> CalleeClass {
    if RUST_BUILTINS.contains(name) {
        CalleeClass::Builtin
    } else if RUST_STDLIB_MODULES.contains(name) {
        CalleeClass::Stdlib
    } else {
        CalleeClass::Unknown
    }
}

/// Classify a bare Rust callee name with **import-context gating** (RFC-0113 Phase 4).
///
/// - **Builtins** never need an import — they fire unconditionally.
/// - **Stdlib module local names** (`fs`, `io`, `env`, …): require that
///   `caller_imports` contains an import path that starts with `std::<name>` or
///   `core::<name>`, covering patterns such as:
///   - `use std::fs;` → stored as `"std::fs"`
///   - `use std::fs::File;` → stored as `"std::fs::File"` (starts with `"std::fs"`)
/// - Everything else: `Unknown`.
///
/// `caller_imports` is the set of import path strings from the caller file's
/// `Imports` edges — e.g. `{"std::fs", "std::io::BufReader"}`.
#[must_use]
pub fn classify_rust_import_gated<S: std::hash::BuildHasher>(
    name: &str,
    caller_imports: &std::collections::HashSet<String, S>,
) -> CalleeClass {
    if RUST_BUILTINS.contains(name) {
        return CalleeClass::Builtin;
    }

    if RUST_STDLIB_MODULES.contains(name) {
        let std_prefix = format!("std::{name}");
        let core_prefix = format!("core::{name}");
        let imported = caller_imports.iter().any(|imp| {
            imp == &std_prefix
                || imp.starts_with(&format!("{std_prefix}::"))
                || imp == &core_prefix
                || imp.starts_with(&format!("{core_prefix}::"))
        });
        return if imported {
            CalleeClass::Stdlib
        } else {
            CalleeClass::Unknown
        };
    }

    CalleeClass::Unknown
}

/// Classify a module-qualified Rust call `receiver::Method()`.
///
/// If the `receiver` is a known Rust stdlib module local name (e.g. `fs`,
/// `io`, `env`), the Rust stdlib root (`std`, `core`, `alloc`), or a
/// qualified path whose last `::` component is a stdlib module (e.g.
/// `std::fs`), the call is **stdlib**. Otherwise returns `Unknown`.
#[must_use]
pub fn classify_rust_qualified(receiver: &str, _method: &str) -> CalleeClass {
    // Direct stdlib module local name: fs, io, env, process, …
    if RUST_STDLIB_MODULES.contains(receiver) {
        return CalleeClass::Stdlib;
    }
    // Stdlib roots qualify everything underneath them.
    if matches!(receiver, "std" | "core" | "alloc") {
        return CalleeClass::Stdlib;
    }
    // Full-path receiver like "std::fs" or "core::mem" — take the last segment.
    let local = receiver.rsplit("::").next().unwrap_or(receiver);
    if local != receiver && RUST_STDLIB_MODULES.contains(local) {
        return CalleeClass::Stdlib;
    }
    CalleeClass::Unknown
}

/// Rust macro builtins and always-available functions — no import required.
static RUST_BUILTINS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        // macros available in every Rust file via the prelude
        "println",
        "print",
        "eprintln",
        "eprint",
        "format",
        "format_args",
        "panic",
        "todo",
        "unimplemented",
        "unreachable",
        "assert",
        "assert_eq",
        "assert_ne",
        "debug_assert",
        "debug_assert_eq",
        "debug_assert_ne",
        "vec",
        "dbg",
        "write",
        "writeln",
        "concat",
        "include",
        "include_str",
        "include_bytes",
        "env",
        "option_env",
        "cfg",
        "compile_error",
        "matches",
        "log",
        // always-available prelude functions
        "drop",
    ]
    .into_iter()
    .collect()
});

/// Rust stdlib module local names — the identifiers used after `use std::<name>`.
static RUST_STDLIB_MODULES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        // I/O and filesystem
        "fs",
        "io",
        "path",
        "net",
        // environment and process
        "env",
        "process",
        // concurrency
        "sync",
        "thread",
        "atomic",
        // data structures
        "collections",
        "vec",
        "slice",
        "str",
        "string",
        // algorithms and utilities
        "cmp",
        "ops",
        "iter",
        "num",
        "convert",
        "borrow",
        // formatting and error
        "fmt",
        "error",
        // memory
        "mem",
        "ptr",
        "alloc",
        // time
        "time",
        // hashing
        "hash",
        // reference-counting
        "rc",
        "cell",
        // marker types
        "marker",
        // misc
        "char",
        "f32",
        "f64",
        "i8",
        "i16",
        "i32",
        "i64",
        "i128",
        "u8",
        "u16",
        "u32",
        "u64",
        "u128",
        "usize",
        "isize",
    ]
    .into_iter()
    .collect()
});

#[cfg(test)]
mod rust_tests {
    use super::*;

    fn rust_imports(mods: &[&str]) -> std::collections::HashSet<String> {
        mods.iter().map(|s| (*s).to_owned()).collect()
    }

    // ── classify_rust (name-only) ─────────────────────────────────────────────

    #[test]
    fn rust_classifies_macro_builtins() {
        assert_eq!(classify_rust("println"), CalleeClass::Builtin);
        assert_eq!(classify_rust("eprintln"), CalleeClass::Builtin);
        assert_eq!(classify_rust("format"), CalleeClass::Builtin);
        assert_eq!(classify_rust("panic"), CalleeClass::Builtin);
        assert_eq!(classify_rust("assert"), CalleeClass::Builtin);
        assert_eq!(classify_rust("assert_eq"), CalleeClass::Builtin);
        assert_eq!(classify_rust("vec"), CalleeClass::Builtin);
        assert_eq!(classify_rust("dbg"), CalleeClass::Builtin);
    }

    #[test]
    fn rust_classifies_drop_as_builtin() {
        assert_eq!(classify_rust("drop"), CalleeClass::Builtin);
    }

    #[test]
    fn rust_classifies_stdlib_module_names() {
        assert_eq!(classify_rust("fs"), CalleeClass::Stdlib);
        assert_eq!(classify_rust("io"), CalleeClass::Stdlib);
        assert_eq!(classify_rust("env"), CalleeClass::Builtin); // env! macro in RUST_BUILTINS wins over std::env module
        assert_eq!(classify_rust("process"), CalleeClass::Stdlib);
        assert_eq!(classify_rust("sync"), CalleeClass::Stdlib);
        assert_eq!(classify_rust("thread"), CalleeClass::Stdlib);
        assert_eq!(classify_rust("collections"), CalleeClass::Stdlib);
    }

    #[test]
    fn rust_unknown_names_stay_unknown() {
        assert_eq!(classify_rust("frobnicate"), CalleeClass::Unknown);
        assert_eq!(classify_rust("my_function"), CalleeClass::Unknown);
    }

    // ── classify_rust_import_gated ────────────────────────────────────────────

    #[test]
    fn rust_import_gate_builtins_fire_without_imports() {
        assert_eq!(
            classify_rust_import_gated("println", &rust_imports(&[])),
            CalleeClass::Builtin
        );
        assert_eq!(
            classify_rust_import_gated("drop", &rust_imports(&[])),
            CalleeClass::Builtin
        );
    }

    #[test]
    fn rust_import_gate_stdlib_module_blocked_without_import() {
        assert_eq!(
            classify_rust_import_gated("fs", &rust_imports(&[])),
            CalleeClass::Unknown
        );
        assert_eq!(
            classify_rust_import_gated("io", &rust_imports(&[])),
            CalleeClass::Unknown
        );
    }

    #[test]
    fn rust_import_gate_stdlib_module_allowed_with_std_import() {
        assert_eq!(
            classify_rust_import_gated("fs", &rust_imports(&["std::fs"])),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn rust_import_gate_sub_import_enables_module_name() {
        // `use std::fs::File;` → import string "std::fs::File" → starts with "std::fs" → fs enabled
        assert_eq!(
            classify_rust_import_gated("fs", &rust_imports(&["std::fs::File"])),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn rust_import_gate_unknown_names_stay_unknown() {
        assert_eq!(
            classify_rust_import_gated("frobnicate", &rust_imports(&["std::fs"])),
            CalleeClass::Unknown
        );
    }

    #[test]
    fn rust_import_gate_wrong_module_does_not_enable() {
        // std::io does NOT enable "fs"
        assert_eq!(
            classify_rust_import_gated("fs", &rust_imports(&["std::io"])),
            CalleeClass::Unknown
        );
    }

    // ── classify_rust_qualified ───────────────────────────────────────────────

    #[test]
    fn rust_qualified_stdlib_module_is_stdlib() {
        assert_eq!(
            classify_rust_qualified("fs", "read_to_string"),
            CalleeClass::Stdlib
        );
        assert_eq!(classify_rust_qualified("io", "stdin"), CalleeClass::Stdlib);
        assert_eq!(
            classify_rust_qualified("process", "exit"),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_rust_qualified("thread", "spawn"),
            CalleeClass::Stdlib
        );
        assert_eq!(classify_rust_qualified("sync", "Arc"), CalleeClass::Stdlib);
    }

    #[test]
    fn rust_qualified_std_root_is_stdlib() {
        assert_eq!(classify_rust_qualified("std", "mem"), CalleeClass::Stdlib);
        assert_eq!(classify_rust_qualified("core", "mem"), CalleeClass::Stdlib);
        assert_eq!(classify_rust_qualified("alloc", "vec"), CalleeClass::Stdlib);
    }

    #[test]
    fn rust_qualified_full_path_receiver_is_stdlib() {
        // "std::fs" as receiver → last "::" segment is "fs" → Stdlib
        assert_eq!(
            classify_rust_qualified("std::fs", "read_to_string"),
            CalleeClass::Stdlib
        );
        assert_eq!(
            classify_rust_qualified("std::io", "stdout"),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn rust_qualified_unknown_receiver_falls_back_to_unknown() {
        assert_eq!(
            classify_rust_qualified("my_module", "helper"),
            CalleeClass::Unknown
        );
        assert_eq!(
            classify_rust_qualified("third_party", "run"),
            CalleeClass::Unknown
        );
    }

    // ── classify_javascript_browser_global Phase 3: dot-qualified names ─────────

    #[test]
    fn js_browser_global_member_call_document_query_selector_is_stdlib() {
        // AC-1 (RFC-0126): synthesized "document.querySelector" → Stdlib
        assert_eq!(
            classify_javascript_browser_global("document.querySelector"),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn js_browser_global_member_call_window_open_is_stdlib() {
        // AC-2 (RFC-0126): synthesized "window.open" → Stdlib
        assert_eq!(
            classify_javascript_browser_global("window.open"),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn js_browser_global_member_call_local_storage_get_item_is_stdlib() {
        // AC-3 (RFC-0126): synthesized "localStorage.getItem" → Stdlib
        assert_eq!(
            classify_javascript_browser_global("localStorage.getItem"),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn js_browser_global_member_call_unknown_receiver_is_unknown() {
        // AC-4 (RFC-0126): unknown receiver → Unknown (no false positive)
        assert_eq!(
            classify_javascript_browser_global("myObj.myMethod"),
            CalleeClass::Unknown
        );
    }

    // ── classify_javascript_browser_global (RFC-0125 Phase 2) ────────────────

    #[test]
    fn js_browser_global_fetch_is_stdlib() {
        // AC-6: fetch is a browser global → Stdlib
        assert_eq!(
            classify_javascript_browser_global("fetch"),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn js_browser_global_document_is_stdlib() {
        // AC-7: document is a DOM global → Stdlib
        assert_eq!(
            classify_javascript_browser_global("document"),
            CalleeClass::Stdlib
        );
    }

    #[test]
    fn js_browser_global_unknown_name_is_unknown() {
        // AC-8: arbitrary function name → Unknown
        assert_eq!(
            classify_javascript_browser_global("myCustomFn"),
            CalleeClass::Unknown
        );
    }
}
