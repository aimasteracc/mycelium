; Mycelium Java capture queries
;
; Capture convention:
;   @definition.<kind>   — creates a node in the Trunk
;   @reference.<kind>    — creates an edge in the Synapse
;   @name                — the identifier used as the node path segment
;
; Tree-sitter grammar: tree-sitter-java ^0.23
; Requires tree-sitter C runtime >= 0.22

; ── Compilation unit (file root) ────────────────────────────────────────

(program) @definition.module

; ── Class declarations ───────────────────────────────────────────────────

(class_declaration
  name: (identifier) @name) @definition.class

; ── Interface declarations ───────────────────────────────────────────────

(interface_declaration
  name: (identifier) @name) @definition.interface

; ── Enum declarations ────────────────────────────────────────────────────

(enum_declaration
  name: (identifier) @name) @definition.class

; ── Method declarations (inside class/interface/enum bodies) ─────────────

(method_declaration
  name: (identifier) @name) @definition.method

; ── Constructor declarations ─────────────────────────────────────────────

(constructor_declaration
  name: (identifier) @name) @definition.method

; ── Import statements (Synapse Imports edges) ────────────────────────────

; import com.example.Foo;
(import_declaration
  (scoped_identifier) @name) @reference.import

; ── Method invocations (Synapse Calls edges) ─────────────────────────────

; Simple: foo()
(method_invocation
  name: (identifier) @name) @reference.call

; Chained: obj.method()
(method_invocation
  object: (_)
  name: (identifier) @name) @reference.call
