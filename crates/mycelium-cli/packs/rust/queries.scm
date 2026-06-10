; Mycelium Rust capture queries
;
; Capture convention:
;   @definition.<kind>   — creates a node in the Trunk
;   @reference.<kind>    — creates an edge in the Synapse
;   @name                — the identifier used as the node path segment
;
; Tree-sitter grammar: tree-sitter-rust ^0.24
; Requires tree-sitter C runtime >= 0.25

; ── Source file (module root) ────────────────────────────────────────

(source_file) @definition.module

; ── Free functions ───────────────────────────────────────────────────

(source_file
  (function_item
    name: (identifier) @name)) @definition.function

; ── Structs ──────────────────────────────────────────────────────────

(source_file
  (struct_item
    name: (type_identifier) @name)) @definition.struct

; ── Enums ────────────────────────────────────────────────────────────

(source_file
  (enum_item
    name: (type_identifier) @name)) @definition.enum

; ── Traits ───────────────────────────────────────────────────────────

(source_file
  (trait_item
    name: (type_identifier) @name)) @definition.trait

; ── Type aliases ─────────────────────────────────────────────────────

(source_file
  (type_item
    name: (type_identifier) @name)) @definition.type_alias

; ── Const items ──────────────────────────────────────────────────────

(source_file
  (const_item
    name: (identifier) @name)) @definition.const

; ── Inline module declarations ───────────────────────────────────────

(source_file
  (mod_item
    name: (identifier) @name)) @definition.mod

; ── Methods and associated functions inside impl blocks ─────────────

(impl_item
  body: (declaration_list
    (function_item
      name: (identifier) @name))) @definition.method

; ── Trait method declarations (signatures with no body) ──────────────
;
; `trait Foo { fn bar(); }` — tree-sitter-rust models this as
; trait_item > body: declaration_list > function_signature_item.
; Previously only the trait name was captured; the method declarations
; were silently dropped. Dogfood-found 2026-06-03 on
; `FileReindexer::reindex`.

(trait_item
  body: (declaration_list
    (function_signature_item
      name: (identifier) @name))) @definition.method

; ── Trait default-method bodies (impl on the trait itself) ──────────
;
; `trait Foo { fn bar() { default_impl } }` — also valid Rust.

(trait_item
  body: (declaration_list
    (function_item
      name: (identifier) @name))) @definition.method

; ── Static items (module-level) ──────────────────────────────────────
;
; `static FOO: T = ...;` — previously only `const` was captured.
; Dogfood-found 2026-06-03 on `static PACK_REGISTRY: OnceLock<...>`.

(source_file
  (static_item
    name: (identifier) @name)) @definition.static

; ── Associated constants on impl blocks ─────────────────────────────
;
; `impl NodeId { pub const NULL: Self = Self(0); }`
; Previously the const was dropped because only `source_file > const_item`
; was matched. Dogfood-found 2026-06-03 on `NodeId::NULL`.

(impl_item
  body: (declaration_list
    (const_item
      name: (identifier) @name))) @definition.associated_const

; ── Associated types on impl blocks ─────────────────────────────────
;
; `impl Trait for Foo { type Output = Bar; }` — frequently used in
; trait implementations. Captured for navigation parity with methods.

(impl_item
  body: (declaration_list
    (type_item
      name: (type_identifier) @name))) @definition.associated_type

; ── Functions and items inside nested module blocks ─────────────────
;
; `mod tests { fn foo() {} fn bar() {} }` — test modules are the dominant
; case. Previously only `source_file > function_item` was captured, so
; functions inside `mod tests` were silently missed when they happened
; to be at certain positions in the body. Catching them explicitly via
; `mod_item > declaration_list > function_item` closes the remaining
; coverage gap. Dogfood-found 2026-06-03 on `types.rs` where ~10 of 12
; test fns were missed.

(mod_item
  body: (declaration_list
    (function_item
      name: (identifier) @name))) @definition.function

(mod_item
  body: (declaration_list
    (struct_item
      name: (type_identifier) @name))) @definition.struct

(mod_item
  body: (declaration_list
    (const_item
      name: (identifier) @name))) @definition.const

; ── Use declarations (Synapse Imports edges) ─────────────────────────

(source_file
  (use_declaration
    argument: (_) @name)) @reference.import

; ── Call expressions (Synapse Calls edges) ──────────────────────────

; Simple calls: foo()
(call_expression
  function: (identifier) @name) @reference.call

; Method calls: self.method() / obj.method()
; @call.receiver captures the receiver expression (RFC-0118 Part B) so the
; post-merge pass can infer its type and disambiguate multi-match methods.
(call_expression
  function: (field_expression
    value: (_) @call.receiver
    field: (field_identifier) @name)) @reference.call

; Scoped / qualified path calls: Type::method() / crate::mod::func()
;
; The captured @name is the LAST identifier (the function or method name).
; Cross-file resolution by `resolve_bare_call_stubs` later links it to the
; concrete definition once both files have been indexed.
;
; Added 2026-06-03 after dogfooding the Mycelium repo against itself surfaced
; that `WatchEngine::drive(...)` produced no Calls edges — the previous query
; only matched `(identifier)` and `(field_expression)` function forms.
(call_expression
  function: (scoped_identifier
    name: (identifier) @name)) @reference.call

; ── RFC-0118 Part B: local constructor bindings (let x = Type::new()) ──
; Captures the local name and the constructor TYPE so the post-merge receiver
; disambiguation pass can bind `x.method()` to `…>Type>method`. Only
; constructor-style RHS (`Type::func(...)`) is matched, never plain `foo()` —
; conservative. `let mut x = ...` also matches: tree-sitter-rust models `mut` as
; a `mutable_specifier` sibling of the `pattern` field, so `pattern:(identifier)`
; still binds the name. The core then keeps only title-case ctor types, dropping
; lowercase utility-module calls like `mem::take`.
(let_declaration
  pattern: (identifier) @binding.local
  value: (call_expression
    function: (scoped_identifier
      path: (identifier) @binding.ctor))) @reference.binding

; ── RFC-0122: function return-type annotation (free functions) ───────────
; `fn get_store() -> Store` → capture fn name + return type so Store helper
; can answer `return_type_of("get_store") = "Store"` for rule-f enrichment.
(source_file
  (function_item
    name: (identifier) @fn.name
    return_type: (type_identifier) @fn.return_type)) @reference.fn_return

; ── RFC-0122 rule-f prerequisite: plain function-call binding ────────────
; `let s = get_store()` → capture `s` + callee `get_store` so the post-merge
; enrich_context pass can look up get_store's return type and synthesise ctor_type.
; Unscoped calls only (Type::new() is already matched above as @binding.ctor).
(let_declaration
  pattern: (identifier) @binding.local
  value: (call_expression
    function: (identifier) @binding.fn_call)) @reference.fn_binding

; ── RFC-0118 Part B rule-b: function parameters with declared types ──────
; `fn f(s: &mut Store)` → bind `s` to `Store` so `s.method()` resolves. The core
; normalizes the type (strips &, mut, lifetimes, generics) and keeps only
; Title-case base types. Scoped to the enclosing function; a param shadowed by a
; conflicting local declines (never mis-bind).
(parameter
  pattern: (identifier) @param.name
  type: (_) @param.type) @reference.param
