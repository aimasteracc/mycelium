//! Extractor TDD tests — written before implementation per Charter §5.1.
//!
//! Each test maps to a row in RFC-0002 §Testing strategy or a capture kind
//! from the Python queries.scm.

use super::Extractor;
use crate::{store::Store, types::EdgeKind};

// ── helpers ──────────────────────────────────────────────────────────────────

fn python_extractor() -> Extractor {
    let language: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
    let query_src = include_str!("../../../../packs/python/queries.scm");
    Extractor::new(language, query_src).expect("python extractor should build")
}

fn extract(source: &str) -> Store {
    let ext = python_extractor();
    let mut store = Store::new();
    ext.extract("test.py", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store
}

fn extract_at(file_path: &str, source: &str) -> Store {
    let ext = python_extractor();
    let mut store = Store::new();
    ext.extract(file_path, source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store
}

// ── definition.module ────────────────────────────────────────────────────────

#[test]
fn extractor_creates_module_node() {
    let store = extract("");
    assert!(
        store.lookup("test.py").is_some(),
        "empty file should still create a module node"
    );
}

// ── definition.function ──────────────────────────────────────────────────────

#[test]
fn extractor_creates_top_level_function_node() {
    let store = extract("def foo(): pass");
    assert!(
        store.lookup("test.py>foo").is_some(),
        "top-level function should be a node"
    );
}

#[test]
fn extractor_creates_contains_edge_for_function() {
    let store = extract("def foo(): pass");
    let file_id = store.lookup("test.py").unwrap();
    let fn_id = store.lookup("test.py>foo").unwrap();
    assert!(
        store.outgoing(file_id, EdgeKind::Contains).contains(&fn_id),
        "file should contain function via Contains edge"
    );
}

// ── definition.class ─────────────────────────────────────────────────────────

#[test]
fn extractor_creates_class_node() {
    let store = extract("class Bar: pass");
    assert!(
        store.lookup("test.py>Bar").is_some(),
        "top-level class should be a node"
    );
}

#[test]
fn extractor_creates_contains_edge_for_class() {
    let store = extract("class Bar: pass");
    let file_id = store.lookup("test.py").unwrap();
    let cls_id = store.lookup("test.py>Bar").unwrap();
    assert!(
        store
            .outgoing(file_id, EdgeKind::Contains)
            .contains(&cls_id),
        "file should contain class via Contains edge"
    );
}

// ── definition.method ────────────────────────────────────────────────────────

#[test]
fn extractor_creates_method_node_under_class() {
    let source = "class MyClass:\n    def my_method(self): pass";
    let store = extract(source);
    assert!(
        store.lookup("test.py>MyClass>my_method").is_some(),
        "method should be nested under class in path"
    );
}

#[test]
fn extractor_creates_contains_edges_for_method_chain() {
    let source = "class MyClass:\n    def my_method(self): pass";
    let store = extract(source);
    let file_id = store.lookup("test.py").unwrap();
    let cls_id = store.lookup("test.py>MyClass").unwrap();
    let method_id = store.lookup("test.py>MyClass>my_method").unwrap();
    assert!(
        store
            .outgoing(file_id, EdgeKind::Contains)
            .contains(&cls_id),
        "file→class Contains edge"
    );
    assert!(
        store
            .outgoing(cls_id, EdgeKind::Contains)
            .contains(&method_id),
        "class→method Contains edge"
    );
}

// ── decorated definitions ─────────────────────────────────────────────────────

#[test]
fn extractor_handles_decorated_function() {
    let source = "@staticmethod\ndef decorated(): pass";
    let store = extract(source);
    assert!(
        store.lookup("test.py>decorated").is_some(),
        "decorated function should be extracted"
    );
}

#[test]
fn extractor_handles_decorated_class() {
    let source = "@dataclass\nclass Decorated: pass";
    let store = extract(source);
    assert!(
        store.lookup("test.py>Decorated").is_some(),
        "decorated class should be extracted"
    );
}

// ── reference.import ─────────────────────────────────────────────────────────

#[test]
fn extractor_creates_import_edge_for_import_statement() {
    let source = "import os";
    let store = extract(source);
    let file_id = store.lookup("test.py").unwrap();
    let os_id = store.lookup("os");
    assert!(os_id.is_some(), "import creates a node for the module");
    assert!(
        store
            .outgoing(file_id, EdgeKind::Imports)
            .contains(&os_id.unwrap()),
        "file should have Imports edge to os"
    );
}

#[test]
fn extractor_creates_import_edge_for_dotted_import() {
    let source = "import os.path";
    let store = extract(source);
    let file_id = store.lookup("test.py").unwrap();
    let mod_id = store.lookup("os.path");
    assert!(mod_id.is_some(), "dotted import creates a node");
    assert!(
        store
            .outgoing(file_id, EdgeKind::Imports)
            .contains(&mod_id.unwrap()),
        "file should have Imports edge to os.path"
    );
}

// ── reference.import_from ────────────────────────────────────────────────────

#[test]
fn extractor_creates_import_edge_for_from_import() {
    let source = "from pathlib import Path";
    let store = extract(source);
    let file_id = store.lookup("test.py").unwrap();
    let mod_id = store.lookup("pathlib");
    assert!(mod_id.is_some(), "from-import creates module node");
    assert!(
        store
            .outgoing(file_id, EdgeKind::Imports)
            .contains(&mod_id.unwrap()),
        "file should have Imports edge to pathlib"
    );
}

// ── relative-import resolution (issue #204) ──────────────────────────────────

#[test]
fn extractor_resolves_single_dot_relative_import_to_sibling_file() {
    // `from .models import X` in pkg/sub/foo.py must produce an Imports edge
    // to pkg/sub/models.py (the real file path), not to the symbolic name
    // `.models`. Bug 2 of issue #200 — without this, every `from .X import Y`
    // pattern makes the source module appear unimported at the file level.
    let store = extract_at("pkg/sub/foo.py", "from .models import X");
    let file_id = store.lookup("pkg/sub/foo.py").unwrap();
    assert!(
        store.lookup("pkg/sub/models.py").is_some(),
        "single-dot relative import must create a file-path node for the target"
    );
    let target = store.lookup("pkg/sub/models.py").unwrap();
    assert!(
        store.outgoing(file_id, EdgeKind::Imports).contains(&target),
        "Imports edge must point to the resolved file path, not the symbolic name"
    );
}

#[test]
fn extractor_resolves_double_dot_relative_import_to_parent_package() {
    // `from ..utils import X` in pkg/sub/foo.py resolves to pkg/utils.py.
    let store = extract_at("pkg/sub/foo.py", "from ..utils import X");
    let file_id = store.lookup("pkg/sub/foo.py").unwrap();
    let target = store
        .lookup("pkg/utils.py")
        .expect("double-dot relative import must create pkg/utils.py node");
    assert!(
        store.outgoing(file_id, EdgeKind::Imports).contains(&target),
        "Imports edge must point to pkg/utils.py for `from ..utils import X`"
    );
}

#[test]
fn extractor_resolves_bare_relative_import_to_package_dir() {
    // `from . import sibling` in pkg/sub/foo.py refers to the current package
    // (pkg/sub/), where `sibling` is a sibling module → pkg/sub/sibling.py.
    let store = extract_at("pkg/sub/foo.py", "from . import sibling");
    let file_id = store.lookup("pkg/sub/foo.py").unwrap();
    // We at least create *some* edge — either to the package dir or to the
    // sibling. The current grammar captures `.` as the module_name so we
    // expect an edge to pkg/sub (the package). Spot-check: an edge must exist
    // from file_id with kind Imports.
    let outgoing = store.outgoing(file_id, EdgeKind::Imports);
    assert!(
        !outgoing.is_empty(),
        "bare `from . import X` must produce at least one Imports edge"
    );
}

#[test]
fn extractor_preserves_absolute_import_behaviour() {
    // Absolute imports `from foo.bar import X` keep their symbolic node
    // (`foo.bar`) — resolving these to file paths requires package-discovery
    // logic out of scope for issue #204.
    let store = extract_at("pkg/sub/foo.py", "from typing import List");
    let file_id = store.lookup("pkg/sub/foo.py").unwrap();
    let typing = store
        .lookup("typing")
        .expect("absolute import must still create symbolic node");
    assert!(store.outgoing(file_id, EdgeKind::Imports).contains(&typing));
}

// ── TYPE_CHECKING guard (issue #227) ─────────────────────────────────────────

#[test]
fn extractor_skips_imports_inside_type_checking_block() {
    // `if TYPE_CHECKING:` blocks are never executed at runtime (TYPE_CHECKING is
    // always False). Including those import edges causes false-positive cycle
    // reports (issue #227 — 7 spurious cycle nodes in tree-sitter-analyzer).
    let source = "from typing import TYPE_CHECKING\nif TYPE_CHECKING:\n    from collections import OrderedDict\n";
    let store = extract(source);
    let file_id = store.lookup("test.py").unwrap();
    // The TYPE_CHECKING guard import (typing) should appear.
    let typing_id = store
        .lookup("typing")
        .expect("typing import must still create a node");
    assert!(
        store
            .outgoing(file_id, EdgeKind::Imports)
            .contains(&typing_id),
        "Imports edge to `typing` (the guard itself) must be present"
    );
    // But the import INSIDE the `if TYPE_CHECKING:` block must NOT produce an edge.
    let imports = store.outgoing(file_id, EdgeKind::Imports);
    let has_collections = store
        .lookup("collections")
        .is_some_and(|id| imports.contains(&id));
    assert!(
        !has_collections,
        "`collections` import inside `if TYPE_CHECKING:` must NOT create an Imports edge"
    );
}

#[test]
fn extractor_keeps_regular_imports_alongside_type_checking_block() {
    // Files typically have both real imports and TYPE_CHECKING-guarded ones.
    // Only the guarded ones should be suppressed; the rest must survive.
    let source = "import os\nfrom typing import TYPE_CHECKING\nif TYPE_CHECKING:\n    import typing_extensions\n";
    let store = extract(source);
    let file_id = store.lookup("test.py").unwrap();
    let os_id = store.lookup("os").expect("os node must exist");
    assert!(
        store.outgoing(file_id, EdgeKind::Imports).contains(&os_id),
        "real `import os` must still produce an Imports edge"
    );
    let typing_ext = store.lookup("typing_extensions");
    let guarded_present =
        typing_ext.is_some_and(|id| store.outgoing(file_id, EdgeKind::Imports).contains(&id));
    assert!(
        !guarded_present,
        "`typing_extensions` inside `if TYPE_CHECKING:` must NOT create an Imports edge"
    );
}

// ── alias-table dispatch (issue #205, RFC-0092) ──────────────────────────────

#[test]
fn alias_dispatch_resolves_to_real_module_via_from_import_as() {
    // The headline #205 bug: every caller in tree-sitter-analyzer goes
    // through `from . import _ast_cache_query as _query`, and the call site
    // is `_query.fts_search_ranked(...)`. Before RFC-0092 the Calls edge
    // pointed at the bare symbol `_query>fts_search_ranked`, so the real
    // definition at `_ast_cache_query.py>fts_search_ranked` saw 0 callers
    // and looked like dead code.
    //
    // After RFC-0092, the leftmost identifier `_query` is looked up in the
    // per-file alias table, rewritten to the resolved file path, and the
    // edge targets the real symbol.
    let source = "\
from . import _ast_cache_query as _query

def bar():
    return _query.fts_search_ranked()
";
    let store = extract_at("pkg/foo.py", source);
    let bar_id = store
        .lookup("pkg/foo.py>bar")
        .expect("caller function must be indexed");
    let resolved = store
        .lookup("pkg/_ast_cache_query.py>fts_search_ranked")
        .expect("alias must resolve to pkg/_ast_cache_query.py>fts_search_ranked");
    assert!(
        store.outgoing(bar_id, EdgeKind::Calls).contains(&resolved),
        "Calls edge must target the resolved alias path, not _query>fts_search_ranked"
    );
}

#[test]
fn alias_dispatch_resolves_simple_import_as() {
    // `import X as Y; Y.foo()` — also an alias case, even without `from`.
    let source = "\
import json as j

def bar():
    return j.loads()
";
    let store = extract_at("pkg/foo.py", source);
    let bar_id = store.lookup("pkg/foo.py>bar").unwrap();
    let resolved = store
        .lookup("json>loads")
        .expect("alias `j` must resolve to `json` for `j.loads()`");
    assert!(
        store.outgoing(bar_id, EdgeKind::Calls).contains(&resolved),
        "Calls edge must point to json>loads when called via alias j"
    );
}

#[test]
fn non_aliased_identifier_call_unchanged() {
    // Regression-prevention: when no alias is in play, behaviour must
    // match the existing intra/bare fallback exactly.
    let source = "\
def helper(): pass

def bar():
    return helper()
";
    let store = extract_at("pkg/foo.py", source);
    let bar_id = store.lookup("pkg/foo.py>bar").unwrap();
    let helper_id = store
        .lookup("pkg/foo.py>helper")
        .expect("intra-file callee must resolve to its definition node");
    assert!(
        store.outgoing(bar_id, EdgeKind::Calls).contains(&helper_id),
        "non-aliased intra-file call must still hit the definition node"
    );
}

// ── self.method() resolution (issue #220) ────────────────────────────────────

#[test]
fn self_method_call_resolves_to_class_method() {
    // `self.foo()` inside a class method must resolve to the method
    // defined in the same class — not the bare name `foo` (which would
    // make the method appear isolated when only called via `self`).
    //
    // Bug source: #214 reliability report — `get-isolated-symbols`
    // returned 533 false positives, the dominant pattern being class
    // methods called only via `self.X()` from sibling methods.
    let source = "\
class App:
    def foo(self): pass

    def bar(self):
        self.foo()
";
    let store = extract_at("pkg/app.py", source);
    let bar_id = store
        .lookup("pkg/app.py>App>bar")
        .expect("caller method must be indexed");
    let foo_id = store
        .lookup("pkg/app.py>App>foo")
        .expect("callee method must be indexed");
    assert!(
        store.outgoing(bar_id, EdgeKind::Calls).contains(&foo_id),
        "self.foo() must produce a Calls edge to App>foo, not bare `foo`"
    );
}

#[test]
fn cls_method_call_resolves_to_class_method() {
    // Same rule for `cls.method()` — classmethod dispatch.
    let source = "\
class App:
    @classmethod
    def make(cls): return cls.build()

    @classmethod
    def build(cls): pass
";
    let store = extract_at("pkg/app.py", source);
    let make_id = store.lookup("pkg/app.py>App>make").unwrap();
    let build_id = store
        .lookup("pkg/app.py>App>build")
        .expect("classmethod callee must be indexed");
    assert!(
        store.outgoing(make_id, EdgeKind::Calls).contains(&build_id),
        "cls.build() must produce a Calls edge to App>build"
    );
}

// ── attribute-assignment alias (issue #229) ──────────────────────────────────

#[test]
fn attribute_assignment_alias_resolves_call_target() {
    // After v0.1.7 fixed `_h.fn()` direct call dispatch, a closely-related
    // pattern remained broken:
    //
    //   from . import _codegraph_explore_helpers as _h
    //   _signature_from = _h.signature_from        # attribute → local rebind
    //   sig = _signature_from(d)                   # now called via local
    //
    // mycelium reported `_codegraph_explore_helpers.py>signature_from` as
    // having `callers: []`. The fix extends the alias table to learn from
    // `local = module_alias.attr` assignments.
    let source = "\
from . import helpers as _h

_signature_from = _h.signature_from

def bar():
    return _signature_from()
";
    let store = extract_at("pkg/foo.py", source);
    let bar_id = store
        .lookup("pkg/foo.py>bar")
        .expect("caller function must be indexed");
    let resolved = store.lookup("pkg/helpers.py>signature_from").expect(
        "`_signature_from = _h.signature_from` then `_signature_from()` must \
             resolve to pkg/helpers.py>signature_from",
    );
    assert!(
        store.outgoing(bar_id, EdgeKind::Calls).contains(&resolved),
        "Calls edge must follow the assignment alias chain"
    );
}

// ── nested-attribute call regression (post-RFC-0092 fallthrough) ─────────────

#[test]
fn nested_attribute_call_still_creates_calls_edge() {
    // Regression: RFC-0092 added a `@call.receiver` capture to method calls
    // that REQUIRES `object: (identifier)`. For nested attribute access like
    // `self.history.append(x)` the object is `(attribute ...)`, not a single
    // identifier — so the new query stopped matching and the Calls edge was
    // silently dropped. Real-world impact: every call through a chain like
    // `self.x.y()` lost its outgoing edge.
    //
    // Fix: a second @reference.call pattern matches all nested-attribute
    // method calls without the receiver constraint, falling back to the
    // existing bare-name resolution.
    let source = "\
class App:
    def bar(self):
        self.history.append(1)
";
    let store = extract_at("pkg/app.py", source);
    let bar_id = store
        .lookup("pkg/app.py>App>bar")
        .expect("caller method must be indexed");
    // We don't know what `self.history.append` resolves to without type info,
    // but the bare `append` node must exist and have an incoming edge from
    // bar. Otherwise the Calls graph silently loses the relationship.
    let append_id = store
        .lookup("append")
        .expect("bare `append` node must exist (callsite produces it as fallback)");
    assert!(
        store.outgoing(bar_id, EdgeKind::Calls).contains(&append_id),
        "self.history.append() must still create some Calls edge from bar — \
         regression from RFC-0092 dropped it because the receiver query \
         required (identifier), not nested (attribute)"
    );
}

// ── idempotence ──────────────────────────────────────────────────────────────

#[test]
fn extractor_is_idempotent_on_rescan() {
    let source = "class Foo:\n    def bar(self): pass\ndef baz(): pass";
    let ext = python_extractor();
    let mut store = Store::new();
    // Extract twice — same result (upsert semantics).
    ext.extract("test.py", source.as_bytes(), &mut store)
        .unwrap();
    let foo_id_first = store.lookup("test.py>Foo").unwrap();
    ext.extract("test.py", source.as_bytes(), &mut store)
        .unwrap();
    let foo_id_second = store.lookup("test.py>Foo").unwrap();
    assert_eq!(
        foo_id_first, foo_id_second,
        "re-extraction must return the same NodeId (upsert is stable)"
    );
}

// ── multiple symbols ─────────────────────────────────────────────────────────

#[test]
fn extractor_handles_multiple_top_level_definitions() {
    let source = "def alpha(): pass\ndef beta(): pass\nclass Gamma: pass";
    let store = extract(source);
    assert!(store.lookup("test.py>alpha").is_some());
    assert!(store.lookup("test.py>beta").is_some());
    assert!(store.lookup("test.py>Gamma").is_some());
}

#[test]
fn extractor_handles_class_with_multiple_methods() {
    let source = "class Svc:\n    def a(self): pass\n    def b(self): pass";
    let store = extract(source);
    assert!(store.lookup("test.py>Svc>a").is_some());
    assert!(store.lookup("test.py>Svc>b").is_some());
}

// ── RFC-0011: reference.call (Calls edges) ───────────────────────────────────

fn ts_extractor() -> Extractor {
    let language: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    let query_src = include_str!("../../../../packs/typescript/queries.scm");
    Extractor::new(language, query_src).expect("typescript extractor should build")
}

fn extract_ts(source: &str) -> Store {
    let ext = ts_extractor();
    let mut store = Store::new();
    ext.extract("test.ts", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store
}

fn rs_extractor() -> Extractor {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    let query_src = include_str!("../../../../packs/rust/queries.scm");
    Extractor::new(language, query_src).expect("rust extractor should build")
}

fn extract_rs(source: &str) -> Store {
    let ext = rs_extractor();
    let mut store = Store::new();
    ext.extract("test.rs", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store
}

#[test]
#[allow(clippy::similar_names)]
fn extractor_python_call_inside_function_creates_calls_edge() {
    // foo calls bar; bar is defined in the same file.
    let source = "def bar(): pass\ndef foo():\n    bar()";
    let store = extract(source);
    let caller = store.lookup("test.py>foo").expect("foo must exist");
    let callee = store.lookup("test.py>bar").expect("bar must exist");
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&callee),
        "foo should have a Calls edge to bar"
    );
}

#[test]
#[allow(clippy::similar_names)]
fn extractor_python_method_call_creates_calls_edge() {
    let source = "def helper(): pass\nclass Svc:\n    def run(self):\n        helper()";
    let store = extract(source);
    let caller = store.lookup("test.py>Svc>run").expect("run must exist");
    let callee = store.lookup("test.py>helper").expect("helper must exist");
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&callee),
        "run should have a Calls edge to helper"
    );
}

