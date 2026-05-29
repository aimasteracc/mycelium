# Advanced example: every method on a class

**Scenario:** The user asks "what does `AuthService` actually expose?" The agent could read the file, but that requires guessing the path. A kind+combinator query lands the answer directly.

**Agent reasoning:** `.class>.method` reads "any method whose direct parent is a class". Add `:has(#AuthService)` (RFC-0003 §3.5 pseudo-class) to scope to the named class.

**Invocation:**

```
mcp__mycelium__query({ "expr": "#AuthService>.method" })
```

**Expected response:**

```json
{
  "matches": [
    "src/auth/session.rs>AuthService>login",
    "src/auth/session.rs>AuthService>logout",
    "src/auth/session.rs>AuthService>refresh_token",
    "src/auth/session.rs>AuthService>is_authenticated"
  ],
  "count": 4
}
```

**Follow-up:** The agent often hands these straight back to the user, or chains to `mycelium_get_source_span` to render the actual method signatures.

**Equivalent CLI:**

```
$ mycelium query "#AuthService>.method"
src/auth/session.rs>AuthService>login
src/auth/session.rs>AuthService>logout
src/auth/session.rs>AuthService>refresh_token
src/auth/session.rs>AuthService>is_authenticated
```

**Note on pseudo-class coverage:** the executor (RFC-0004) ships incremental support for `:calls()`, `:imports()`, etc. Check `crates/mycelium-hyphae/src/evaluator.rs` for the current coverage matrix; the parser accepts the full grammar but unsupported pseudo-classes return an empty match set rather than an error.
