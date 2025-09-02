use indexmap::IndexMap;
use once_cell::sync::Lazy;

use crate::{
    database::SchemaProvider,
    parser::{
        aggregators_helper::AggregateRegistry, analyzer::{AggregateResolver, AnalyzedIdentifier, AnalyzedQuery, AnalyzerError, ColumnKey, ColumnResolver, IdentifierResolver, JoinResolver, OrderByResolver, PredicateResolver, ScalarResolver, TypeInference}, ast::{Collection, Query}
    }
};

// Use a single shared default registry, safely.
static DEFAULT_REGISTRY: Lazy<AggregateRegistry> = Lazy::new(AggregateRegistry::default_aggregate_registry);

pub struct AnalysisContext<'a> {
    /// map visible name -> underlying collection ref (alias or table)
    pub collections: IndexMap<String, String>,
    /// access to schemas
    pub schemas: &'a dyn SchemaProvider,
    /// access to aggregate implementations
    pub aggregates: &'a AggregateRegistry,
}

impl<'a> AnalysisContext<'a> {
    pub fn new_with_aggregates(
        schemas: &'a dyn SchemaProvider,
        aggregates: &'a AggregateRegistry,
    ) -> Self {
        Self { collections: IndexMap::new(), schemas, aggregates }
    }

    /// Backward-compatible: uses the default registry.
    pub fn new(schemas: &'a dyn SchemaProvider) -> Self {
        let reg: &AggregateRegistry = &DEFAULT_REGISTRY;
        Self::new_with_aggregates(schemas, reg)
    }

    pub fn add_collection(&mut self, visible: impl Into<String>, backing: impl Into<String>) {
        self.collections.insert(visible.into(), backing.into());
    }

    pub fn add_collection_alias(&mut self, visible: impl Into<String>, backing: impl Into<String>) {
        self.collections.insert(visible.into(), backing.into());
    }

    pub fn build_context_from_query(
        q: &Query,
        sp: &'a dyn SchemaProvider,
        aggregates: &'a AggregateRegistry,
    ) -> Result<Self, AnalyzerError> {
        let mut ctx = Self::new_with_aggregates(sp, aggregates);
        for c in &q.collections {
            match c {
                Collection::Table { name, alias } => {
                    let visible = alias.clone().unwrap_or_else(|| name.clone());
                    ctx.add_collection(visible, name.clone());
                }
                Collection::Query => {
                    // you can extend to support subqueries later
                    return Err(AnalyzerError::Other("Collection::Query not yet supported in analyzer".into()));
                }
            }
        }
        // Joins often introduce new visible names too (if you allow `JOIN t AS x`)
        for join in &q.joins {
            match &join.collection {
                Collection::Table { name, alias } => {
                    let visible = alias.clone().unwrap_or_else(|| name.clone());
                    ctx.add_collection(visible, name.clone());
                }
                Collection::Query => {
                    return Err(AnalyzerError::Other("Join of subquery not yet supported in analyzer".into()));
                }
            }
        }

        Ok(ctx)
    }

