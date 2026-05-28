# Mycelium Governance

> Who decides what, and how decisions become binding.

## Model

Mycelium uses a **BDFL + AI Hive** model:

- One human **Benevolent Dictator for Life (BDFL)** with final authority.
- A team of **AI agents** ("The Hive") performing day-to-day engineering under defined constraints.
- A community of **contributors** participating via Issues, PRs, RFCs, and Discussions.

This model is chosen because:

1. The project is young and direction-critical decisions still need decisive ownership.
2. The Hive provides leverage but cannot replace strategic judgment.
3. As the community grows, this charter can amend the model toward a maintainer council via a `meta` RFC.

## Roles

### BDFL — @aimasteracc

**Authority:**

- Final approval on RFCs, releases, license changes.
- Veto on any merge.
- Sole signing authority for `main` (`release-plz` bot acts as agent).
- Kill switch for the Hive (closing issue #1).
- Decides who becomes a Maintainer.

**Responsibilities:**

- Daily review of `.hive/audit/` log.
- Weekly review of `.hive/memory/anti-patterns.jsonl` consolidation.
- Monthly direction calibration (quarterly milestone on GitHub Projects).
- Triage of escalated decisions from the Hive.

**Succession:** if the BDFL becomes inactive for > 90 days, the senior
Maintainer (longest-tenured human Maintainer) becomes acting BDFL, with the
project's deferred decisions queued until restoration or formal handover.

### Maintainers

**Initial:** the BDFL only. Expanded by BDFL decree.

**Authority:**

- Merge rights on `develop`.
- Triage of issues and PRs.
- Approve RFCs (1 maintainer approval needed; BDFL approval needed for `meta` and `breaking` RFCs).

**Becoming a Maintainer:** sustained quality contribution (≥10 merged PRs
across ≥3 months, or equivalent demonstrated trust). Nomination by BDFL.

### Hive Agents

Defined in `.hive/`. Each agent has a constrained role and bound skills.
**Agents are not Maintainers.** They cannot merge their own PRs. They can
draft, propose, review, and assist, but a human must press the merge button
(BDFL override allowed for emergencies, with audit log entry).

The Hive's authority and escalation paths are governed by
[`.hive/_orchestrator.md`](.hive/_orchestrator.md).

### Contributors

Anyone who participates. Bound by [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
Welcomed by the Triage agent on first contribution. Recognized in CHANGELOG.md.

## Decision Types and Routes

| Decision class | Route | Approver(s) |
|---|---|---|
| Typo, doc fix | PR | 1 maintainer |
| Implementation under accepted RFC | PR + RFC ref | 2 reviewers (1 human) |
| New small feature, no public API change | PR with rationale | 2 reviewers (1 human) |
| Public API change | **RFC** then PR | RFC: maintainer + BDFL; PR: 2 reviewers |
| Storage format change | **RFC + migration plan** | BDFL required |
| Performance SLA amendment | **RFC** | BDFL required |
| License change | **meta RFC** | BDFL required + 30-day notice |
| Hive agent definition change | PR under `.hive/` | BDFL + 1 other reviewer |
| Charter amendment | **meta RFC** | BDFL required |
| Emergency hotfix | hotfix branch, post-hoc RFC if structural | BDFL or 2 maintainers |
| Removing or banning a contributor | private discussion | BDFL + 1 maintainer; recorded in audit log |

## RFC Lifecycle

```
Draft → Discussion → Final Comment Period (7 days) → Accepted | Rejected | Withdrawn
                                                          ↓
                                                    Implementation
                                                          ↓
                                                       Shipped
```

- **Draft:** PR opened under `rfcs/XXXX-title.md`.
- **Discussion:** unbounded; any contributor can comment.
- **FCP:** 7 calendar days, kicked off by a maintainer when discussion stabilizes.
- **Accepted:** merged to `develop`. Status changes to `accepted`. Implementation can begin.
- **Implementation:** opens an issue with `rfc:XXXX` label tracking progress.
- **Shipped:** when implementation lands. RFC status → `shipped`.

RFCs are immutable after merge except for status updates and clarifications
agreed to in the linked discussion.

## Communication Channels

- **Async, durable**: GitHub Issues, PRs, Discussions.
- **Sync, ephemeral**: Discord / Matrix (TBD).
- **Audit trail**: `.hive/audit/YYYY-MM-DD.jsonl` (public, append-only).
- **Memory**: `.hive/memory/*.jsonl` (public, append-only).
- **Decisions**: `.hive/memory/decisions.jsonl` mirrors Issue/RFC closures.

## Conflict of Interest

Agents and Maintainers must recuse themselves from review of their own work.
The BDFL can self-merge in emergencies but must log a justification in
`.hive/audit/` and follow up with a post-mortem if the action turns out to be premature.

## Trademark and Branding

The "Mycelium" name and logo are owned by the BDFL on behalf of the project.
Forks must rename if they intend to be presented as a distinct project. See
`assets/TRADEMARK.md` (created with first release).

## Code of Conduct Enforcement

Violations are reported privately to the BDFL (see [SECURITY.md](SECURITY.md)
for the contact channel; CoC reports use the same channel labeled `[CoC]`).
Outcomes: warning → temporary ban → permanent ban. All enforcement actions
are recorded (with personal details redacted) in the audit log.

## Funding and Money

- All sponsorship received via GitHub Sponsors goes to the BDFL.
- The BDFL commits to allocating funds toward: infrastructure (CI minutes,
  runners), AI API costs, design assets, and direct compensation to
  significant human contributors (transparent disbursement reports
  quarterly in `SPONSORS.md`).
- The project will not accept funding from entities the BDFL judges to
  pose a conflict of interest with the project's open-source mission.

## Amendments

This document is amended via a `meta` RFC. BDFL must approve. Amendments
take effect on merge.

---

*Power, when made small and explicit, becomes trust.*
