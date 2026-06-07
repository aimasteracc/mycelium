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

// ── RFC-0096: TypeImports edge kind ─────────────────────────────────────────
// TYPE_CHECKING imports must produce TypeImports edges (not be silently dropped).
// This lets agents query type-only dependency graphs while keeping detect-cycles
// clean (Imports-only by default, same as before).

#[test]
fn type_checking_import_emits_type_imports_edge() {
    // `from collections import OrderedDict` inside `if TYPE_CHECKING:` must
    // produce a TypeImports edge (RFC-0096) — not be dropped silently.
    let source = "from typing import TYPE_CHECKING\nif TYPE_CHECKING:\n    from collections import OrderedDict\n";
    let store = extract(source);
    let file_id = store.lookup("test.py").unwrap();
    let collections_id = store
        .lookup("collections")
        .expect("collections node must exist (TypeImports)");
    assert!(
        store
            .outgoing(file_id, EdgeKind::TypeImports)
            .contains(&collections_id),
        "import inside `if TYPE_CHECKING:` must produce a TypeImports edge"
    );
    // Must NOT appear as a regular Imports edge (would pollute cycle detection)
    assert!(
        !store
            .outgoing(file_id, EdgeKind::Imports)
            .contains(&collections_id),
        "import inside `if TYPE_CHECKING:` must NOT produce a regular Imports edge"
    );
}

#[test]
fn regular_imports_not_in_type_imports() {
    // Regular imports (`import os`) must stay as Imports, not TypeImports.
    let source = "import os\nfrom typing import TYPE_CHECKING\nif TYPE_CHECKING:\n    import typing_extensions\n";
    let store = extract(source);
    let file_id = store.lookup("test.py").unwrap();
    let os_id = store.lookup("os").expect("os node must exist");
    // Regular import: Imports edge present, TypeImports absent
    assert!(
        store.outgoing(file_id, EdgeKind::Imports).contains(&os_id),
        "regular `import os` must produce an Imports edge"
    );
    assert!(
        !store
            .outgoing(file_id, EdgeKind::TypeImports)
            .contains(&os_id),
        "regular `import os` must NOT produce a TypeImports edge"
    );
    // TYPE_CHECKING import: TypeImports present, Imports absent
    let te_id = store
        .lookup("typing_extensions")
        .expect("typing_extensions node must exist (TypeImports)");
    assert!(
        store
            .outgoing(file_id, EdgeKind::TypeImports)
            .contains(&te_id),
        "`typing_extensions` inside TYPE_CHECKING must produce a TypeImports edge"
    );
    assert!(
        !store.outgoing(file_id, EdgeKind::Imports).contains(&te_id),
        "`typing_extensions` inside TYPE_CHECKING must NOT produce an Imports edge"
    );
}

#[test]
fn type_imports_wire_string_is_type_imports() {
    assert_eq!(EdgeKind::TypeImports.as_str(), "type_imports");
    assert_eq!(EdgeKind::TypeImports.to_string(), "type_imports");
}

// ── RFC-0096 Phase 2: TypeScript `import type` → TypeImports ────────────────
// `import type { Foo } from 'mod'` must emit a TypeImports edge, not Imports.
// This is the TypeScript analog of the Python TYPE_CHECKING implementation.

#[test]
fn typescript_type_import_emits_type_imports_edge() {
    // `import type { Foo } from './foo'` is purely type-annotation syntax in
    // TypeScript — never imported at runtime. Must produce a TypeImports edge.
    let store = extract_ts("import type { Foo } from './foo';");
    let file_id = store.lookup("test.ts").unwrap();
    let foo_id = store
        .lookup("foo.ts")
        .or_else(|| store.lookup("./foo"))
        .expect("foo module node must exist (TypeImports)");
    assert!(
        store
            .outgoing(file_id, EdgeKind::TypeImports)
            .contains(&foo_id),
        "`import type` must produce a TypeImports edge"
    );
    assert!(
        !store.outgoing(file_id, EdgeKind::Imports).contains(&foo_id),
        "`import type` must NOT produce a regular Imports edge"
    );
}

#[test]
fn typescript_regular_import_not_in_type_imports() {
    // Regular `import { Foo } from './foo'` must stay as Imports, not TypeImports.
    let store = extract_ts("import { Foo } from './bar';");
    let file_id = store.lookup("test.ts").unwrap();
    let bar_id = store
        .lookup("bar.ts")
        .or_else(|| store.lookup("./bar"))
        .expect("bar module node must exist");
    assert!(
        store.outgoing(file_id, EdgeKind::Imports).contains(&bar_id),
        "regular import must produce an Imports edge"
    );
    assert!(
        !store
            .outgoing(file_id, EdgeKind::TypeImports)
            .contains(&bar_id),
        "regular import must NOT produce a TypeImports edge"
    );
}