    pub fn analyze_query(
        query: &Query,
        schema_provider: &'a dyn SchemaProvider,
        aggregates: &'a AggregateRegistry,
    ) -> Result<AnalyzedQuery, AnalyzerError> {
        let ctx = Self::build_context_from_query(query, schema_provider, aggregates)?;

        // expand wildcards in projection
        let expanded_proj = IdentifierResolver::expand_projection_idents(&query.projection, &ctx)?;

        // qualify + fold + type inference
        let mut analyzed_proj = Vec::with_capacity(expanded_proj.len());
        for id in expanded_proj {
            // qualify (no wildcards remain)
            let qexpr = ScalarResolver::qualify_scalar(&id.expression, &ctx)?;
            // fold constants
            let fexpr = ScalarResolver::fold_scalar(&qexpr);
            // infer type
            let (ty, nullable) = TypeInference::infer_scalar(&fexpr, &ctx)?;
            analyzed_proj.push(AnalyzedIdentifier {
                expression: fexpr,
                alias: id.alias.clone(),
                ty,
                nullable,
            });
        }

        let analyzed_joins = JoinResolver::qualify_and_fold_joins(query, &ctx)?;

        // qualify + fold predicates
        let criteria_qualified = match &query.criteria {
            Some(predicate) => Some(PredicateResolver::qualify_predicate(predicate, &ctx)?),
            None => None
        };
        let criteria = criteria_qualified.as_ref().map(PredicateResolver::fold_predicate);

        let having_qualified = match &query.having {
            Some(predicate) => Some(PredicateResolver::qualify_predicate(predicate, &ctx)?),
            None => None
        };
        let having = having_qualified.as_ref().map(PredicateResolver::fold_predicate);

        // qualify group_by columns
        let mut group_by = Vec::with_capacity(query.group_by.len());
        let mut group_set = std::collections::HashSet::<ColumnKey>::new();
        for c in &query.group_by {
            let (qc, _) = ColumnResolver::qualify_column(c, &ctx)?;
            group_set.insert(ColumnKey::of(&qc));
            group_by.push(qc);
        }

        // detect aggregate query
        let is_agg_query = !group_by.is_empty()
            || analyzed_proj.iter().any(|id| AggregateResolver::contains_aggregate(&id.expression))
            || having_qualified.as_ref().is_some_and(AggregateResolver::predicate_contains_aggregate);

        // If HAVING exists but no group-by and no aggregate anywhere, it's invalid
        if !is_agg_query && having_qualified.is_some() {
            return Err(AnalyzerError::Other("HAVING without GROUP BY must reference an aggregate".into()));
        }

        // WHERE must not contain aggregates (check on qualified, pre-fold form)
        if let Some(pq) = &criteria_qualified {
            if AggregateResolver::predicate_contains_aggregate(pq) {
                return Err(AnalyzerError::Other("Aggregates are not allowed in WHERE".into()));
            }
        }

        // Validate SELECT and HAVING in aggregate queries
        if is_agg_query {
            // SELECT expressions must use only group-by columns outside aggregate args
            for id in &analyzed_proj {
                if !AggregateResolver::uses_only_group_by(&id.expression, &group_set, false) {
                    return Err(AnalyzerError::Other("SELECT expression references columns not in GROUP BY and outside aggregates".into()));
                }
            }
            // HAVING (if present)
            if let Some(hv_q) = &having_qualified {
                if !AggregateResolver::predicate_uses_only_group_by_or_agg(hv_q, &group_set) {
                    return Err(AnalyzerError::Other("HAVING references columns not in GROUP BY and outside aggregates".into()));
                }
            }
        }

        // ORDER BY resolution (aliases, positional indexes, qualification, folding, validation)
        let order_by = OrderByResolver::qualify_order_by(&query.order_by, &analyzed_proj, &ctx, &group_set)?;

        Ok(AnalyzedQuery {
            projection: analyzed_proj,
            collections: ctx.collections.iter().map(|(v, b)| (v.clone(), b.clone())).collect(),
            joins: analyzed_joins,
            criteria,
            group_by,
            having,
            order_by,
            limit: query.limit,
            offset: query.offset,
        })
    }
}


#[cfg(test)]
mod tests {
    use crate::{
        database::FieldInfo,
        parser::ast::{Column, ComparatorOp, Function, Identifier, Literal, OrderBy, Predicate, ScalarExpr, Truth},
        JsonPrimitive, SchemaDict
    };

    use super::*;
    use indexmap::IndexMap;

    // ---------- helpers ----------

    struct DummySchemas {
        // backing collection name -> schema
        by_name: std::collections::HashMap<String, SchemaDict>,
    }
    impl DummySchemas {
        fn new() -> Self { Self { by_name: std::collections::HashMap::new() } }
        fn with(mut self, name: &str, fields: Vec<(&str, JsonPrimitive, bool)>) -> Self {
            let mut map: IndexMap<String, FieldInfo> = IndexMap::new();
            for (k, ty, nullable) in fields {
                map.insert(k.to_string(), FieldInfo { ty, nullable });
            }
            self.by_name.insert(name.to_string(), SchemaDict { fields: map });
            self
        }
    }
    impl SchemaProvider for DummySchemas {
        fn schema_of(&self, backing_collection: &str) -> Option<SchemaDict> {
            self.by_name.get(backing_collection).cloned()
        }
    }

    fn simple_ctx_for<'a>(query: &'a Query, sp: &'a DummySchemas) -> AnalysisContext<'a> {
        AnalysisContext::build_context_from_query(query, sp, &DEFAULT_REGISTRY).expect("build context")
    }

