; Mycelium Ruby capture queries
;
; Capture convention:
;   @definition.<kind>   — creates a node in the Trunk
;   @reference.<kind>    — creates an edge in the Synapse
;   @name                — the identifier used as the node path segment
;
; Tree-sitter grammar: tree-sitter-ruby ^0.23
; Requires tree-sitter C runtime >= 0.22
;
; AST shape notes:
;   (program)
;   (class name: (constant) body: (body_statement (method name: (identifier))))
;   (module name: (constant) body: (body_statement (method name: (identifier))))

; ── Program (file root) ─────────────────────────────────────────────────

(program) @definition.module

; ── Class definitions ────────────────────────────────────────────────────

(class
  name: (constant) @name) @definition.class

; Namespaced class: Foo::Bar
(class
  name: (scope_resolution) @name) @definition.class

; ── Module definitions ───────────────────────────────────────────────────

(module
  name: (constant) @name) @definition.class

; Namespaced module: Foo::Bar
(module
  name: (scope_resolution) @name) @definition.class

; ── Method definitions ───────────────────────────────────────────────────

; Instance methods — inside class/module body_statement
(class
  body: (body_statement
    (method
      name: (identifier) @name))) @definition.method

(module
  body: (body_statement
    (method
      name: (identifier) @name))) @definition.method

; Singleton/class methods — def self.foo
(class
  body: (body_statement
    (singleton_method
      name: (identifier) @name))) @definition.method

(module
  body: (body_statement
    (singleton_method
      name: (identifier) @name))) @definition.method

; Top-level methods
(program
  (method
    name: (identifier) @name)) @definition.function

; ── require / require_relative (Synapse Imports edges) ──────────────────

; require 'foo' or require_relative 'bar'
(call
  method: (identifier) @_method
  arguments: (argument_list
    (string
      (string_content) @name))
  (#match? @_method "^require")) @reference.import

; ── Method calls / send (Synapse Calls edges) ────────────────────────────

; receiver.method(args)
(call
  receiver: (_)
  method: (identifier) @name) @reference.call

; standalone method calls
(call
  method: (identifier) @name) @reference.call
