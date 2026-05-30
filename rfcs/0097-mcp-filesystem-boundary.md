# RFC-0097 — MCP Server Filesystem Access Boundary

**Status**: Implemented  
**Author**: orchestrator  
**Date**: 2026-05-30  
**Ref**: Issue #301  

## Problem

`mycelium serve --mcp` accepts arbitrary filesystem paths via `mycelium_index_workspace` and `mycelium_load_index` with zero validation:

1. Any path can be indexed (e.g. `/etc`, `/Users`) — reads private files.  
2. `<path>/.mycelium/index.rmp` is written to arbitrary locations.  
3. Relative paths like `../../etc` are not canonicalized or rejected.  
4. Indexing large roots (`/`, `/Users`) causes unbounded CPU/disk consumption.  

Root cause: `crates/mycelium-mcp/src/lib.rs:1516` — `PathBuf::from(&req.path)` with no validation.

## Design

### Allowed-roots allowlist

Add an optional allowlist to `MyceliumServer`. When non-empty, every path-based MCP call MUST canonicalize the input and verify it is under (i.e., prefixed by) at least one allowed root. If it is not, return an MCP `is_error: true` response without touching the filesystem.

Empty allowlist = unrestricted (backward-compatible default for in-process tests).  
When launched via CLI (`mycelium serve --mcp`), the allowlist defaults to `[CWD]`.

### Public API changes

**`MyceliumServer`** gains a field `allowed_roots: Arc<Vec<PathBuf>>`.

New constructors:
```rust
// Unrestricted — for tests only
pub fn new() -> Self { ... }

// Pre-loaded root + explicit allowlist (used by CLI)
pub async fn with_root_and_allowed_roots(
    root: PathBuf,
    allowed_roots: Vec<PathBuf>,
) -> anyhow::Result<Self>
```

**`serve_stdio`** signature change:
```rust
pub async fn serve_stdio(
    root: Option<PathBuf>,
    allowed_roots: Vec<PathBuf>,
) -> anyhow::Result<()>
```

**CLI `Serve` command** gains `--allowed-roots`:
```
mycelium serve --mcp [--root <dir>] [--allowed-roots <dir>]...
```
Default when omitted: `[CWD]`.

### Validation logic (path check)

```rust
fn check_path_in_allowed_roots(
    raw: &str,
    allowed_roots: &[PathBuf],
) -> Result<PathBuf, String> {
    if allowed_roots.is_empty() {
        return Ok(PathBuf::from(raw));
    }
    let canonical = std::fs::canonicalize(raw)
        .map_err(|e| format!("path not accessible: {e}"))?;
    if allowed_roots.iter().any(|root| canonical.starts_with(root)) {
        Ok(canonical)
    } else {
        Err(format!(
            "path '{}' is outside allowed roots {:?}",
            canonical.display(),
            allowed_roots
        ))
    }
}
```

Applied in: `mycelium_index_workspace`, `mycelium_load_index`.  
Snapshot write path (`<root>/.mycelium/index.rmp`) inherits validation because it is derived from the already-validated root.

## Acceptance Criteria

- [x] `MyceliumServer` has `allowed_roots: Arc<Vec<PathBuf>>`
- [x] `serve_stdio` accepts `allowed_roots: Vec<PathBuf>` parameter
- [x] CLI `--allowed-roots` flag; defaults to `[CWD]`
- [x] `mycelium_index_workspace` rejects paths outside allowed roots with `is_error: true`
- [x] `mycelium_load_index` rejects paths outside allowed roots with `is_error: true`
- [x] Path traversal (`../../etc`) is rejected after canonicalization
- [x] Empty allowed_roots = unrestricted (tests pass unchanged)
- [x] 3 TDD tests (RED before impl, GREEN after): outside-root rejected, traversal rejected, inside-root accepted
- [x] All existing tests continue to pass
