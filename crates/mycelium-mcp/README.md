# mycelium-rcig-mcp

MCP (Model Context Protocol) server for Mycelium code intelligence.

## Output format

Every query tool (tools that return data, not tools that mutate state) accepts
an optional `output_format` field that controls the response encoding.

| Value     | Description                                                        |
|-----------|--------------------------------------------------------------------|
| `"text"`  | Indented `key: value` / `- item` layout — default (see below)     |
| `"json"`  | Standard JSON (compact, machine-parseable)                         |
| `"msgpack"` | Binary MessagePack encoded as ASCII hex (smallest byte count)  |

### Default format per transport

| Transport      | Default `output_format` | Rationale                                                                  |
|----------------|------------------------|----------------------------------------------------------------------------|
| **stdio MCP**  | `text`                 | LLM agents are the primary consumer; text is token-efficient (RFC-0094 §"Why Mycelium ought to lead here") |
| **CLI**        | `json`                 | Human operators pipe CLI output through `jq`; JSON is the universal baseline |

> **Note (RFC-0094 §Rollout plan)**: the stdio MCP default will change from `json`
> to `text` in **v0.2.0** as a coordinated breaking change alongside RFC-0093.
> Until then, callers receive JSON by default if they do not pass `output_format`.
> Pass `output_format: "text"` explicitly to opt in now.

### Text format grammar

The `text` format is a TOON-inspired indented representation designed to be parsed
by both LLM agents and a small round-trip reference parser:

- **Scalar**: `key: value` (bare strings, no quotes)
- **List items**: `- item` per line, indented under the parent key
- **Empty list**: `[]`; **empty object**: `{}`
- **Reserved strings** (leading `[`, `{`, `-`, `"`, or containing `: `) are
  re-emitted as standard JSON string literals for unambiguous parsing
- **Null**: bare word `null`; **booleans**: `true` / `false`

### Examples

```
# output_format: "text" (default on stdio)
callers:
  - src/api/routes.rs>handle_login
  - src/cli/main.rs>cli_login
count: 2

# output_format: "json"
{"callers":["src/api/routes.rs>handle_login","src/cli/main.rs>cli_login"],"count":2}
```

## Transport

Start the MCP server over stdio (the standard MCP transport):

```bash
mycelium mcp --root /path/to/repo
```

The server reads from stdin and writes to stdout using the JSON-RPC framing
defined by the Model Context Protocol spec (rmcp 1.7).

## Parity

Per RFC-0090 (Three-Surface Rule), every tool exposed here is also available as a
CLI subcommand (`mycelium <tool-name>`) and covered by at least one `skills/` entry.
The tool name, description, argument schema, and JSON output are byte-identical
across CLI and MCP.

## Reference

- RFC-0094: [rfcs/0094-token-efficient-output.md](../../rfcs/0094-token-efficient-output.md)
- RFC-0090: [rfcs/0090-cli-mcp-skill-parity.md](../../rfcs/0090-cli-mcp-skill-parity.md)
- Formatter source: [src/formatter.rs](src/formatter.rs)
