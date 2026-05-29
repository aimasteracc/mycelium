# Worked example: are there circular imports in this codebase?

**Scenario:** A user inherited a Python project that "sometimes fails to start" and suspects circular imports.

**Step 1 — fast cycle scan:**

```
mcp__mycelium__detect_cycles({ "edge_kind": "imports", "limit": 100 })
```

Returns:
```json
{
  "cycles": [
    ["src/auth/session.py", "src/db.py", "src/auth/session.py"],
    ["src/api/handlers.py", "src/middleware.py", "src/api/handlers.py"]
  ],
  "count": 2
}
```

Two cycles. The first is a 2-cycle (mutual import), the second is also a 2-cycle. The agent surfaces these to the user as concrete files to fix.

**Step 2 — get the full SCC picture:**

```
mcp__mycelium__get_scc_groups({ "edge_kind": "imports" })
```

Useful when cycles overlap into a single bigger component — `detect_cycles` returns minimal cycles, `get_scc_groups` returns the maximal connected blob.

**Step 3 — order what *is* acyclic:**

```
mcp__mycelium__topological_sort({ "edge_kind": "imports" })
```

If the project has cycles (it does, from Step 1), this returns an error envelope: `{ "error": "graph has cycles; topological sort impossible" }`. After Step 1's cycles are fixed, this gives the build/load order.

**Step 4 — for the long-term cleanup, see the dependency layers:**

```
mcp__mycelium__get_dependency_layers({ "edge_kind": "imports" })
```

Returns the layered decomposition: layer 0 = pure leaves (no imports), layer 1 = depends only on layer 0, etc. Files high in the stack are the "leaves" everyone depends on; refactor them with care.

**Follow-up:** Combine with the `centrality` Skill — high-betweenness nodes within an SCC are the *most damaging* parts of a cycle. Break those first.
