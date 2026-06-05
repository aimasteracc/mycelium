# RFC-0103: Import-aware cross-file reference resolution

- **Status**: Implemented (initial target — `Extends` inheritance stubs)
- **Author(s)**: orchestrator (Hive AI agent)
- **Created**: 2026-06-01
- **Last updated**: 2026-06-05
- **Tracking issue**: [#381](https://github.com/aimasteracc/mycelium/issues/381)
- **Extends**: [RFC-0014](0014-cross-file-call-resolution.md),
  [RFC-0015](0015-watch-stub-resolution.md),
  [RFC-0092](0092-cross-language-alias-resolution.md)
- **Affected source paths**:
  - `crates/mycelium-core/src/store/` - post-index resolver
  - `crates/mycelium-core/src/extractor/` - import evidence if existing edges
    are insufficient
  - `crates/mycelium-cli/src/index.rs` - full-index integration
  - `crates/mycelium-mcp/src/lib.rs` - full-index and watch integration
  - `skills/inheritance/SKILL.md` - documentation updates for inherited lookup

## Summary

Extend Mycelium's post-index bare-stub resolver from RFC-0014 so it can resolve
ambiguous cross-file references using import evidence and deterministic ranking.
The initial target is inheritance correctness: `Extends` edges that currently
point at a bare stub such as `LanguagePlugin` should be redirected to the
qualified definition path when the referencing file imports or otherwise clearly
selects that definition.

The resolver must remain conservative. It may redirect a stub only when the
best candidate is uniquely justified by import evidence or a deterministic
high-confidence rank. Ambiguous cases stay unresolved and visible for future
work rather than being guessed.

## Motivation

RFC-0014 intentionally resolved only unambiguous bare call stubs. It explicitly
left multi-candidate import-aware resolution to a future RFC. Issue #381 is that
future work.

The real failure mode is inheritance analysis. A query such as
`subclasses-tree "LanguagePlugin"` can find many subclasses through bare-name
edges, but `subclasses-tree "plugins/base.py>LanguagePlugin"` misses them
because their `Extends` edges still target the unresolved bare node. The same
gap affects `get-descendants --include-inherited`, where methods inherited from
cross-file base classes are incorrectly treated as missing.

## Detailed design

### Resolver entry point

Add a more general post-index resolver:

```rust
pub struct CrossFileResolutionStats {
    pub references_seen: usize,
    pub resolved: usize,
    pub ambiguous: usize,
    pub unresolved: usize,
}

impl Store {
    pub fn resolve_cross_file_references(&mut self) -> CrossFileResolutionStats;
}
```

`resolve_bare_call_stubs()` becomes a compatibility wrapper around the new
resolver or delegates to its `Calls` subset. The public index response may keep
the existing `stubs_resolved` field and add `cross_file_refs_resolved` in both
CLI and MCP JSON outputs in the same PR.

### Reference classes

The first implementation covers bare-stub targets of these edge kinds:

- `Calls`
- `Extends`
- `Implements`

`Imports` and `TypeImports` are evidence for resolution, not targets to
redirect in the first phase.

### Candidate discovery

For a bare stub path `LanguagePlugin`:

1. Find definition candidates whose final path segment equals the stub name.
2. Find incoming edges to the stub. Each incoming source gives a referencing
   symbol and a referencing file.
3. Resolve each incoming edge independently, because different files may import
   the same bare name from different modules.

This is a key difference from RFC-0014: a single bare stub may need to be
replaced by different qualified definitions depending on the source edge.

### Import evidence

The resolver first attempts to use evidence already present in the graph:

- `Imports` / `TypeImports` edges from the referencing file or a symbol in that
  file to a candidate file/module
- alias-resolved call/import paths produced by RFC-0092
- same-file definitions, which always outrank cross-file candidates

If existing import edges are insufficient for a required RED test, the
implementation may add a minimal derived `ImportBinding` structure:

```rust
pub struct ImportBinding {
    pub file_path: String,
    pub local_name: String,
    pub resolved_prefix: String,
    pub is_type_only: bool,
}
```

Adding persistent import bindings is a storage/data-structure decision. If that
path is chosen, the implementation PR must include either a new ADR or an
explicit update to the governing storage ADR, plus `#[serde(default)]` migration
coverage for legacy MessagePack snapshots and a redb table plan for RFC-0100.

### Ranking

When import evidence produces no exact match, rank candidates deterministically:

1. Same file.
2. Imported exact symbol path.
3. Imported module/file containing the candidate.
4. Same directory as the referencing file.
5. Longest shared package/path prefix.
6. Shortest path distance.
7. Lexicographic path tie-breaker.

Only ranks 1-3 are high-confidence by default. Ranks 4-7 may resolve only when
there is exactly one candidate after applying the strongest available rank. If
two candidates remain tied, leave the reference unresolved.

### Edge rewrite semantics

Do not redirect the whole stub node globally when multiple source files point to
it. Instead, rewrite the individual incoming edge:

```text
source --Extends--> LanguagePlugin
```

becomes:

```text
source --Extends--> plugins/base.py>LanguagePlugin
```

After all incoming edges are processed, remove the bare stub only if it has no
remaining incoming or outgoing edges. This prevents one file's import evidence
from incorrectly changing another file's unresolved reference.

### Integration points

Run the resolver:

- after full CLI indexing
- after MCP `mycelium_index_workspace`
- after each watch-mode batch, after changed files are merged into the store

The watch path must be careful not to make stale import evidence sticky. If a
changed file's imports change, any derived import evidence for that file must be
recomputed or discarded before resolution.

## Drawbacks

- Conservative resolution means some real references will remain unresolved.
  That is preferable to false edges that corrupt inheritance answers.
- Per-edge rewriting is more complex than RFC-0014's global stub redirect.
- Import semantics differ by language. The first implementation should target
  Python inheritance because #381's concrete failure is Python plugin classes,
  then extend through language-pack-specific tests.

## Alternatives

1. **Keep RFC-0014 exact-single-candidate behavior.** Rejected because it leaves
   the known inheritance gap unresolved.
2. **Always choose the closest path.** Rejected because large repos often have
   repeated class names in sibling packages; path distance alone creates false
   edges.
3. **Persist full unresolved_refs as a new table immediately.** Deferred. The
   first implementation should prove whether existing graph evidence is enough
   before adding storage surface.
4. **Resolve at query time instead of index time.** Rejected for inheritance
   queries because every caller would need to duplicate the same heuristic and
   cache invalidation logic.

## Prior art

- RFC-0013: two-pass extraction for intra-file forward references.
- RFC-0014: unambiguous cross-file bare call stub resolution.
- RFC-0015: watch-mode invocation of the RFC-0014 resolver.
- RFC-0092: alias table pass for import alias call resolution.
- Language-server symbol resolution: import-aware lookup with conservative
  ambiguity handling.

## Migration

If the implementation uses only existing graph edges, there is no index-format
migration. Re-indexing improves edge quality.

If a new `ImportBinding` field or table is introduced:

- legacy MessagePack snapshots must load with `#[serde(default)]`
- redb storage must either derive import bindings during migration or treat them
  as rebuildable metadata
- CHANGELOG must document that re-indexing is recommended for best inheritance
  accuracy

## Testing strategy

Tests must be written RED-first.

- Unit tests:
  - per-edge rewrite does not globally redirect a shared bare stub
  - tie-ranked candidates remain unresolved
  - same-file definitions outrank imports
- Integration tests:
  - `subclasses-tree "LanguagePlugin"` and
    `subclasses-tree "plugins/base.py>LanguagePlugin"` return the same set on a
    fixture with multiple plugin subclasses
  - `get-descendants --include-inherited` sees inherited methods through a
    cross-file base class
  - import evidence beats same-directory ranking
  - watch-mode batch resolves a newly added base class without a full re-index
- Regression:
  - RFC-0014 single-candidate call-stub behavior remains green
  - ambiguous multi-candidate stubs remain present and counted

## Performance impact

| SLA | Current | After this RFC | Delta |
|---|---|---|---|
| Cold query | < 5 ms | unchanged after index | no query-time resolver |
| 3-hop traversal | < 1 ms | unchanged | better edges, same traversal |
| Reactive refresh | < 10 ms target | resolver runs after watch batch | must stay within 20 percent on changed-file fixtures |
| Token efficiency | unchanged | unchanged | no output-format change |

Full-index resolver cost should be near-linear in the number of bare stubs plus
incoming edges to those stubs. If watch-mode cost exceeds the reactive refresh
target on large fixtures, the implementation must add a file-scoped resolver
path before enabling it in watch mode.

## Acceptance criteria

- [ ] RFC accepted before implementation starts.
- [ ] `Store::resolve_cross_file_references()` handles `Calls`, `Extends`, and
      `Implements` bare-stub targets conservatively.
- [x] Per-edge rewrites allow one bare stub to resolve differently for different
      source files. *(Issue #555: `Synapse::remove_edge` + per-edge
      `resolve_import_aware_extends_stubs` implemented)*
- [ ] `subclasses-tree "LanguagePlugin"` and
      `subclasses-tree "plugins/base.py>LanguagePlugin"` return the same fixture
      result set.
- [ ] `get-descendants --include-inherited` includes inherited methods from a
      cross-file base class.
- [ ] Full CLI index, MCP index, and watch-mode batch all run the resolver.
- [ ] Ambiguous ties remain unresolved and visible in stats.
- [ ] At least five RED-first tests cover import evidence, ranking, ambiguity,
      per-edge rewrites, and watch integration.
- [ ] If new import-binding storage is introduced, an ADR/update plus legacy
      snapshot migration tests ship in the same PR.
- [ ] Quality gate remains green.

## Open questions

1. Are existing `Imports` and `TypeImports` edges sufficient evidence for the
   Python fixture, or is a derived `ImportBinding` required?
2. Should the first implementation include JavaScript/TypeScript inheritance
   fixtures, or keep the scope Python-first?
3. Should unresolved/ambiguous counts be exposed in `server_status`, or only in
   index responses?

## Future possibilities

- ~~**Per-edge mixed-site resolution (follow-up to the initial Extends target).**~~
  **IMPLEMENTED (Issue #555).** `AdjacencyList::remove_edge`, `Synapse::remove_edge`,
  and `Store::remove_edge` added. `resolve_import_aware_extends_stubs` now resolves
  each `(subclass → stub)` Extends edge independently; the stub is removed only
  after all incoming edges are redirected. Mixed-import sites (different subclasses
  importing different defs) are now fully handled.
- Persist an `unresolved_refs` diagnostic table for agents to inspect directly.
- Extend ranking with language-pack-specific import resolvers.
- Use RFC-0103's improved edges as higher-quality input for RFC-0101
  `mycelium_context`.

