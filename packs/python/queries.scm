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

; ── Call expressions (Synapse Calls edges) ──────────────────────────

; Simple function calls: foo()
(call
  function: (identifier) @name) @reference.call

; Method calls: obj.method() — capture the method name only
(call
  function: (attribute
    attribute: (identifier) @name)) @reference.call
