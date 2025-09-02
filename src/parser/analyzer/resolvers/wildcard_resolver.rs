use crate::parser::{analyzer::{AnalysisContext, AnalyzerError}, ast::{Column, ScalarExpr}};

pub struct WildcardResolver;

impl WildcardResolver {
    pub fn expand_wildcard(expr: &ScalarExpr, ctx: &AnalysisContext) -> Result<Vec<ScalarExpr>, AnalyzerError> {
        match expr {
            ScalarExpr::WildCard => {
                // expand all visible collections in insertion order
                let mut result = Vec::new();
                for (visible_collection, backing) in &ctx.collections {
                    if let Some(schema) = ctx.schemas.schema_of(backing) {
                        for (col, _fi) in schema.fields {
                            result.push(ScalarExpr::Column(
                                Column::WithCollection { collection: visible_collection.clone(), name: col }
                            ));
                        }
                    } else {
                        return Err(AnalyzerError::UnknownCollection(backing.clone()));
                    }
                }
                Ok(result)
            }
            ScalarExpr::WildCardWithCollection(collection) => {
                let backing = ctx.collections.get(collection)
                    .ok_or_else(|| AnalyzerError::UnknownCollection(collection.clone()))?;
                let schema = ctx.schemas.schema_of(backing)
                    .ok_or_else(|| AnalyzerError::UnknownCollection(backing.clone()))?;
                let mut result = Vec::new();
                for (col, _fi) in schema.fields {
                    result.push(ScalarExpr::Column(
                        Column::WithCollection { collection: collection.clone(), name: col }
                    ));
                }
                Ok(result)
            }
            _ => Ok(vec![expr.clone()])
        }
    }

    /// Expand a whole projection vector that may contain wildcards.
    pub fn expand_projection(exprs: &[ScalarExpr], cx: &AnalysisContext) -> Result<Vec<ScalarExpr>, AnalyzerError> {
        let mut result = Vec::new();
        for e in exprs {
            let expanded = Self::expand_wildcard(e, cx)?;
            result.extend(expanded);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::{database::{FieldInfo, SchemaProvider}, parser::ast::Function, JsonPrimitive, SchemaDict};

    use super::*;
    use indexmap::IndexMap;

    // ---- tiny SchemaProvider + ctx helpers ----
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

    fn ctx_with<'a>(sp: &'a DummySchemas, pairs: &'a [(&'a str, Option<&'a str>)]) -> AnalysisContext<'a> {
        let mut ctx = AnalysisContext::new(sp);
        for (backing, alias) in pairs {
            ctx.add_collection(alias.unwrap_or(backing).to_string(), (*backing).to_string());
        }
        ctx
    }

    // ---- tests ----

    #[test]
    fn star_expands_all_visible_collections_and_all_fields() {
        // two tables: t1(id,name), t2(x)
        let sp = DummySchemas::new()
            .with("t1", vec![
                ("id",   JsonPrimitive::Int,    false),
                ("name", JsonPrimitive::String, false),
            ])
            .with("t2", vec![
                ("x", JsonPrimitive::Int, false),
            ]);
        let ctx = ctx_with(&sp, &[("t1", None), ("t2", None)]);

        let out = WildcardResolver::expand_wildcard(&ScalarExpr::WildCard, &ctx)
            .expect("expand *");

        // We only assert the set of results and per-collection field order; we don’t rely on
        // the iteration order of visible collections (your ctx may be IndexMap or similar).
        let mut t1_cols = vec![];
        let mut t2_cols = vec![];
        for e in out {
            match e {
                ScalarExpr::Column(Column::WithCollection { collection, name }) if collection == "t1" => t1_cols.push(name),
                ScalarExpr::Column(Column::WithCollection { collection, name }) if collection == "t2" => t2_cols.push(name),
                other => panic!("unexpected expr from wildcard: {other:?}"),
            }
        }
        assert_eq!(t1_cols, vec!["id".to_string(), "name".to_string()], "field order for t1 preserved");
        assert_eq!(t2_cols, vec!["x".to_string()], "field order for t2 preserved");
    }

    #[test]
    fn table_star_expands_only_that_collection() {
        let sp = DummySchemas::new()
            .with("users", vec![
                ("id", JsonPrimitive::Int, false),
                ("name", JsonPrimitive::String, false),
            ])
            .with("orders", vec![
                ("total", JsonPrimitive::Float, false),
            ]);
        // visible aliases u -> users, o -> orders
        let ctx = ctx_with(&sp, &[("users", Some("u")), ("orders", Some("o"))]);

        let out = WildcardResolver::expand_wildcard(&ScalarExpr::WildCardWithCollection("u".into()), &ctx)
            .expect("expand u.*");

        let cols: Vec<(String,String)> = out.into_iter().map(|e| match e {
            ScalarExpr::Column(Column::WithCollection { collection, name }) => (collection, name),
            other => panic!("expected Column::WithCollection, got {other:?}"),
        }).collect();

        assert_eq!(cols, vec![
            ("u".into(), "id".into()),
            ("u".into(), "name".into()),
        ]);
    }

    #[test]
    fn non_wildcard_expression_is_returned_as_singleton() {
        // COUNT(*) should remain untouched (WildcardResolver does not descend)
        let expr = ScalarExpr::Function(Function {
            name: "count".into(),
            args: vec![ScalarExpr::WildCard],
            distinct: false,
        });
        // even with empty provider/ctx, it should just return the same expr
        let sp = DummySchemas::new();
        let ctx = ctx_with(&sp, &[]);
        let out = WildcardResolver::expand_wildcard(&expr, &ctx).expect("no-op");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], expr);
    }

    #[test]
    fn table_star_unknown_visible_collection_errors() {
        let sp = DummySchemas::new().with("t", vec![("a", JsonPrimitive::Int, false)]);
        let ctx = ctx_with(&sp, &[("t", None)]);

        let err = WildcardResolver::expand_wildcard(&ScalarExpr::WildCardWithCollection("v".into()), &ctx);
        match err {
            Err(AnalyzerError::UnknownCollection(c)) => assert_eq!(c, "v"),
            other => panic!("expected UnknownCollection(\"v\"), got {other:?}"),
        }
    }

    #[test]
    fn star_errors_when_a_visible_collection_has_no_schema() {
        // ctx maps v -> backing "t", but provider has no "t" schema
        let sp = DummySchemas::new(); // empty
        let ctx = ctx_with(&sp, &[("t", Some("v"))]);

        // expand * should fail at the collection lacking schema
        let err = WildcardResolver::expand_wildcard(&ScalarExpr::WildCard, &ctx);
        match err {
            Err(AnalyzerError::UnknownCollection(b)) => assert_eq!(b, "t"),
            other => panic!("expected UnknownCollection(\"t\"), got {other:?}"),
        }
    }

    #[test]
    fn table_star_on_empty_schema_returns_empty_vec() {
        // provider has an empty schema for "empty" (no fields)
        let sp = {
            let m = IndexMap::new();
            let empty_schema = SchemaDict { fields: m };
            let mut by = std::collections::HashMap::new();
            by.insert("empty".to_string(), empty_schema);
            DummySchemas { by_name: by }
        };
        let ctx = ctx_with(&sp, &[("empty", None)]);

        let out = WildcardResolver::expand_wildcard(&ScalarExpr::WildCardWithCollection("empty".into()), &ctx)
            .expect("expand empty.*");
        assert!(out.is_empty(), "expansion over empty schema should be empty");
    }
}
