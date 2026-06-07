; Mycelium C# capture queries
;
; Capture convention:
;   @definition.<kind>   — creates a node in the Trunk
;   @reference.<kind>    — creates an edge in the Synapse
;   @name                — the identifier used as the node path segment
;
; Tree-sitter grammar: tree-sitter-c-sharp ^0.23
; Covers: classes, interfaces, methods, namespaces, enums, structs,
;         using directives, and invocation expressions.
;
; Partial classes: each partial class_declaration is treated as a
; separate node.  Cross-file merging is deferred to a future RFC.

; ── Source file (compilation unit root) ─────────────────────────────

(compilation_unit) @definition.module

; ── Namespace declarations ───────────────────────────────────────────

(namespace_declaration
  name: (_) @name) @definition.namespace

; ── Class declarations ───────────────────────────────────────────────

(class_declaration
  name: (identifier) @name) @definition.class

; ── Interface declarations ───────────────────────────────────────────

(interface_declaration
  name: (identifier) @name) @definition.interface

; ── Struct declarations ──────────────────────────────────────────────

(struct_declaration
  name: (identifier) @name) @definition.struct

; ── Enum declarations ────────────────────────────────────────────────

(enum_declaration
  name: (identifier) @name) @definition.enum

; ── Method / constructor declarations ────────────────────────────────
; Anchor on the ENCLOSING type body so build_class_chain yields `Type>member`
; (anchoring on the member node yields the wrong `Type>member>member`, and a
; bare constructor anchored flat collided with the class node). Mirrors Java.

(class_declaration
  body: (declaration_list
    (method_declaration name: (identifier) @name))) @definition.method
(struct_declaration
  body: (declaration_list
    (method_declaration name: (identifier) @name))) @definition.method
(interface_declaration
  body: (declaration_list
    (method_declaration name: (identifier) @name))) @definition.method
(record_declaration
  body: (declaration_list
    (method_declaration name: (identifier) @name))) @definition.method

(class_declaration
  body: (declaration_list
    (constructor_declaration name: (identifier) @name))) @definition.method
(struct_declaration
  body: (declaration_list
    (constructor_declaration name: (identifier) @name))) @definition.method
(record_declaration
  body: (declaration_list
    (constructor_declaration name: (identifier) @name))) @definition.method

; ── Using directives (Synapse Imports edges) ─────────────────────────

; Simple: using System;
(using_directive
  (identifier) @name) @reference.import

; Qualified: using System.Collections.Generic;
(using_directive
  (qualified_name) @name) @reference.import

; ── Invocation expressions (Synapse Calls edges) ─────────────────────

; Simple calls: DoWork()
(invocation_expression
  function: (identifier) @name) @reference.call

; Member calls: obj.Method() or this.Helper()
(invocation_expression
  function: (member_access_expression
    name: (identifier) @name)) @reference.call

; ── RFC-0118 Part B: receiver capture + declared-type local bindings ──────
; Method call on a plain-identifier receiver: capture it for type inference.
(invocation_expression
  function: (member_access_expression
    expression: (identifier) @call.receiver
    name: (identifier) @name)) @reference.call

; Local with an explicit DECLARED type (`Store s = …`). C# is statically typed,
; so the declared type is authoritative regardless of RHS (no @binding.rebind);
; the core keeps only Title-case types so `var`/primitives decline.
(variable_declaration
  type: (identifier) @binding.ctor
  (variable_declarator
    name: (identifier) @binding.local)) @reference.binding
