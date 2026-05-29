# Worked example: blast-radius before a refactor

**Scenario:** The user wants to rename `AuthService::login` to `AuthService::authenticate` and asks "what will I break?"

**Step 1 — direct callers:**

```
mcp__mycelium__get_callers({ "path": "src/auth/session.rs>AuthService>login" })
```

Returns:
```json
{
  "callers": [
    "src/api/routes.rs>handle_login",
    "src/cli/main.rs>cli_login",
    "tests/integration/auth.rs>test_login_happy_path"
  ],
  "count": 3
}
```

Three direct call sites. Fast change.

**Step 2 — transitive blast radius (depth 3):**

```
mcp__mycelium__get_caller_tree({
  "path": "src/auth/session.rs>AuthService>login",
  "max_depth": 3
})
```

Returns a nested tree. If a leaf is `{ ..., "cycle": true }`, the path back into `login` is part of a recursion or a mutual-call loop — flag for special attention.

**Step 3 — confirm the symbol isn't an entry point** (deletions of entry points are usually mistakes):

```
mcp__mycelium__get_entry_points({ "limit": 1000 })
```

If `AuthService>login` appears in the list, it's reachable from "outside" (CLI command, HTTP handler, test framework — depends on the project). Refactor with that in mind.

**Follow-up:** Once the rename is done, re-index and rerun `get_callers` to confirm zero hits on the old name (the index should no longer show it).

**Why a tree, not a flat set?** A flat set of transitive callers loses the structure. The tree shows you *how* each top-level caller reaches `login`, which is what you need to write the right changelog entry and migration note.
