# Worked example: find a symbol and inspect it

**Scenario:** A user pastes "I'm getting `AuthService::login` to fail" and asks for context.

**Step 1 — locate every candidate by name:**

```
mcp__mycelium__search_symbol({ "query": "login", "limit": 20 })
```

Returns:
```json
[
  "src/auth/session.rs>AuthService>login",
  "src/auth/web.rs>login",
  "tests/integration/auth.rs>login_smoke_test>login"
]
```

**Step 2 — pick the right one (the `AuthService` method) and inspect it:**

```
mcp__mycelium__get_symbol_info({ "path": "src/auth/session.rs>AuthService>login" })
```

Returns:
```json
{
  "path": "src/auth/session.rs>AuthService>login",
  "kind": "method",
  "language": "rust",
  "file": "src/auth/session.rs",
  "start_line": 42,
  "end_line": 87,
  "parents": ["src/auth/session.rs>AuthService", "src/auth/session.rs"],
  "in_calls": 12,
  "out_calls": 5,
  "in_imports": 0,
  "out_imports": 0
}
```

**Step 3 — what else is on this class?**

```
mcp__mycelium__get_siblings({ "path": "src/auth/session.rs>AuthService>login" })
```

Returns the other methods on `AuthService`: `logout`, `refresh_token`, `is_authenticated`.

**Follow-up:** Now the agent knows the method's location and has the file/line range. If the user wants to know "who calls login", graduate to the `call-graph` Skill. If the user wants to read the source, use `get_source_span` to fetch the file+lines and pass to a file reader.
