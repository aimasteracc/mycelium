# Language Packs

Mycelium uses **language packs** to parse source files and extract the symbol graph.
Each pack is a self-contained directory of two files: a `pack.toml` manifest and a
`queries.scm` tree-sitter query file.

Charter §4 hard limit: **≤ 3 files per pack** (`pack.toml` + `queries.scm` + an
optional `README.md`). No changes to core engine code are ever required to add a
language.

---

## Bundled packs

Ten packs ship compiled into the Mycelium binary and are always available as
fallback, even when no `MYCELIUM_PACKS_DIR` is set:

| Language  | Extensions (dispatch)         | Grammar source                       |
|-----------|-------------------------------|--------------------------------------|
| Python    | `.py`, `.pyi`                 | `npm:tree-sitter-python@^0.21`       |
| JavaScript| `.js`, `.jsx`                 | `npm:tree-sitter-javascript@^0.25`   |
| TypeScript| `.ts`, `.tsx`                 | `npm:tree-sitter-typescript@^0.23`   |
| Rust      | `.rs`                         | `crates.io:tree-sitter-rust@^0.23`   |
| Go        | `.go`                         | `crates.io:tree-sitter-go@^0.23`     |
| Java      | `.java`                       | `crates.io:tree-sitter-java@^0.23`   |
| C         | `.c`, `.h`                    | `crates.io:tree-sitter-c@^0.23`      |
| C++       | `.cpp`, `.cc`, `.cxx`, `.hpp` | `crates.io:tree-sitter-cpp@^0.23`    |
| C#        | `.cs`                         | `crates.io:tree-sitter-c-sharp@^0.23`|
| Ruby      | `.rb`                         | `crates.io:tree-sitter-ruby@^0.23`   |

> **Note on `.h` files:** C++ packs list `.h` as a `secondary_extension` (ambiguous
> with C). The C pack takes priority for `.h` when no language hint is present.

---

## Loading custom packs at runtime

To add support for a new language or to override queries for a bundled language,
point Mycelium at a directory of packs — no recompilation required.

### Environment variable

```sh
export MYCELIUM_PACKS_DIR=/path/to/my-packs
mycelium index .
```

The cortex engine (`mycelium-core`) reads `MYCELIUM_PACKS_DIR` at startup and
dispatches file extensions through the registry before falling back to the bundled
static queries.

### CLI flag

```sh
mycelium index --packs-dir /path/to/my-packs .
```

The `--packs-dir` flag overrides `MYCELIUM_PACKS_DIR` for the duration of that
`index` invocation. Extensions already covered by the 10 bundled grammars are
unaffected; the registry supplements for unknown extensions.

---

## Pack directory layout

```
my-packs/
└── kotlin/
    ├── pack.toml   # required — metadata and extension dispatch
    └── queries.scm # required — tree-sitter capture patterns
```

Each sub-directory that contains a `pack.toml` is treated as one language pack.
Sub-directories without `pack.toml` are silently skipped.

---

## pack.toml reference

```toml
# Required fields
[meta]
name       = "kotlin"
extensions = [".kt", ".kts"]
grammar    = "npm:tree-sitter-kotlin@^0.3"

# Optional fields
description = "Kotlin language pack for Mycelium."
```

### Field reference

