use crate::parser::{analyzer::{AnalysisContext, AnalyzerError, ResolvedField}, ast::Column};

pub struct ColumnResolver;

impl ColumnResolver {
    pub fn qualify_column(col: &Column, ctx: &AnalysisContext) -> Result<(Column, ResolvedField), AnalyzerError> {
        match col {
            Column::WithCollection { collection, name } => {
                let coll_ref = ctx.collections.get(collection)
                    .ok_or_else(|| AnalyzerError::UnknownCollection(collection.clone()))?;
                let schema = ctx.schemas.schema_of(coll_ref)
                    .ok_or_else(|| AnalyzerError::UnknownCollection(coll_ref.clone()))?;
                let field_info = schema.get(name).ok_or_else(|| {
                    AnalyzerError::UnknownColumn {
                        name: format!("{}.{}", collection, name),
                        candidates: schema.fields.keys().cloned().collect()
                    }
                })?;

                Ok((col.clone(), ResolvedField {
                    collection: collection.clone(),
                    name: name.clone(),
                    ty: field_info.ty,
                    nullable: field_info.nullable
                }))
            }
            Column::Name { name } => {
                // search each visible collection’s schema for this column
                let mut matches: Vec<(String, ResolvedField)> = Vec::new();
                for (visible_coll, backing) in &ctx.collections {
                    if let Some(schema) = ctx.schemas.schema_of(backing) {
                        if let Some(field_info) = schema.get(name) {
                            matches.push((
                                visible_coll.clone(),
                                ResolvedField {
                                    collection: visible_coll.clone(),
                                    name: name.clone(),
                                    ty: field_info.ty,
                                    nullable: field_info.nullable
                                }
                            ));
                        }
                    }
                }
                match matches.len() {
                    0 => Err(AnalyzerError::UnknownColumn { name: name.clone(), candidates: vec![] }),
                    1 => {
                        let (collection, resolved_field) = matches.into_iter().next().unwrap();
                        Ok((
                            Column::WithCollection {
                                collection,
                                name: name.clone()
                            },
                            resolved_field
                        ))
                    }
                    _ => Err(AnalyzerError::AmbiguousColumn {
                        name: name.clone(),
                        matches: matches.into_iter().map(|(coll_name, rf)| (coll_name, rf.name)).collect()
                    }),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{database::{FieldInfo, SchemaProvider}, JsonPrimitive, SchemaDict};

    use super::*;
    use indexmap::IndexMap;

    // ---------- Dummy SchemaProvider ----------
    struct DummySchemas {
        // backing name -> schema
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
        fn without(mut self, name: &str) -> Self {
            self.by_name.remove(name);
            self
        }
    }
    impl SchemaProvider for DummySchemas {
        fn schema_of(&self, backing_collection: &str) -> Option<SchemaDict> {
            self.by_name.get(backing_collection).cloned()
        }
    }

    fn ctx_with_alias<'a>(sp: &'a DummySchemas, backing: &'a str, visible: &'a str) -> AnalysisContext<'a> {
        let mut ctx = AnalysisContext::new(sp);
        ctx.add_collection(visible.to_string(), backing.to_string());
        ctx
    }

    // ---------- tests ----------

    #[test]
    fn qualify_with_collection_uses_visible_alias_and_schema() {
        // backing table "users" with (id:int, name:string), visible as "u"
        let sp = DummySchemas::new().with("users", vec![
            ("id",   JsonPrimitive::Int,    false),
            ("name", JsonPrimitive::String, false),
        ]);
        let ctx = ctx_with_alias(&sp, "users", "u");

        let (qualified, rf) = ColumnResolver::qualify_column(
            &Column::WithCollection { collection: "u".into(), name: "name".into() }, &ctx
        ).expect("qualify");

        // stays WithCollection using the visible name, and resolves type/nullability
        match qualified {
            Column::WithCollection { collection, name } => {
                assert_eq!(collection, "u");
                assert_eq!(name, "name");
            }
            other => panic!("expected WithCollection, got {other:?}"),
        }
        assert_eq!(rf.collection, "u");
        assert_eq!(rf.name, "name");
        assert_eq!(rf.ty, JsonPrimitive::String);
        assert!(!rf.nullable);
    }

    #[test]
    fn qualify_unqualified_column_when_unique_among_visible_collections() {
        // two backings: users(id,name) and orders(id,total)
        // visible as "u" and "o"; column "total" exists only in "orders"
        let sp = DummySchemas::new()
            .with("users",  vec![("id", JsonPrimitive::Int, false), ("name", JsonPrimitive::String, false)])
            .with("orders", vec![("id", JsonPrimitive::Int, false), ("total", JsonPrimitive::Float, false)]);
        let mut ctx = AnalysisContext::new(&sp);
        ctx.add_collection("u", "users");
        ctx.add_collection("o", "orders");

        let (qualified, rf) = ColumnResolver::qualify_column(&Column::Name { name: "total".into() }, &ctx)
            .expect("qualify unqualified");

        // Should qualify to o.total
        match qualified {
            Column::WithCollection { collection, name } => {
                assert_eq!(collection, "o");
                assert_eq!(name, "total");
            }
            other => panic!("expected qualified WithCollection, got {other:?}"),
        }
        assert_eq!(rf.collection, "o");
        assert_eq!(rf.name, "total");
        assert_eq!(rf.ty, JsonPrimitive::Float);
    }

    #[test]
    fn error_unknown_visible_collection() {
        let sp = DummySchemas::new().with("users", vec![("id", JsonPrimitive::Int, false)]);
        let ctx = AnalysisContext::new(&sp);

        let err = ColumnResolver::qualify_column(
            &Column::WithCollection { collection: "u".into(), name: "id".into() }, &ctx
        );
        assert!(matches!(err, Err(AnalyzerError::UnknownCollection(c)) if c == "u"));
    }

    #[test]
    fn error_backing_collection_has_no_schema() {
        // ctx points "u" -> "users", but SchemaProvider has NO "users" entry
        let sp = DummySchemas::new(); // empty map
        let mut ctx = AnalysisContext::new(&sp);
        ctx.add_collection("u", "users");

        let err = ColumnResolver::qualify_column(
            &Column::WithCollection { collection: "u".into(), name: "id".into() }, &ctx
        );
        // qualify_column maps "u" -> "users", then asks schema_of("users") and gets None,
        // which your code maps to UnknownCollection(backing)
        assert!(matches!(err, Err(AnalyzerError::UnknownCollection(b)) if b == "users"));
    }

    #[test]
    fn error_unknown_column_reports_candidates() {
        // backing has columns (id, name), but we ask for "age"
        let sp = DummySchemas::new().with("users", vec![
            ("id",   JsonPrimitive::Int,    false),
            ("name", JsonPrimitive::String, false),
        ]);
        let mut ctx = AnalysisContext::new(&sp);
        ctx.add_collection("u", "users");

        let err = ColumnResolver::qualify_column(
            &Column::WithCollection { collection: "u".into(), name: "age".into() }, &ctx
        );
        match err {
            Err(AnalyzerError::UnknownColumn { name, candidates }) => {
                assert_eq!(name, "u.age");
                // candidates come from schema.fields.keys(); order doesn’t matter, use sets
                let got: std::collections::HashSet<String> = candidates.into_iter().collect();
                let mut expected = std::collections::HashSet::new();
                expected.insert("id".into());
                expected.insert("name".into());
                assert_eq!(got, expected);
            }
            other => panic!("expected UnknownColumn with candidates, got {other:?}"),
        }
    }

    #[test]
    fn error_ambiguous_unqualified_when_column_in_multiple_collections() {
        // both "users" and "orders" have "id"
        let sp = DummySchemas::new()
            .with("users",  vec![("id", JsonPrimitive::Int, false)])
            .with("orders", vec![("id", JsonPrimitive::Int, false)]);
        let mut ctx = AnalysisContext::new(&sp);
        ctx.add_collection("u", "users");
        ctx.add_collection("o", "orders");

        let err = ColumnResolver::qualify_column(&Column::Name { name: "id".into() }, &ctx);
        match err {
            Err(AnalyzerError::AmbiguousColumn { name, matches }) => {
                assert_eq!(name, "id");
                // matches is Vec<(coll, col)>; order from HashMap may vary, check as a set
                let got: std::collections::HashSet<(String,String)> = matches.into_iter().collect();
                let mut expected = std::collections::HashSet::new();
                expected.insert(("u".into(), "id".into()));
                expected.insert(("o".into(), "id".into()));
                assert_eq!(got, expected);
            }
            other => panic!("expected AmbiguousColumn, got {other:?}"),
        }
    }

    #[test]
    fn qualify_unqualified_skips_visible_collections_without_schema() {
        // Build provider with users(id) and orders(id), then remove "orders" schema.
        let sp = DummySchemas::new()
            .with("users",  vec![("id", JsonPrimitive::Int, false)])
            .with("orders", vec![("id", JsonPrimitive::Int, false)])
            .without("orders");

        // Context still exposes both aliases "u" -> users, "o" -> orders
        let mut ctx = AnalysisContext::new(&sp);
        ctx.add_collection("u", "users");
        ctx.add_collection("o", "orders");

        // Unqualified "id" should resolve to the only *schema-backed* match: u.id
        let (qualified, rf) = ColumnResolver::qualify_column(&Column::Name { name: "id".into() }, &ctx)
            .expect("should resolve uniquely to users.id");

        match qualified {
            Column::WithCollection { collection, name } => {
                assert_eq!(collection, "u");
                assert_eq!(name, "id");
            }
            other => panic!("expected qualified WithCollection, got {other:?}"),
        }
        assert_eq!(rf.collection, "u");
        assert_eq!(rf.name, "id");
        assert_eq!(rf.ty, JsonPrimitive::Int);
    }
}
