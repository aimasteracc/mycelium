# Mycelium Code Intelligence — GitHub Action

Run [Mycelium](https://github.com/aimasteracc/mycelium) in CI to post a
**structural code-intelligence summary** on every pull request — symbol count,
relationship count, dead symbols, isolated symbols, and entry points — straight
from the RCIG graph. Installs the prebuilt CLI from npm; **no Rust toolchain**.

## Usage

```yaml
# .github/workflows/code-intel.yml
name: Code intelligence
on: [pull_request]

permissions:
  contents: read
  pull-requests: write   # needed only for comment-on-pr

jobs:
  mycelium:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: "20"
      - uses: aimasteracc/mycelium/integrations/github-action@develop
        with:
          path: "."
          comment-on-pr: "true"
```

This writes the summary to the **job summary** every run, and (with
`comment-on-pr: "true"`) maintains a single sticky **PR comment**.

## Inputs

| Input | Default | Description |
|---|---|---|
| `path` | `.` | Directory to index. |
| `version` | `latest` | `@aimasteracc/mycelium` version/dist-tag. |
| `comment-on-pr` | `false` | Post a sticky summary comment (needs `pull-requests: write`). |
| `github-token` | `${{ github.token }}` | Token for the PR comment. |

## Outputs

| Output | Description |
|---|---|
| `summary` | The Markdown summary (also appended to `$GITHUB_STEP_SUMMARY`). |

## Notes

- **Structural, not type-resolved** — Mycelium surfaces graph intelligence, not a
  language server ([ADR-0010](../../docs/adr/0010-no-live-lsp-prefer-scip-ingestion.md)).
- The summary grows richer as new capabilities land (health grade, architectural
  constraints, test-gap) — they slot into the same action without a workflow change.
- A thin **consumer** of the published CLI; it adds no engine code.

## License

MIT
