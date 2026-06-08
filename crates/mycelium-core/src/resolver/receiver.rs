//! Pure receiver-type inference for method-call disambiguation (RFC-0118 Part B).
//!
//! All functions are side-effect-free and accept only plain structs — no Store
//! or I/O access. The extractor captures per-call-site context into
//! [`ReceiverContext`] at parse time; the post-merge pass calls
//! [`infer_receiver_type`] + [`disambiguate`] against the complete candidate set.

/// An import/alias binding visible at the call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasBinding {
    /// The local alias name as written (e.g. `"store"` in `use crate::Store as store`).
    pub local: String,
    /// The fully-qualified path the alias resolves to (e.g. `"mycelium_core::store::Store"`).
    pub resolved_path: String,
}

/// A local variable binding whose RHS is a constructor call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalBinding {
    /// The local variable name.
    pub name: String,
    /// `Some("Store")` when RHS is a constructor: `Store::new()` / `Store(...)`.
    pub ctor_type: Option<String>,
}

/// A function parameter with an optional type annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamBinding {
    /// The parameter name.
    pub name: String,
    /// The declared type annotation, if present.
    pub declared_type: Option<String>,
}

/// A struct/class field with an optional type annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldBinding {
    /// The field name.
    pub name: String,
    /// The declared type annotation, if present.
    pub declared_type: Option<String>,
}

/// Per-call-site receiver context extracted at parse time.
///
/// All binding facts are local to the enclosing function scope. No cross-function
/// tracking, no flow-sensitive analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiverContext {
    /// The receiver identifier as written at this call site (e.g. `"store"`).
    pub receiver: String,
    /// The method name being called.
    pub method: String,
    /// Import/alias bindings in scope.
    pub imports: Vec<AliasBinding>,
    /// Local variable bindings in scope.
    pub locals: Vec<LocalBinding>,
    /// The enclosing impl/class type (present when inside an `impl` block or class).
    pub self_type: Option<String>,
    /// Parameter bindings of the enclosing function.
    pub params: Vec<ParamBinding>,
    /// Field bindings of the enclosing type.
    pub fields: Vec<FieldBinding>,
}

/// A statically inferred type name for a receiver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeName {
    /// A simple (unqualified) type name, e.g. `"Store"`.
    Simple(String),
    /// A path-qualified type name, e.g. `"mycelium_core::Store"`.
    Path(String),
}

/// A definition candidate for a method call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Full node path (e.g. `"src/store.rs>Store>upsert_node"`).
    pub node_path: String,
}

/// The result of [`disambiguate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Exactly one candidate matches — bind the call to this path.
    Unique(String),
    /// Zero or multiple candidates match — fall back to the conservative stub.
    Ambiguous,
}

/// Infer the receiver type for a call site using static evidence only.
///
/// Precedence (highest first):
/// - **a.** `self`/`cls`/`this` → `ctx.self_type` (enclosing impl/class type).
/// - **b.** Param name with `declared_type` (annotation-driven, high precision).
/// - **c.** Local name with `ctor_type` (constructor-driven, high precision).
/// - **d.** Field name with `declared_type`.
/// - **e.** Import alias → terminal path segment (medium precision).
///
/// Returns `None` when no rule fires. Never crosses function boundaries,
/// tracks reassignment, or performs flow-sensitive analysis.
#[must_use]
pub fn infer_receiver_type(ctx: &ReceiverContext) -> Option<TypeName> {
    let rcv = ctx.receiver.as_str();

    // a. self / cls / this → enclosing impl/class type
    if rcv == "self" || rcv == "cls" || rcv == "this" {
        return ctx.self_type.as_ref().map(|t| TypeName::Simple(t.clone()));
    }

    // b. param with declared_type
    for p in &ctx.params {
        if p.name == rcv {
            if let Some(t) = &p.declared_type {
                return Some(TypeName::Simple(t.clone()));
            }
        }
    }

    // c. local with ctor_type — if the name is shadowed (appears more than once)
    //    the binding is ambiguous; decline rather than risk a mis-bind.
    let local_matches: Vec<_> = ctx.locals.iter().filter(|l| l.name == rcv).collect();
    match local_matches.as_slice() {
        [] => {}
        [l] => {
            if let Some(t) = &l.ctor_type {
                return Some(TypeName::Simple(t.clone()));
            }
        }
        _ => return None,
    }

    // d. field with declared_type
    for f in &ctx.fields {
        if f.name == rcv {
            if let Some(t) = &f.declared_type {
                return Some(TypeName::Simple(t.clone()));
            }
        }
    }

    // e. import alias → terminal segment
    for alias in &ctx.imports {
        if alias.local == rcv {
            // rsplit("::")  is more efficient than split(..).last() on a DEI.
            let terminal = alias
                .resolved_path
                .rsplit("::")
                .next()
                .or_else(|| alias.resolved_path.rsplit('>').next())
                .unwrap_or(alias.resolved_path.as_str());
            return Some(TypeName::Simple(terminal.to_owned()));
        }
    }

    None
}

