; Mycelium C++ capture queries
;
; Capture convention:
;   @definition.<kind>   — creates a node in the Trunk
;   @reference.<kind>    — creates an edge in the Synapse
;   @name                — the identifier used as the node path segment
;
; Tree-sitter grammar: tree-sitter-cpp ^0.23
; Covers: functions, methods, classes, structs, namespaces, enums,
;         templates, preprocessor includes, and call expressions.

; ── Source file (translation unit root) ─────────────────────────────

(translation_unit) @definition.module

; ── Free functions ───────────────────────────────────────────────────

(translation_unit
  (function_definition
    declarator: (function_declarator
      declarator: (identifier) @name))) @definition.function

; Pointer-returning free functions: int *foo()
(translation_unit
  (function_definition
    declarator: (pointer_declarator
      declarator: (function_declarator
        declarator: (identifier) @name)))) @definition.function

; ── Namespaces ───────────────────────────────────────────────────────

(namespace_definition
  name: (namespace_identifier) @name) @definition.namespace

; ── Classes ──────────────────────────────────────────────────────────

(class_specifier
  name: (type_identifier) @name) @definition.class

; ── Structs ──────────────────────────────────────────────────────────

(struct_specifier
  name: (type_identifier) @name) @definition.struct

; ── Enums ────────────────────────────────────────────────────────────

(enum_specifier
  name: (type_identifier) @name) @definition.enum

; ── Methods (functions inside class/struct body) ─────────────────────

(field_declaration_list
  (function_definition
    declarator: (function_declarator
      declarator: (field_identifier) @name))) @definition.method

; ── Template declarations ─────────────────────────────────────────────

(template_declaration
  (class_specifier
    name: (type_identifier) @name)) @definition.template_class

(template_declaration
  (function_definition
    declarator: (function_declarator
      declarator: (identifier) @name))) @definition.template_function

; ── Preprocessor includes (Synapse Imports edges) ────────────────────

(preproc_include
  path: (string_literal) @name) @reference.import

(preproc_include
  path: (system_lib_string) @name) @reference.import

; ── Call expressions (Synapse Calls edges) ───────────────────────────

; Simple calls: foo()
(call_expression
  function: (identifier) @name) @reference.call

; Qualified calls: ns::foo()
(call_expression
  function: (qualified_identifier
    name: (identifier) @name)) @reference.call

; Method calls: obj.method() or ptr->method()
(call_expression
  function: (field_expression
    field: (field_identifier) @name)) @reference.call