#[test]
fn typescript_mixed_imports_segregated() {
    // File with both regular and type-only imports — each goes to the right edge kind.
    let source = "import { readFile } from 'fs';\nimport type { ReadOptions } from './options';";
    let store = extract_ts(source);
    let file_id = store.lookup("test.ts").unwrap();
    let fs_id = store.lookup("fs").expect("fs node must exist");
    let opts_id = store
        .lookup("options.ts")
        .or_else(|| store.lookup("./options"))
        .expect("options node must exist");
    // 'fs' — regular import → Imports only
    assert!(
        store.outgoing(file_id, EdgeKind::Imports).contains(&fs_id),
        "'fs' must be in Imports"
    );
    assert!(
        !store
            .outgoing(file_id, EdgeKind::TypeImports)
            .contains(&fs_id),
        "'fs' must NOT be in TypeImports"
    );
    // './options' — type import → TypeImports only
    assert!(
        store
            .outgoing(file_id, EdgeKind::TypeImports)
            .contains(&opts_id),
        "'./options' must be in TypeImports"
    );
    assert!(
        !store
            .outgoing(file_id, EdgeKind::Imports)
            .contains(&opts_id),
        "'./options' must NOT be in Imports"
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
fn from_relative_submodule_import_creates_correct_alias_and_call_edge() {
    // Issue #214 Pattern 2: `from .models import AnalysisResult` should bind
    // `AnalysisResult` → `pkg/sub/models.py>AnalysisResult` (a symbol inside
    // a relative submodule), NOT `pkg/sub/models.py/AnalysisResult.py` (wrong
    // file path). The latter was produced by the (true, None) arm in
    // build_alias_target when src was non-bare (e.g. ".models").
    //
    // Consequence: code that calls `AnalysisResult()` after such an import
    // must create a Calls edge to `pkg/sub/models.py>AnalysisResult`, not to
    // a spurious bare stub.
    let source = "\
from .models import AnalysisResult

def create():
    return AnalysisResult()
";
    let store = extract_at("pkg/sub/consumer.py", source);

    let create_id = store
        .lookup("pkg/sub/consumer.py>create")
        .expect("caller function must be indexed");

    let target_id = store
        .lookup("pkg/sub/models.py>AnalysisResult")
        .expect("AnalysisResult must resolve to pkg/sub/models.py>AnalysisResult via alias table");

    assert!(
        store
            .outgoing(create_id, EdgeKind::Calls)
            .contains(&target_id),
        "Calls edge must target pkg/sub/models.py>AnalysisResult, not a bare stub"
    );
}

#[test]
fn from_bare_relative_import_still_resolves_to_module_file() {
    // Regression guard: `from . import sibling` must still resolve to
    // `pkg/sub/sibling.py` (bare relative: local IS a module, not a symbol).
    // This test ensures the Pattern 2 fix does not break the existing
    // bare-relative behaviour.
    let source = "\
from . import sibling

def caller():
    return sibling.func()
";
    let store = extract_at("pkg/sub/consumer.py", source);

    let caller_id = store
        .lookup("pkg/sub/consumer.py>caller")
        .expect("caller function must be indexed");

    let target_id = store
        .lookup("pkg/sub/sibling.py>func")
        .expect("`from . import sibling; sibling.func()` must resolve to sibling.py>func");

    assert!(
        store
            .outgoing(caller_id, EdgeKind::Calls)
            .contains(&target_id),
        "bare relative import must still resolve module file, not treat sibling as a symbol"
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
fn direct_method_call_creates_edge_depth_one_chain_works() {
    // `self.method()` (depth-1 chain, object = identifier) is handled by the
    // `@call.receiver` pattern and still creates a Calls edge.  This test
    // guards that the issue #214 Pattern 3 fix did NOT regress the common case.
    let source = "\
class App:
    def method(self):
        pass
    def bar(self):
        self.method()
";
    let store = extract_at("pkg/app.py", source);
    let bar_id = store
        .lookup("pkg/app.py>App>bar")
        .expect("caller method must be indexed");
    let method_id = store
        .lookup("pkg/app.py>App>method")
        .expect("callee method must be indexed");
    assert!(
        store.outgoing(bar_id, EdgeKind::Calls).contains(&method_id),
        "self.method() (depth-1 chain) must still create a Calls edge from bar"
    );
}

// ── issue #214 Pattern 3: nested-chain false-caller suppression ───────────────

#[test]
fn nested_attribute_chain_does_not_create_global_bare_stub() {
    // Issue #214 Pattern 3: `self.history.append(x)` caused the bare node
    // "append" to be created globally and linked as a callee of any method
    // that contained such a chain call.  Because Python's `list.append` and
    // user-defined `HealthHistory.append` share the same bare name, every
    // chain call across an entire codebase was spuriously attributed to the
    // user-defined symbol, producing 1,472 false callers.
    //
    // Correct behaviour: when the receiver chain depth > 1 (object is not a
    // single identifier), the call target is unresolvable without type info.
    // Emit NO edge rather than a global bare stub that collides with real
    // symbols.  `self.method()` (depth 1) continues to work via the existing
    // `@call.receiver` pattern.
    let source = "\
class App:
    def bar(self):
        self.history.append(1)
";
    let store = extract_at("pkg/app.py", source);
    // The bare global node must NOT exist: the fallback query is removed.
    assert!(
        store.lookup("append").is_none(),
        "depth-2+ chain call must NOT create a global bare `append` stub \
         (issue #214 Pattern 3)"
    );
    // The bar method itself is still indexed.
    assert!(
        store.lookup("pkg/app.py>App>bar").is_some(),
        "caller method must still be indexed"
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
fn extractor_rust_receiver_type_binds_multi_match_method_f5() {
    // RFC-0118 Part B end-to-end (the F5 fix): `upsert_node` is a method on TWO
    // types. A bare `s.upsert_node()` is multi-match, so the single-match passes
    // decline and get-callers would be 0. The local binding `let s = Store::new()`
    // must let the post-merge pass bind the call to Store>upsert_node — and NOT
    // Trunk>upsert_node.
    let source = "\
struct Store;
impl Store { fn upsert_node(&self) {} }
struct Trunk;
impl Trunk { fn upsert_node(&self) {} }
fn run() {
    let s = Store::new();
    s.upsert_node();
}
";
    let ext = rs_extractor();
    let mut store = Store::new();
    ext.extract("test.rs", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let run = store.lookup("test.rs>run").expect("run exists");
    let store_m = store
        .lookup("test.rs>Store>upsert_node")
        .expect("Store::upsert_node exists");
    let trunk_m = store
        .lookup("test.rs>Trunk>upsert_node")
        .expect("Trunk::upsert_node exists");

    assert!(
        store.incoming(store_m, EdgeKind::Calls).contains(&run),
        "run() must be a caller of Store>upsert_node after receiver inference"
    );
    assert!(
        !store.incoming(trunk_m, EdgeKind::Calls).contains(&run),
        "run() must NOT be mis-bound to Trunk>upsert_node"
    );
}

#[test]
fn extractor_rust_receiver_context_recorded_for_second_call_site() {
    // Regression: after the first `a.upsert_node()` creates a bare stub, the
    // second `b.upsert_node()` finds the same stub (resolved=true).  Without
    // the kind_of() guard the second caller's ReceiverContext would not be
    // recorded, so get-callers would be 1 instead of 2.
    let source = "\
struct Store;
impl Store { fn upsert_node(&self) {} }
struct Trunk;
impl Trunk { fn upsert_node(&self) {} }
fn caller_a() { let s = Store::new(); s.upsert_node(); }
fn caller_b() { let s = Store::new(); s.upsert_node(); }
";
    let ext = rs_extractor();
    let mut store = Store::new();
    ext.extract("test.rs", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let ca = store.lookup("test.rs>caller_a").expect("caller_a exists");
    let cb = store.lookup("test.rs>caller_b").expect("caller_b exists");
    let store_m = store
        .lookup("test.rs>Store>upsert_node")
        .expect("Store::upsert_node exists");

    let callers = store.incoming(store_m, EdgeKind::Calls);
    assert!(
        callers.contains(&ca),
        "caller_a must be a caller of Store>upsert_node"
    );
    assert!(
        callers.contains(&cb),
        "caller_b must also be a caller of Store>upsert_node (second call-site fix)"
    );
}

#[test]
fn extractor_rust_receiver_inference_no_cross_function_leak() {
    // Two functions, the SAME local name `h`, DIFFERENT constructor types, and a
    // method `run` defined on both types. Each call must bind to ITS OWN type —
    // a binding from one function must never leak into the other (the "never
    // mis-bind" invariant, independent-reviewer finding). Also covers `let mut`.
    let source = "\
struct Store;
impl Store { fn run(&self) {} }
struct Trunk;
impl Trunk { fn run(&self) {} }
fn alpha() {
    let h = Trunk::new();
    h.run();
}
fn beta() {
    let mut h = Store::new();
    h.run();
}
";
    let ext = rs_extractor();
    let mut store = Store::new();
    ext.extract("test.rs", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let alpha = store.lookup("test.rs>alpha").expect("alpha exists");
    let beta = store.lookup("test.rs>beta").expect("beta exists");
    let store_run = store
        .lookup("test.rs>Store>run")
        .expect("Store::run exists");
    let trunk_run = store
        .lookup("test.rs>Trunk>run")
        .expect("Trunk::run exists");

    // alpha's `h: Trunk` → Trunk>run only.
    assert!(store.incoming(trunk_run, EdgeKind::Calls).contains(&alpha));
    assert!(
        !store.incoming(store_run, EdgeKind::Calls).contains(&alpha),
        "alpha must NOT leak to Store>run"
    );
    // beta's `let mut h: Store` → Store>run only (also proves `let mut` is captured).
    assert!(store.incoming(store_run, EdgeKind::Calls).contains(&beta));
    assert!(
        !store.incoming(trunk_run, EdgeKind::Calls).contains(&beta),
        "beta must NOT leak to Trunk>run"
    );
}

#[test]
fn extractor_rust_shadowed_binding_declines_no_misbind() {
    // Block shadowing: the same name `s` is bound to two different types in one
    // function. We don't track block scopes, so inference must DECLINE (leave the
    // conservative stub) rather than guess — never mis-bind (Codex P2 #635).
    let source = "\
struct Store;
impl Store { fn upsert_node(&self) {} }
struct Trunk;
impl Trunk { fn upsert_node(&self) {} }
fn run() {
    let s = Store::new();
    let s = Trunk::new();
    s.upsert_node();
}
";
    let ext = rs_extractor();
    let mut store = Store::new();
    ext.extract("test.rs", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let run = store.lookup("test.rs>run").expect("run exists");
    let store_m = store.lookup("test.rs>Store>upsert_node").expect("exists");
    let trunk_m = store.lookup("test.rs>Trunk>upsert_node").expect("exists");
    // Conflicting shadowed bindings → neither precise edge is added (declined).
    assert!(
        !store.incoming(store_m, EdgeKind::Calls).contains(&run),
        "must not guess Store on a shadowed binding"
    );
    assert!(
        !store.incoming(trunk_m, EdgeKind::Calls).contains(&run),
        "must not guess Trunk on a shadowed binding"
    );
}

#[test]
fn extractor_python_receiver_type_binds_multi_match_method_f5() {
    // RFC-0118 Part B (Python): `upsert_node` is a method on TWO classes. A bare
    // `s.upsert_node()` is multi-match → declines → get-callers 0. The local
    // binding `s = Store()` must let the post-merge pass bind the call to
    // Store>upsert_node and NOT Trunk>upsert_node.
    let source = "\
class Store:
    def upsert_node(self): pass
class Trunk:
    def upsert_node(self): pass
def run():
    s = Store()
    s.upsert_node()
";
    let ext = python_extractor();
    let mut store = Store::new();
    ext.extract("test.py", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let run = store.lookup("test.py>run").expect("run exists");
    let store_m = store
        .lookup("test.py>Store>upsert_node")
        .expect("Store.upsert_node exists");
    let trunk_m = store
        .lookup("test.py>Trunk>upsert_node")
        .expect("Trunk.upsert_node exists");
    assert!(
        store.incoming(store_m, EdgeKind::Calls).contains(&run),
        "run() must be a caller of Store>upsert_node after receiver inference"
    );
    assert!(
        !store.incoming(trunk_m, EdgeKind::Calls).contains(&run),
        "run() must NOT be mis-bound to Trunk>upsert_node"
    );
}

#[test]
fn extractor_python_shadowed_binding_declines_no_misbind() {
    // Same name `s` bound to two types in one function → must DECLINE (we don't
    // track block scopes), never guess. Symmetric with the Rust guard.
    let source = "\
class Store:
    def upsert_node(self): pass
class Trunk:
    def upsert_node(self): pass
def run():
    s = Store()
    s = Trunk()
    s.upsert_node()
";
    let ext = python_extractor();
    let mut store = Store::new();
    ext.extract("test.py", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let run = store.lookup("test.py>run").expect("run exists");
    let store_m = store.lookup("test.py>Store>upsert_node").expect("exists");
    let trunk_m = store.lookup("test.py>Trunk>upsert_node").expect("exists");
    assert!(
        !store.incoming(store_m, EdgeKind::Calls).contains(&run),
        "must not guess Store on a shadowed binding"
    );
    assert!(
        !store.incoming(trunk_m, EdgeKind::Calls).contains(&run),
        "must not guess Trunk on a shadowed binding"
    );
}

#[test]
fn extractor_typescript_receiver_type_binds_multi_match_method_f5() {
    // RFC-0118 Part B (TypeScript): `const s = new Store()` must bind the
    // multi-match `s.upsert_node()` to Store>upsert_node, not Trunk>upsert_node.
    let source = "\
class Store { upsert_node(): void {} }
class Trunk { upsert_node(): void {} }
function run(): void {
    const s = new Store();
    s.upsert_node();
}
";
    let ext = ts_extractor();
    let mut store = Store::new();
    ext.extract("test.ts", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let run = store.lookup("test.ts>run").expect("run exists");
    let store_m = store
        .lookup("test.ts>Store>upsert_node")
        .expect("Store.upsert_node exists");
    let trunk_m = store
        .lookup("test.ts>Trunk>upsert_node")
        .expect("Trunk.upsert_node exists");
    assert!(
        store.incoming(store_m, EdgeKind::Calls).contains(&run),
        "run() must be a caller of Store>upsert_node after receiver inference"
    );
    assert!(
        !store.incoming(trunk_m, EdgeKind::Calls).contains(&run),
        "run() must NOT be mis-bound to Trunk>upsert_node"
    );
}

#[test]
fn extractor_typescript_shadowed_binding_declines_no_misbind() {
    // Same name `s` bound to two types via `new` → must DECLINE. Symmetric guard.
    let source = "\
class Store { upsert_node(): void {} }
class Trunk { upsert_node(): void {} }
function run(): void {
    let s = new Store();
    s = new Trunk();
    s.upsert_node();
}
";
    let ext = ts_extractor();
    let mut store = Store::new();
    ext.extract("test.ts", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let run = store.lookup("test.ts>run").expect("run exists");
    let store_m = store.lookup("test.ts>Store>upsert_node").expect("exists");
    let trunk_m = store.lookup("test.ts>Trunk>upsert_node").expect("exists");
    assert!(
        !store.incoming(store_m, EdgeKind::Calls).contains(&run),
        "must not guess Store on a shadowed binding"
    );
    assert!(
        !store.incoming(trunk_m, EdgeKind::Calls).contains(&run),
        "must not guess Trunk on a shadowed binding"
    );
}

#[test]
fn extractor_python_rebind_to_non_ctor_declines_no_misbind() {
    // The "never mis-bind" invariant under dynamic typing (Codex P1 #647): a local
    // first bound to a Title-case ctor, then REASSIGNED to a non-constructor RHS,
    // must DECLINE — the original ctor type is stale at the call site. Pre-fix this
    // mis-bound to Store because the lowercase rebind was dropped by the title-case
    // filter before the conflict could be seen.
    // Two types define `upsert_node` (multi-match), so binding to Store is only
    // possible via receiver inference — the single-match fallback cannot bind it.
    let source = "\
class Store:
    def upsert_node(self): pass
class Trunk:
    def upsert_node(self): pass
def make_trunk(): return Trunk()
def run():
    s = Store()
    s = make_trunk()
    s.upsert_node()
";
    let ext = python_extractor();
    let mut store = Store::new();
    ext.extract("test.py", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let run = store.lookup("test.py>run").expect("run exists");
    let store_m = store.lookup("test.py>Store>upsert_node").expect("exists");
    assert!(
        !store.incoming(store_m, EdgeKind::Calls).contains(&run),
        "must DECLINE: `s` was reassigned to a non-constructor, so its type is unknown"
    );
}

#[test]
fn extractor_typescript_rebind_to_non_ctor_declines_no_misbind() {
    // Same invariant for TypeScript: `const s = new Store(); s = factory();` —
    // the factory reassignment is a call_expression (not new_expression), so the
    // declared type is stale. Must DECLINE, not bind to Store.
    // Two types define `upsert_node` (multi-match) so only receiver inference
    // could bind Store — the single-match fallback cannot.
    let source = "\
class Store { upsert_node(): void {} }
class Trunk { upsert_node(): void {} }
function factory(): Trunk { return new Trunk(); }
function run(): void {
    let s = new Store();
    s = factory();
    s.upsert_node();
}
";
    let ext = ts_extractor();
    let mut store = Store::new();
    ext.extract("test.ts", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let run = store.lookup("test.ts>run").expect("run exists");
    let store_m = store.lookup("test.ts>Store>upsert_node").expect("exists");
    assert!(
        !store.incoming(store_m, EdgeKind::Calls).contains(&run),
        "must DECLINE: `s` was reassigned via a non-constructor call"
    );
}

#[test]
fn extractor_python_receiver_inference_no_cross_function_leak() {
    // A binding in `alpha` must never affect inference in `beta` (symmetric with
    // the Rust no-cross-function-leak guard).
    let source = "\
class Store:
    def run(self): pass
class Trunk:
    def run(self): pass
def alpha():
    h = Trunk()
    h.run()
def beta():
    h = Store()
    h.run()
";
    let ext = python_extractor();
    let mut store = Store::new();
    ext.extract("test.py", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let alpha = store.lookup("test.py>alpha").expect("alpha exists");
    let beta = store.lookup("test.py>beta").expect("beta exists");
    let store_run = store.lookup("test.py>Store>run").expect("exists");
    let trunk_run = store.lookup("test.py>Trunk>run").expect("exists");
    assert!(store.incoming(trunk_run, EdgeKind::Calls).contains(&alpha));
    assert!(
        !store.incoming(store_run, EdgeKind::Calls).contains(&alpha),
        "alpha must NOT leak to Store>run"
    );
    assert!(store.incoming(store_run, EdgeKind::Calls).contains(&beta));
    assert!(
        !store.incoming(trunk_run, EdgeKind::Calls).contains(&beta),
        "beta must NOT leak to Trunk>run"
    );
}

#[test]
fn extractor_typescript_receiver_inference_no_cross_function_leak() {
    let source = "\
class Store { run(): void {} }
class Trunk { run(): void {} }
function alpha(): void {
    const h = new Trunk();
    h.run();
}
function beta(): void {
    const h = new Store();
    h.run();
}
";
    let ext = ts_extractor();
    let mut store = Store::new();
    ext.extract("test.ts", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let alpha = store.lookup("test.ts>alpha").expect("alpha exists");
    let beta = store.lookup("test.ts>beta").expect("beta exists");
    let store_run = store.lookup("test.ts>Store>run").expect("exists");
    let trunk_run = store.lookup("test.ts>Trunk>run").expect("exists");
    assert!(store.incoming(trunk_run, EdgeKind::Calls).contains(&alpha));
    assert!(
        !store.incoming(store_run, EdgeKind::Calls).contains(&alpha),
        "alpha must NOT leak to Store>run"
    );
    assert!(store.incoming(store_run, EdgeKind::Calls).contains(&beta));
    assert!(
        !store.incoming(trunk_run, EdgeKind::Calls).contains(&beta),
        "beta must NOT leak to Trunk>run"
    );
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

/// Dogfood-discovered regression (2026-06-03): the Rust extractor missed
/// `Type::method()` call expressions because the only call queries matched
/// `(identifier)` and `(field_expression)` function expressions. `WatchEngine::drive(...)`
/// in real code produced zero Calls edges. This test pins the additive
/// `(scoped_identifier name: ...)` query and tracks the regression at the
/// extractor layer; cross-file resolution to the typed method body is a
/// separate concern that the bare-stub resolver handles.
#[test]
#[allow(clippy::similar_names)]
fn extractor_rust_scoped_method_call_creates_calls_edge() {
    let source = "fn outer() { WatchEngine::drive(); }";
    let store = extract_rs(source);
    let caller = store.lookup("test.rs>outer").expect("outer must exist");
    // Cross-file resolution produces a bare stub `drive` at the top level
    // (later resolved by `resolve_bare_call_stubs` against the typed body).
    let callee = store
        .lookup("drive")
        .expect("`drive` bare stub must exist after extraction");
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&callee),
        "outer should have a Calls edge to the `drive` stub for `WatchEngine::drive(...)`"
    );
}

/// Module-qualified path-style call: `crate::module::function()`.
#[test]
#[allow(clippy::similar_names)]
fn extractor_rust_qualified_path_call_creates_calls_edge() {
    let source = "fn outer() { crate::watch::start(); }";
    let store = extract_rs(source);
    let caller = store.lookup("test.rs>outer").expect("outer must exist");
    let callee = store
        .lookup("start")
        .expect("`start` bare stub must exist for `crate::watch::start()`");
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&callee),
        "outer should have a Calls edge to the `start` stub"
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

// ── issue #247 diagnostics: import-alias and callback false positives ────────

/// Pattern 1 (issue #247): import alias resolution.
/// Calling the aliased name must create a Calls edge to the original definition.
#[test]
#[allow(clippy::similar_names)]
fn extractor_import_alias_call_resolves_to_original() {
    let ext = python_extractor();
    let mut store = Store::new();
    // Index the utility module first (definition side).
    ext.extract("pkg/_utils.py", b"def helper(): pass", &mut store)
        .unwrap();
    // Then index the importer (alias + call site).
    ext.extract(
        "pkg/main.py",
        b"from ._utils import helper as _helper\ndef do_work():\n    _helper()",
        &mut store,
    )
    .unwrap();

    let caller = store
        .lookup("pkg/main.py>do_work")
        .expect("do_work must exist");
    let callee = store
        .lookup("pkg/_utils.py>helper")
        .expect("helper must exist in _utils.py");
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&callee),
        "import alias `_helper` should resolve to pkg/_utils.py>helper"
    );
}

/// Pattern 2: `run_with_cb(callback)` — callback passed as positional arg
/// must produce a Calls edge so `callback` is NOT isolated.
#[test]
fn extractor_callback_arg_not_isolated() {
    let source = "def callback(): pass\ndef caller():\n    run_with_cb(callback)";
    let store = extract(source);
    let cb = store
        .lookup("test.py>callback")
        .expect("callback must exist");
    let degree = store.node_degree(cb);
    assert!(
        degree.in_calls > 0 || degree.out_calls > 0,
        "callback passed as argument should have at least one Calls edge to avoid dead-code false positive"
    );
}

// ── reference.extends (issue #245) ───────────────────────────────────────────

#[test]
fn extractor_python_extends_same_file_base() {
    let source = "class Base:\n    pass\n\nclass Sub(Base):\n    pass";
    let store = extract(source);
    let sub = store.lookup("test.py>Sub").expect("Sub must exist");
    let base = store.lookup("test.py>Base").expect("Base must exist");
    assert!(
        store.outgoing(sub, EdgeKind::Extends).contains(&base),
        "Sub should have an Extends edge to same-file Base"
    );
}

#[test]
fn extractor_python_extends_external_base() {
    let source = "class Sub(ExternalBase):\n    pass";
    let store = extract(source);
    let sub = store.lookup("test.py>Sub").expect("Sub must exist");
    let base = store.lookup("ExternalBase");
    assert!(base.is_some(), "ExternalBase stub node must be created");
    assert!(
        store
            .outgoing(sub, EdgeKind::Extends)
            .contains(&base.unwrap()),
        "Sub should have an Extends edge to ExternalBase stub"
    );
}

#[test]
fn extractor_python_extends_multiple_inheritance() {
    let source = "class Sub(Base1, Base2):\n    pass";
    let store = extract(source);
    let sub = store.lookup("test.py>Sub").expect("Sub must exist");
    let base1 = store.lookup("Base1").expect("Base1 stub must exist");
    let base2 = store.lookup("Base2").expect("Base2 stub must exist");
    assert!(
        store.outgoing(sub, EdgeKind::Extends).contains(&base1),
        "Sub should extend Base1"
    );
    assert!(
        store.outgoing(sub, EdgeKind::Extends).contains(&base2),
        "Sub should extend Base2"
    );
}

// ── issue #261: cross-file Extends resolution ─────────────────────────────────

#[test]
#[allow(clippy::similar_names)]
fn extractor_python_extends_cross_file_resolves_to_definition() {
    // Sub(Base): Base defined in base.py, Sub in sub.py.
    // After resolve_bare_call_stubs, the Extends edge must point to
    // base.py>Base (the definition), not the bare "Base" stub.
    let ext = python_extractor();
    let mut store = Store::new();
    ext.extract("base.py", b"class Base:\n    pass", &mut store)
        .unwrap();
    ext.extract("sub.py", b"class Sub(Base):\n    pass", &mut store)
        .unwrap();

    let resolved = store.resolve_bare_call_stubs();

    assert_eq!(resolved, 1, "exactly one stub (Base) should be resolved");
    let sub = store.lookup("sub.py>Sub").expect("sub.py>Sub must exist");
    let base = store
        .lookup("base.py>Base")
        .expect("base.py>Base must exist");
    assert!(
        store.outgoing(sub, EdgeKind::Extends).contains(&base),
        "sub.py>Sub must have an Extends edge to base.py>Base after stub resolution"
    );
    assert!(
        store.lookup("Base").is_none(),
        "bare stub 'Base' must be removed after resolution"
    );
}

// ── issues #267/#268: cross-file Extends with multiple candidates ───────────

/// When multiple files define a class with the same name (real def + test mocks),
/// `resolve_bare_call_stubs()` cannot disambiguate (`matches.len() > 1`) and leaves
/// the Extends edge pointing to the bare stub. The fix: use the alias table at
/// extraction time to pick the correct file-path definition.
#[test]
#[allow(clippy::similar_names)]
fn extractor_cross_file_extends_alias_table_wins_over_bare_stub_ambiguity() {
    // Two mock files also define "Base" — this would make resolve_bare_call_stubs
    // give up (3 candidates). With alias table fix, the extractor creates the
    // Extends edge pointing to base.py>Base directly at extraction time.
    let ext = python_extractor();
    let mut store = Store::new();

    ext.extract("mock1.py", b"class Base:\n    pass", &mut store)
        .unwrap();
    ext.extract("mock2.py", b"class Base:\n    pass", &mut store)
        .unwrap();
    ext.extract("base.py", b"class Base:\n    pass", &mut store)
        .unwrap();
    // sub.py explicitly imports Base from base — the alias table knows the source.
    ext.extract(
        "sub.py",
        b"from base import Base\nclass Sub(Base):\n    pass",
        &mut store,
    )
    .unwrap();

    store.resolve_bare_call_stubs();

    let sub = store.lookup("sub.py>Sub").expect("sub.py>Sub must exist");
    let base = store
        .lookup("base.py>Base")
        .expect("base.py>Base must exist");
    assert!(
        store.outgoing(sub, EdgeKind::Extends).contains(&base),
        "sub.py>Sub must extend base.py>Base (the explicitly imported definition, not a mock)"
    );
}

/// Issue #268: `get-descendants --include-inherited` returns 0 inherited methods
/// when the base class is in a different file with multiple same-name candidates.
/// After the alias table fix, the `inherited_descendants_of_path()` call must find
/// the base class methods.
#[test]
#[allow(clippy::similar_names)]
fn extractor_cross_file_inherited_descendants_resolves_via_alias_table() {
    let ext = python_extractor();
    let mut store = Store::new();

    // Two extra "Base" classes to create ambiguity for resolve_bare_call_stubs.
    ext.extract("mock.py", b"class Base:\n    pass", &mut store)
        .unwrap();
    ext.extract(
        "base.py",
        b"class Base:\n    def method_a(self): pass\n    def method_b(self): pass",
        &mut store,
    )
    .unwrap();
    // sub.py imports Base explicitly and overrides only method_a.
    ext.extract(
        "sub.py",
        b"from base import Base\nclass Sub(Base):\n    def method_a(self): pass",
        &mut store,
    )
    .unwrap();

    store.resolve_bare_call_stubs();

    let inherited = store
        .inherited_descendants_of_path("sub.py>Sub")
        .unwrap_or_default();
    assert!(
        inherited.iter().any(|(p, _)| p.ends_with(">method_b")),
        "method_b (not overridden) must appear in inherited descendants"
    );
    assert!(
        !inherited.iter().any(|(p, _)| p.ends_with(">method_a")),
        "method_a (overridden in Sub) must NOT appear in inherited descendants"
    );
}

// ── RFC-0092 Phase 2: TypeScript import alias resolution ─────────────────────

/// `import { foo as bar } from './module'; bar()` must emit a Calls edge to
/// `src/module.ts>foo`, not to a bare stub `bar`.
#[test]
#[allow(clippy::similar_names)]
fn extractor_ts_named_import_alias_resolves_direct_call() {
    let ext = ts_extractor();
    let mut store = Store::new();
    ext.extract(
        "src/module.ts",
        b"export function foo(): void {}",
        &mut store,
    )
    .unwrap();
    ext.extract(
        "src/consumer.ts",
        b"import { foo as bar } from './module';\nfunction run(): void { bar(); }",
        &mut store,
    )
    .unwrap();
    let caller = store.lookup("src/consumer.ts>run").expect("run must exist");
    let callee = store.lookup("src/module.ts>foo").expect("foo must exist");
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&callee),
        "bar() must resolve via alias table to src/module.ts>foo, not a bare stub"
    );
}

/// `import * as ns from './module'; ns.greet()` must emit a Calls edge to
/// `src/module.ts>greet`.
#[test]
#[allow(clippy::similar_names)]
fn extractor_ts_namespace_import_alias_resolves_method_call() {
    let ext = ts_extractor();
    let mut store = Store::new();
    ext.extract(
        "src/module.ts",
        b"export function greet(): void {}",
        &mut store,
    )
    .unwrap();
    ext.extract(
        "src/consumer.ts",
        b"import * as ns from './module';\nfunction run(): void { ns.greet(); }",
        &mut store,
    )
    .unwrap();
    let caller = store.lookup("src/consumer.ts>run").expect("run must exist");
    let callee = store
        .lookup("src/module.ts>greet")
        .expect("greet must exist");
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&callee),
        "ns.greet() must resolve via namespace alias to src/module.ts>greet"
    );
}

/// `import { foo } from './module'; foo()` (no `as`) must emit a Calls edge to
/// `src/module.ts>foo` via implicit alias binding.
#[test]
#[allow(clippy::similar_names)]
fn extractor_ts_named_import_no_alias_resolves_direct_call() {
    let ext = ts_extractor();
    let mut store = Store::new();
    ext.extract(
        "src/module.ts",
        b"export function foo(): void {}",
        &mut store,
    )
    .unwrap();
    ext.extract(
        "src/consumer.ts",
        b"import { foo } from './module';\nfunction run(): void { foo(); }",
        &mut store,
    )
    .unwrap();
    let caller = store.lookup("src/consumer.ts>run").expect("run must exist");
    let callee = store.lookup("src/module.ts>foo").expect("foo must exist");
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&callee),
        "foo() (imported from './module') must resolve to src/module.ts>foo"
    );
}

// ── RFC-0092 Phase 2: JavaScript alias resolution ─────────────────────────────

fn js_extractor() -> Extractor {
    let language: tree_sitter::Language = tree_sitter_javascript::LANGUAGE.into();
    let query_src = include_str!("../../../../packs/javascript/queries.scm");
    Extractor::new(language, query_src).expect("javascript extractor should build")
}

#[test]
fn extractor_javascript_receiver_type_binds_multi_match_method_f5() {
    // RFC-0118 Part B (JavaScript): `save` is a method on TWO classes (multi-match),
    // so a bare `s.save()` declines and get-callers would be 0. The local binding
    // `const s = new Store()` must bind the call to Store>save, not Cache>save.
    let source = "\
class Store { save() {} }
class Cache { save() {} }
function run() {
    const s = new Store();
    s.save();
}
";
    let ext = js_extractor();
    let mut store = Store::new();
    ext.extract("test.js", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let run = store.lookup("test.js>run").expect("run exists");
    let store_m = store
        .lookup("test.js>Store>save")
        .expect("Store.save exists");
    let cache_m = store
        .lookup("test.js>Cache>save")
        .expect("Cache.save exists");
    assert!(
        store.incoming(store_m, EdgeKind::Calls).contains(&run),
        "run() must be a caller of Store>save after receiver inference"
    );
    assert!(
        !store.incoming(cache_m, EdgeKind::Calls).contains(&run),
        "run() must NOT be mis-bound to Cache>save"
    );
}

#[test]
fn extractor_javascript_shadowed_binding_declines_no_misbind() {
    // Same name `s` bound to two types via `new` → must DECLINE (no block-scope
    // tracking), never guess. Symmetric with the Rust/TS guards.
    let source = "\
class Store { save() {} }
class Cache { save() {} }
function run() {
    let s = new Store();
    s = new Cache();
    s.save();
}
";
    let ext = js_extractor();
    let mut store = Store::new();
    ext.extract("test.js", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let run = store.lookup("test.js>run").expect("run exists");
    let store_m = store.lookup("test.js>Store>save").expect("exists");
    let cache_m = store.lookup("test.js>Cache>save").expect("exists");
    assert!(!store.incoming(store_m, EdgeKind::Calls).contains(&run));
    assert!(!store.incoming(cache_m, EdgeKind::Calls).contains(&run));
}

#[test]
fn extractor_javascript_rebind_to_non_ctor_declines_no_misbind() {
    // `const s = new Store(); s = factory();` — the factory reassignment is a
    // call_expression (not new_expression), so the declared type is stale. Must
    // DECLINE under JS dynamic typing (Codex P1 #647 rebind-invalidation).
    let source = "\
class Store { save() {} }
class Cache { save() {} }
function factory() { return new Cache(); }
function run() {
    let s = new Store();
    s = factory();
    s.save();
}
";
    let ext = js_extractor();
    let mut store = Store::new();
    ext.extract("test.js", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let run = store.lookup("test.js>run").expect("run exists");
    let store_m = store.lookup("test.js>Store>save").expect("exists");
    assert!(
        !store.incoming(store_m, EdgeKind::Calls).contains(&run),
        "must DECLINE: `s` was reassigned via a non-constructor call"
    );
}

#[test]
fn extractor_javascript_sibling_arrow_binding_no_leak() {
    // Codex P2 #653: a binding inside one arrow must NOT leak to a call in the
    // enclosing body / a sibling arrow where the receiver is a free variable.
    // arrow_function must be its own binding scope (never mis-bind). `save` is
    // multi-match so only a leaked binding could (wrongly) bind it.
    let source = "\
class Store { save() {} }
class Cache { save() {} }
function outer() {
    const make = () => { const s = new Store(); return s; };
    s.save();
}
";
    let ext = js_extractor();
    let mut store = Store::new();
    ext.extract("test.js", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let outer = store.lookup("test.js>outer").expect("outer exists");
    let store_m = store.lookup("test.js>Store>save").expect("exists");
    assert!(
        !store.incoming(store_m, EdgeKind::Calls).contains(&outer),
        "binding inside `make` arrow must NOT leak to `s.save()` in outer body"
    );
}

#[test]
fn extractor_javascript_outer_binding_used_in_arrow_binds() {
    // Recall: a binding in the enclosing function IS visible to a call inside a
    // nested arrow (lexical closure capture) — the scope-chain walk must find it.
    // `save` is multi-match so this only resolves via receiver inference.
    let source = "\
class Store { save() {} }
class Cache { save() {} }
function outer() {
    const s = new Store();
    const run = () => { s.save(); };
}
";
    let ext = js_extractor();
    let mut store = Store::new();
    ext.extract("test.js", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let outer = store.lookup("test.js>outer").expect("outer exists");
    let store_m = store.lookup("test.js>Store>save").expect("exists");
    assert!(
        store.incoming(store_m, EdgeKind::Calls).contains(&outer),
        "outer-scope binding must be visible to a call inside a nested arrow"
    );
}

#[test]
#[allow(clippy::similar_names)]
fn extractor_js_named_import_alias_resolves_direct_call() {
    let ext = js_extractor();
    let mut store = Store::new();
    ext.extract("src/module.js", b"export function foo() {}", &mut store)
        .unwrap();
    ext.extract(
        "src/consumer.js",
        b"import { foo as bar } from './module';\nfunction run() { bar(); }",
        &mut store,
    )
    .unwrap();
    let caller = store.lookup("src/consumer.js>run").expect("run must exist");
    let callee = store.lookup("src/module.js>foo").expect("foo must exist");
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&callee),
        "bar() must resolve via alias table to src/module.js>foo"
    );
}

#[test]
#[allow(clippy::similar_names)]
fn extractor_js_namespace_import_alias_resolves_method_call() {
    let ext = js_extractor();
    let mut store = Store::new();
    ext.extract("src/module.js", b"export function greet() {}", &mut store)
        .unwrap();
    ext.extract(
        "src/consumer.js",
        b"import * as ns from './module';\nfunction run() { ns.greet(); }",
        &mut store,
    )
    .unwrap();
    let caller = store.lookup("src/consumer.js>run").expect("run must exist");
    let callee = store
        .lookup("src/module.js>greet")
        .expect("greet must exist");
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&callee),
        "ns.greet() must resolve via namespace alias to src/module.js>greet"
    );
}

#[test]
#[allow(clippy::similar_names)]
fn extractor_js_named_import_no_alias_resolves_direct_call() {
    let ext = js_extractor();
    let mut store = Store::new();
    ext.extract("src/module.js", b"export function foo() {}", &mut store)
        .unwrap();
    ext.extract(
        "src/consumer.js",
        b"import { foo } from './module';\nfunction run() { foo(); }",
        &mut store,
    )
    .unwrap();
    let caller = store.lookup("src/consumer.js>run").expect("run must exist");
    let callee = store.lookup("src/module.js>foo").expect("foo must exist");
    assert!(
        store.outgoing(caller, EdgeKind::Calls).contains(&callee),
        "foo() (imported from './module') must resolve to src/module.js>foo"
    );
}

#[test]
fn from_relative_import_creates_symbol_level_imports_edge_for_dead_symbol_check() {
    // Issue #286: `from .models import AnalysisResult` must create an Imports
    // edge from consumer.py to models.py>AnalysisResult so that dead_symbols
    // does NOT flag AnalysisResult as dead when it has no direct Calls edges.
    let source = "from .models import AnalysisResult\n";
    let store = extract_at("pkg/sub/consumer.py", source);

    let consumer_id = store
        .lookup("pkg/sub/consumer.py")
        .expect("consumer file node must exist");
    let symbol_id = store
        .lookup("pkg/sub/models.py>AnalysisResult")
        .expect("symbol node must be created via alias binding");

    assert!(
        store
            .outgoing(consumer_id, EdgeKind::Imports)
            .contains(&symbol_id)
            || store
                .incoming(symbol_id, EdgeKind::Imports)
                .contains(&consumer_id),
        "Imports edge consumer.py → models.py>AnalysisResult must exist (Issue #286)"
    );

    // Verify dead_symbols does not flag AnalysisResult.
    let dead = store.dead_symbols(None);
    assert!(
        !dead.contains(&"pkg/sub/models.py>AnalysisResult".to_owned()),
        "AnalysisResult must not appear in dead_symbols — it is imported by consumer.py"
    );
}

#[test]
fn from_absolute_import_creates_symbol_level_imports_edge() {
    // Issue #286 (absolute case): `from output_manager import output_data`
    // must create an Imports edge to output_manager>output_data so the symbol
    // is excluded from get-dead-symbols output.
    let source = "from output_manager import output_data\n";
    let store = extract_at("pkg/cli.py", source);

    let symbol_id = store
        .lookup("output_manager>output_data")
        .expect("symbol node must be created via alias binding");

    let dead = store.dead_symbols(None);
    assert!(
        !dead.contains(&"output_manager>output_data".to_owned()),
        "output_data must not appear in dead_symbols — it is imported by cli.py"
    );
    let _ = symbol_id; // silence unused warning
}

// ── Issue #293: JS function-expression definitions ───────────────────────────

#[test]
fn extractor_js_const_function_expression_creates_definition() {
    // `const localize = function(key) { return key; }` must create a definition
    // node for `localize` so callee-tree and call edges work.
    let ext = js_extractor();
    let mut store = Store::new();
    ext.extract(
        "src/nls.js",
        b"const localize = function(key) { return key; };",
        &mut store,
    )
    .unwrap();
    assert!(
        store.lookup("src/nls.js>localize").is_some(),
        "const-assigned function expression must create a definition node"
    );
}

#[test]
fn extractor_js_const_function_expression_calls_edge() {
    // A function defined via `const name = function(...) {...}` must emit Calls
    // edges for calls made inside its body, just like arrow functions do.
    let ext = js_extractor();
    let mut store = Store::new();
    ext.extract(
        "src/nls.js",
        b"function _format(k) {}\nconst localize = function(key) { return _format(key); };",
        &mut store,
    )
    .unwrap();
    let localize_id = store
        .lookup("src/nls.js>localize")
        .expect("localize must exist as a definition");
    let format_id = store
        .lookup("src/nls.js>_format")
        .expect("_format must exist");
    assert!(
        store
            .outgoing(localize_id, EdgeKind::Calls)
            .contains(&format_id),
        "localize must have a Calls edge to _format"
    );
}

#[test]
fn extractor_js_exported_function_expression_creates_definition() {
    // `export const localize = function(...) {...}` must also create a definition.
    let ext = js_extractor();
    let mut store = Store::new();
    ext.extract(
        "src/nls.js",
        b"export const localize = function(key) { return key; };",
        &mut store,
    )
    .unwrap();
    assert!(
        store.lookup("src/nls.js>localize").is_some(),
        "exported const-assigned function expression must create a definition node"
    );
}

#[test]
fn from_import_with_alias_creates_symbol_level_imports_edge() {
    // Issue #286 (aliased case): `from ._ast_cache_schema import apply_migration_v3 as _apply`
    // must create an Imports edge to _ast_cache_schema.py>apply_migration_v3.
    let source = "from ._ast_cache_schema import apply_migration_v3 as _apply\n";
    let store = extract_at("pkg/ast_cache.py", source);

    let symbol_id = store
        .lookup("pkg/_ast_cache_schema.py>apply_migration_v3")
        .expect("symbol node must be created via alias binding");

    let dead = store.dead_symbols(None);
    assert!(
        !dead.contains(&"pkg/_ast_cache_schema.py>apply_migration_v3".to_owned()),
        "apply_migration_v3 must not appear in dead_symbols — it is imported by ast_cache.py"
    );
    let _ = symbol_id;
}

// ── Issue #295: Java Extends/Implements edges ─────────────────────────────────

fn java_extractor() -> Extractor {
    let language: tree_sitter::Language = tree_sitter_java::LANGUAGE.into();
    let query_src = include_str!("../../../../packs/java/queries.scm");
    Extractor::new(language, query_src).expect("java extractor should build")
}

#[test]
fn extractor_java_method_path_is_class_method_not_doubled() {
    // Pre-existing Java bug (surfaced by Part B): @definition.method anchored on
    // the method node made build_class_chain emit `Class>method>method`. Methods
    // must be at `Class>method` (with no `>method>method` phantom).
    let ext = java_extractor();
    let mut store = Store::new();
    ext.extract("test.java", b"class Foo { void bar() {} }", &mut store)
        .expect("extraction should succeed");
    let m = store
        .lookup("test.java>Foo>bar")
        .expect("method must be at Foo>bar");
    assert_eq!(
        store.kind_of(m),
        Some(crate::types::NodeKind::Method),
        "Foo>bar must be a Method (not a kindless intermediate)"
    );
    assert!(
        store.lookup("test.java>Foo>bar>bar").is_none(),
        "must NOT create a doubled `Foo>bar>bar` phantom"
    );
}

#[test]
fn extractor_java_receiver_type_binds_multi_match_method_f5() {
    // RFC-0118 Part B (Java): `save` is a method on TWO classes (multi-match).
    // Java declares local types, so `Store s = new Store()` (or any RHS) binds
    // `s` to the DECLARED type `Store` → `s.save()` resolves to Store>save, not
    // Cache>save. Also requires method_declaration in FUNCTION_KINDS so the
    // caller is attributed to Runner>run (not the file).
    let source = "\
class Store { void save() {} }
class Cache { void save() {} }
class Runner {
    void run() {
        Store s = new Store();
        s.save();
    }
}
";
    let ext = java_extractor();
    let mut store = Store::new();
    ext.extract("test.java", source.as_bytes(), &mut store)
        .expect("extraction should succeed");
    store.resolve_bare_call_stubs();

    let run = store
        .lookup("test.java>Runner>run")
        .expect("Runner.run exists");
    let store_m = store
        .lookup("test.java>Store>save")
        .expect("Store.save exists");
    let cache_m = store
        .lookup("test.java>Cache>save")
        .expect("Cache.save exists");
    assert!(
        store.incoming(store_m, EdgeKind::Calls).contains(&run),
        "run() must be a caller of Store>save after receiver inference"
    );
    assert!(
        !store.incoming(cache_m, EdgeKind::Calls).contains(&run),
        "run() must NOT be mis-bound to Cache>save"
    );
}

#[test]
fn extractor_java_extends_creates_extends_edge() {
    // `class Sub extends Base` must create an Extends edge Sub → Base.
    // Base is in a separate file so resolution requires resolve_bare_call_stubs().
    let ext = java_extractor();
    let mut store = Store::new();
    ext.extract("src/Base.java", b"class Base {}", &mut store)
        .unwrap();
    ext.extract("src/Sub.java", b"class Sub extends Base {}", &mut store)
        .unwrap();
    store.resolve_bare_call_stubs();
    let sub = store.lookup("src/Sub.java>Sub").expect("Sub must exist");
    let base = store
        .lookup("src/Base.java>Base")
        .expect("Base must be resolved");
    assert!(
        store.outgoing(sub, EdgeKind::Extends).contains(&base),
        "Sub must have an Extends edge to src/Base.java>Base after stub resolution"
    );
}

#[test]
fn extractor_java_implements_creates_implements_edge() {
    // `class Foo implements Runnable` must create an Implements edge Foo → Runnable.
    let ext = java_extractor();
    let mut store = Store::new();
    ext.extract(
        "src/Foo.java",
        b"class Foo implements Runnable {}",
        &mut store,
    )
    .unwrap();
    let foo = store.lookup("src/Foo.java>Foo").expect("Foo must exist");
    let runnable = store
        .lookup("Runnable")
        .expect("Runnable stub must be created");
    assert!(
        store
            .outgoing(foo, EdgeKind::Implements)
            .contains(&runnable),
        "Foo must have an Implements edge to Runnable"
    );
}

#[test]
fn extractor_java_interface_extends_creates_extends_edge() {
    // `interface Sub extends Base` must create an Extends edge Sub → Base.
    let ext = java_extractor();
    let mut store = Store::new();
    ext.extract("src/Sub.java", b"interface Sub extends Base {}", &mut store)
        .unwrap();
    let sub = store
        .lookup("src/Sub.java>Sub")
        .expect("Sub interface must exist");
    let base = store.lookup("Base").expect("Base stub must be created");
    assert!(
        store.outgoing(sub, EdgeKind::Extends).contains(&base),
        "Sub interface must have an Extends edge to Base"
    );
}

// ── Issue #296: Python Extends edges for attribute-form base classes ───────────

#[test]
fn extractor_python_extends_dotted_base_creates_extends_edge() {
    // `class SimpleTestCase(unittest.TestCase):` — attribute-form base class.
    // Must create an Extends edge and a stub node for "unittest.TestCase".
    let store = extract("class SimpleTestCase(unittest.TestCase):\n    pass");
    let sub = store
        .lookup("test.py>SimpleTestCase")
        .expect("SimpleTestCase must exist");
    let base = store
        .lookup("unittest.TestCase")
        .expect("unittest.TestCase stub must be created");
    assert!(
        store.outgoing(sub, EdgeKind::Extends).contains(&base),
        "SimpleTestCase must have an Extends edge to unittest.TestCase"
    );
}

#[test]
fn extractor_python_extends_dotted_and_simple_mixed_inheritance() {
    // `class Foo(bar.Base, LocalBase):` — mix of attribute and identifier forms.
    // Both base classes must produce Extends edges.
    let store = extract("class LocalBase:\n    pass\n\nclass Foo(bar.Base, LocalBase):\n    pass");
    let foo = store.lookup("test.py>Foo").expect("Foo must exist");
    let dotted = store
        .lookup("bar.Base")
        .expect("bar.Base stub must be created");
    let local = store
        .lookup("test.py>LocalBase")
        .expect("LocalBase must exist");
    assert!(
        store.outgoing(foo, EdgeKind::Extends).contains(&dotted),
        "Foo must extend bar.Base"
    );
    assert!(
        store.outgoing(foo, EdgeKind::Extends).contains(&local),
        "Foo must extend LocalBase"
    );
}

// ── issue #381: import-aware stub resolution (second pass) ──────────────────

/// When two files define the same symbol and the simple pass cannot disambiguate,
/// the import-aware second pass uses the caller file's Imports edges to pick the
/// correct definition. We manually construct the graph to simulate this scenario
/// since the Python extractor's alias table handles `from X import Y` at extraction
/// time.
#[test]
#[allow(clippy::similar_names)]
fn store_import_aware_stub_resolution_picks_imported_def() {
    let ext = python_extractor();
    let mut store = Store::new();

    // Two files define "helper" with full paths.
    ext.extract("a.py", b"def helper():\n    pass", &mut store)
        .unwrap();
    ext.extract("b.py", b"def helper():\n    pass", &mut store)
        .unwrap();
    // Third file calls helper at module level, creating a bare stub.
    ext.extract("caller.py", b"helper()", &mut store).unwrap();

    // Manually add Imports edge: caller.py imports a.py
    let caller_file = store.lookup("caller.py").expect("caller.py must exist");
    let a_file = store.lookup("a.py").expect("a.py must exist");
    store.upsert_edge(EdgeKind::Imports, caller_file, a_file);

    let resolved = store.resolve_bare_call_stubs();
    assert!(
        resolved >= 1,
        "at least one stub must be resolved, got {resolved}"
    );

    // The bare "helper" stub should be gone, resolved to a.py>helper
    assert!(
        store.lookup("helper").is_none(),
        "bare stub 'helper' must be removed after resolution"
    );
    let a_helper = store.lookup("a.py>helper").expect("a.py>helper must exist");
    let b_helper = store.lookup("b.py>helper").expect("b.py>helper must exist");

    // caller.py has an outgoing Calls edge — it should point to a.py>helper
    let calls_targets = store.outgoing(caller_file, EdgeKind::Calls);
    assert!(
        calls_targets.contains(&a_helper),
        "caller must call a.py>helper (the imported definition)"
    );
    assert!(
        !calls_targets.contains(&b_helper),
        "caller must NOT call b.py>helper (not imported)"
    );
}

#[test]
fn extractor_python_extends_metaclass_kwarg_not_captured_as_base() {
    // `class Foo(Base, metaclass=Meta):` — metaclass keyword argument must NOT
    // produce an Extends edge (it is NOT a base class).
    let store = extract("class Foo(Base, metaclass=Meta):\n    pass");
    let foo = store.lookup("test.py>Foo").expect("Foo must exist");
    let base = store.lookup("Base").expect("Base stub must exist");
    assert!(
        store.outgoing(foo, EdgeKind::Extends).contains(&base),
        "Foo must extend Base"
    );
    // metaclass=Meta must NOT create an Extends edge
    let meta = store.lookup("Meta");
    let meta_extends = meta.is_some_and(|m| store.outgoing(foo, EdgeKind::Extends).contains(&m));
    assert!(
        !meta_extends,
        "metaclass keyword argument must not produce an Extends edge"
    );
}
/// Dogfood-discovered precision tests (2026-06-04). Each test pins a Rust
/// language construct that the v0.1.18 extractor was silently dropping.
/// Together they raise per-file symbol recall from 67% → 99.8% on the
/// Mycelium repo itself (vs naive ground-truth from a string-strip regex
/// over module-level definitions).

#[test]
fn extractor_rust_trait_signature_method_is_extracted() {
    let source = "pub trait FileReindexer { fn reindex(&self); }";
    let store = extract_rs(source);
    assert!(
        store.lookup("test.rs>FileReindexer").is_some(),
        "trait FileReindexer must be extracted"
    );
    assert!(
        store.lookup("test.rs>FileReindexer>reindex").is_some(),
        "trait method signature `reindex` must be captured as a method node"
    );
}

#[test]
fn extractor_rust_trait_default_method_body_is_extracted() {
    let source = "trait Foo { fn bar() {} }";
    let store = extract_rs(source);
    assert!(
        store.lookup("test.rs>Foo>bar").is_some(),
        "default trait-method body `bar` must be a method node under Foo"
    );
}

#[test]
fn extractor_rust_static_item_is_extracted() {
    let source = "static PACK_REGISTRY: u32 = 42;";
    let store = extract_rs(source);
    assert!(
        store.lookup("test.rs>PACK_REGISTRY").is_some(),
        "module-level `static PACK_REGISTRY` must be extracted"
    );
}

#[test]
fn extractor_rust_associated_const_in_impl_is_extracted() {
    let source = "struct NodeId(u64); impl NodeId { pub const NULL: Self = Self(0); }";
    let store = extract_rs(source);
    assert!(
        store.lookup("test.rs>NodeId").is_some(),
        "struct NodeId must be extracted"
    );
    // Associated const may land either at module level or nested; either
    // counts as captured (Mycelium does not yet thread impl parent into
    // the trunk path for non-method items, but the node exists).
    assert!(
        store.lookup("test.rs>NULL").is_some() || store.lookup("test.rs>NodeId>NULL").is_some(),
        "associated const NULL must be extracted"
    );
}

#[test]
fn extractor_rust_function_inside_nested_mod_is_extracted() {
    // Tests inside `mod tests { ... }` were previously missed for
    // positions interior to the mod body — only the first / last few
    // surfaced, depending on tree-sitter parse error recovery. The
    // explicit `mod_item > function_item` query closes this.
    let source = "\
mod tests {
    fn first() {}
    fn middle_a() {}
    fn middle_b() {}
    fn middle_c() {}
    fn last() {}
}
";
    let store = extract_rs(source);
    for name in ["first", "middle_a", "middle_b", "middle_c", "last"] {
        assert!(
            store.lookup(&format!("test.rs>tests>{name}")).is_some()
                || store.lookup(&format!("test.rs>{name}")).is_some(),
            "function `{name}` inside `mod tests` must be extracted"
        );
    }
}

#[test]
fn extractor_rust_static_item_has_constant_kind() {
    // Codex P2 catch on PR #492 (2026-06-04): the `definition.static`
    // capture was inserted without a kind, so `get-symbols-by-kind constant`
    // omitted statics and the Salsa FileIndex fell back to reporting them
    // as `file`. Map static / associated_const → NodeKind::Constant.
    let source = "static FOO: u32 = 42;";
    let store = extract_rs(source);
    let id = store.lookup("test.rs>FOO").expect("FOO node must exist");
    assert_eq!(
        store.kind_of(id),
        Some(crate::types::NodeKind::Constant),
        "static FOO must be kinded as Constant"
    );
}

#[test]
fn extractor_rust_associated_const_has_constant_kind() {
    let source = "struct NodeId(u64); impl NodeId { pub const NULL: Self = Self(0); }";
    let store = extract_rs(source);
    let id = store
        .lookup("test.rs>NULL")
        .or_else(|| store.lookup("test.rs>NodeId>NULL"))
        .expect("NULL associated const must exist");
    assert_eq!(
        store.kind_of(id),
        Some(crate::types::NodeKind::Constant),
        "associated const NULL must be kinded as Constant"
    );
}

#[test]
fn extractor_rust_associated_type_has_type_alias_kind() {
    let source = "trait Tr { type Out; } struct Foo; impl Tr for Foo { type Out = u32; }";
    let store = extract_rs(source);
    // Implementation's associated type is the one carrying our new
    // capture (`impl_item > type_item`).
    let id = store
        .lookup("test.rs>Out")
        .expect("Out associated type must exist");
    assert_eq!(
        store.kind_of(id),
        Some(crate::types::NodeKind::TypeAlias),
        "associated type Out must be kinded as TypeAlias"
    );
}

#[test]
fn every_pack_definition_suffix_maps_to_a_kind() {
    // Guard against a whole bug class: a pack that uses `@definition.<suffix>`
    // with no `cap_suffix_to_kind` mapping mints KIND-LESS nodes, which then
    // silently vanish from kind-gated queries (search_symbol de-noise,
    // get-symbols-by-kind). This was real: type / namespace / template_class /
    // template_function / constructor (Go/C/C++/C#) were unmapped. Scanning the
    // canonical packs keeps the mapping table honest as new languages land.
    use std::collections::BTreeSet;
    let packs_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../packs");
    let mut suffixes: BTreeSet<String> = BTreeSet::new();
    for entry in std::fs::read_dir(packs_dir).expect("packs dir readable") {
        let scm = entry.unwrap().path().join("queries.scm");
        let Ok(src) = std::fs::read_to_string(&scm) else {
            continue;
        };
        for frag in src.split("@definition.").skip(1) {
            let suffix: String = frag
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !suffix.is_empty() {
                suffixes.insert(suffix);
            }
        }
    }
    assert!(
        suffixes.len() >= 15,
        "expected to discover many definition suffixes, found {}: {suffixes:?}",
        suffixes.len()
    );
    for suffix in &suffixes {
        assert!(
            super::cap_suffix_to_kind(suffix).is_some(),
            "pack uses @definition.{suffix} but cap_suffix_to_kind returns None → \
             kind-less nodes that vanish from search/by-kind queries"
        );
    }
}

#[test]
fn extractor_go_type_definition_is_kinded_and_searchable() {
    // Regression (PR #651 review): Go `type X struct {}` uses @definition.type,
    // which was unmapped → kind-less → dropped by the kind-annotated search
    // de-noise. After mapping `type` → TypeAlias it is kinded and searchable.
    let language: tree_sitter::Language = tree_sitter_go::LANGUAGE.into();
    let query_src = include_str!("../../../../packs/go/queries.scm");
    let ext = Extractor::new(language, query_src).expect("go extractor should build");
    let mut store = Store::new();
    ext.extract(
        "user.go",
        b"package main\ntype UserStore struct {}\n",
        &mut store,
    )
    .expect("extraction should succeed");

    let id = store
        .lookup("user.go>UserStore")
        .expect("Go type UserStore must be a node");
    assert_eq!(
        store.kind_of(id),
        Some(crate::types::NodeKind::TypeAlias),
        "Go `type` definition must be kinded (was kind-less)"
    );
    // Kind-annotated store (extractor set the File kind) → search de-noises, but
    // the now-kinded Go type must still be returned.
    let results = store.search_symbol("userstore", 10);
    assert!(
        results.iter().any(|p| p == "user.go>UserStore"),
        "Go type definition must be searchable, got: {results:?}"
    );
}