    fn make_query_with_table(
        table: &str,
        projection: Vec<Identifier>,
        criteria: Option<Predicate>,
        group_by: Vec<Column>,
        having: Option<Predicate>,
        order_by: Vec<OrderBy>,
    ) -> Query {
        Query {
            projection,
            collections: vec![Collection::Table { name: table.to_string(), alias: None }],
            joins: vec![],
            criteria,
            group_by,
            having,
            order_by,
            ..Default::default()
        }
    }

    // ---------- tests ----------

    #[test]
    fn select_group_by_validation_error() {
        // table t(a:int, b:int)
        let sp = DummySchemas::new().with("t", vec![
            ("a", JsonPrimitive::Int,  false),
            ("b", JsonPrimitive::Int,  false),
        ]);

        // SELECT a, SUM(b) FROM t GROUP BY a  -- OK
        // SELECT a, b FROM t GROUP BY a       -- ERROR (b not in group and not aggregated)
        let q_ok = make_query_with_table(
            "t",
            vec![
                Identifier { expression: ScalarExpr::Column(Column::Name { name: "a".into() }), alias: None },
                Identifier { expression: ScalarExpr::Function(Function { name: "sum".into(), args: vec![ScalarExpr::Column(Column::Name { name: "b".into() })], distinct: false }), alias: None },
            ],
            None,
            vec![Column::Name { name: "a".into() }],
            None,
            vec![],
        );
        let q_err = make_query_with_table(
            "t",
            vec![
                Identifier { expression: ScalarExpr::Column(Column::Name { name: "a".into() }), alias: None },
                Identifier { expression: ScalarExpr::Column(Column::Name { name: "b".into() }), alias: None },
            ],
            None,
            vec![Column::Name { name: "a".into() }],
            None,
            vec![],
        );

        // OK case
        let analyzed_ok = AnalysisContext::analyze_query(&q_ok, &sp, &DEFAULT_REGISTRY);
        assert!(analyzed_ok.is_ok(), "expected OK, got: {:?}", analyzed_ok);

        // Error case
        let analyzed_err = AnalysisContext::analyze_query(&q_err, &sp, &DEFAULT_REGISTRY);
        assert!(analyzed_err.is_err(), "expected GROUP BY validation error");
        let msg = format!("{analyzed_err:?}");
        assert!(msg.to_lowercase().contains("group by"), "err msg should mention group by; got: {msg}");
    }

    #[test]
    fn where_rejects_aggregates() {
        let sp = DummySchemas::new().with("t", vec![
            ("a", JsonPrimitive::Int,  false),
            ("b", JsonPrimitive::Int,  false),
        ]);

        // WHERE SUM(b) > 10  --> invalid
        let crit = Some(Predicate::Compare {
            left:  ScalarExpr::Function(Function { name: "sum".into(), args: vec![ScalarExpr::Column(Column::Name { name: "b".into() })], distinct: false }),
            op:    ComparatorOp::Gt,
            right: ScalarExpr::Literal(Literal::Int(10)),
        });

        let q = make_query_with_table(
            "t",
            vec![ Identifier { expression: ScalarExpr::Column(Column::Name { name: "a".into() }), alias: None } ],
            crit,
            vec![],
            None,
            vec![],
        );

        let res = AnalysisContext::analyze_query(&q, &sp, &DEFAULT_REGISTRY);
        assert!(res.is_err(), "aggregates in WHERE should error");
        let msg = format!("{res:?}");
        assert!(msg.to_lowercase().contains("where"), "err msg should mention WHERE; got: {msg}");
    }

    #[test]
    fn having_allows_aggregates() {
        let sp = DummySchemas::new().with("t", vec![
            ("a", JsonPrimitive::Int,  false),
            ("b", JsonPrimitive::Int,  false),
        ]);

        // SELECT a, COUNT(*) FROM t GROUP BY a HAVING COUNT(*) > 1
        let having = Some(Predicate::Compare {
            left:  ScalarExpr::Function(Function { name: "count".into(), args: vec![ScalarExpr::WildCard], distinct: false }),
            op:    ComparatorOp::Gt,
            right: ScalarExpr::Literal(Literal::Int(1)),
        });

        let q = make_query_with_table(
            "t",
            vec![
                Identifier { expression: ScalarExpr::Column(Column::Name { name: "a".into() }), alias: None },
                Identifier { expression: ScalarExpr::Function(Function { name: "count".into(), args: vec![ScalarExpr::WildCard], distinct: false }), alias: None },
            ],
            None,
            vec![Column::Name { name: "a".into() }],
            having,
            vec![],
        );

        let res = AnalysisContext::analyze_query(&q, &sp, &DEFAULT_REGISTRY);
        assert!(res.is_ok(), "HAVING with aggregate should be accepted: {:?}", res.err());
    }

