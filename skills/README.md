# Mycelium Skills

This directory holds the **Skill** half of the [1:1:1 rule](../rfcs/0090-cli-mcp-skill-parity.md): every Mycelium feature ships as a CLI command, an MCP tool, **and** a Skill bundle teaching agents when to call the MCP tool and how to interpret the result.

Skills are Claude Code skill bundles — `SKILL.md` with YAML frontmatter, optional `examples/`, and `tests/` for parity assertions. They are installable directly (`/skills install <path>`) or as a plugin marketplace entry.

## Layout

```
skills/
├── README.md              ← you are here
├── _template/             ← copy this when adding a new skill
│   ├── SKILL.md
│   ├── examples/
│   │   └── basic.md
│   └── tests/
│       └── parity.test.json
└── <feature>/             ← one folder per CLI subcommand
    ├── SKILL.md
    ├── examples/
    └── tests/
```

## The 1:1:1 invariant

For every feature `<feat>`:

| Surface | Location | Identifier |
|---|---|---|
| CLI | `crates/mycelium-cli/src/<feat>.rs` | `mycelium <feat>` |
| MCP | `crates/mycelium-mcp/src/tools/<feat>.rs` | tool name `<feat>` (snake_case) |
| Skill | `skills/<feat>/SKILL.md` | folder name = CLI subcommand |

These four parity invariants are CI-enforced (see `.github/workflows/parity.yml`):

1. **Name parity** — folder name = CLI subcommand = MCP tool name (mod case).
2. **Description parity** — one-line `description` is byte-identical across CLI `--help`, MCP tool schema, and SKILL.md frontmatter.
3. **Argument parity** — required CLI args = required MCP fields, all documented in SKILL.md.
4. **Output parity** — CLI `--format=json` = MCP structured response, byte-for-byte modulo timestamps. `tests/parity.test.json` asserts one input pair minimum.

## Adding a new skill

```bash
# 1. Copy the template
cp -r skills/_template skills/my-feature

# 2. Edit SKILL.md frontmatter: name, description, allowed-tools
# 3. Write the when-to-invoke prose
# 4. Add 2+ worked examples under examples/
# 5. Add at least one parity test under tests/
# 6. Verify locally:
cargo run -p mycelium-cli -- parity-check skills/my-feature
```

## Reference

- [Charter §5.13](../CHARTER.md#513--the-111-rule-feature-parity-across-cli--mcp--skill) — the rule
- [RFC-0090](../rfcs/0090-cli-mcp-skill-parity.md) — full design
- [ADR-0007](../docs/adr/0007-cli-mcp-skill-parity.md) — architectural decision
- [Claude Code skill spec](https://docs.claude.com/en/docs/claude-code/skills) — upstream format