| Field                 | Required | Description |
|-----------------------|----------|-------------|
| `name`                | yes | Human-readable language name (e.g. `"python"`). |
| `extensions`          | yes* | File extensions this pack handles, with leading dot (e.g. `[".py", ".pyi"]`). Used as dispatch extensions when `primary_extensions` is absent. |
| `grammar`             | yes | Grammar reference string (see [Grammar strings](#grammar-strings)). |
| `primary_extensions`  | no  | Unambiguous extensions for dispatch. When present, replaces `extensions` for extension routing. Use this when a language shares an extension with another (e.g. C++ and C both use `.h`). |
| `secondary_extensions`| no  | Ambiguous extensions (e.g. `.h` shared by C++ and C). These are never used for automatic dispatch; they require an explicit language hint. |
| `description`         | no  | Human-readable description of the pack. |

\* `extensions` is required unless `primary_extensions` is non-empty.

### Ambiguous extensions example (C++)

```toml
[meta]
name        = "cpp"
grammar     = "crates.io:tree-sitter-cpp@^0.23"
primary_extensions   = [".cpp", ".cc", ".cxx", ".hpp"]
secondary_extensions = [".h"]
extensions           = [".cpp", ".cc", ".cxx", ".hpp", ".h"]
```

---

## Grammar strings

The `grammar` field is a reference string that maps to a compiled-in tree-sitter
grammar. The format follows the scheme `<registry>:<package>@<version>`:

| Prefix       | Meaning                        | Example                               |
|--------------|--------------------------------|---------------------------------------|
| `npm:`       | npm / Node.js grammar package  | `npm:tree-sitter-python@^0.21`        |
| `crates.io:` | Rust crate grammar             | `crates.io:tree-sitter-rust@^0.23`    |

**Important:** Mycelium must have the grammar compiled in to use it. A custom pack
whose `grammar` string does not match any compiled-in grammar is silently skipped
with a `WARN` log message. The grammar string is matched by substring: any string
containing `"tree-sitter-python"` maps to the Python grammar regardless of registry
or version.

Currently compiled-in grammars: `tree-sitter-python`, `tree-sitter-javascript`,
`tree-sitter-typescript`, `tree-sitter-rust`, `tree-sitter-go`, `tree-sitter-java`,
`tree-sitter-c`, `tree-sitter-cpp`, `tree-sitter-c-sharp`, `tree-sitter-ruby`.

---

## queries.scm reference

The `queries.scm` file uses
[tree-sitter query syntax](https://tree-sitter.github.io/tree-sitter/using-parsers/queries)
with Mycelium's capture naming conventions:

| Capture name             | Edge / node created          | Notes |
|--------------------------|------------------------------|-------|
| `@definition.module`     | Module node                  | One per file |
| `@definition.function`   | Function node + Containment  | |
| `@definition.class`      | Class node + Containment     | |
| `@definition.method`     | Method node + Containment    | |
| `@reference.call`        | `Calls` edge                 | `@name` = callee name |
| `@reference.import`      | `Imports` edge (module-level)| `@name` = module name |
| `@reference.import_from` | `Imports` edge (from-import) | `@name` = module name |
| `@reference.extends`     | `Extends` edge               | |
| `@reference.implements`  | `Implements` edge            | |
| `@reference.alias_binding`| Alias table entry           | `@alias.local` + `@alias.original_name` |

See the bundled packs under `packs/` for full examples of each capture pattern.

---

## End-to-end example

### 1. Write the pack

```sh
mkdir -p my-packs/mylang
```

`my-packs/mylang/pack.toml`:
```toml
[meta]
name       = "mylang"
extensions = [".ml"]
grammar    = "npm:tree-sitter-python@^0.21"   # reuse Python grammar for testing
description = "My custom language pack."
```

`my-packs/mylang/queries.scm` — copy from `packs/python/queries.scm` as a starting
point and adapt the patterns for your language's grammar.

### 2. Index with the custom pack

```sh
mycelium index --packs-dir my-packs /path/to/project
```

Files with `.ml` extensions are now parsed through the Python grammar and your
custom queries. All other extensions fall back to the bundled packs.

### 3. Verify

```sh
mycelium server-status --root /path/to/project
```

The `files_indexed` count should include your custom-extension files.

---

## Registry API (Rust)

For Rust code that embeds the Mycelium engine:

```rust
use mycelium_pack::PackRegistry;

let registry = PackRegistry::load(Path::new("my-packs"))?;
if let Some(pack) = registry.lookup_by_ext(".ml") {
    println!("Found pack: {}", pack.name());
}
```

`PackRegistry` is available in `mycelium-rcig-pack` on crates.io.
