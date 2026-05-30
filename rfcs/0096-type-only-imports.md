# RFC-0096: Type-Only Import Edge Kind

- **Status**: draft
- **Author(s)**: @aimasteracc (orchestrator dispatch)
- **Created**: 2026-05-30
- **Last updated**: 2026-05-30
- **Tracking issue**: #227
- **Affected source paths**:
  - `crates/mycelium-core/src/types.rs` — new `EdgeKind` variant
  - `crates/mycelium-core/src/extractor/mod.rs` — pass-1.5 type-block detection
  - `packs/python/queries.scm` — `(if_statement ...)` capture
  - `packs/typescript/queries.scm` — `import type {...}` capture
  - `crates/mycelium-mcp/src/lib.rs` — every `detect-cycles`-family tool: add `--edge-kind` opt-in for `TypeImports`

## Summary

Add a new `EdgeKind::TypeImports` variant alongside the existing
`Imports`. Type-only imports — Python's `if TYPE_CHECKING:` block,
TypeScript's `import type {...}`, Java's `@Deprecated`-style
annotations on import statements — create `TypeImports` edges instead
of regular `Imports` edges. `detect-cycles --edge-kind imports` (the
default) sees only the real runtime graph, eliminating the
false-positive cycles reported in #227.

## Motivation

### The bug

From #227, against tree-sitter-analyzer (1703 files):

```python
# plugins/base.py
from typing import TYPE_CHECKING
if TYPE_CHECKING:
    from ..core.analysis_engine import AnalysisRequest  # type-only, never executed
```

```python
# core/analysis_engine.py
def _ensure_plugin_manager(plugin_manager):
    from ..plugins.manager import PluginManager  # deferred (avoids real cycle)
```

`mycelium v0.1.6 detect-cycles --edge-kind imports` reports **7
cycles**. Manual analysis: **zero true runtime cycles** — the
`TYPE_CHECKING` block is a documented Python pattern explicitly
intended to break cycles cleanly.

False positives in cycle detection have the worst possible failure
mode for AI agents: the agent suggests refactoring to "fix" a cycle
that doesn't exist, breaking real working code.

### Why a new edge kind, not a flag

A `detect-cycles --include-type-imports` boolean flag would solve the
immediate `detect-cycles` case. But:

- Other graph queries (`get-imports`, `get-importers`, `page-rank`,
  `betweenness-centrality`) would silently inherit type-only imports
  in their inputs and produce equally-wrong outputs.
- A flag forces every consumer to remember to set it.
- A separate edge kind makes the type-only relationship a
  first-class citizen — visible in the graph, queryable, and opt-in
  per analysis.

The cost: one new `EdgeKind` variant + per-language extractor work.

### Cross-language scope

| Language | Type-only-import construct |
|---|---|
| Python | `if TYPE_CHECKING: from X import Y` |
| TypeScript | `import type { Foo } from 'mod'` |
| Flow | `import type Foo from 'mod'` |
| Rust | `use foo::Bar;` inside `#[cfg(test)]` or a `mod tests {}` block (loose analog) |

