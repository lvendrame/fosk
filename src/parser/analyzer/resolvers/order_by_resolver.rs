use crate::parser::{analyzer::{AggregateResolver, AnalysisContext, AnalyzedIdentifier, AnalyzerError, ColumnKey, ScalarResolver}, ast::{Column, Literal, OrderBy, ScalarExpr}};

pub struct OrderByResolver;

impl OrderByResolver {
    pub fn qualify_order_by(
        order_bys: &[OrderBy],
        projection: &[AnalyzedIdentifier], // qualified & folded
        ctx: &AnalysisContext,
        group_set: &std::collections::HashSet<ColumnKey>,
    ) -> Result<Vec<OrderBy>, AnalyzerError> {
        // alias map (case-insensitive)
        let mut alias_map = std::collections::HashMap::<String, &ScalarExpr>::new();
        for analyzed_id in projection {
            if let Some(alias) = &analyzed_id.alias {
                alias_map.insert(alias.to_ascii_lowercase(), &analyzed_id.expression);
            }
        }

        let mut out = Vec::with_capacity(order_bys.len());
        for order_by in order_bys {
            // positional (1-based)
            if let ScalarExpr::Literal(Literal::Int(value)) = &order_by.expr {
                let pos = *value as usize;
                if pos == 0 || pos > projection.len() {
                    return Err(AnalyzerError::Other(format!("ORDER BY position {} out of range [1..{}]", pos, projection.len())));
                }
                let expr = projection[pos - 1].expression.clone();
                out.push(OrderBy { expr, ascending: order_by.ascending });
                continue;
            }

            // alias match: only when bare column name
            if let ScalarExpr::Column(Column::Name { name }) = &order_by.expr {
                if let Some(src) = alias_map.get(&name.to_ascii_lowercase()) {
                    out.push(OrderBy { expr: (*src).clone(), ascending: order_by.ascending });
                    continue;
                }
            }

            // normal path
            let qualified = ScalarResolver::qualify_scalar(&order_by.expr, ctx)?;
            let folded = ScalarResolver::fold_scalar(&qualified);

            if !AggregateResolver::uses_only_group_by(&folded, group_set, false) {
                return Err(AnalyzerError::Other("ORDER BY references columns not in GROUP BY and outside aggregates".into()));
            }

            out.push(OrderBy { expr: folded, ascending: order_by.ascending });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use crate::{database::{FieldInfo, SchemaProvider}, parser::ast::Function, JsonPrimitive, SchemaDict};

    use super::*;
    use indexmap::IndexMap;
    use std::collections::HashSet;

    // ------ minimal schema provider & ctx helpers ------
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

    fn build_ctx_with_table<'a>(sp: &'a DummySchemas, table: &'a str, alias: Option<&'a str>) -> AnalysisContext<'a> {
        let mut ctx = AnalysisContext::new(sp);
        let visible = alias.unwrap_or(table);
        ctx.add_collection(visible.to_string(), table.to_string());
        ctx
    }

    fn proj_id(expr: ScalarExpr, alias: Option<&str>, ty: JsonPrimitive, nullable: bool) -> AnalyzedIdentifier {
        AnalyzedIdentifier { expression: expr, alias: alias.map(|s| s.to_string()), ty, nullable }
    }

    // ------ tests ------

    #[test]
    fn order_by_alias_and_positional_resolve_correctly() {
        // table t(name:string, age:int)
        let sp = DummySchemas::new().with("t", vec![
            ("name", JsonPrimitive::String, false),
            ("age",  JsonPrimitive::Int,    false),
        ]);
        let ctx = build_ctx_with_table(&sp, "t", None);

        // projection: SELECT name AS n, age
        let projection = vec![
            proj_id(ScalarExpr::Column(Column::WithCollection{ collection: "t".into(), name: "name".into() }), Some("n"), JsonPrimitive::String, false),
            proj_id(ScalarExpr::Column(Column::WithCollection{ collection: "t".into(), name: "age".into()  }), None,       JsonPrimitive::Int,    false),
        ];

        // group set includes both (simulate GROUP BY name, age) – not strictly required for this test
        let mut group_set = HashSet::new();
        group_set.insert(ColumnKey { column: "t".into(), name: "name".into() });
        group_set.insert(ColumnKey { column: "t".into(), name: "age".into() });

        // ORDER BY n ASC, 2 DESC
        let order = vec![
            OrderBy { expr: ScalarExpr::Column(Column::Name { name: "n".into() }), ascending: true },
            OrderBy { expr: ScalarExpr::Literal(Literal::Int(2)), ascending: false },
        ];

        let out = OrderByResolver::qualify_order_by(&order, &projection, &ctx, &group_set).expect("order by");
        assert_eq!(out.len(), 2);

        // alias resolved to t.name
        match &out[0].expr {
            ScalarExpr::Column(Column::WithCollection{ collection, name }) => {
                assert_eq!(collection, "t");
                assert_eq!(name, "name");
            }
            other => panic!("first ORDER BY should be qualified column, got {other:?}"),
        }

        // positional 2 resolved to second projection (t.age)
        match &out[1].expr {
            ScalarExpr::Column(Column::WithCollection{ name, .. }) => assert_eq!(name, "age"),
            other => panic!("second ORDER BY should be the 2nd projection expr, got {other:?}"),
        }
    }

