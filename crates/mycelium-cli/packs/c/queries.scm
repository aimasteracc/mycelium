; Mycelium C capture queries
;
; Capture convention:
;   @definition.<kind>   — creates a node in the Trunk
;   @reference.<kind>    — creates an edge in the Synapse
;   @name                — the identifier used as the node path segment
;
; Tree-sitter grammar: tree-sitter-c ^0.24
; Requires tree-sitter C runtime >= 0.22

; ── Translation unit (file root) ────────────────────────────────────────

(translation_unit) @definition.module

; ── Function definitions ─────────────────────────────────────────────────

; int foo(void) { ... }
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

; Pointer-returning: int *foo(void) { ... }
(function_definition
  declarator: (pointer_declarator
    declarator: (function_declarator
      declarator: (identifier) @name))) @definition.function

; ── Struct/union specifiers ──────────────────────────────────────────────

; struct Foo { ... };
(struct_specifier
  name: (type_identifier) @name) @definition.type

; union Bar { ... };
(union_specifier
  name: (type_identifier) @name) @definition.type

; ── Enum specifiers ──────────────────────────────────────────────────────

(enum_specifier
  name: (type_identifier) @name) @definition.type

; ── Include directives (Synapse Imports edges) ───────────────────────────

; #include <stdio.h> or #include "foo.h"
(preproc_include
  path: [
    (system_lib_string) @name
    (string_literal) @name
  ]) @reference.import

; ── Call expressions (Synapse Calls edges) ──────────────────────────────

; foo(args)
(call_expression
  function: (identifier) @name) @reference.call

; ptr->method(args) or obj.field(args)
(call_expression
  function: (field_expression
    field: (field_identifier) @name)) @reference.call
