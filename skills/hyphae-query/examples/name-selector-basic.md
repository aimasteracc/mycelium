# Basic example: find every symbol named `login`

**Scenario:** The user pastes a stack trace and asks "where is `login` defined?". The codebase is large; ripgrep returns 40 matches across tests, fixtures, and source. The agent wants only the symbol declarations.

**Agent reasoning:** A name-selector Hyphae query is exactly the right shape — one round trip to MCP returns *all* declared symbols named `login`, ordered by path.

**Invocation:**

```
mcp__mycelium__query({ "expr": "#login" })
```

**Expected response:**

```json
{
  "matches": [
    "src/auth/session.rs>AuthService>login",
    "src/auth/web.rs>login",
    "tests/integration/auth.rs>login_smoke_test>login"
  ],
  "count": 3
}
```

**Follow-up:** The agent typically calls `mycelium_get_symbol_info` on each match to inspect signatures, then picks the right one based on the user's context.

**Equivalent CLI:**

```
$ mycelium query "#login"
src/auth/session.rs>AuthService>login
src/auth/web.rs>login
tests/integration/auth.rs>login_smoke_test>login
```