    #[test]
    fn order_by_alias_and_positional_and_validation() {
        // table t(name:string, age:int)
        let sp = DummySchemas::new().with("t", vec![
            ("name", JsonPrimitive::String, false),
            ("age",  JsonPrimitive::Int,    false),
        ]);

        // SELECT name AS n, age FROM t GROUP BY name, age
        // ORDER BY n ASC, 2 DESC
        let q = make_query_with_table(
            "t",
            vec![
                Identifier { expression: ScalarExpr::Column(Column::Name { name: "name".into() }), alias: Some("n".into()) },
                Identifier { expression: ScalarExpr::Column(Column::Name { name: "age".into()  }), alias: None },
            ],
            None,
            vec![Column::Name { name: "name".into() }, Column::Name { name: "age".into() }],
            None,
            vec![
                OrderBy { expr: ScalarExpr::Column(Column::Name { name: "n".into() }), ascending: true },
                OrderBy { expr: ScalarExpr::Literal(Literal::Int(2)), ascending: false },
            ],
        );

        let analyzed = AnalysisContext::analyze_query(&q, &sp, &DEFAULT_REGISTRY).expect("analyze");
        // first ORDER BY should resolve to the `name` column expr; second to the 2nd projection (age)
        assert_eq!(analyzed.order_by.len(), 2);
        match &analyzed.order_by[0].expr {
            ScalarExpr::Column(Column::WithCollection { collection, name }) => {
                assert_eq!(name, "name");
                assert_eq!(collection, "t");
            }
            e => panic!("unexpected first order by expr: {e:?}"),
        }
        match &analyzed.order_by[1].expr {
            ScalarExpr::Column(Column::WithCollection { name, .. }) => assert_eq!(name, "age"),
            e => panic!("unexpected second order by expr: {e:?}"),
        }

        // Now trigger ORDER BY validation error: reference non-grouped col outside aggregate
        // SELECT COUNT(*) FROM t GROUP BY name ORDER BY age
        let q_bad = make_query_with_table(
            "t",
            vec![ Identifier { expression: ScalarExpr::Function(Function { name: "count".into(), args: vec![ScalarExpr::WildCard], distinct: false }), alias: None } ],
            None,
            vec![Column::Name { name: "name".into() }],
            None,
            vec![ OrderBy { expr: ScalarExpr::Column(Column::Name { name: "age".into() }), ascending: true } ],
        );
        let err = AnalysisContext::analyze_query(&q_bad, &sp, &DEFAULT_REGISTRY);
        assert!(err.is_err(), "ORDER BY should error when referencing non-grouped columns outside aggregates");
        let msg = format!("{err:?}");
        assert!(msg.to_lowercase().contains("order by"), "err msg should mention ORDER BY; got: {msg}");
    }

    #[test]
    fn wildcard_expansion_is_stable() {
        // Two collections in insertion order: t1 then t2
        let sp = DummySchemas::new()
            .with("t1", vec![
                ("id",  JsonPrimitive::Int,    false),
                ("name",JsonPrimitive::String, false),
            ])
            .with("t2", vec![
                ("x", JsonPrimitive::Int, false),
            ]);

        let query = Query {
            projection: vec![ Identifier { expression: ScalarExpr::WildCard, alias: None } ],
            collections: vec![
                Collection::Table { name: "t1".into(), alias: None },
                Collection::Table { name: "t2".into(), alias: None },
            ],
            joins: vec![],
            criteria: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            ..Default::default()
        };

        // build ctx & expand (indirectly via analyze)
        let analyzed = AnalysisContext::analyze_query(&query, &sp, &DEFAULT_REGISTRY).expect("analyze");
        // Expect order: t1.id, t1.name, t2.x
        let cols: Vec<(String,String)> = analyzed.projection.iter().filter_map(|id| {
            if let ScalarExpr::Column(Column::WithCollection{collection, name}) = &id.expression {
                Some((collection.clone(), name.clone()))
            } else { None }
        }).collect();

        assert_eq!(cols, vec![
            ("t1".into(), "id".into()),
            ("t1".into(), "name".into()),
            ("t2".into(), "x".into()),
        ]);
    }

