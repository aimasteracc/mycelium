# Security Policy

## Reporting a Vulnerability

**Do not open a public issue for security vulnerabilities.**

Use GitHub's private vulnerability reporting:

1. Go to https://github.com/aimasteracc/mycelium/security
2. Click **"Report a vulnerability"**
3. Fill in the details

Alternatively, email `aimasteracc <at> proton.me` with subject prefix
`[SECURITY]` (or `[CoC]` for Code of Conduct reports).

## What to Include

- Affected version (`mycelium --version`)
- Vulnerability type (memory safety, parser DoS, secrets leak, etc.)
- Reproduction steps
- Suggested fix or mitigation, if any
- Whether you wish to be credited and how

## Our Response

| Severity | First response | Patch target |
|---|---|---|
| Critical (RCE, data exfiltration, key compromise) | 24 hours | 7 days |
| High (DoS, privilege boundary breach) | 48 hours | 14 days |
| Medium (information leak, parser crash) | 7 days | 30 days |
| Low (best-practice deviation) | 14 days | next minor release |

We will:

1. Acknowledge receipt within the timeline above.
2. Investigate and confirm the issue.
3. Develop a fix in a private branch.
4. Coordinate a disclosure timeline with you.
5. Release a patch as a hotfix (see [GITFLOW.md](GITFLOW.md) §3).
6. Publish a GitHub Security Advisory.
7. Credit you (with your consent) in the advisory and changelog.

## Scope

In scope:

- The `mycelium` binary and crates in this repository
- The MCP server
- The CLI
- The official npm and PyPI bindings under `aimasteracc`
- The Hive automation that has write access to the repository

Out of scope:

- Third-party tree-sitter grammar bugs (report upstream)
- The user's own code that Mycelium happens to index
- Brute-force attacks against the founder's GitHub account (report to GitHub)

## Supported Versions

Until v1.0:

| Version | Status |
|---|---|
| `main` / latest tag | Supported |
| any pre-release older than the latest | Best-effort only |

After v1.0, we will support the **latest minor** of the **two most recent majors**.

## Security Practices

- All dependencies are pinned (`Cargo.lock` in repo, lockfiles for bindings).
- `cargo-audit` and `cargo-deny` run in CI on every PR and nightly.
- `cargo-fuzz` runs nightly against parser-facing entry points.
- Release binaries are signed via Sigstore.
- npm packages are published with provenance.
- Secrets in CI are managed via GitHub Environments with least-privilege.
- Hive agents operate under scoped tokens that cannot push to `main`.
- The Hive kill switch (issue #1) halts autonomous activity in ≤60 seconds.

## Privacy

See [PRIVACY.md](PRIVACY.md). Mycelium does not phone home. Telemetry is
opt-in and never includes code content.

## Hall of Fame

Researchers who responsibly disclose vulnerabilities are recognized here
(with consent). Currently empty — be the first.
