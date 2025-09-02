use crate::parser::{analyzer::{AnalysisContext, AnalyzerError, WildcardResolver}, ast::Identifier};

pub struct IdentifierResolver;

impl IdentifierResolver {
    pub fn expand_projection_idents(proj: &[Identifier], ctx: &AnalysisContext) -> Result<Vec<Identifier>, AnalyzerError> {
        let mut result = Vec::new();
        for id in proj {
            let expr = WildcardResolver::expand_wildcard(&id.expression, ctx)?;
            if expr.len() == 1 {
                result.push(Identifier { expression: expr.into_iter().next().unwrap(), alias: id.alias.clone() });
            } else {
                // for expanded columns, keep alias = None; client can alias later
                for e in expr {
                    result.push(Identifier { expression: e, alias: None });
                }
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::{database::{FieldInfo, SchemaProvider}, parser::ast::{Column, ScalarExpr}, JsonPrimitive, SchemaDict};

    use super::*;
    use indexmap::IndexMap;

    // ---- minimal SchemaProvider & ctx helpers ----
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

    fn ctx_with_tables<'a>(sp: &'a DummySchemas, pairs: &'a [(&'a str, Option<&'a str>)]) -> AnalysisContext<'a> {
        let mut ctx = AnalysisContext::new(sp);
        for (name, alias) in pairs {
            let visible = alias.unwrap_or(name).to_string();
            ctx.add_collection(visible, (*name).to_string());
        }
        ctx
    }

    fn ident(expr: ScalarExpr, alias: Option<&str>) -> Identifier {
        Identifier { expression: expr, alias: alias.map(|s| s.to_string()) }
    }

    // ---- tests ----

    #[test]
    fn no_wildcard_preserves_alias() {
        let sp = DummySchemas::new().with("t", vec![
            ("a", JsonPrimitive::Int, false),
        ]);
        let ctx = ctx_with_tables(&sp, &[("t", None)]);

        let proj = vec![ ident(ScalarExpr::Column(Column::Name { name: "a".into() }), Some("aa")) ];
        let out = IdentifierResolver::expand_projection_idents(&proj, &ctx).expect("expand");

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].alias.as_deref(), Some("aa"));
        // expression should be unchanged (qualification happens later)
        assert!(matches!(out[0].expression, ScalarExpr::Column(Column::Name { ref name }) if name == "a"));
    }

    #[test]
    fn star_expands_all_visible_collections_in_order_and_drops_aliases() {
        // visible order: t1, t2; field order respected
        let sp = DummySchemas::new()
            .with("t1", vec![("id", JsonPrimitive::Int, false), ("name", JsonPrimitive::String, false)])
            .with("t2", vec![("x", JsonPrimitive::Int, false)]);
        let ctx = ctx_with_tables(&sp, &[("t1", None), ("t2", None)]);

        let proj = vec![ ident(ScalarExpr::WildCard, Some("ignored")), ];
        let out = IdentifierResolver::expand_projection_idents(&proj, &ctx).expect("expand");

        // Expect: t1.id, t1.name, t2.x  (as Column::WithCollection)
        let cols: Vec<(String,String,Option<String>)> = out.into_iter().map(|id| {
            let alias = id.alias;
            match id.expression {
                ScalarExpr::Column(Column::WithCollection{ collection, name }) => (collection, name, alias),
                other => panic!("expected qualified column after wildcard expand, got {other:?}"),
            }
        }).collect();

        assert_eq!(cols, vec![
            ("t1".into(), "id".into(), None),
            ("t1".into(), "name".into(), None),
            ("t2".into(), "x".into(), None),
        ]);
    }

    #[test]
    fn table_star_expands_only_that_collection_and_keeps_alias_when_single_column() {
        // t1 has 1 column, t2 has 2
        let sp = DummySchemas::new()
            .with("t1", vec![("only", JsonPrimitive::Int, false)])
            .with("t2", vec![("a", JsonPrimitive::Int, false), ("b", JsonPrimitive::Int, false)]);
        // expose both; we will expand only t1.*
        let ctx = ctx_with_tables(&sp, &[("t1", None), ("t2", None)]);

        // SELECT t1.* AS alias  -> since expansion size == 1, alias should be preserved
        let proj = vec![ ident(ScalarExpr::WildCardWithCollection("t1".into()), Some("alias")) ];
        let out = IdentifierResolver::expand_projection_idents(&proj, &ctx).expect("expand");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].alias.as_deref(), Some("alias"));