    #[test]
    fn folding_like_case_insensitive_with_escape_and_in_null_unknown() {
        // LIKE folding
        let p1 = Predicate::Like {
            expr:     ScalarExpr::Literal(Literal::String("Hello".into())),
            pattern:  ScalarExpr::Literal(Literal::String("he%".into())),
            negated:  false,
        };
        match PredicateResolver::fold_predicate(&p1) {
            Predicate::Const3(Truth::True) => {},
            other => panic!("expected Const3(True), got {other:?}"),
        }

        // Escape: value "he%llo", pattern r"he\%l%"
        let p2 = Predicate::Like {
            expr:     ScalarExpr::Literal(Literal::String("he%llo".into())),
            pattern:  ScalarExpr::Literal(Literal::String(r"he\%l%".into())),
            negated:  false,
        };
        match PredicateResolver::fold_predicate(&p2) {
            Predicate::Const3(Truth::True) => {},
            other => panic!("expected Const3(True) for escaped %, got {other:?}"),
        }

        // IN with NULL → Unknown when no match found
        let p3 = Predicate::InList {
            expr:    ScalarExpr::Literal(Literal::Int(2)),
            list:    vec![ScalarExpr::Literal(Literal::Int(1)), ScalarExpr::Literal(Literal::Null)],
            negated: false,
        };
        match PredicateResolver::fold_predicate(&p3) {
            Predicate::Const3(Truth::Unknown) => {},
            other => panic!("expected Const3(Unknown) for IN with NULL, got {other:?}"),
        }
    }

    #[test]
    fn type_inference_for_aggregates() {
        // table t(i:int, f:float, s:string)
        let sp = DummySchemas::new().with("t", vec![
            ("i", JsonPrimitive::Int,   false),
            ("f", JsonPrimitive::Float, false),
            ("s", JsonPrimitive::String,false),
        ]);

        let q_base = Query {
            projection: vec![],
            collections: vec![Collection::Table { name: "t".into(), alias: None }],
            joins: vec![],
            criteria: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            ..Default::default()
        };
        let ctx = simple_ctx_for(&q_base, &sp);

        // COUNT(*)
        let cnt = ScalarExpr::Function(Function { name: "count".into(), args: vec![ScalarExpr::WildCard], distinct: false });
        let (ty, nullable) = TypeInference::infer_scalar(&cnt, &ctx).expect("type");
        assert_eq!(ty, JsonPrimitive::Int);
        assert!(!nullable);

        // SUM(i) -> Int, nullable
        let sum_i = ScalarExpr::Function(Function { name: "sum".into(), args: vec![ScalarExpr::Column(Column::Name { name: "i".into() })], distinct: false });
        let (ty, nullable) = TypeInference::infer_scalar(&sum_i, &ctx).expect("type");
        assert_eq!(ty, JsonPrimitive::Int);
        assert!(nullable);

        // SUM(f) -> Float, nullable
        let sum_f = ScalarExpr::Function(Function { name: "sum".into(), args: vec![ScalarExpr::Column(Column::Name { name: "f".into() })], distinct: false });
        let (ty, nullable) = TypeInference::infer_scalar(&sum_f, &ctx).expect("type");
        assert_eq!(ty, JsonPrimitive::Float);
        assert!(nullable);

        // AVG(i) -> Float, nullable
        let avg_i = ScalarExpr::Function(Function { name: "avg".into(), args: vec![ScalarExpr::Column(Column::Name { name: "i".into() })], distinct: false });
        let (ty, nullable) = TypeInference::infer_scalar(&avg_i, &ctx).expect("type");
        assert_eq!(ty, JsonPrimitive::Float);
        assert!(nullable);

        // MIN(s) -> String, nullable
        let min_s = ScalarExpr::Function(Function { name: "min".into(), args: vec![ScalarExpr::Column(Column::Name { name: "s".into() })], distinct: false });
        let (ty, nullable) = TypeInference::infer_scalar(&min_s, &ctx).expect("type");
        assert_eq!(ty, JsonPrimitive::String);
        assert!(nullable);
    }
}
