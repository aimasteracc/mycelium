# Worked example: how does HTTP handler reach the database?

**Scenario:** The user asks "I want to understand how `handle_login` ends up hitting `users::find_by_email`. Show me the path."

**Step 1 — get the concrete chain:**

```
mcp__mycelium__get_shortest_path({
  "from": "src/api/handler.rs>handle_login",
  "to": "src/db.rs>users>find_by_email",
  "edge_kind": "calls"
})
```

Returns:
```json
{
  "path": [
    "src/api/handler.rs>handle_login",
    "src/auth/session.rs>AuthService>login",
    "src/db.rs>users>find_by_email"
  ],
  "length": 3
}
```

Three hops. The agent can now narrate the chain to the user: handler → service → database.

**Step 2 (optional) — enrich each hop:**

For each path entry, the agent can pipeline to the `basic-queries` Skill:

```
mcp__mycelium__get_symbol_info({ "path": "src/auth/session.rs>AuthService>login" })
```

Gives the file/line range and kind for inline rendering.

**Step 3 (optional) — branch points:**

If the user asks "are there other paths from handler to db?", widen the search:

```
mcp__mycelium__get_reachable({
  "path": "src/api/handler.rs>handle_login",
  "edge_kind": "calls",
  "max_depth": 5
})
```

If `find_by_email` appears AND there are multiple chain candidates, the user has multiple call paths to consider — likely a caller doing manual cache lookups vs falling through to the database.

**Why not `get_callees` recursively?** You could, but the agent would have to walk the tree itself and reassemble the path. `get_shortest_path` is one call.
