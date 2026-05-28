# Privacy Policy

> **Short version**: Mycelium does not phone home. Your code never leaves
> your machine. Telemetry is opt-in. We see what you choose to show us in
> public Issues, PRs, and Discussions — nothing else.

## What Mycelium Collects

### From the CLI / engine: **Nothing**

By default, the Mycelium binary makes **zero network requests**. It:

- Reads your source files
- Writes a local `.myc` index
- Serves queries over stdio (CLI) or stdio/HTTP (MCP)
- Watches your filesystem locally

No background sync. No usage reporting. No update check. No anonymous stats.

### From opt-in telemetry: **Anonymous performance numbers only**

If you explicitly enable telemetry (via `mycelium config set telemetry.enabled true`,
which writes to `~/.config/mycelium/config.toml`), we collect:

- Mycelium version
- OS family (linux, macos, windows) and arch (x86_64, aarch64)
- Anonymous installation ID (random UUID, generated locally)
- Query latency percentiles (no query content)
- Index size buckets (no file paths, no symbol names)
- Crash reports (no source code, no stack traces from your code — only Mycelium's own)

We never collect:

- Source code content of any kind
- File paths or symbol names
- Project structure
- Anything that could identify your codebase
- Anything tied to your personal identity unless you provide it (GitHub username in an Issue, etc.)

You can disable at any time and the local config holds the choice.

### From GitHub interactions

When you open an Issue, PR, or Discussion, GitHub sees and stores that
according to [GitHub's Privacy Statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).
We see what you write.

### From sponsorship

GitHub Sponsors and OpenCollective handle payment processing. We see the
sponsor's chosen display name and tier. We do not see payment details.

## Hive Activity Privacy

The Hive (our AI development team) runs locally on the founder's machine
and operates on this repository's public contents. The Hive does not have
access to your code, your machine, or any private data unless you
explicitly share it in a public Issue or PR.

The Hive's audit log (`.hive/audit/`) is public and contains:

- Agent names
- Timestamps
- Actions taken (file paths, commits)
- Outcomes

It does not contain user data beyond what is already public on the repository.

## Cookies

The Mycelium documentation site uses no cookies and no analytics.

## Data Subject Rights (GDPR / similar)

Mycelium does not store personal data on our infrastructure. If you have
participated in the project (Issues, PRs, Discussions) and want your
contributions anonymized or removed, contact `aimasteracc <at> proton.me`.

For data held by GitHub on your behalf, contact GitHub directly under their
privacy policy.

## Changes to this Policy

Any change is announced via:

- A `meta` RFC PR
- An entry in `CHANGELOG.md`
- A note in the next release announcement

## Contact

`aimasteracc <at> proton.me` — please prefix with `[PRIVACY]`.
