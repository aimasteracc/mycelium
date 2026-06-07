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

; ── Method / constructor declarations ───────────────────────────────────
; Anchor on the ENCLOSING type (class/interface/enum), not the method node:
; the extractor's build_class_chain treats the anchor as the innermost container
; and appends ITS name, so anchoring on `method_declaration` yields the wrong
; `Type>method>method` path (with `Type>method` left as a kindless intermediate).
; Anchoring on the type → build_class_chain = [Type], + @name (the method) →
; the correct `Type>method`. Mirrors the Python/TypeScript method patterns.

(class_declaration
  body: (class_body
    (method_declaration
      name: (identifier) @name))) @definition.method

(class_declaration
  body: (class_body
    (constructor_declaration
      name: (identifier) @name))) @definition.method

(interface_declaration
  body: (interface_body
    (method_declaration
      name: (identifier) @name))) @definition.method

; Enum bodies (methods/constructors live under enum_body_declarations).
(enum_declaration
  body: (enum_body
    (enum_body_declarations
      (method_declaration
        name: (identifier) @name)))) @definition.method

(enum_declaration
  body: (enum_body
    (enum_body_declarations
      (constructor_declaration
        name: (identifier) @name)))) @definition.method

; Records (Java 16+) — their body is a class_body.
(record_declaration
  body: (class_body
    (method_declaration
      name: (identifier) @name))) @definition.method

(record_declaration
  body: (class_body
    (constructor_declaration
      name: (identifier) @name))) @definition.method

; ── Import statements (Synapse Imports edges) ────────────────────────────

; import com.example.Foo;
(import_declaration
  (scoped_identifier) @name) @reference.import

; ── Class inheritance (Synapse Extends edges) ────────────────────────────

; class Sub extends Base
(class_declaration
  superclass: (superclass (type_identifier) @name)) @reference.extends

; interface Sub extends Base, OtherBase
(interface_declaration
  (extends_interfaces (type_list (type_identifier) @name))) @reference.extends

; ── Interface implementation (Synapse Implements edges) ──────────────────

; class Foo implements Bar, Baz
(class_declaration
  interfaces: (super_interfaces (type_list (type_identifier) @name))) @reference.implements

; ── Method invocations (Synapse Calls edges) ─────────────────────────────

; Simple: foo()
(method_invocation
  name: (identifier) @name) @reference.call

; Chained: obj.method()
(method_invocation
  object: (_)
  name: (identifier) @name) @reference.call

; ── RFC-0118 Part B: receiver capture + local declared-type bindings ──────
; Method call on a plain-identifier receiver: capture the receiver so the
; post-merge pass can infer its type and disambiguate a multi-class method.
; (Coexists with the bare method_invocation patterns above; the Calls edge is
; idempotent and only this pattern records a ReceiverContext.)
(method_invocation
  object: (identifier) @call.receiver
  name: (identifier) @name) @reference.call

; Local variable with an explicit DECLARED type (`Store s = …`). Java is
; statically typed, so the declared type is the receiver type regardless of the
; RHS — higher recall than matching `new T()`, and reassignment cannot change it
; (no @binding.rebind needed; Java has no same-name block shadowing either). The
; core keeps only Title-case types (Java convention), so primitives/`var` decline.
(local_variable_declaration
  type: (type_identifier) @binding.ctor
  declarator: (variable_declarator
    name: (identifier) @binding.local)) @reference.binding
