; Mycelium Go capture queries
;
; Capture convention:
;   @definition.<kind>   — creates a node in the Trunk
;   @reference.<kind>    — creates an edge in the Synapse
;   @name                — the identifier used as the node path segment
;
; Tree-sitter grammar: tree-sitter-go ^0.25

; ── Source file (package root) ────────────────────────────────────────

(source_file) @definition.module

; ── Top-level functions ───────────────────────────────────────────────

(source_file
  (function_declaration
    name: (identifier) @name)) @definition.function

; ── Methods (receiver functions) ─────────────────────────────────────
; Go methods are top-level symbols without an OOP class chain.
; Use @definition.function so they are flat file-level nodes.

(source_file
  (method_declaration
    name: (field_identifier) @name)) @definition.function

; ── Type declarations (struct, interface, alias, etc.) ───────────────

(source_file
  (type_declaration
    (type_spec
      name: (type_identifier) @name))) @definition.type

; ── Constants ────────────────────────────────────────────────────────

(source_file
  (const_declaration
    (const_spec
      name: (identifier) @name))) @definition.const

; ── Variables (package-level) ────────────────────────────────────────

(source_file
  (var_declaration
    (var_spec
      name: (identifier) @name))) @definition.variable

; ── Import references ─────────────────────────────────────────────────
; Go: import "path/to/pkg" or import alias "path/to/pkg"

(import_spec
  path: (interpreted_string_literal) @name) @reference.import

; ── Call references ───────────────────────────────────────────────────

(call_expression
  function: [
    (identifier) @name
    (selector_expression
      field: (field_identifier) @name)
  ]) @reference.call
