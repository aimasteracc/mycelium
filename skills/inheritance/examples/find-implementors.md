# Worked example: find every concrete `UserRepository` implementation

**Scenario:** The user is reading code that calls a trait method on `dyn UserRepository` and asks "which concrete struct actually runs here?"

**Step 1 — direct implementors:**

```
mcp__mycelium__implementors_tree({
  "path": "src/repo.rs>UserRepository",
  "max_depth": 3
})
```

Returns a tree:
```json
{
  "tree": {
    "path": "src/repo.rs>UserRepository",
    "children": [
      { "path": "src/db/postgres.rs>PostgresUserRepo" },
      { "path": "src/db/sqlite.rs>SqliteUserRepo" },
      { "path": "tests/fakes/user_repo.rs>FakeUserRepo" }
    ]
  }
}
```

Three concrete impls — production Postgres, dev/test SQLite, test fake.

**Step 2 — pick the production one and inspect its `find_by_email`:**

```
mcp__mycelium__get_symbol_info({ "path": "src/db/postgres.rs>PostgresUserRepo>find_by_email" })
```

(via the `basic-queries` Skill — chain across Skills is normal.)

**Step 3 (optional) — confirm runtime dispatch:**

If the user wants to know which impl runs at a specific call site, they need to look at the construction code. Find where `PostgresUserRepo` is *constructed*:

```
mcp__mycelium__get_callers({ "path": "src/db/postgres.rs>PostgresUserRepo>new" })
```

(via `call-graph`.) The callers tell you where each impl actually gets wired into the system.

**Why a tree, not a flat set?** Because Java/C# interfaces can extend other interfaces. If `UserRepository: Repository`, the tree shows you both `Repository` implementors and the `UserRepository` ones in their proper hierarchy.
