# RFC-0009 — Gitignore-aware Indexing and Watch Filtering

| Field      | Value                       |
|------------|-----------------------------|
| RFC        | 0009                        |
| Title      | Ignore filtering            |
| Status     | Implemented                       |
| Author     | Hive / rust-implementer     |
| Created    | 2026-05-29                  |

---

## Motivation

Without ignore filtering, `mycelium index .` in a Node.js project walks
`node_modules/` (potentially millions of files) and `mycelium serve --mcp
--root .` tries to index all of them.  The watch loop fires on every
`npm install`.

This RFC adds `.gitignore`-aware filtering to both the indexing walk and
the watch event pipeline.

---

## Design

### Dependency

Add `ignore = "0.4"` to the workspace.  This is the same library used
by ripgrep and fd — battle-tested, zero-unsafe, cross-platform.

`ignore::WalkBuilder` is a drop-in upgrade from `walkdir::WalkDir` that
respects:
- `.gitignore` files at every directory level
- `.ignore` files (ripgrep-style)
- `.myceliumignore` at the workspace root (treated as an extra git-like
  ignore file)
- Platform-specific hidden-file conventions

### Indexing walk

Replace `walkdir::WalkDir::new(root)` with `ignore::WalkBuilder::new(root)`
in both the MCP `run_index` helper and the CLI `index_path` function.

Hard-coded additional always-ignore patterns (regardless of `.gitignore`):
- `.mycelium/` — snapshot directory
- `target/` — Rust build artifacts (also usually in `.gitignore`)

### Watch event filtering

Watcher events deliver absolute paths.  Before processing, check each path
against an `ignore::gitignore::Gitignore` matcher built from the root's
`.gitignore` + the hard-coded extras.  Skip any path that matches.

---

## Acceptance criteria

| # | Criterion |
|---|-----------|
| 1 | `node_modules/` is not indexed when a `.gitignore` containing it exists |
| 2 | `target/` is always skipped even without `.gitignore` |
| 3 | `.mycelium/` (snapshot dir) is always skipped |
| 4 | `.myceliumignore` is honoured as a custom ignore file |
| 5 | Files not matching ignore rules continue to be indexed correctly |
| 6 | Watch events for ignored paths are silently dropped |
| 7 | All existing 136 tests continue to pass |

---

## Testing strategy

- Unit: index a temp dir containing a `node_modules/` subdirectory with a
  matching `.gitignore`; verify no `node_modules/` nodes in the store.
- Unit: index without `.gitignore`; verify `target/` is still skipped.
- Unit: `.myceliumignore` pattern skips its matching directory.
- Unit: unignored files are still indexed after adding ignore rules.

---

## Non-goals

- Custom glob UI / configuration file beyond `.myceliumignore`.
- Ignoring individual symbol paths (the ignore layer operates on files).
