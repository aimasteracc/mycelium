# Worked example: enrich a Hyphae match set with one batch call

**Scenario:** The agent ran `mycelium_query` with selector `.function:in(src/auth/)` and got back 12 matches. The user wants context for each.

**Naive approach** (12 round trips, ~12× the tokens):

```
for path in matches:
    get_symbol_info(path)
```

**Batch approach** (1 round trip):

```
mcp__mycelium__batch_symbol_info({
  "paths": [
    "src/auth/session.rs>AuthService>login",
    "src/auth/session.rs>AuthService>logout",
    "src/auth/oauth.rs>OAuth2Session>authorize",
    /* ... 9 more ... */
  ]
})
```

Returns a list of info records:

```json
[
  { "path": "src/auth/session.rs>AuthService>login",
    "kind": "method", "file": "src/auth/session.rs",
    "start_line": 42, "end_line": 87,
    "in_calls": 12, "out_calls": 5 },
  /* ... 11 more ... */
]
```

**Token math** (Charter §2 SLA target ≤ 30% of raw JSON-MCP):

- Naive: 12 × (~200 token request + ~150 token response) ≈ 4 200 tokens
- Batch: 1 × (~300 token request + ~1 500 token response) ≈ 1 800 tokens
- Reduction: ~43% → comfortably under the 30%-of-baseline target when the batch includes the full set.

**Follow-up:** The agent renders the enriched matches as a table for the user. If the user picks one for deeper inspection, the agent drops back to single-call tools (`get_callers`, `get_source_span`, etc.) — batch tools are for the *survey* step, not the *deep-dive* step.
