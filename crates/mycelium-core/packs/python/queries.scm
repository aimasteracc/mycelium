; Mycelium Python capture queries
;
; Capture convention:
;   @definition.<kind>   — creates a node in the Trunk
;   @reference.<kind>    — creates an edge in the Synapse
;   @name                — the identifier used as the node path segment
;
; Tree-sitter grammar: tree-sitter-python ^0.21
; Requires tree-sitter C runtime >= 0.22

; ── Module (file root) ───────────────────────────────────────────────

(module) @definition.module

; ── Top-level function definitions ──────────────────────────────────

(module
  (function_definition
    name: (identifier) @name)) @definition.function

(module
  (decorated_definition
    definition: (function_definition
      name: (identifier) @name))) @definition.function

; ── Class definitions ────────────────────────────────────────────────

(module
  (class_definition
    name: (identifier) @name)) @definition.class

(module
  (decorated_definition
    definition: (class_definition
      name: (identifier) @name))) @definition.class

; ── Methods (functions inside a class body) ─────────────────────────

(class_definition
  body: (block
    (function_definition
      name: (identifier) @name))) @definition.method

(class_definition
  body: (block
    (decorated_definition
      definition: (function_definition
        name: (identifier) @name)))) @definition.method

; ── Nested functions ─────────────────────────────────────────────────

(function_definition
  body: (block
    (function_definition
      name: (identifier) @name))) @definition.function

; ── Import statements (Synapse Imports edges) ───────────────────────

(import_statement
  name: (dotted_name) @name) @reference.import

(import_statement
  name: (aliased_import
    name: (dotted_name) @name)) @reference.import

(import_from_statement
  module_name: (dotted_name) @name) @reference.import_from

(import_from_statement
  module_name: (relative_import) @name) @reference.import_from

; ── Alias bindings (RFC-0092 — feeds the per-file alias table) ──────
;
; `import json as j`         → binds `j` to module `json`
; `from . import m as n`     → binds `n` to (resolved) sibling module
; `from M import X as Y`     → binds `Y` to `M>X`
;
; The @alias.local capture is the local identifier; @alias.source is the
; module or symbol it points to. The extractor combines them with bug
; #204's relative-import resolver to produce the final binding target.

(import_statement
  name: (aliased_import
    name: (dotted_name) @alias.source
    alias: (identifier) @alias.local)) @reference.alias_binding

(import_from_statement
  module_name: (_) @alias.source
  name: (aliased_import
    name: (_) @alias.original_name
    alias: (identifier) @alias.local)) @reference.alias_binding

; `from . import M` (no `as`) — binds `M` to the resolved sibling module.
(import_from_statement
  module_name: (relative_import) @alias.source
  name: (dotted_name) @alias.local) @reference.alias_binding

; `from M import X` (absolute, no `as`) — implicit alias: X binds to M>X.
; issues #267/#268: feeds the per-file alias table so that `class Sub(X):` can
; resolve the Extends edge to M>X at extraction time, bypassing the ambiguous
; bare-stub path that fails when multiple files define a class named X.
(import_from_statement
  module_name: (dotted_name) @alias.source
  name: (dotted_name) @alias.original_name) @reference.alias_binding

; Issue #229: attribute-assignment alias.
; `local = module_alias.fn` — binds `local` to `module_alias>fn`.
; If `module_alias` is itself a local alias (from any import-alias above),
; the extractor chain-resolves at call time.
(assignment
  left: (identifier) @alias.local
  right: (attribute
    object: (identifier) @alias.source
    attribute: (identifier) @alias.original_name)) @reference.alias_binding

; ── Callback / higher-order function arguments (issue #247) ─────────
;
; When an identifier is passed as a positional argument to a call
; (e.g. `run(callback)` or `sorted(items, key=fn)`), the function object
; is "reached" even though it is never called directly. Without this query
; `get-isolated-symbols` reports such identifiers as dead code.
;
; We emit a Calls edge from the enclosing function to the argument
; identifier, representing "the caller reaches this function object".
; Keyword-argument values are matched by the `@name` capture on the
; `keyword_argument value: (identifier)` branch.
;
; Note: `(_)` in `@name` position here is intentional — we want only the
; *function-valued* arguments. We can't distinguish function from non-function
; at parse time, so we capture all identifier arguments and rely on the
; extractor to limit the blast-radius (only creates edges, never deletes).
(call
  arguments: (argument_list
    (identifier) @name)) @reference.arg_callback

(call
  arguments: (argument_list
    (keyword_argument
      value: (identifier) @name))) @reference.arg_callback

; ── Class inheritance (Extends edges) ────────────────────────────────
; `class Sub(Base1, Base2):` — one match per superclass identifier.
; The class_definition node is the anchor; @name captures the base class.
; The extractor reads the subclass name from anchor.child_by_field_name("name").
(class_definition
  superclasses: (argument_list
    (identifier) @name)) @reference.extends

; Attribute-form base class: `class Foo(module.Base):` — @name captures
; the full "module.Base" text; handler stores a dotted stub node.
(class_definition
  superclasses: (argument_list
    (attribute) @name)) @reference.extends

; ── Call expressions (Synapse Calls edges) ──────────────────────────

; Simple function calls: foo()
(call
  function: (identifier) @name) @reference.call

; Method calls: obj.method() — capture both the receiver and the method
; so the extractor can rewrite obj via the alias table (RFC-0092).
(call
  function: (attribute
    object: (identifier) @call.receiver
    attribute: (identifier) @name)) @reference.call

; NOTE (issue #214 Pattern 3): the original fallback here matched
; `self.history.append(x)` (depth-2+ attribute chains) and created a
; global bare stub `append`, causing 1,472 false callers when any
; user-defined method happened to share the bare name.  Removed.
; Depth-2+ chains are unresolvable without type inference — no edge is
; emitted rather than a misleading global stub.  Direct `obj.method()`
; calls (depth 1) continue via the `@call.receiver` pattern above.

; ── RFC-0118 Part B: local constructor bindings (x = Ctor()) ──────────
; Captures a local name and the constructor TYPE so the post-merge receiver
; disambiguation pass can bind `x.method()` to `…>Ctor>method`. The query itself
; matches only a bare-name call RHS (`x = Ctor()` / `x = make()`); attribute
; calls `x = obj.m()` don't match (function field is `(attribute)`, not
; `(identifier)`). The core then keeps only Title-case ctor names, dropping
; lowercase factory/utility calls like `x = make()`. Conservative: declines on
; any ambiguity — a shadowed rebinding (handled by the de-shadow conflict pass)
; or a reassignment to a non-constructor (handled by @binding.rebind below).
; NOTE: scope detection is scope-aware — a lambda is its own binding scope
; (BINDING_SCOPE_KINDS) and the call site walks the enclosing scope chain, so a
; lambda-local binding never leaks to a sibling lambda/outer body while a legit
; outer-scope binding captured by a lambda still resolves (Codex P2 #653).
(assignment
  left: (identifier) @binding.local
  right: (call
    function: (identifier) @binding.ctor)) @reference.binding

; Rebind invalidation (RFC-0118 Part B, Codex P1 #647): capture ANY assignment
; target identifier. The core compares the rebind count per name against the
; recognized-constructor-binding count; if a name was reassigned to a
; non-constructor (count exceeds ctor bindings), inference DECLINES rather than
; trusting the stale declared type — preserving "never mis-bind" under Python's
; dynamic typing.
(assignment
  left: (identifier) @binding.rebind)
