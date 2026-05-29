# 0005. MessagePack as the wire format for snapshots and compact MCP output

- **Status**: accepted
- **Date**: 2026-05-29
- **RFC**: RFC-0006 (persistence), RFC-0090 (compact mode)

## Context

Mycelium needs a serialization format for:
1. **Snapshot persistence** (`.mycelium/index.rmp`) — loading the index without re-parsing
2. **MCP compact mode** — Charter §2 requires AI token efficiency ≤ 30% of JSON

## Decision

Use **MessagePack** (`rmp-serde`) for both use cases.

### Snapshot persistence
- `Store::save()` / `Store::load()` use `rmp_serde::encode/decode`
- ~40-65% smaller than JSON for typical Store payloads
- Already a workspace dependency; no new deps

### Compact MCP mode
- `mycelium_set_compact_mode(true)` switches `mycelium_search_symbol` to return
  hex-encoded MessagePack: `{ "fmt": "msgpack_hex", "data": "...", "bytes": N }`
- AI agents can decode if needed, or request plain JSON

## Consequences

**⚠️ Important finding (2026-05-29):**
MessagePack alone does **not** achieve the ≤ 30% token efficiency SLA.
- Small payload (3 symbols): ratio = 0.94 (worse than JSON)
- Large payload (100 records): ratio ≈ 0.65 (still not ≤ 0.30)

The correct path to ≤ 30% is **Hyphae DSL field projection** — AI specifies
only the fields it needs, and the engine only serializes those.
MessagePack is a complementary compression layer on top of field projection.

RFC-0003/0004 (Hyphae) is the primary mechanism for the token efficiency SLA.