/// Disambiguate a multi-match stub given an inferred receiver type.
///
/// Returns `Resolution::Unique(path)` when:
/// - There is exactly one candidate (preserves today's single-match behaviour), or
/// - `inferred` is `Some(T)` and exactly one candidate path contains the segment `>T>`.
///
/// Returns [`Resolution::Ambiguous`] in all other cases — no wrong edge is ever made.
#[must_use]
pub fn disambiguate(inferred: Option<TypeName>, candidates: &[Candidate]) -> Resolution {
    if candidates.len() == 1 {
        return Resolution::Unique(candidates[0].node_path.clone());
    }
    let Some(type_name) = inferred else {
        return Resolution::Ambiguous;
    };
    let type_str = match &type_name {
        TypeName::Simple(s) | TypeName::Path(s) => s.as_str(),
    };
    let segment = format!(">{type_str}>");
    let matched: Vec<&str> = candidates
        .iter()
        .filter(|c| c.node_path.contains(&segment))
        .map(|c| c.node_path.as_str())
        .collect();
    if matched.len() == 1 {
        Resolution::Unique(matched[0].to_owned())
    } else {
        Resolution::Ambiguous
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_default() -> ReceiverContext {
        ReceiverContext {
            receiver: String::new(),
            method: String::new(),
            imports: vec![],
            locals: vec![],
            self_type: None,
            params: vec![],
            fields: vec![],
        }
    }

    // AC-1: local ctor binding → infers type
    #[test]
    fn infer_local_ctor_binding() {
        let ctx = ReceiverContext {
            receiver: "store".to_owned(),
            method: "upsert_node".to_owned(),
            locals: vec![LocalBinding {
                name: "store".to_owned(),
                ctor_type: Some("Store".to_owned()),
            }],
            ..ctx_default()
        };
        assert_eq!(
            infer_receiver_type(&ctx),
            Some(TypeName::Simple("Store".to_owned()))
        );
    }

    // AC-2: disambiguate unique type match
    #[test]
    fn disambiguate_unique_type_match() {
        let candidates = vec![
            Candidate {
                node_path: "a.rs>Store>upsert_node".to_owned(),
            },
            Candidate {
                node_path: "b.rs>Trunk>upsert_node".to_owned(),
            },
        ];
        let result = disambiguate(Some(TypeName::Simple("Store".to_owned())), &candidates);
        assert!(
            matches!(&result, Resolution::Unique(p) if p == "a.rs>Store>upsert_node"),
            "expected Unique(a.rs>Store>upsert_node), got {result:?}"
        );
    }

    #[test]
    fn infer_self_type() {
        let ctx = ReceiverContext {
            receiver: "self".to_owned(),
            self_type: Some("MyStruct".to_owned()),
            ..ctx_default()
        };
        assert_eq!(
            infer_receiver_type(&ctx),
            Some(TypeName::Simple("MyStruct".to_owned()))
        );
    }

    #[test]
    fn infer_self_missing_type() {
        let ctx = ReceiverContext {
            receiver: "self".to_owned(),
            ..ctx_default()
        };
        assert_eq!(infer_receiver_type(&ctx), None);
    }

    #[test]
    fn infer_param_declared_type() {
        let ctx = ReceiverContext {
            receiver: "db".to_owned(),
            params: vec![ParamBinding {
                name: "db".to_owned(),
                declared_type: Some("Database".to_owned()),
            }],
            ..ctx_default()
        };
        assert_eq!(
            infer_receiver_type(&ctx),
            Some(TypeName::Simple("Database".to_owned()))
        );
    }

    #[test]
    fn infer_param_no_type_annotation() {
        let ctx = ReceiverContext {
            receiver: "db".to_owned(),
            params: vec![ParamBinding {
                name: "db".to_owned(),
                declared_type: None,
            }],
            ..ctx_default()
        };
        assert_eq!(infer_receiver_type(&ctx), None);
    }

    #[test]
    fn infer_field_declared_type() {
        let ctx = ReceiverContext {
            receiver: "engine".to_owned(),
            fields: vec![FieldBinding {
                name: "engine".to_owned(),
                declared_type: Some("Engine".to_owned()),
            }],
            ..ctx_default()
        };
        assert_eq!(
            infer_receiver_type(&ctx),
            Some(TypeName::Simple("Engine".to_owned()))
        );
    }

    #[test]
    fn infer_import_alias_terminal() {
        let ctx = ReceiverContext {
            receiver: "store".to_owned(),
            imports: vec![AliasBinding {
                local: "store".to_owned(),
                resolved_path: "mycelium_core::store::Store".to_owned(),
            }],
            ..ctx_default()
        };
        assert_eq!(
            infer_receiver_type(&ctx),
            Some(TypeName::Simple("Store".to_owned()))
        );
    }

    #[test]
    fn infer_none_no_match() {
        let ctx = ReceiverContext {
            receiver: "unknown".to_owned(),
            ..ctx_default()
        };
        assert_eq!(infer_receiver_type(&ctx), None);
    }

    #[test]
    fn disambiguate_single_candidate() {
        let candidates = vec![Candidate {
            node_path: "a.rs>Foo>bar".to_owned(),
        }];
        assert_eq!(
            disambiguate(None, &candidates),
            Resolution::Unique("a.rs>Foo>bar".to_owned())
        );
    }

    #[test]
    fn disambiguate_ambiguous_no_inference() {
        let candidates = vec![
            Candidate {
                node_path: "a.rs>A>method".to_owned(),
            },
            Candidate {
                node_path: "b.rs>B>method".to_owned(),
            },
        ];
        assert_eq!(disambiguate(None, &candidates), Resolution::Ambiguous);
    }

    #[test]
    fn disambiguate_ambiguous_multi_match() {
        // Two candidates both contain >Foo> — should be ambiguous.
        let candidates = vec![
            Candidate {
                node_path: "a.rs>Foo>bar".to_owned(),
            },
            Candidate {
                node_path: "b.rs>Foo>bar".to_owned(),
            },
        ];
        let result = disambiguate(Some(TypeName::Simple("Foo".to_owned())), &candidates);
        assert_eq!(result, Resolution::Ambiguous);
    }

    #[test]
    fn precedence_self_over_local() {
        let ctx = ReceiverContext {
            receiver: "self".to_owned(),
            self_type: Some("Owner".to_owned()),
            locals: vec![LocalBinding {
                name: "self".to_owned(),
                ctor_type: Some("WrongType".to_owned()),
            }],
            ..ctx_default()
        };
        assert_eq!(
            infer_receiver_type(&ctx),
            Some(TypeName::Simple("Owner".to_owned()))
        );
    }

    #[test]
    fn precedence_param_over_local() {
        // param (rule b) takes precedence over local (rule c).
        let ctx = ReceiverContext {
            receiver: "x".to_owned(),
            params: vec![ParamBinding {
                name: "x".to_owned(),
                declared_type: Some("ParamType".to_owned()),
            }],
            locals: vec![LocalBinding {
                name: "x".to_owned(),
                ctor_type: Some("LocalType".to_owned()),
            }],
            ..ctx_default()
        };
        assert_eq!(
            infer_receiver_type(&ctx),
            Some(TypeName::Simple("ParamType".to_owned()))
        );
    }

    // AC-shadowed: when the same local name is bound twice (shadowing), decline — ambiguous.
    #[test]
    fn infer_shadowed_local_returns_none() {
        let ctx = ReceiverContext {
            receiver: "s".to_owned(),
            locals: vec![
                LocalBinding {
                    name: "s".to_owned(),
                    ctor_type: Some("Store".to_owned()),
                },
                LocalBinding {
                    name: "s".to_owned(),
                    ctor_type: Some("Trunk".to_owned()),
                },
            ],
            ..ctx_default()
        };
        // Both `s → Store` and `s → Trunk` in the table — ambiguous → None.
        assert_eq!(infer_receiver_type(&ctx), None);
    }
}