Python ships first (closes #227). TypeScript follows on community
demand.

## Detailed design

### New `EdgeKind` variant

```rust
// crates/mycelium-core/src/types.rs
#[derive(...)]
pub enum EdgeKind {
    Contains,
    Imports,
    TypeImports,   // NEW
    Calls,
    Extends,
    Implements,
    // ... existing variants
}
```

Wire string: `"type_imports"`. Affects `EdgeKind::parse_wire_string`,
`AsRef<str>`, `Display`, and the JSON / MessagePack serde mappings.

### Python extractor

#### Capture

```scheme
; A `TYPE_CHECKING` if-block — capture so the extractor can mark
; nested imports as type-only.
(if_statement
  condition: (identifier) @if.type_checking_cond
  consequence: (block
    [(import_statement) (import_from_statement)] @if.type_only_import))
```

Filtered at extractor-side: only matches with
`@if.type_checking_cond` text == `"TYPE_CHECKING"`.

#### Pass logic

Insert a new **Pass 1.5: type-block detection**:

```rust
let mut type_only_imports: HashSet<TreeSitterNodeId> = HashSet::new();
{
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&self.query, root, source);
    while let Some(m) = matches.next() {
        let cond_text = m.captures.iter().find_map(|c| {
            if names[c.index as usize] == "if.type_checking_cond" {
                c.node.utf8_text(source).ok()
            } else { None }
        });
        if cond_text != Some("TYPE_CHECKING") { continue; }
        for c in m.captures.iter() {
            if names[c.index as usize] == "if.type_only_import" {
                type_only_imports.insert(c.node.id());
            }
        }
    }
}
```

#### Pass 2 dispatch

When the existing `reference.import` / `reference.import_from`
handler creates an edge, check if its anchor node is in
`type_only_imports`. If yes, use `EdgeKind::TypeImports`; otherwise
`EdgeKind::Imports`.

### TypeScript extractor (Phase 2)

```scheme
; `import type { Foo } from 'mod'` — type-only by definition
(import_statement
  "type"
  source: (string) @ts.type_only_module)

; `import { type Foo } from 'mod'` — partial type-only (TS 4.5+)
; treated as type-only edge per imported binding
```

Same downstream logic.

### Tool-side changes

- `detect-cycles --edge-kind=imports` (default) — sees `Imports` only.
- `detect-cycles --edge-kind=type_imports` — opt-in, type-only graph.
- `detect-cycles --edge-kind=all_imports` — convenience alias for
  union of both (rarely useful but symmetric).

Same pattern for `get-imports`, `get-import-tree`, `get-importers-tree`.

### Skill catalog updates

`skills/import-graph/SKILL.md` gains a documented note: cycle
detection by default excludes type-only imports because they're
runtime-no-op by design; opt in with `--edge-kind=type_imports` only
when investigating type-relationship cycles.

## Drawbacks

- **New EdgeKind costs storage.** Synapse adjacency lists are per
  EdgeKind, so adding one allocates a new adjacency. Negligible for
  this case (type-only imports are 1-5% of total imports in real
  codebases), but worth measuring with a Criterion bench.

- **Edge-kind discoverability.** Now consumers must know whether
  they want `imports` or `type_imports`. The default (`imports`) is
  the right one for ~95% of cases; the rest are documented in the
  Skill catalog.

- **Cross-language consistency.** Python's `TYPE_CHECKING` and TS's
  `import type` have similar semantics but different syntax. The
  Three-Surface Rule says we ship the abstraction consistently
  across languages — Python first, TS Phase 2.

## Alternatives

1. **Boolean flag on each tool.** Rejected — pushes the burden to
   consumers; doesn't fix downstream metrics.

2. **Track in a different store.** Rejected — splits the graph,
   making cross-edge queries hard.

3. **Heuristic: silently exclude imports inside `if` blocks named
   `TYPE_CHECKING`.** Fragile (what about
   `if TYPE_CHECKING and SOMETHING_ELSE`?), and discards information
   downstream consumers may want.

## Acceptance criteria

- [ ] `EdgeKind::TypeImports` variant + wire string
- [ ] Python extractor: type-block detection Pass 1.5, dispatches
  `Imports` vs `TypeImports` per anchor
- [ ] Integration test: indexing `from typing import TYPE_CHECKING`
  + `if TYPE_CHECKING: from foo import Bar` produces a `TypeImports`
  edge (not `Imports`) targeting `foo>Bar`
- [ ] `detect-cycles --edge-kind=imports` excludes type-only edges
  (regression test: #227 fixture reports `0 cycles`)
- [ ] `detect-cycles --edge-kind=type_imports` includes them
- [ ] Skill catalog updates for `import-graph` family
- [ ] CHANGELOG `[Unreleased]` Added entry

## Rollout plan

Single PR introducing the EdgeKind variant + Python pack +
`detect-cycles` family update. TypeScript follows as a separate PR
once Python is proven.

Target release: **v0.2.0** (alongside the breaking-change wave).
This is technically a non-breaking addition (existing `Imports`
behaviour preserved; new `TypeImports` is purely additive), but
v0.2.0 is the natural narrative home.
