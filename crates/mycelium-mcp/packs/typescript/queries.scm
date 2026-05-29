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

; ── Call expressions (Synapse Calls edges) ──────────────────────────

; Simple calls: foo()
(call_expression
  function: (identifier) @name) @reference.call

; Method calls: obj.method()
(call_expression
  function: (member_expression
    property: (property_identifier) @name)) @reference.call
