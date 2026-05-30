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

; ── Alias bindings (RFC-0092 — feeds the per-file alias table) ──────
;
; `import json as j`         → binds `j` to module `json`
; `from . import m as n`     → binds `n` to (resolved) sibling module
; `from M import X as Y`     → binds `Y` to `M>X`
;
; The @alias.local capture is the local identifier; @alias.source is the
; module or symbol it points to. The extractor combines them with bug
; #204's relative-import resolver to produce the final binding target.

(import_statement
  name: (aliased_import
    name: (dotted_name) @alias.source
    alias: (identifier) @alias.local)) @reference.alias_binding

(import_from_statement
  module_name: (_) @alias.source
  name: (aliased_import
    name: (_) @alias.original_name
    alias: (identifier) @alias.local)) @reference.alias_binding

; `from . import M` (no `as`) — binds `M` to the resolved sibling module.
(import_from_statement
  module_name: (relative_import) @alias.source
  name: (dotted_name) @alias.local) @reference.alias_binding

; Issue #229: attribute-assignment alias.
; `local = module_alias.fn` — binds `local` to `module_alias>fn`.
; If `module_alias` is itself a local alias (from any import-alias above),
; the extractor chain-resolves at call time.
(assignment
  left: (identifier) @alias.local
  right: (attribute
    object: (identifier) @alias.source
    attribute: (identifier) @alias.original_name)) @reference.alias_binding

; ── Call expressions (Synapse Calls edges) ──────────────────────────

; Simple function calls: foo()
(call
  function: (identifier) @name) @reference.call

; Method calls: obj.method() — capture both the receiver and the method
; so the extractor can rewrite obj via the alias table (RFC-0092).
(call
  function: (attribute
    object: (identifier) @call.receiver
    attribute: (identifier) @name)) @reference.call
