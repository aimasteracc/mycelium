# 0003. MIT license + DCO (no CLA)

- **Status**: accepted
- **Date**: 2026-05-28
- **RFC**: founders' decision, ratified into Charter §5.8 and §5.10

## Context

Mycelium needs an open-source license and a contributor agreement model.
The choices have long-term implications:

- License affects who can use the project commercially and how.
- Contributor agreement (CLA vs DCO) affects whether the project can be
  re-licensed or sold later, and how easy it is to accept contributions.

## Decision

- License: **MIT**, permanently.
- Contributor agreement: **Developer Certificate of Origin (DCO)**, signed via `Signed-off-by:` on every commit. **Not** a CLA.

## Consequences

### Positive

- MIT is the most permissive popular license; maximum adoption potential.
- DCO is trustless and lightweight; no central paperwork; first-time contributors can contribute in minutes.
- The project is fork-friendly forever; no single entity owns it.
- Aligned with widely understood open-source norms (Linux kernel uses DCO).

### Negative

- The project **cannot be re-licensed** later without unanimous consent of all contributors. Practically impossible.
- The project **cannot be sold** to a single company. Commercial value capture must come from services, support, or hosted offerings on top.
- No central control of contributor identity; abuse / disputes must be handled per-PR.

### Neutral / Trade-offs

- We give up flexibility in exchange for trust and contributor velocity. We believe this is the right trade for an infrastructure project.

## Alternatives considered

### Alternative A: Apache-2.0
- Pros: explicit patent grant, also widely accepted.
- Cons: slightly more verbose; some communities prefer MIT for simplicity.
- **Rejected**: MIT is sufficient; patent provisions can be added via separate CONTRIBUTING.md guidance if ever needed.

### Alternative B: AGPL
- Pros: forces downstream services to share source.
- Cons: hostile to the AI agent use case (every embedder of Mycelium would face AGPL obligations); kills adoption.
- **Rejected**: contradicts the embedding-everywhere strategy.

### Alternative C: BSL (Business Source License)
- Pros: revenue-protecting; converts to OSS after N years.
- Cons: not OSI-approved during the proprietary window; chills community.
- **Rejected**: not aligned with the open-source mission.

### Alternative D: MIT + CLA
- Pros: ability to relicense, transferable rights.
- Cons: paperwork overhead; chills first-time contributions; centralized control violates the "fork-friendly forever" principle.
- **Rejected**: explicitly considered and traded away for community velocity.

## Future possibility

If commercial development of derivative products becomes desirable, those
will live in **separate repositories** under separate licenses, on top of
the MIT core. The core stays MIT + DCO.
