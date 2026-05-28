# mycelium-core

The reactive code intelligence graph engine.

This crate implements:

- **Trunk** — containment tree (file → class → method) as a materialized-path index
- **Synapse** — cross-cutting edges (calls, extends, implements, …) as per-kind adjacency lists
- **Store** — the unified read/write surface over both

For the design rationale and SLAs, see [RFC-0001](../../rfcs/0001-trunk-and-synapse.md)
and the [Charter §2](../../CHARTER.md#2-performance-sla-the-contract).

## Status

Pre-alpha. Public API may change. Versions before 1.0 follow `0.MINOR.PATCH`
with **`MINOR` bumps potentially breaking** per Cargo semver convention.

## Quick taste

```rust,no_run
use mycelium_core::trunk::{Trunk, TrunkPath};

let mut trunk = Trunk::new();
let id = trunk.upsert(TrunkPath::parse("src/auth.rs>AuthService>login")?);
assert_eq!(trunk.lookup_path("src/auth.rs>AuthService>login"), Some(id));
# Ok::<(), mycelium_core::Error>(())
```