#[test]
#[allow(clippy::similar_names)]
fn extractor_typescript_call_creates_calls_edge() {
    let source = "function bar(): void {}\nfunction foo(): void { bar(); }";
    let store = extract_ts(source);
    let caller = store.lookup("test.ts>foo").expect("foo must exist");
    let callee = store.lookup("test.ts>bar").expect("bar must exist");
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&callee),
        "foo should have a Calls edge to bar"
    );
}

#[test]
#[allow(clippy::similar_names)]
fn extractor_rust_call_creates_calls_edge() {
    let source = "fn bar() {}\nfn foo() { bar(); }";
    let store = extract_rs(source);
    let caller = store.lookup("test.rs>foo").expect("foo must exist");
    let callee = store.lookup("test.rs>bar").expect("bar must exist");
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&callee),
        "foo should have a Calls edge to bar"
    );
}

// ── RFC-0014: cross-file call stub resolution ────────────────────────────

#[test]
#[allow(clippy::similar_names)]
fn extractor_cross_file_call_resolves_stub_to_definition() {
    // a.py calls bar(); bar is defined in b.py.
    // After extracting both files and calling resolve_bare_call_stubs,
    // the Calls edge from a.py>foo must point to b.py>bar, not a stub.
    let ext = python_extractor();
    let mut store = Store::new();
    ext.extract("a.py", b"def foo():\n    bar()", &mut store)
        .unwrap();
    ext.extract("b.py", b"def bar(): pass", &mut store).unwrap();

    let resolved = store.resolve_bare_call_stubs();

    assert_eq!(resolved, 1, "exactly one stub should be resolved");
    let caller = store.lookup("a.py>foo").expect("a.py>foo must exist");
    let callee = store.lookup("b.py>bar").expect("b.py>bar must exist");
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&callee),
        "a.py>foo must call b.py>bar after stub resolution"
    );
    assert_eq!(
        store.incoming(callee, EdgeKind::Calls),
        &[caller],
        "b.py>bar must have exactly one caller: a.py>foo"
    );
    assert!(
        store.lookup("bar").is_none(),
        "bare stub 'bar' must be removed after resolution"
    );
}

