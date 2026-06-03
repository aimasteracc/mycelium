; Mycelium Rust capture queries
;
; Capture convention:
;   @definition.<kind>   — creates a node in the Trunk
;   @reference.<kind>    — creates an edge in the Synapse
;   @name                — the identifier used as the node path segment
;
; Tree-sitter grammar: tree-sitter-rust ^0.24
; Requires tree-sitter C runtime >= 0.25

; ── Source file (module root) ────────────────────────────────────────

(source_file) @definition.module

; ── Free functions ───────────────────────────────────────────────────

(source_file
  (function_item
    name: (identifier) @name)) @definition.function

; ── Structs ──────────────────────────────────────────────────────────

(source_file
  (struct_item
    name: (type_identifier) @name)) @definition.struct

; ── Enums ────────────────────────────────────────────────────────────

(source_file
  (enum_item
    name: (type_identifier) @name)) @definition.enum

; ── Traits ───────────────────────────────────────────────────────────

(source_file
  (trait_item
    name: (type_identifier) @name)) @definition.trait

; ── Type aliases ─────────────────────────────────────────────────────

(source_file
  (type_item
    name: (type_identifier) @name)) @definition.type_alias

; ── Const items ──────────────────────────────────────────────────────

(source_file
  (const_item
    name: (identifier) @name)) @definition.const

; ── Inline module declarations ───────────────────────────────────────

(source_file
  (mod_item
    name: (identifier) @name)) @definition.mod

; ── Methods and associated functions inside impl blocks ─────────────

(impl_item
  body: (declaration_list
    (function_item
      name: (identifier) @name))) @definition.method

; ── Use declarations (Synapse Imports edges) ─────────────────────────

(source_file
  (use_declaration
    argument: (_) @name)) @reference.import

; ── Call expressions (Synapse Calls edges) ──────────────────────────

; Simple calls: foo()
(call_expression
  function: (identifier) @name) @reference.call

; Method calls: self.method() / obj.method()
(call_expression
  function: (field_expression
    field: (field_identifier) @name)) @reference.call

; Scoped / qualified path calls: Type::method() / crate::mod::func()
;
; The captured @name is the LAST identifier (the function or method name).
; Cross-file resolution by `resolve_bare_call_stubs` later links it to the
; concrete definition once both files have been indexed.
;
; Added 2026-06-03 after dogfooding the Mycelium repo against itself surfaced
; that `WatchEngine::drive(...)` produced no Calls edges — the previous query
; only matched `(identifier)` and `(field_expression)` function forms.
(call_expression
  function: (scoped_identifier
    name: (identifier) @name)) @reference.call