    #[test]
    fn order_by_positional_oob_errors() {
        let sp = DummySchemas::new().with("t", vec![
            ("a", JsonPrimitive::Int, false),
        ]);
        let ctx = build_ctx_with_table(&sp, "t", None);

        let projection = vec![
            proj_id(ScalarExpr::Column(Column::WithCollection{ collection: "t".into(), name: "a".into() }), None, JsonPrimitive::Int, false),
        ];
        let group_set = {
            let mut s = HashSet::new();
            s.insert(ColumnKey { column: "t".into(), name: "a".into() });
            s
        };

        // 0 is invalid (must be 1-based)
        let err0 = OrderByResolver::qualify_order_by(
            &[OrderBy { expr: ScalarExpr::Literal(Literal::Int(0)), ascending: true }],
            &projection, &ctx, &group_set);
        assert!(err0.is_err());

        // > len is invalid
        let err2 = OrderByResolver::qualify_order_by(
            &[OrderBy { expr: ScalarExpr::Literal(Literal::Int(2)), ascending: true }],
            &projection, &ctx, &group_set);
        assert!(err2.is_err());
    }

    #[test]
    fn order_by_rejects_non_grouped_column_outside_aggregates_in_agg_query() {
        // table t(name, age)
        let sp = DummySchemas::new().with("t", vec![
            ("name", JsonPrimitive::String, false),
            ("age",  JsonPrimitive::Int,    false),
        ]);
        let ctx = build_ctx_with_table(&sp, "t", None);

        // projection: COUNT(*) (agg query), no group by
        let projection = vec![
            proj_id(ScalarExpr::Function(Function { name: "count".into(), args: vec![ScalarExpr::WildCard], distinct: false }),
                    None, JsonPrimitive::Int, false),
        ];

        // group set empty (no GROUP BY)
        let group_set = HashSet::<ColumnKey>::new();

        // ORDER BY age  → not allowed in agg query (outside aggregate and not in group set)
        let order = vec![
            OrderBy { expr: ScalarExpr::Column(Column::Name { name: "age".into() }), ascending: true }
        ];

        let err = OrderByResolver::qualify_order_by(&order, &projection, &ctx, &group_set);
        assert!(err.is_err(), "should reject non-grouped column in ORDER BY for agg query");
        let msg = format!("{err:?}").to_lowercase();
        assert!(msg.contains("order by"), "error message should mention ORDER BY; got {msg}");
    }

    #[test]
    fn order_by_allows_aggregate_expressions() {
        let sp = DummySchemas::new().with("t", vec![
            ("a", JsonPrimitive::Int, false),
        ]);
        let ctx = build_ctx_with_table(&sp, "t", None);

        // projection: COUNT(*)
        let projection = vec![
            proj_id(ScalarExpr::Function(Function { name: "count".into(), args: vec![ScalarExpr::WildCard], distinct: false }),
                    None, JsonPrimitive::Int, false),
        ];
        let group_set = HashSet::<ColumnKey>::new();

        // ORDER BY COUNT(*)  → allowed
        let order = vec![
            OrderBy { expr: ScalarExpr::Function(Function { name: "count".into(), args: vec![ScalarExpr::WildCard], distinct: false }), ascending: false }
        ];

        let out = OrderByResolver::qualify_order_by(&order, &projection, &ctx, &group_set).expect("order by");
        assert_eq!(out.len(), 1);
        // still a function after qualification & folding
        assert!(matches!(out[0].expr, ScalarExpr::Function(Function { ref name, .. }) if name.eq_ignore_ascii_case("count")));
    }

