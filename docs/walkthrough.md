# First 5 Minutes with Mycelium

A copy-paste walkthrough from zero to your first code intelligence query.

**Time**: 5 minutes  
**Prerequisites**: Rust 1.75+, a git repository containing Python, TypeScript, JavaScript, Rust, Go, Java, C, C++, C#, or Ruby source files

---

## Step 1: Install (30 seconds)

```bash
cargo install mycelium-rcig-cli
```

This installs the `mycelium` binary from crates.io. The crate is published under the `mycelium-rcig-*` prefix because the short names were taken by unrelated projects.

Prefer installing from source for the latest build:

```bash
cargo install --git https://github.com/aimasteracc/mycelium mycelium-rcig-cli
```

Verify it worked:

```bash
mycelium version
# mycelium 0.1.4
```

---

## Step 2: Index your first project (1 minute)

Point Mycelium at any directory with source code. Use your own project or clone something you know:

```bash
cd /path/to/your-project
mycelium index .
```

Expected output:

```
Indexing /path/to/your-project …
Done.  147 file(s) indexed, 0 error(s).
Index saved to .mycelium/index.rmp
```

The index is a MessagePack snapshot written to `.mycelium/index.rmp`. Every subsequent command in this directory reads from that snapshot — no network, no daemon. Add `.mycelium/` to your `.gitignore` if you do not want to commit it.

The bundled grammars cover Python (`.py`, `.pyi`), TypeScript (`.ts`, `.tsx`), JavaScript (`.js`, `.jsx`), Rust (`.rs`), Go (`.go`), Java (`.java`), C (`.c`, `.h`), C++ (`.cpp`, `.cc`, `.cxx`, `.hpp`), C# (`.cs`), and Ruby (`.rb`). Files with other extensions are skipped silently.

---

## Step 3: Your first query — find a symbol (1 minute)

Search for any symbol by name fragment. The match is case-insensitive against the final path segment:

```bash
mycelium search-symbol authenticate
```

Expected output (one full symbol path per line):

```
src/auth/session.py>AuthService>authenticate
src/auth/utils.py>authenticate_token
tests/test_auth.py>TestAuth>test_authenticate_valid
```

Symbol paths use `>` as a separator: `file>Class>method` or `file>function`.

Pick one result and get its full structural summary in a single call:

```bash
mycelium get-symbol-info "src/auth/session.py>AuthService>authenticate"
```

Expected output:

```
path:     src/auth/session.py>AuthService>authenticate
kind:     method
language: python
file:     src/auth/session.py
lines:    42–67
parent:   src/auth/session.py>AuthService
callers:  3
callees:  5
```

Use `--format=json` on any subcommand to get structured output suitable for scripts or further processing.

---

## Step 4: Follow the call graph (1 minute)

Find everything that calls `authenticate`:

```bash
mycelium get-callers "src/auth/session.py>AuthService>authenticate"
```

```
src/api/routes.py>login_view
src/cli/main.py>cli_login
src/middleware/auth.py>AuthMiddleware>process_request
```

Find everything that `authenticate` itself calls:

```bash
mycelium get-callees "src/auth/session.py>AuthService>authenticate"
```

```
src/db/user_repository.py>UserRepository>find_by_email
src/crypto/hashing.py>verify_password
src/auth/tokens.py>issue_token
src/audit/logger.py>log_auth_attempt
src/cache/session_store.py>SessionStore>write
```

Go deeper with a recursive tree (default depth 3):

```bash
mycelium get-callee-tree "src/auth/session.py>AuthService>authenticate" --max-depth 3
```

This returns a nested tree of every function `authenticate` transitively depends on, with cycle detection. Useful before a refactor: you see the full blast radius at a glance.

To find the call path between two specific symbols:

```bash
mycelium find-call-path \
  --from "src/api/routes.py>login_view" \
  --to "src/crypto/hashing.py>verify_password"
```

---

## Step 5: Connect to your AI assistant (2 minutes)

Running Mycelium as an MCP server lets Claude (or any MCP-compatible client) query the symbol graph directly in conversation.

Start the server against your indexed project:

```bash
mycelium serve --mcp --root /path/to/your-project
```

The server speaks JSON-RPC over stdio and exposes 93 tools — one for every CLI subcommand, with identical names, arguments, and output (plus 3 RFC-0107/0108 SUBSCRIBE tools that are MCP-only by EXCEPTION per Charter §5.13).

### Add to Claude Desktop

Open `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or the equivalent on your platform, and add a `mycelium` entry under `mcpServers`:

```json
{
  "mcpServers": {
    "mycelium": {
      "command": "mycelium",
      "args": ["serve", "--mcp", "--root", "/path/to/your-project"]
    }
  }
}
```

Restart Claude Desktop. The Mycelium tools appear in Claude's tool list automatically.

### Add to VS Code (Copilot MCP)

In your `.vscode/mcp.json`:

```json
{
  "servers": {
    "mycelium": {
      "type": "stdio",
      "command": "mycelium",
      "args": ["serve", "--mcp", "--root", "${workspaceFolder}"]
    }
  }
}
```

### Try it in Claude

Once connected, ask Claude questions about your codebase in plain English:

- "What calls the `authenticate` function?"
- "What does `UserRepository.find_by_email` transitively call?"
- "Are there any dead functions in `src/payments/`?"
- "What are the entry points of this project?"

Claude resolves these using `get_callers`, `get_callee_tree`, `get_dead_symbols`, and `get_entry_points` — the same operations you ran from the terminal in steps 3 and 4.

---

## What's next

**Skills** — prebuilt tool bundles for common workflows:

- `skills/basic-queries/SKILL.md` — symbol lookup, source spans, structural inventory
- `skills/call-graph/SKILL.md` — callers, callees, dead code, entry points
- Browse all skills under `skills/` for import graphs, inheritance, centrality, and more

**Language packs** — adding a new language requires two files and zero core changes:

- `docs/packs.md` — full pack authoring reference

**Governance and architecture:**

- `CHARTER.md` — the project constitution; read this before contributing
- `rfcs/` — every design decision is recorded here
- [GitHub repository](https://github.com/aimasteracc/mycelium) — issues, PRs, and the public roadmap

**Full CLI reference:**

```bash
mycelium --help
mycelium <subcommand> --help
```

Every subcommand accepts `--format=json` for machine-readable output and `--root <dir>` to point at a project directory other than the current one.
