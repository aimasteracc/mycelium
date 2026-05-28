# Hive Memory Index

> The PM agent curates this file. Every entry points to a JSONL line or a
> cluster of them. Use grep to find specifics; use this to find the right
> file to grep in.

**Files in this directory:**

- `decisions.jsonl` — every notable decision (append-only)
- `anti-patterns.jsonl` — mistakes we promised not to repeat (append-only)
- `lessons.jsonl` — generalized learnings, often consolidated from anti-patterns
- `glossary.jsonl` — project terminology, canonical spellings, translations

## Reading Order for New Agents

If you are a new agent invocation, read in this order:

1. `CHARTER.md` (repo root)
2. `.hive/_orchestrator.md`
3. `.hive/<your-role>.agent.md`
4. **This file** — for the map of memory
5. The relevant slice of `anti-patterns.jsonl` (`grep` your domain)
6. The relevant slice of `decisions.jsonl` (recent 50 entries)

## Index by Domain

> Maintained by PM. As of Day-0, mostly empty placeholders.

### Engine architecture

- `decisions.jsonl`: search `"domain":"engine"`
- `lessons.jsonl`: search `"domain":"engine"`
- See also: rfcs/0001-trunk-and-synapse.md

### Reactivity (Salsa, dependency tracking)

- `decisions.jsonl`: search `"domain":"reactivity"`
- `anti-patterns.jsonl`: search `"domain":"reactivity"`

### Hyphae query language

- `decisions.jsonl`: search `"domain":"hyphae"`
- See also: rfcs/ (post-RFC-0001)

### Language packs

- `decisions.jsonl`: search `"domain":"pack"`
- `lessons.jsonl`: search `"agent":"pack-author"`

### CI / release process

- `decisions.jsonl`: search `"domain":"ci"` or `"agent":"release"`
- `anti-patterns.jsonl`: search `"domain":"ci"`

### Async / concurrency

- `anti-patterns.jsonl`: search `"domain":"async"`

### Security / dependencies

- `decisions.jsonl`: search `"agent":"security"`
- `lessons.jsonl`: search `"type":"advisory|ban|secret"`

## Recent Highlight Decisions (curated)

> PM updates weekly during synthesis. Empty at Day-0.

- *(none yet — the network is just sprouting)*

## Crosswalk: RFC ↔ ADR ↔ Decisions

| RFC | ADR | Key decisions referenced |
|---|---|---|
| 0001 (Trunk + Synapse) | 0001 (Rust as engine), 0002 (tree-sitter as parser) | TBD |

## Glossary Snapshot

> Pull from `glossary.jsonl` for full list. Highlights:

- **Mycelium**: the project name and the underground network metaphor
- **Hyphae**: the query language; literally, a single thread of fungal mycelium
- **Trunk**: the containment-tree storage layer (radix trie)
- **Synapse**: the cross-cutting-edge storage layer (CSR adjacency)
- **The Hive**: the team of AI agents developing Mycelium
- **Pre-flight / Post-flight**: mandatory startup / shutdown rituals for every agent invocation

---

*Memory is what makes the Hive smarter than any one of its members.*
