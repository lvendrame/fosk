use crate::parser::{
    analyzer::{AnalysisContext, AnalyzerError, ColumnResolver},
    ast::{Function, Literal, ScalarExpr}
};

pub struct ScalarResolver;

impl ScalarResolver {
    pub fn scalar_literal(expr: &ScalarExpr) -> Option<Literal> {
        match expr {
            ScalarExpr::Literal(l) => Some(l.clone()),
            // you can add folding for nested functions here later
            _ => None
        }
    }

    pub fn fold_scalar(expr: &ScalarExpr) -> ScalarExpr {
        match expr {
            ScalarExpr::Function(Function { name, args , distinct}) => {
                let lname = name.to_ascii_lowercase();
                if matches!(lname.as_str(), "count" | "sum" | "avg" | "min" | "max") {
                    return ScalarExpr::Function(Function { name: name.clone(), args: args.clone(), distinct: *distinct });
                }

                // Fold args first
                let folded_args: Vec<ScalarExpr> = args.iter().map(Self::fold_scalar).collect();

                // If all literals, try to fold
                let mut lit_args = Vec::with_capacity(folded_args.len());
                for arg in &folded_args {
                    if let ScalarExpr::Literal(l) = arg { lit_args.push(l.clone()); } else {
                        return ScalarExpr::Function(Function { name: name.clone(), args: folded_args, distinct: *distinct });
                    }
                }

                let folded = match (lname.as_str(), lit_args.as_slice()) {
                    ("upper",  [Literal::String(value)]) => Some(Literal::String(value.to_uppercase())),
                    ("lower",  [Literal::String(value)]) => Some(Literal::String(value.to_lowercase())),
                    ("trim",   [Literal::String(value)]) => Some(Literal::String(value.trim().to_string())),
                    ("length", [Literal::String(value)]) => Some(Literal::Int(value.chars().count() as i64)),
                    _ => None,
                };

                folded.map(ScalarExpr::Literal)
                      .unwrap_or_else(|| ScalarExpr::Function(Function { name: name.clone(), args: folded_args, distinct: *distinct }))
            }
            _ => expr.clone()
        }
    }