        match &out[0].expression {
            ScalarExpr::Column(Column::WithCollection{ collection, name }) => {
                assert_eq!(collection, "t1");
                assert_eq!(name, "only");
            }
            other => panic!("expected t1.only after table star, got {other:?}"),
        }

        // Now test t2.* (2 columns) → alias must be dropped for each expanded item
        let proj2 = vec![ ident(ScalarExpr::WildCardWithCollection("t2".into()), Some("dropme")) ];
        let out2 = IdentifierResolver::expand_projection_idents(&proj2, &ctx).expect("expand");
        assert_eq!(out2.len(), 2);
        assert!(out2.iter().all(|id| id.alias.is_none()));
        let names: Vec<_> = out2.iter().map(|id| match &id.expression {
            ScalarExpr::Column(Column::WithCollection{ collection, name }) => (collection.clone(), name.clone()),
            other => panic!("expected column, got {other:?}"),
        }).collect();
        assert_eq!(names, vec![("t2".into(), "a".into()), ("t2".into(), "b".into())]);
    }

    #[test]
    fn mixed_projection_expands_in_place_order() {
        let sp = DummySchemas::new()
            .with("t", vec![("a", JsonPrimitive::Int, false), ("b", JsonPrimitive::Int, false)]);
        let ctx = ctx_with_tables(&sp, &[("t", None)]);

        // SELECT a, t.*, b
        let proj = vec![
            ident(ScalarExpr::Column(Column::Name { name: "a".into() }), Some("aa")),
            ident(ScalarExpr::WildCardWithCollection("t".into()), None),
            ident(ScalarExpr::Column(Column::Name { name: "b".into() }), None),
        ];
        let out = IdentifierResolver::expand_projection_idents(&proj, &ctx).expect("expand");

        // Expect order: a, t.a, t.b, b
        assert_eq!(out.len(), 4);

        // 1) original 'a' (alias preserved, unqualified column)
        assert_eq!(out[0].alias.as_deref(), Some("aa"));
        assert!(matches!(out[0].expression, ScalarExpr::Column(Column::Name{ ref name }) if name=="a"));

        // 2) and 3) t.* expands to qualified t.a, t.b
        for (i, nm) in ["a","b"].into_iter().enumerate() {
            match &out[1+i].expression {
                ScalarExpr::Column(Column::WithCollection{ collection, name }) => {
                    assert_eq!(collection, "t");
                    assert_eq!(name, nm);
                }
                other => panic!("expected qualified t.{nm}, got {other:?}"),
            }
            assert!(out[1+i].alias.is_none());
        }

        // 4) trailing 'b'
        assert!(matches!(out[3].expression, ScalarExpr::Column(Column::Name{ ref name }) if name=="b"));
    }

    #[test]
    fn table_star_unknown_visible_collection_errors() {
        let sp = DummySchemas::new().with("t", vec![("a", JsonPrimitive::Int, false)]);
        let ctx = ctx_with_tables(&sp, &[("t", None)]);

        // v.* where 'v' is not a visible collection
        let proj = vec![ ident(ScalarExpr::WildCardWithCollection("v".into()), None) ];
        let err = IdentifierResolver::expand_projection_idents(&proj, &ctx);
        assert!(matches!(err, Err(AnalyzerError::UnknownCollection(c)) if c == "v"));
    }
}
