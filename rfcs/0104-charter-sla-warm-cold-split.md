# RFC-0104: Split Charter §2 latency SLAs into warm vs cold-open budgets

- **RFC**: 0104
- **Type**: `meta` (amends the Charter — requires founder ratification per Charter §0 "Amendment process: Open a `meta` RFC")
- **Status**: Draft — **awaiting founder sign-off**
- **Author**: rust-implementer (Hive AI agent)
- **Created**: 2026-06-02
- **Decision gate**: Charter §2 (Performance SLA, *the contract*). This is the
  one blocker on the RFC-0100 redb default-flip.
- **Depends on**: [ADR-0008](../docs/adr/0008-redb-storage-engine.md) Decision 4
- **Tracking**: [#426](https://github.com/aimasteracc/mycelium/issues/426) (redb Phase 3 readiness)

---

## 1. Summary

Charter §2 currently states three latency targets as **single, unconditional**
numbers measured "on a 100k-node graph":

| Metric | Target |
|---|---|
| Cold small query (single symbol lookup) | < 5 ms |
| 3-hop graph traversal (callers, depth 3) | < 1 ms |
| Reactive re-query after file change | < 10 ms |

These numbers were written for the all-in-RAM self-built engine (RFC-0001).
RFC-0100 (founder-authorized) replaced that engine with **redb**, which is
memory-mapped: the first touch of a cold page is a **disk read**. As
**ADR-0008 Decision 4** records:

> mmap cold pages are disk reads — `<1ms` 3-hop is **physically impossible cold**.
> We measure, then commit the number to the Charter.

So the Charter §2 contract, as written, is **unsatisfiable by the very engine
the Charter §3 now mandates** — and "CI gates them" (§2 line 30) is therefore
false for the redb cold path. This RFC reconciles that by splitting the three
latency rows into a **warm (steady-state)** budget — the existing numbers, which
the engine must hit once pages are resident — and a separate, measured
**cold-open** budget.

This is the **last governance blocker** before redb can become the default
backend (RFC-0100 Phase 3): we will not flip the default against a contract the
engine cannot meet.

## 2. Proposed Charter §2 amendment

Replace the three latency rows (Charter §2 lines 34–36) with:

```
| Cold-open single-symbol lookup (first query after open, p50) | < COLD_LOOKUP ms |
| Warm single-symbol lookup (steady state, pages resident)     | < 5 ms          |
| Cold-open 3-hop traversal (first query after open, p50)      | < COLD_3HOP ms  |
| Warm 3-hop traversal (steady state)                          | < 1 ms          |
| Reactive re-query after file change (warm)                   | < 10 ms         |
```

`COLD_LOOKUP` and `COLD_3HOP` are **not guessed** — they are filled in from the
measured nightly `redb-sla-100k` numbers (the gate added in #440 / RFC-0100
Phase 3) once a few nights of data exist, and from the T1 mmap cold-page spike.
A `mycelium`-side **warm-up scan on open** (ADR-0008 Decision 4) may be added to
shrink the cold tail; if it makes cold ≈ warm, the split collapses back to the
original single numbers and this RFC is withdrawn.

The remaining §2 rows (token efficiency, coverage, heavy-graph, CI duration,
language onboarding) are **unchanged**.

## 3. Why not just keep one number

- **Keep `<1ms` cold:** impossible on mmap (ADR-0008); the engine §3 mandates
  cannot pass, so the gate is permanently red or permanently ignored — both rot
  the contract.
- **Relax `<1ms` to a single cold-tolerant number:** punishes the steady state,
  which *is* sub-ms; loses the regression signal that matters for 99% of queries.
- **Split (this RFC):** keeps the strict steady-state guarantee that defines the
  "nervous system" feel, and adds an honest, measured cold-open budget. Standard
  practice for mmap-backed stores.

## 4. What this unblocks

Once ratified and the cold numbers are committed from nightly data:

1. The `redb-sla-100k` nightly gate asserts the **warm** numbers (already does).
2. A cold-open assertion is added against `COLD_*`.
3. RFC-0100 Phase 3 can **flip redb to the default backend** and retire the
   legacy MessagePack-snapshot + journal path — the actual realization of the
   storage vision.

## 5. Acceptance criteria

- [ ] Founder ratifies the §2 split (this is a `meta` RFC; the Charter is *locked*).
- [ ] `COLD_LOOKUP` / `COLD_3HOP` filled from ≥ 3 nights of `redb-sla-100k` data.
- [ ] Charter §2 amended with the warm/cold rows + the measured cold numbers.
- [ ] `redb_sla.rs` gains a cold-open assertion against the committed cold budget.
- [ ] (Optional) warm-up-scan-on-open implemented if it materially shrinks the cold tail.

## 6. Status / next step

**Draft — needs founder decision.** Nothing in the Charter changes until ratified.
No code in this RFC; it is a governance proposal that unblocks the redb default-flip.
