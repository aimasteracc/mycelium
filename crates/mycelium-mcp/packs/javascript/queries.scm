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

; ── Top-level function expressions (const name = function(...) {...}) ─
; Issue #293: `const localize = function(key) {...}` — CommonJS/UMD pattern.
; Mirrors the arrow_function patterns above but matches function_expression.

(program
  (lexical_declaration
    (variable_declarator
      name: (identifier) @name
      value: (function_expression)))) @definition.function

(program
  (export_statement
    declaration: (lexical_declaration
      (variable_declarator
        name: (identifier) @name
        value: (function_expression))))) @definition.function

; ── Class declarations ───────────────────────────────────────────────

(program
  (class_declaration
    name: (identifier) @name)) @definition.class

(program
  (export_statement
    declaration: (class_declaration
      name: (identifier) @name))) @definition.class

; ── Methods (inside class body) ─────────────────────────────────────

(class_declaration
  body: (class_body
    (method_definition
      name: (property_identifier) @name))) @definition.method

; ── Import statements (Synapse Imports edges) ───────────────────────

(import_statement
  source: (string
    (string_fragment) @name)) @reference.import

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
