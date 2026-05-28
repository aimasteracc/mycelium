# RFC-0011 — Call Graph: Calls Edges from Source

| Field    | Value                              |
|----------|------------------------------------|
| RFC      | 0011                               |
| Status   | Accepted                           |
| Author   | rust-implementer (Hive AI agent)   |
| Date     | 2026-05-29                         |
| Refs     | RFC-0001 (Synapse), RFC-0002 (Extractor) |

## Summary

Populate `EdgeKind::Calls` edges during extraction by capturing `call_expression`
patterns in all language packs and resolving caller/callee identity in the
extractor.

## Motivation

The Synapse already supports `EdgeKind::Calls`, but nothing writes those edges.
AI agents that want to answer "what does function X call?" or "what calls X?"
need actual call graph data in the store.

## Design

### Query captures

Each language pack gains `reference.call` patterns using `@name` for the callee:

**Python** — `call` node (`function:` field)  
**TypeScript/JavaScript** — `call_expression` node  
**Rust** — `call_expression` and `macro_invocation`

For method calls (e.g., `obj.method()`), only the method name is captured, not
the receiver type. Cross-type resolution is deferred to RFC-0012.

### Extractor changes

New arm in the `match cap_name` block for `"reference.call"`:

1. **Find caller**: Walk ancestors of `anchor` looking for a function-like node
   (`function_definition`, `function_declaration`, `arrow_function`,
   `method_definition`, `function_expression`, `function_item`).
   Build the caller path using the same chain logic as `definition.method`.
   If no enclosing function is found, the caller is the file node.

2. **Find callee**: Try `store.lookup(&format!("{file_path}>{callee_name}"))`.
   If found, use that node. Otherwise upsert a bare-name node (`callee_name`).
   This gives accurate intra-file edges; cross-file edges are resolved in RFC-0012.

3. **Add edge**: `store.upsert_edge(EdgeKind::Calls, caller_id, callee_id)`.

### Helper: `find_enclosing_function`

New private function in `extractor/mod.rs`:

```rust
fn find_enclosing_function(node: Node<'_>, source: &[u8]) -> Option<(String, Vec<String>)>
```

Returns `(function_name, class_chain)`. `class_chain` is built by the existing
`build_class_chain` logic applied to the enclosing container of the function.

## Acceptance Criteria

- [ ] Python: `foo()` call inside a function creates `Calls` edge.
- [ ] Python: method call `self.bar()` inside a method creates `Calls` edge.
- [ ] TypeScript: `foo()` and `obj.method()` calls are captured.
- [ ] Rust: `foo()` and `self.method()` calls are captured.
- [ ] Intra-file calls resolve to the callee's definition node, not a bare stub.
- [ ] Extractor tests green. All prior tests pass.
