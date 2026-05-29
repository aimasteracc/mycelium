# Worked example: find a circular import

**Scenario:** Build is failing with "cyclic dependency detected" and the user wants to find the loop.

**Step 1 — pick a likely-involved file (from the error message) and walk its import tree:**

```
mcp__mycelium__get_import_tree({
  "path": "src/auth/session.rs",
  "max_depth": 8
})
```

If `session.rs` is in a cycle, the result will include a leaf marked `{ "path": "...", "cycle": true }`. The path on that leaf names the cycle's re-entry point.

**Step 2 — confirm from the other direction:**

```
mcp__mycelium__get_importers_tree({
  "path": "src/db.rs",       // the suspected other end of the cycle
  "max_depth": 8
})
```

If `session.rs` appears here AND `db.rs` appears in `session.rs`'s import tree, you've found the loop: `session.rs → db.rs → ... → session.rs`.

**Step 3 — break the cycle.** The fix depends on the language and the architecture: extract a shared types module, lazy-import inside a function, etc. Mycelium just identifies the loop; the design call is yours.

**Follow-up:** After breaking the cycle, re-index and re-run `get_import_tree` to confirm no `cycle: true` leaves remain on the affected files.
