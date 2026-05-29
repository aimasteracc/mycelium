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

; ── Method declarations ──────────────────────────────────────────────

(method_declaration
  name: (identifier) @name) @definition.method

; ── Constructor declarations ─────────────────────────────────────────

(constructor_declaration
  name: (identifier) @name) @definition.constructor

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
