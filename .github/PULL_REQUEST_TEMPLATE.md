<!--
Thanks for contributing to Mycelium! 🍄
Please fill out the sections below. PRs without these fields may be auto-labeled `needs-attention`.
-->

## Summary

<!-- One paragraph: what does this PR do and why? -->

## Type of Change

<!-- Pick one (delete the others) -->

- `feat:` — new user-facing feature
- `fix:` — bug fix
- `docs:` — documentation only
- `refactor:` — no behavior change
- `perf:` — performance improvement
- `test:` — test coverage / test infra
- `build:` / `ci:` — build or CI configuration
- `chore:` — maintenance, dependencies, tooling
- `meta:` — governance, charter, RFC, ADR

## RFC / Issue Reference

<!-- Required for `feat`, `perf`, breaking changes. Optional for `fix`. -->

- Implements: RFC-XXXX §_._
- Closes: #NN
- Refs: #NN

## Changes

<!-- Bullet list of meaningful changes -->

-
-
-

## Tests

<!-- Per Charter §5.1: tests must precede implementation. Describe the test changes. -->

- [ ] Added or updated unit tests
- [ ] Added or updated integration tests
- [ ] Added or updated property tests (proptest)
- [ ] Added or updated snapshot tests (insta)
- [ ] Added or updated benchmarks (criterion)
- [ ] Added or updated e2e tests
- [ ] N/A — pure docs / chore change

## Quality Gates

- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` clean
- [ ] `cargo test --all` green
- [ ] `cargo llvm-cov` shows ≥ 90% line coverage on changed crates (or `// coverage:skip` justified)
- [ ] `cargo deny check` clean
- [ ] `cargo audit` clean

## Breaking Changes

<!-- If this PR introduces a breaking change, describe it here and reference the RFC migration plan. -->

- [ ] No breaking changes
- [ ] Yes — see RFC-XXXX §migration

## Performance Impact

<!-- Required for any PR touching crates/mycelium-core/. Post benchmark deltas. -->

| Benchmark | Before | After | Δ |
|---|---|---|---|
|  |  |  |  |

## Documentation

- [ ] Rustdoc updated for changed public items
- [ ] mdbook chapter updated (if user-visible)
- [ ] `CHANGELOG.md` Unreleased section updated
- [ ] README updated (if surface-level)

## Compliance

- [ ] PR targets `develop` (not `main`)
- [ ] Every commit signed with DCO (`git commit -s`)
- [ ] Conventional Commits format
- [ ] No forbidden dependencies introduced
- [ ] Language-pack PRs: ≤ 3 files in `packs/<lang>/`, 0 changes in `crates/`

## Notes for the Reviewer

<!-- Anything the reviewer should pay extra attention to? -->
