---
name: inheritance
description: Navigate type relationships — extends/implements, subclass/implementor trees, and concrete-to-abstract paths.
allowed-tools:
  - mcp__mycelium__get_extends
  - mcp__mycelium__get_extends_tree
  - mcp__mycelium__get_subclasses_tree
  - mcp__mycelium__find_extends_path
  - mcp__mycelium__get_implements
  - mcp__mycelium__get_implements_tree
  - mcp__mycelium__get_implementors_tree
  - mcp__mycelium__find_implements_path
---

# `inheritance` — class/trait/interface relationships

This Skill bundles the 8 `Extends` and `Implements` edge tools. Reach for it when the question involves type hierarchies — class extension chains, interface implementations, trait dispatch resolution.

**Language note:** Rust has no class inheritance, but trait `impl X for Y` produces `Implements` edges. C++/C#/Java/Python/TypeScript projects exercise both edge kinds heavily.

## When to invoke this Skill

Use **when**:

- The user asks "who extends/implements X", "what does X extend", "what's the parent class of X".
- Tracking down where polymorphism resolves at runtime — "which concrete `Repository` impls exist".
- Refactoring an abstract base — need the full set of subclasses or implementors.

Do **NOT** use when:

- The relationship is `Calls` (use `call-graph`) or `Imports` (use `import-graph`).
- The user wants multi-edge-kind reachability (use `reachability`).

## Capabilities under this umbrella

### `get_extends` — direct parent class(es)

```
mcp__mycelium__get_extends({ "path": "src/auth/oauth.py>OAuth2Session" })
→ { "extends": ["src/auth/base.py>BaseSession"], "count": 1 }
```

### `extends_tree` — recursive extends chain

Walks up: this class → its parent → grandparent → ... up to `max_depth`.

```
mcp__mycelium__extends_tree({ "path": "src/auth/oauth.py>OAuth2Session", "max_depth": 5 })
```

### `subclasses_tree` — recursive subclass tree

Walks down: this class → its subclasses → their subclasses. The "who extends this" tree.

```
mcp__mycelium__subclasses_tree({ "path": "src/auth/base.py>BaseSession", "max_depth": 5 })
```

### `find_extends_path` — concrete-to-abstract chain

"Show me the inheritance chain from `OAuth2Session` to `BaseSession`":

```
mcp__mycelium__find_extends_path({
  "from": "src/auth/oauth.py>OAuth2Session",
  "to": "src/auth/base.py>BaseSession"
})
→ { "path": ["src/auth/oauth.py>OAuth2Session", "src/auth/jwt.py>JwtSession", "src/auth/base.py>BaseSession"], "length": 3 }
```

Returns `{ "path": null }` if the symbols are not in the same chain.

### `get_implements` — what interfaces/traits does X implement

```
mcp__mycelium__get_implements({ "path": "src/db/postgres.rs>PostgresUserRepo" })
→ { "implements": ["src/repo.rs>UserRepository", "src/repo.rs>Healthcheckable"], "count": 2 }
```

### `implements_tree` — recursive implements chain

For interfaces/traits that extend other interfaces/traits — Rust's "supertrait" pattern, Java's `interface A extends B`.

### `implementors_tree` — recursive "who implements this"

The trait/interface analog of `subclasses_tree`. Returns every concrete type that implements the given trait/interface, walking down through trait inheritance.

### `find_implements_path` — concrete-to-interface chain

Counterpart of `find_extends_path` for the `Implements` edge.

## Common chains

- **"Where does this trait/interface get dispatched?"** → `implementors_tree`.
- **"Which subclasses inherit X?"** → `subclasses_tree`.
- **"What does X bring along when extended?"** → `extends_tree` (parents' contracts).
- **"Is X a subclass / implementor of Y?"** → `find_extends_path` / `find_implements_path`.

## Equivalent CLI

```bash
mycelium subclasses-tree "src/auth/base.py>BaseSession" --max-depth 5 --format=json
mycelium find-implements-path --from "src/db/postgres.rs>PostgresUserRepo" --to "src/repo.rs>UserRepository"
mycelium implementors-tree "src/repo.rs>UserRepository" --max-depth 3
```

## Parity contract

Per [RFC-0090](../../rfcs/0090-cli-mcp-skill-parity.md). `tests/parity.test.json` uses a small Python fixture with a 3-class extends chain plus a trait/interface scenario.

## Cross-references

- Related Skill: `call-graph` — for method-level dispatch (which `impl` actually runs).
- Related Skill: `reachability` — for cross-edge-kind navigation.
- Related Skill: `basic-queries` — to find the class/trait symbol first.