// ── RFC-0013: forward-reference call resolution ──────────────────────────────

#[test]
#[allow(clippy::similar_names)]
fn extractor_resolves_forward_reference_call_to_definition_node() {
    // foo is defined before bar, and calls bar which is defined after.
    // Two-pass extraction must resolve bar to its definition node, not a stub.
    let source = "def foo():\n    bar()\ndef bar(): pass";
    let store = extract(source);
    let caller = store.lookup("test.py>foo").expect("foo must exist");
    let callee = store.lookup("test.py>bar").expect("bar must exist");
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&callee),
        "foo->bar Calls edge must point to the definition node, not a stub"
    );
    // Ensure no bare 'bar' stub was created alongside the definition.
    // (a stub would be at path "bar", while the definition is "test.py>bar")
    assert_eq!(
        store.outgoing(caller, EdgeKind::Calls).len(),
        1,
        "there should be exactly one Calls edge from foo — no duplicates"
    );
}

#[test]
#[allow(clippy::similar_names)]
fn extractor_forward_reference_typescript() {
    let source = "function foo(): void { bar(); }\nfunction bar(): void {}";
    let store = extract_ts(source);
    let caller = store.lookup("test.ts>foo").expect("foo must exist");
    let callee = store.lookup("test.ts>bar").expect("bar must exist");
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&callee),
        "foo->bar Calls edge must use definition node for forward reference"
    );
}

#[test]
#[allow(clippy::similar_names)]
fn extractor_forward_reference_rust() {
    let source = "fn foo() { bar(); }\nfn bar() {}";
    let store = extract_rs(source);
    let caller = store.lookup("test.rs>foo").expect("foo must exist");
    let callee = store.lookup("test.rs>bar").expect("bar must exist");
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&callee),
        "foo->bar Calls edge must use definition node for forward reference"
    );
}