    #[test]
    fn order_by_regular_expression_path() {
        // table t(name:string)
        let sp = DummySchemas::new().with("t", vec![
            ("name", JsonPrimitive::String, false),
        ]);
        let ctx = build_ctx_with_table(&sp, "t", None);

        // projection: name
        let projection = vec![
            proj_id(ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"name".into() }), None, JsonPrimitive::String, false),
        ];

        // group by name so validation passes
        let mut group_set = HashSet::new();
        group_set.insert(ColumnKey { column:"t".into(), name:"name".into() });

        // ORDER BY upper(name)
        let order = vec![
            OrderBy {
                expr: ScalarExpr::Function(Function {
                    name: "upper".into(),
                    distinct: false,
                    args: vec![ScalarExpr::Column(Column::Name { name: "name".into() })],
                }),
                ascending: true
            }
        ];

        let out = OrderByResolver::qualify_order_by(&order, &projection, &ctx, &group_set).expect("order by");
        assert_eq!(out.len(), 1);
        match &out[0].expr {
            ScalarExpr::Function(Function { name, args, .. }) => {
                assert_eq!(name.to_ascii_lowercase(), "upper");
                assert_eq!(args.len(), 1);
                match &args[0] {
                    ScalarExpr::Column(Column::WithCollection { collection, name }) => {
                        assert_eq!(collection, "t");
                        assert_eq!(name, "name");
                    }
                    other => panic!("argument should be qualified column, got {other:?}"),
                }
            }
            other => panic!("expected function in ORDER BY, got {other:?}"),
        }
    }

    #[test]
    fn order_by_alias_not_found_falls_back_and_errors_on_unknown_column() {
        // schema: t(a int)
        let sp = DummySchemas::new().with("t", vec![
            ("a", JsonPrimitive::Int, false),
        ]);
        let ctx = build_ctx_with_table(&sp, "t", None);

        // projection: SELECT a AS aa
        let projection = vec![
            proj_id(
                ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"a".into()}),
                Some("aa"),
                JsonPrimitive::Int,
                false
            ),
        ];

        // group by a so validation wouldn’t be the reason
        let mut group_set = std::collections::HashSet::new();
        group_set.insert(ColumnKey { column:"t".into(), name:"a".into() });

        // ORDER BY bogus alias -> Column::Name("zzz") should *not* match alias map,
        // then qualify_scalar will try to resolve a column "zzz" and fail.
        let order = vec![ OrderBy {
            expr: ScalarExpr::Column(Column::Name { name: "zzz".into() }),
            ascending: true
        }];

        let err = OrderByResolver::qualify_order_by(&order, &projection, &ctx, &group_set);
        assert!(err.is_err());
        let msg = format!("{err:?}").to_lowercase();
        assert!(msg.contains("unknowncolumn") || msg.contains("unknown column") || msg.contains("unknown"), "unexpected error: {msg}");
    }

    #[test]
    fn order_by_positional_negative_is_out_of_range() {
        // schema: t(a int)
        let sp = DummySchemas::new().with("t", vec![
            ("a", JsonPrimitive::Int, false),
        ]);
        let ctx = build_ctx_with_table(&sp, "t", None);

        let projection = vec![
            proj_id(ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"a".into() }), None, JsonPrimitive::Int, false),
        ];

        let group_set = {
            let mut s = std::collections::HashSet::new();
            s.insert(ColumnKey { column:"t".into(), name:"a".into() });
            s
        };

        // ORDER BY -1 -> should error (out of range)
        let order = vec![ OrderBy { expr: ScalarExpr::Literal(Literal::Int(-1)), ascending: true } ];
        let err = OrderByResolver::qualify_order_by(&order, &projection, &ctx, &group_set);
        assert!(err.is_err());
        let msg = format!("{err:?}").to_lowercase();
        assert!(msg.contains("order by position") || msg.contains("out of range"), "unexpected error: {msg}");
    }

    #[test]
    fn order_by_scalar_over_non_grouped_column_is_rejected_in_agg_query() {
        // schema: t(age int)
        let sp = DummySchemas::new().with("t", vec![
            ("age", JsonPrimitive::Int, false),
        ]);
        let ctx = build_ctx_with_table(&sp, "t", None);

        // projection is COUNT(*) => aggregate query
        let projection = vec![
            proj_id(
                ScalarExpr::Function(Function { name: "count".into(), args: vec![ScalarExpr::WildCard], distinct: false }),
                None, JsonPrimitive::Int, false
            ),
        ];
        let group_set = std::collections::HashSet::<ColumnKey>::new();

        // ORDER BY UPPER(age) — still outside aggregate and not grouped → error
        let order = vec![ OrderBy {
            expr: ScalarExpr::Function(Function {
                name: "upper".into(),
                distinct: false,
                args: vec![ScalarExpr::Column(Column::Name { name: "age".into() })]
            }),
            ascending: true
        }];

        let err = OrderByResolver::qualify_order_by(&order, &projection, &ctx, &group_set);
        assert!(err.is_err());
        let msg = format!("{err:?}").to_lowercase();
        assert!(msg.contains("order by") && msg.contains("group by"), "unexpected error: {msg}");
    }
}
