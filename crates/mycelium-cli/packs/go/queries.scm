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
;
; NOTE on @definition.* anchoring (PR #750 follow-up, 2026-06-11): the
; @definition capture sits on the ITEM node (the function_declaration, or
; the per-name type_spec/const_spec/var_spec inside grouped declarations),
; NOT on the file-root `source_file` — the extractor takes the SPAN from
; the @definition anchor, so anchoring on the root made every top-level
; symbol's span cover the whole file. Anchoring on the *_spec (not the
; *_declaration) gives each name in a grouped `type (…)` / `const (…)` /
; `var (…)` block its own span. Paths derive from @name text only, so
; re-anchoring changes spans only.

(source_file
  (function_declaration
    name: (identifier) @name) @definition.function)

; ── Methods (receiver functions) ─────────────────────────────────────
; A Go method's receiver type is its container: `func (s *Server) Run()` →
; Server>Run. The extractor reads the receiver type from the method_declaration
; `receiver` field (go_receiver_type), so anchor @definition.method on the
; method node itself (not source_file).

(method_declaration
  name: (field_identifier) @name) @definition.method

; ── Type declarations (struct, interface, alias, etc.) ───────────────

(source_file
  (type_declaration
    (type_spec
      name: (type_identifier) @name) @definition.type))

; ── Constants ────────────────────────────────────────────────────────

(source_file
  (const_declaration
    (const_spec
      name: (identifier) @name) @definition.const))

; ── Variables (package-level) ────────────────────────────────────────

(source_file
  (var_declaration
    (var_spec
      name: (identifier) @name) @definition.variable))

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

; ── RFC-0118 Part B: receiver capture + local composite-literal bindings ──
; Method call on a plain-identifier receiver: capture it for type inference.
(call_expression
  function: (selector_expression
    operand: (identifier) @call.receiver
    field: (field_identifier) @name)) @reference.call

; Local bound to a composite literal: `s := Server{}` (and `s := &Server{}`).
; The literal's type is the receiver type; the core keeps only Title-case types
; (exported Go types are Title-case) and declines on reassignment conflicts.
(short_var_declaration
  left: (expression_list (identifier) @binding.local)
  right: (expression_list
    (composite_literal
      type: (type_identifier) @binding.ctor))) @reference.binding

(short_var_declaration
  left: (expression_list (identifier) @binding.local)
  right: (expression_list
    (unary_expression
      operand: (composite_literal
        type: (type_identifier) @binding.ctor)))) @reference.binding

; Rebind invalidation: any `:=` / `=` to a plain identifier (Go allows reusing a
; name with `:=` in a new scope and reassigning with `=`); decline if a bound
; name is later reassigned to a non-constructor.
(short_var_declaration
  left: (expression_list (identifier) @binding.rebind)) @reference.binding
(assignment_statement
  left: (expression_list (identifier) @binding.rebind)) @reference.binding