    pub fn qualify_scalar(expr: &ScalarExpr, ctx: &AnalysisContext) -> Result<ScalarExpr, AnalyzerError> {
        match expr {
            ScalarExpr::Column(c) => Ok(ScalarExpr::Column(ColumnResolver::qualify_column(c, ctx)?.0)),

            ScalarExpr::Function(Function { name, args, distinct }) => {
                let lname = name.to_ascii_lowercase();

                // Special-case COUNT(*) so we don't try to qualify the wildcard
                if lname == "count" && args.len() == 1 && matches!(args[0], ScalarExpr::WildCard) {
                    return Ok(ScalarExpr::Function(Function {
                        name: name.clone(),
                        args: vec![ScalarExpr::WildCard], // keep as-is
                        distinct: *distinct,
                    }));
                }

                // Otherwise, qualify all args normally (wildcards are illegal outside COUNT)
                let mut new_args = Vec::with_capacity(args.len());
                for arg in args {
                    new_args.push(Self::qualify_scalar(arg, ctx)?);
                }
                Ok(ScalarExpr::Function(Function { name: name.clone(), args: new_args, distinct: *distinct }))
            }

            // Wildcards should already have been expanded — except COUNT(*), handled above.
            ScalarExpr::WildCard | ScalarExpr::WildCardWithCollection(_) => {
                Err(AnalyzerError::Other("wildcards must be expanded before qualification".into()))
            }

            _ => Ok(expr.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{database::{FieldInfo, SchemaProvider}, parser::ast::Column, JsonPrimitive, SchemaDict};

    use super::*;
    use indexmap::IndexMap;

    // ---- minimal schema provider & context helpers ----
    struct DummySchemas {
        by_name: std::collections::HashMap<String, SchemaDict>,
    }
    impl DummySchemas {
        fn new() -> Self { Self { by_name: std::collections::HashMap::new() } }
        fn with(mut self, name: &str, fields: Vec<(&str, JsonPrimitive, bool)>) -> Self {
            let mut m = IndexMap::new();
            for (k, ty, nullable) in fields {
                m.insert(k.to_string(), FieldInfo { ty, nullable });
            }
            self.by_name.insert(name.to_string(), SchemaDict { fields: m });
            self
        }
    }
    impl SchemaProvider for DummySchemas {
        fn schema_of(&self, backing_collection: &str) -> Option<SchemaDict> {
            self.by_name.get(backing_collection).cloned()
        }
    }

    fn ctx_for_single_table<'a>(sp: &'a DummySchemas, table: &'a str, alias: Option<&'a str>) -> AnalysisContext<'a> {
        // Build a tiny context with 1 collection (optionally aliased)
        let mut ctx = AnalysisContext::new(sp);
        let visible = alias.unwrap_or(table).to_string();
        ctx.add_collection(visible, table.to_string());
        ctx
    }

    // ---- tests ----

    #[test]
    fn scalar_literal_only_for_literal() {
        assert_eq!(
            ScalarResolver::scalar_literal(&ScalarExpr::Literal(Literal::Int(42))),
            Some(Literal::Int(42))
        );
        assert!(ScalarResolver::scalar_literal(&ScalarExpr::Column(
            Column::Name { name: "x".into() }
        )).is_none());
    }

    #[test]
    fn fold_scalar_folds_simple_and_nested_scalar_functions() {
        // upper(lower(trim("  HeLLo "))) -> "HELLO"
        let expr = ScalarExpr::Function(Function {
            name: "upper".into(),
            distinct: false,
            args: vec![ScalarExpr::Function(Function {
                name: "lower".into(),
                distinct: false,
                args: vec![ScalarExpr::Function(Function {
                    name: "trim".into(),
                    distinct: false,
                    args: vec![ScalarExpr::Literal(Literal::String("  HeLLo ".into()))],
                })],
            })],
        });

        let folded = ScalarResolver::fold_scalar(&expr);
        assert_eq!(folded, ScalarExpr::Literal(Literal::String("hello".to_uppercase())));
    }

    #[test]
    fn fold_scalar_does_not_fold_when_args_not_all_literals() {
        // length(name) where name is a column → should remain a function (after recursive attempt)
        let expr = ScalarExpr::Function(Function {
            name: "length".into(),
            distinct: false,
            args: vec![ScalarExpr::Column(Column::Name { name: "name".into() })],
        });
        let folded = ScalarResolver::fold_scalar(&expr);
        assert!(matches!(folded, ScalarExpr::Function(Function { name, .. }) if name.eq_ignore_ascii_case("length")));
    }

    #[test]
    fn fold_scalar_does_not_fold_aggregates() {
        // sum(1) must remain a function (aggregates are never constant-folded)
        let expr = ScalarExpr::Function(Function {
            name: "sum".into(),
            distinct: false,
            args: vec![ScalarExpr::Literal(Literal::Int(1))],
        });
        let folded = ScalarResolver::fold_scalar(&expr);
        assert_eq!(folded, expr);
    }

    #[test]
    fn qualify_scalar_allows_count_star_and_keeps_wildcard() {
        let sp = DummySchemas::new().with("t", vec![
            ("id", JsonPrimitive::Int, false),
        ]);
        let ctx = ctx_for_single_table(&sp, "t", None);

        let expr = ScalarExpr::Function(Function {
            name: "COUNT".into(), // case-insensitive
            distinct: false,
            args: vec![ScalarExpr::WildCard],
        });

        let qualified = ScalarResolver::qualify_scalar(&expr, &ctx).expect("qualify COUNT(*)");
        // Wildcard should be preserved inside COUNT
        match qualified {
            ScalarExpr::Function(Function { name, args, .. }) => {
                assert_eq!(name.to_ascii_lowercase(), "count");
                assert_eq!(args.len(), 1);
                assert!(matches!(args[0], ScalarExpr::WildCard));
            }
            other => panic!("expected Function(count,*), got {other:?}"),
        }
    }

    #[test]
    fn qualify_scalar_errors_on_wildcard_outside_count() {
        let sp = DummySchemas::new().with("t", vec![
            ("id", JsonPrimitive::Int, false),
        ]);
        let ctx = ctx_for_single_table(&sp, "t", None);

        let expr = ScalarExpr::Function(Function {
            name: "length".into(),
            distinct: false,
            args: vec![ScalarExpr::WildCard],
        });

        let err = ScalarResolver::qualify_scalar(&expr, &ctx);
        assert!(err.is_err(), "wildcard outside COUNT should error");
        let msg = format!("{err:?}").to_lowercase();
        assert!(msg.contains("wildcards must be expanded"), "unexpected error: {msg}");
    }

    #[test]
    fn qualify_scalar_qualifies_columns_inside_function_args() {
        // table t(name string)
        let sp = DummySchemas::new().with("t", vec![
            ("name", JsonPrimitive::String, false),
        ]);
        let ctx = ctx_for_single_table(&sp, "t", None);

        // upper(name) → argument must become Column::WithCollection { collection:"t", name:"name" }
        let expr = ScalarExpr::Function(Function {
            name: "upper".into(),
            distinct: false,
            args: vec![ScalarExpr::Column(Column::Name { name: "name".into() })],
        });

        let qualified = ScalarResolver::qualify_scalar(&expr, &ctx).expect("qualify");
        match qualified {
            ScalarExpr::Function(Function { name, args, .. }) => {
                assert_eq!(name.to_ascii_lowercase(), "upper");
                assert_eq!(args.len(), 1);
                match &args[0] {
                    ScalarExpr::Column(Column::WithCollection { collection, name }) => {
                        assert_eq!(collection, "t");
                        assert_eq!(name, "name");
                    }
                    other => panic!("expected qualified column in arg, got {other:?}"),
                }
            }
            other => panic!("expected Function, got {other:?}"),
        }
    }
}
