; Mycelium TypeScript capture queries
;
; Capture convention:
;   @definition.<kind>   — creates a node in the Trunk
;   @reference.<kind>    — creates an edge in the Synapse
;   @name                — the identifier used as the node path segment
;
; Tree-sitter grammar: tree-sitter-typescript ^0.23 (LANGUAGE_TYPESCRIPT)
; Requires tree-sitter C runtime >= 0.25

; ── Module (file root) ───────────────────────────────────────────────

(program) @definition.module

; ── Top-level function declarations ─────────────────────────────────

(program
  (function_declaration
    name: (identifier) @name)) @definition.function

(program
  (export_statement
    declaration: (function_declaration
      name: (identifier) @name))) @definition.function

; ── Top-level arrow functions / const fn assignments ────────────────

(program
  (lexical_declaration
    (variable_declarator
      name: (identifier) @name
      value: (arrow_function)))) @definition.function

(program
  (export_statement
    declaration: (lexical_declaration
      (variable_declarator
        name: (identifier) @name
        value: (arrow_function))))) @definition.function

; ── Class declarations ───────────────────────────────────────────────

(program
  (class_declaration
    name: (type_identifier) @name)) @definition.class

(program
  (export_statement
    declaration: (class_declaration
      name: (type_identifier) @name))) @definition.class

; ── Methods (inside class body) ─────────────────────────────────────

(class_declaration
  body: (class_body
    (method_definition
      name: (property_identifier) @name))) @definition.method

; ── Interface declarations ───────────────────────────────────────────

(program
  (interface_declaration
    name: (type_identifier) @name)) @definition.interface

(program
  (export_statement
    declaration: (interface_declaration
      name: (type_identifier) @name))) @definition.interface

; ── Type alias declarations ──────────────────────────────────────────

(program
  (type_alias_declaration
    name: (type_identifier) @name)) @definition.type_alias

(program
  (export_statement
    declaration: (type_alias_declaration
      name: (type_identifier) @name))) @definition.type_alias

; ── Import statements (Synapse Imports edges) ───────────────────────

(import_statement
  source: (string
    (string_fragment) @name)) @reference.import

; ── Alias bindings (RFC-0092 Phase 2 — feeds the per-file alias table) ──────
;
; `import { X as Y } from './mod'`  → binds Y to <resolved-mod>>X
; `import { X } from './mod'`       → implicit binding X → <resolved-mod>>X
; `import * as ns from './mod'`     → binds ns to <resolved-mod>
; `import Foo from './mod'`         → binds Foo to <resolved-mod> (default)
;
; @alias.source    — the module specifier string (e.g. `./module`)
; @alias.local     — the local binding identifier
; @alias.original_name — the exported name, if different from the local name

; Note on module specifier capture in TypeScript:
; The query validator (tree-sitter 0.26) treats the `source:` field and the
; `import_clause` child of `import_statement` as mutually exclusive, because
; `source:` is contributed by the inline rule `_from_clause` which the
; validator does not see when analysing the `import_clause` alternative.
; At *runtime* both coexist, so the Rust extractor reads the module specifier
; directly from the anchor node via `child_by_field_name("source")` when
; `@alias.source` is absent from the match captures.

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

; Reassignment to a constructor (`x = new Ctor()`) — TypeScript's structural
; typing lets a variable be reassigned to a different same-shaped class, so a
; declared type can change at the call site. Capturing the reassignment makes a
; type-conflicting rebinding DECLINE (never mis-bind) instead of trusting the
; original declarator.
(assignment_expression
  left: (identifier) @binding.local
  right: (new_expression
    constructor: (identifier) @binding.ctor)) @reference.binding

; Rebind invalidation (RFC-0118 Part B, Codex P1 #647): capture ANY local
; declaration or reassignment target. The core compares the rebind count per
; name against the recognized-constructor-binding count; a name reassigned to a
; non-constructor (e.g. `s = factory()`) DECLINES rather than trusting the stale
; declared type — preserving "never mis-bind" under TypeScript structural typing.
(variable_declarator
  name: (identifier) @binding.rebind)
(assignment_expression
  left: (identifier) @binding.rebind)
