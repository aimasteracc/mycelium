; Mycelium JavaScript capture queries
;
; Capture convention:
;   @definition.<kind>   — creates a node in the Trunk
;   @reference.<kind>    — creates an edge in the Synapse
;   @name                — the identifier used as the node path segment
;
; Tree-sitter grammar: tree-sitter-javascript ^0.25
; Also handles .jsx (JSX nodes are transparent to symbol extraction).
; Requires tree-sitter C runtime >= 0.25
;
; Note: unlike tree-sitter-typescript, class names in this grammar use
; `identifier`, not `type_identifier`.

; ── Module (file root) ───────────────────────────────────────────────

(program) @definition.module

; ── Top-level function declarations ─────────────────────────────────
;
; NOTE on @definition.* anchoring (PR #750 follow-up, 2026-06-11): the
; @definition capture sits on the ITEM node (the export_statement for
; exported items, so the span keeps the `export` keyword), NOT on the
; file-root `program` — the extractor takes the SPAN from the @definition
; anchor, so anchoring on the root made every top-level symbol's span
; cover the whole file. Paths derive from @name text only, so re-anchoring
; changes spans only. @definition.method captures stay anchored on the
; class container (the extractor builds the `Class>method` path from that
; anchor and recovers the precise span via the issue-#657
; METHOD_DECL_KINDS walk-up).

(program
  (function_declaration
    name: (identifier) @name) @definition.function)

(program
  (export_statement
    declaration: (function_declaration
      name: (identifier) @name)) @definition.function)

; ── Top-level arrow functions / const fn assignments ────────────────

(program
  (lexical_declaration
    (variable_declarator
      name: (identifier) @name
      value: (arrow_function))) @definition.function)

(program
  (export_statement
    declaration: (lexical_declaration
      (variable_declarator
        name: (identifier) @name
        value: (arrow_function)))) @definition.function)

; ── Top-level function expressions (const name = function(...) {...}) ─
; Issue #293: `const localize = function(key) {...}` — CommonJS/UMD pattern.
; Mirrors the arrow_function patterns above but matches function_expression.

(program
  (lexical_declaration
    (variable_declarator
      name: (identifier) @name
      value: (function_expression))) @definition.function)

(program
  (export_statement
    declaration: (lexical_declaration
      (variable_declarator
        name: (identifier) @name
        value: (function_expression)))) @definition.function)

; ── Class declarations ───────────────────────────────────────────────

(program
  (class_declaration
    name: (identifier) @name) @definition.class)

(program
  (export_statement
    declaration: (class_declaration
      name: (identifier) @name)) @definition.class)

; ── Methods (inside class body) ─────────────────────────────────────

(class_declaration
  body: (class_body
    (method_definition
      name: (property_identifier) @name))) @definition.method

; ── Import statements (Synapse Imports edges) ───────────────────────

(import_statement
  source: (string
    (string_fragment) @name)) @reference.import

; ── CJS require() imports (RFC-0125 Phase 1) ────────────────────────────
;
; CommonJS `require('module')` is a call_expression, not an import_statement,
; so without these patterns CJS files produce no @reference.import captures.
; With empty caller_imports, classify_typescript_import_gated silently falls
; through to "unknown" for every callee regardless of allowlist match.
;
; Both forms produce an Imports edge feeding caller_imports for the existing
; classify_typescript_import_gated — zero changes to classify.rs or queries.rs.

; `const fs = require('fs')` — whole-module assignment
(lexical_declaration
  (variable_declarator
    value: (call_expression
      function: (identifier) @_req (#eq? @_req "require")
      arguments: (arguments
        (string (string_fragment) @name))))) @reference.import

; `const { readFileSync } = require('fs')` — destructured assignment
(lexical_declaration
  (variable_declarator
    name: (object_pattern)
    value: (call_expression
      function: (identifier) @_req2 (#eq? @_req2 "require")
      arguments: (arguments
        (string (string_fragment) @name))))) @reference.import

; ── Alias bindings (RFC-0092 Phase 2 — feeds the per-file alias table) ──────
;
; Same patterns as the TypeScript pack. The module specifier is NOT captured
; via `source:` because the query validator (tree-sitter 0.26) treats
; `source:` and `import_clause` as mutually exclusive for `import_statement`
; (the `_from_clause` inline rule is invisible to the validator). The Rust
; extractor reads the specifier at runtime via `child_by_field_name("source")`.

; `import { X as Y }` — named import with explicit alias
(import_statement
  (import_clause
    (named_imports
      (import_specifier
        name: (_) @alias.original_name
        alias: (identifier) @alias.local)))) @reference.alias_binding

; `import { X }` (no `as`) — implicit alias: X binds to <mod>>X
(import_statement
  (import_clause
    (named_imports
      (import_specifier
        name: (identifier) @alias.original_name)))) @reference.alias_binding

; `import * as ns` — namespace import: ns binds to the whole module
(import_statement
  (import_clause
    (namespace_import
      (identifier) @alias.local))) @reference.alias_binding

; `import Foo from './mod'` — default import: Foo binds to the module
(import_statement
  (import_clause
    (identifier) @alias.local)) @reference.alias_binding

; ── Call expressions (Synapse Calls edges) ──────────────────────────

; Simple calls: foo()
(call_expression
  function: (identifier) @name) @reference.call

; Method calls: obj.method() — capture receiver for alias rewriting (RFC-0092 Phase 2)
; Only matches depth-1 chains (object is a plain identifier); deeper chains
; (a.b.c()) are unresolvable without type inference — no edge emitted.
(call_expression
  function: (member_expression
    object: (identifier) @call.receiver
    property: (property_identifier) @name)) @reference.call

; ── RFC-0118 Part B: local constructor bindings (const x = new Ctor()) ──
; Captures a local name and the constructor TYPE so the post-merge receiver
; disambiguation pass can bind `x.method()` to `…>Ctor>method`. Matches the
; idiomatic `new Ctor()` RHS; the core keeps only Title-case ctor names and
; declines on any ambiguity (a shadowed rebinding, or a reassignment to a
; non-constructor — see @binding.rebind below) rather than guessing.
; Scope-aware: arrow functions are their own binding scope (BINDING_SCOPE_KINDS)
; and the call site walks the enclosing scope chain — an arrow-local binding never
; leaks to a sibling arrow or the outer body (no false caller), while a legit
; outer-scope binding captured by a nested arrow still resolves (Codex P2 #653).
(variable_declarator
  name: (identifier) @binding.local
  value: (new_expression
    constructor: (identifier) @binding.ctor)) @reference.binding

; Reassignment to a constructor (`x = new Ctor()`) — JavaScript's dynamic typing
; lets a variable be reassigned to a different class, so a declared type can
; change at the call site. Capturing the reassignment makes a type-conflicting
; rebinding DECLINE (never mis-bind) instead of trusting the original declarator.
(assignment_expression
  left: (identifier) @binding.local
  right: (new_expression
    constructor: (identifier) @binding.ctor)) @reference.binding

; Rebind invalidation (RFC-0118 Part B, Codex P1 #647): capture ANY local
; declaration or reassignment target. The core compares the rebind count per
; name against the recognized-constructor-binding count; a name reassigned to a
; non-constructor (e.g. `s = factory()`) DECLINES rather than trusting the stale
; declared type — preserving "never mis-bind" under JavaScript dynamic typing.
(variable_declarator
  name: (identifier) @binding.rebind)
(assignment_expression
  left: (identifier) @binding.rebind)
