# RFC-NNNN: <Title>

- **Status**: draft
- **Author(s)**: @username
- **Created**: YYYY-MM-DD
- **Last updated**: YYYY-MM-DD
- **Tracking issue**: TBD
- **Affected source paths** (pin them — Doc Sync watches drift here):
  - `crates/...`
  - `packs/...`

## Summary

One paragraph. What is this RFC proposing, in plain language?

## Motivation

Why are we doing this? What problem are we solving? Who feels the pain
today? What outcome do we want?

Be concrete. Numbers help.

## Detailed design

The bulk of the RFC. Specify the change such that it can be implemented
by someone other than you.

For an API change: include the proposed API in Rust syntax, with rustdoc.
For a storage change: include the proposed schema and a migration plan.
For a query language change: include grammar in BNF/EBNF + examples.
For a Hive change: include the proposed agent role markdown.

### Sub-sections as needed

#### Data structures

#### Algorithms

#### Public API

#### Error handling

#### Concurrency

## Drawbacks

What are the costs? Why might we not do this? Be honest. RFCs without
drawbacks are usually under-considered.

## Alternatives

What other approaches were considered? Why are they not the proposal?

- **Alternative A**: ...
  - Pros: ...
  - Cons: ...
  - Rejected because: ...

- **Alternative B**: ...

## Prior art

What can we learn from? Cite:

- Other code-intelligence systems (codegraph, rust-analyzer, Sourcegraph, …)
- Academic papers
- Other projects (Salsa, tree-sitter, Neo4j, DuckDB, …)

Link them. Compare. Note where we adopt their patterns and where we diverge.

## Migration

If this is a breaking change:

- How will existing users be affected?
- How will existing `.myc` indexes be migrated?
- What is the deprecation timeline?

If non-breaking: state so explicitly.

## Testing strategy

Per Charter §5.1: tests precede implementation. List the tests this RFC will
require:

- Unit tests: ...
- Integration tests: ...
- Property tests (proptest): ...
- Snapshot tests (insta): ...
- Benchmarks (criterion): ...
- Fuzz targets: ...
- E2E: ...

Acceptance criterion: "when all listed tests pass and benchmark SLAs are
met, this RFC is implemented."

## Performance impact

For every Charter §2 SLA, predict the impact:

| SLA | Current | After this RFC | Δ |
|---|---|---|---|
| Cold query | < 5 ms | < ?? ms | ?? |
| 3-hop traversal | < 1 ms | < ?? ms | ?? |
| Reactive refresh | < 10 ms | < ?? ms | ?? |
| Token efficiency | ≤ 30% JSON | ≤ ??% JSON | ?? |

If you cannot predict, say so and propose a benchmark plan as part of the
implementation.

## Open questions

Numbered list. Each should be answerable; if not, this RFC is not ready
for FCP.

1. ?
2. ?

## Future possibilities

What might this RFC enable later that is not part of its current scope?
Note them so future RFCs can reference back.

---

*Once accepted, this RFC is the contract. Update it via a superseding RFC.*
