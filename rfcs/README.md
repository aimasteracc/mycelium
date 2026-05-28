# Mycelium RFCs

> Substantial changes to Mycelium go through a Request for Comments (RFC)
> process. This directory holds them.

## Why RFCs

Mycelium is built TDD-style and spec-driven (Charter §5.1). Before code,
there is a contract. The RFC is that contract.

## What Needs an RFC

| Class of change | RFC required? |
|---|---|
| Bug fix | ❌ Issue + PR is enough |
| Internal refactor with no API impact | ❌ |
| New CLI flag | 🟡 Lightweight RFC if it changes behavior |
| Public API addition | ✅ Yes |
| Public API change or removal | ✅ Yes |
| Storage format change | ✅ Yes — with migration plan |
| New language pack | ❌ Issue (use the language-pack template) |
| Performance SLA amendment | ✅ Yes — requires founder approval |
| Charter amendment | ✅ Yes — `meta` RFC, founder approval |
| License change | ✅ Yes — `meta` RFC, founder approval, 30-day notice |
| New Hive agent role | ✅ Yes |
| Dependency policy change (deny.toml) | 🟡 Lightweight RFC |

## Lifecycle

```
1. Draft         — copy 0000-template.md → XXXX-title.md, PR to develop
2. Discussion    — unbounded comments on the PR
3. FCP           — Final Comment Period, 7 days, kicked off by a maintainer
4. Outcome       — Accepted | Rejected | Withdrawn
5. Implementation — opens tracking issue with rfc:XXXX label
6. Shipped       — RFC status → 'shipped' when implementation merges
```

After merge, RFCs are immutable except for status updates and clarifications
agreed in the linked discussion. To change an accepted RFC, write a new RFC
that supersedes it.

## Numbering

Sequential. Find the highest existing number, add 1, pad to 4 digits.

## Anatomy

See [`0000-template.md`](0000-template.md). Required sections:

1. Summary
2. Motivation
3. Detailed design
4. Drawbacks
5. Alternatives
6. Prior art
7. Migration
8. Testing strategy
9. Performance impact (vs Charter §2 SLAs)
10. Open questions
11. Future possibilities

## Status Values

- `draft` — in active discussion
- `accepted` — approved, awaiting implementation
- `shipped` — implemented and released
- `superseded by RFC-YYYY` — replaced
- `withdrawn` — author abandoned
- `rejected` — rejected after FCP

## Current RFCs

| # | Title | Status |
|---|---|---|
| [0001](0001-trunk-and-synapse.md) | Trunk + Synapse storage layer | draft |

## Authoring Tips (from the Spec Author agent)

- Quantify everything quantifiable
- List alternatives — this is what makes an RFC reviewable
- Predict SLA impact explicitly (Charter §2)
- Keep open questions to ≤ 5 before requesting FCP
- Cite prior art (codegraph, tree-sitter-analyzer, Salsa, rust-analyzer, etc.)

---

*A good RFC turns "maybe" into "yes" or "no", quickly.*
