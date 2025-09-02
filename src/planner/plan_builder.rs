use std::collections::{HashMap, HashSet};

use crate::{parser::{analyzer::{AggregateResolver, AnalyzedQuery, AnalyzerError}, ast::{Column, JoinType, Predicate, ScalarExpr, Truth}}, planner::{aggregate_call::AggregateCall, logical_plan::LogicalPlan}};

pub struct PlanBuilder;

impl PlanBuilder {
    pub fn from_analyzed(aq: &AnalyzedQuery) -> Result<LogicalPlan, AnalyzerError> {
        // Source: single collection only (joins later)
        if aq.collections.is_empty() {
            return Err(AnalyzerError::Other("Planner: no collections to scan".into()));
        }
        // base scan = the first visible/backing pair
        let (visible0, backing0) = aq.collections[0].clone();
        let mut from: LogicalPlan = LogicalPlan::Scan { backing: backing0, visible: visible0 };

        if aq.collections.len() > 1 {
            for (visible, backing) in aq.collections.iter().skip(1).cloned() {
                let right = LogicalPlan::Scan { backing, visible };
                from = LogicalPlan::Join {
                    left: Box::new(from),
                    right: Box::new(right),
                    join_type: JoinType::Inner,                    // CROSS JOIN semantics
                    on: Predicate::Const3(Truth::True),            // always true
                };
            }
        }

        // Apply joins in order they appear
        for j in &aq.joins {
            let (visible, backing) = match &j.collection {
                crate::parser::ast::Collection::Table { name, alias } => {
                    let vis = alias.clone().unwrap_or_else(|| name.clone());
                    (vis, name.clone())
                }
                crate::parser::ast::Collection::Query => {
                    return Err(AnalyzerError::Other("Planner: subquery in JOIN not supported".into()));
                }
            };
            let right = LogicalPlan::Scan { backing, visible };
            from = LogicalPlan::Join {
                left: Box::new(from),
                right: Box::new(right),
                join_type: j.join_type.clone(),
                on: j.predicate.clone(), // already qualified + folded
            };
        }

        let mut plan = from;

        // WHERE (criteria)
        if let Some(pred) = &aq.criteria {
            plan = LogicalPlan::Filter { input: Box::new(plan), predicate: pred.clone() };
        }

        // Decide if we need aggregation
        let needs_agg =
            !aq.group_by.is_empty()
            || aq.projection.iter().any(|id| AggregateResolver::contains_aggregate(&id.expression))
            || aq.having.as_ref().map(AggregateResolver::predicate_contains_aggregate).unwrap_or(false);

        // If not an aggregate query, proceed with projection/sort/limit directly
        if !needs_agg {
            // Project
            plan = LogicalPlan::Project { input: Box::new(plan), exprs: aq.projection.clone() };
            // ORDER BY
            if !aq.order_by.is_empty() {
                plan = LogicalPlan::Sort { input: Box::new(plan), keys: aq.order_by.clone() };
            }
            // LIMIT/OFFSET
            if aq.limit.is_some() || aq.offset.is_some() {
                plan = LogicalPlan::Limit {
                    input: Box::new(plan),
                    limit: aq.limit,
                    offset: aq.offset,
                };
            }
            return Ok(plan);
        }

        // ---- Aggregate path ----

        // 1) Collect aggregate calls from SELECT and HAVING (deduplicated)
        let mut calls: Vec<AggregateCall> = Vec::new();
        let mut call_to_index: HashMap<AggregateCall, usize> = HashMap::new();

        // from SELECT projection
        for id in &aq.projection {
            Self::collect_aggregates_in_scalar(&id.expression, &mut call_to_index, &mut calls);
        }
        // from HAVING, if present
        if let Some(h) = &aq.having {
            Self::collect_aggregates_in_predicate(h, &mut call_to_index, &mut calls);
        }

        // 2) Assign output names per call (avoid clashes with group key names)
        let mut used: HashSet<String> = HashSet::new();
        for c in &aq.group_by {
            let key = match c {
                Column::WithCollection { collection, name } => format!("{}.{}", collection, name),
                Column::Name { name } => name.clone(),
            };
            used.insert(key);
        }

        // Prefer SELECT aliases for matching calls; otherwise func, func_1, ...
        let mut call_to_name: HashMap<AggregateCall, String> = HashMap::new();

        // Prefer aliases from projection if provided
        for id in &aq.projection {
            if let ScalarExpr::Function(f) = &id.expression {
                if AggregateResolver::is_aggregate_name(&f.name) {
                    let key: AggregateCall = f.into();
                    if call_to_index.contains_key(&key) {
                        let base = if let Some(a) = &id.alias {
                            a.to_ascii_lowercase()
                        } else {
                            key.func.clone()
                        };
                        let mut name = base.clone();
                        let mut k = 1usize;
                        while used.contains(&name) {
                            name = format!("{}_{}", base, k);
                            k += 1;
                        }
                        used.insert(name.clone());
                        call_to_name.insert(key.clone(), name);
                    }
                }
            }
        }

        // For any calls not named yet (e.g., only appear in HAVING), assign names
        for key in call_to_index.keys() {
            if !call_to_name.contains_key(key) {
                let base = key.func.clone();
                let mut name = base.clone();
                let mut k = 1usize;
                while used.contains(&name) {
                    name = format!("{}_{}", base, k);
                    k += 1;
                }
                used.insert(name.clone());
                call_to_name.insert(key.clone(), name);
            }
        }

        // 3) Build the Aggregate node
        plan = LogicalPlan::Aggregate {
            input: Box::new(plan),
            group_keys: aq.group_by.clone(),
            aggs: calls.clone(), // executor uses func/args/distinct
        };

        // 4) HAVING: rewrite to refer to aggregate outputs, then Filter
        if let Some(pred) = &aq.having {
            let having_rewritten = AggregateCall::rewrite_predicate_using_call_names(pred, &call_to_name);
            plan = LogicalPlan::Filter { input: Box::new(plan), predicate: having_rewritten };
        }

        // 5) Project: rewrite aggregate functions in SELECT to Column(Name)
        let mut final_proj = aq.projection.clone();
        for id in &mut final_proj {
            id.expression = AggregateCall::rewrite_scalar_using_call_names(&id.expression, &call_to_name);
        }
        plan = LogicalPlan::Project { input: Box::new(plan), exprs: final_proj };

        // 6) ORDER BY (unchanged; analyzer already qualified them)
        if !aq.order_by.is_empty() {
            plan = LogicalPlan::Sort { input: Box::new(plan), keys: aq.order_by.clone() };
        }

        // 7) LIMIT/OFFSET
        if aq.limit.is_some() || aq.offset.is_some() {
            plan = LogicalPlan::Limit {
                input: Box::new(plan),
                limit: aq.limit,
                offset: aq.offset,
            };
        }

        Ok(plan)
    }

    fn collect_aggregates_in_scalar(
        e: &ScalarExpr,
        table: &mut HashMap<AggregateCall, usize>,
        calls: &mut Vec<AggregateCall>,
    ) {
        if let ScalarExpr::Function(f) = e {
            if AggregateResolver::is_aggregate_name(&f.name) {
                let key: AggregateCall = f.into();
                if !table.contains_key(&key) {
                    table.insert(key.clone(), calls.len());
                    calls.push(key);
                }
            } else {
                for a in &f.args {
                    Self::collect_aggregates_in_scalar(a, table, calls);
                }
            }
        }
    }

    /// Collect aggregate calls appearing in a predicate (recursively).
    fn collect_aggregates_in_predicate(
        p: &Predicate,
        table: &mut HashMap<AggregateCall, usize>,
        calls: &mut Vec<AggregateCall>,
    ) {
        match p {
            Predicate::And(v) | Predicate::Or(v) => {
                for x in v { Self::collect_aggregates_in_predicate(x, table, calls); }
            }
            Predicate::Compare { left, right, .. } => {
                Self::collect_aggregates_in_scalar(left, table, calls);
                Self::collect_aggregates_in_scalar(right, table, calls);
            }
            Predicate::IsNull { expr, .. } => {
                Self::collect_aggregates_in_scalar(expr, table, calls);
            }
            Predicate::InList { expr, list, .. } => {
                Self::collect_aggregates_in_scalar(expr, table, calls);
                for e in list { Self::collect_aggregates_in_scalar(e, table, calls); }
            }
            Predicate::Like { expr, pattern, .. } => {
                Self::collect_aggregates_in_scalar(expr, table, calls);
                Self::collect_aggregates_in_scalar(pattern, table, calls);
            }
            Predicate::Const3(_) => {}
        }
    }
}

// src/planner/plan_builder_tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::analyzer::{AnalyzedQuery, AnalyzedIdentifier};
    use crate::parser::ast::{Column, ComparatorOp, Function, Literal, OrderBy, Predicate, ScalarExpr, Truth};
    use crate::JsonPrimitive;

    fn col_t(name: &str) -> Column {
        Column::WithCollection { collection: "t".into(), name: name.into() }
    }
    fn id_col_t(name: &str) -> AnalyzedIdentifier {
        AnalyzedIdentifier {
            expression: ScalarExpr::Column(col_t(name)),
            alias: None,
            ty: JsonPrimitive::Int,
            nullable: false,
        }
    }
    fn id_fun(name: &str, args: Vec<ScalarExpr>) -> AnalyzedIdentifier {
        AnalyzedIdentifier {
            expression: ScalarExpr::Function(Function{ name: name.into(), args, distinct: false }),
            alias: Some(name.into()),
            ty: if name.eq_ignore_ascii_case("avg") { JsonPrimitive::Float } else { JsonPrimitive::Int },
            nullable: true,
        }
    }

    #[test]
    fn plan_for_simple_select_where_order_limit() {
        let aq = AnalyzedQuery {
            projection: vec![id_col_t("id")],
            collections: vec![("t".into(), "t".into())],
            joins: vec![],
            criteria: Some(Predicate::Compare {
                left: ScalarExpr::Column(col_t("id")),
                op: crate::parser::ast::ComparatorOp::Gt,
                right: ScalarExpr::Literal(Literal::Int(10)),
            }),
            group_by: vec![],
            having: None,
            order_by: vec![OrderBy { expr: ScalarExpr::Column(col_t("id")), ascending: true }],
            limit: Some(5),
            offset: Some(10),
        };

        let plan = PlanBuilder::from_analyzed(&aq).expect("plan");
        // Assert shape
        match plan {
            LogicalPlan::Limit { input, limit, offset } => {
                assert_eq!(limit, Some(5));
                assert_eq!(offset, Some(10));
                match *input {
                    LogicalPlan::Sort { input, keys } => {
                        assert_eq!(keys.len(), 1);
                        match *input {
                            LogicalPlan::Project { input, exprs } => {
                                assert_eq!(exprs.len(), 1);
                                match *input {
                                    LogicalPlan::Filter { input, predicate: _ } => {
                                        match *input {
                                            LogicalPlan::Scan { backing, visible } => {
                                                assert_eq!(backing, "t");
                                                assert_eq!(visible, "t");
                                            }
                                            other => panic!("expected Scan, got {other:?}"),
                                        }
                                    }
                                    other => panic!("expected Filter, got {other:?}"),
                                }
                            }
                            other => panic!("expected Project, got {other:?}"),
                        }
                    }
                    other => panic!("expected Sort, got {other:?}"),
                }
            }
            other => panic!("expected Limit root, got {other:?}"),
        }
    }

    #[test]
    fn plan_for_group_by_aggregate_and_having() {
        let aq = AnalyzedQuery {
            projection: vec![
                // SELECT t.category, SUM(t.amount) AS sum
                AnalyzedIdentifier {
                    expression: ScalarExpr::Column(col_t("category")),
                    alias: None,
                    ty: JsonPrimitive::String,
                    nullable: false,
                },
                id_fun("sum", vec![ScalarExpr::Column(col_t("amount"))]),
            ],
            collections: vec![("t".into(), "t".into())],
            joins: vec![],
            criteria: None,
            group_by: vec![col_t("category")],
            having: Some(Predicate::Compare {
                left: ScalarExpr::Function(Function {
                    name: "sum".into(),
                    args: vec![ScalarExpr::Column(col_t("amount"))],
                    distinct: false,
                }),
                op: crate::parser::ast::ComparatorOp::Gt,
                right: ScalarExpr::Literal(Literal::Int(100)),
            }),
            order_by: vec![],
            limit: None,
            offset: None,
        };

        let plan = PlanBuilder::from_analyzed(&aq).expect("plan");
        match plan {
            LogicalPlan::Project { input, exprs } => {
                assert_eq!(exprs.len(), 2);
                match *input {
                    LogicalPlan::Filter { input, .. } => {
                        match *input {
                            LogicalPlan::Aggregate { input, group_keys, aggs } => {
                                assert_eq!(group_keys.len(), 1);
                                assert_eq!(aggs.len(), 1);
                                assert_eq!(aggs[0].func, "sum");
                                match *input {
                                    LogicalPlan::Scan { backing, .. } => {
                                        assert_eq!(backing, "t");
                                    }
                                    other => panic!("expected Scan below Aggregate, got {other:?}"),
                                }
                            }
                            other => panic!("expected Aggregate, got {other:?}"),
                        }
                    }
                    other => panic!("expected Filter (HAVING), got {other:?}"),
                }
            }
            other => panic!("expected Project root, got {other:?}"),
        }
    }

    #[test]
    fn planner_builds_cross_join_for_multiple_from_items() {
        // FROM a, b  (no explicit JOINs) → CROSS JOIN
        let aq = AnalyzedQuery {
            projection: vec![AnalyzedIdentifier {
                // any proj is fine; planner doesn't validate here
                expression: ScalarExpr::Column(Column::WithCollection { collection: "a".into(), name: "id".into() }),
                alias: None,
                ty: JsonPrimitive::Int,
                nullable: false,
            }],
            collections: vec![
                ("a".into(), "a".into()),
                ("b".into(), "b".into()),
            ],
            joins: vec![],
            criteria: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
            offset: None,
        };

        let plan = PlanBuilder::from_analyzed(&aq).expect("planner should support FROM a, b");

        // Expect: Project over Join(Scan a, Scan b) with on = Const3(True) and JoinType::Inner
        match plan {
            LogicalPlan::Project { input, exprs: _ } => {
                match *input {
                    LogicalPlan::Join { left, right, join_type, on } => {
                        // left scan = a
                        match *left {
                            LogicalPlan::Scan { backing, visible } => {
                                assert_eq!(backing, "a");
                                assert_eq!(visible, "a");
                            }
                            other => panic!("expected left Scan(a), got {other:?}"),
                        }
                        // right scan = b
                        match *right {
                            LogicalPlan::Scan { backing, visible } => {
                                assert_eq!(backing, "b");
                                assert_eq!(visible, "b");
                            }
                            other => panic!("expected right Scan(b), got {other:?}"),
                        }
                        assert!(matches!(join_type, JoinType::Inner), "CROSS JOIN should be Inner");
                        assert!(matches!(on, Predicate::Const3(Truth::True)), "CROSS JOIN ON must be TRUE");
                    }
                    other => panic!("expected Join under Project, got {other:?}"),
                }
            }
            other => panic!("expected Project at root, got {other:?}"),
        }
    }

    #[test]
    fn planner_no_longer_rejects_multiple_collections_without_joins() {
        // FROM a, b, c should be accepted and chained as CROSS JOINs
        let aq = AnalyzedQuery {
            projection: vec![AnalyzedIdentifier {
                expression: ScalarExpr::Column(Column::WithCollection { collection: "a".into(), name: "id".into() }),
                alias: None,
                ty: JsonPrimitive::Int,
                nullable: false,
            }],
            collections: vec![
                ("a".into(), "a".into()),
                ("b".into(), "b".into()),
                ("c".into(), "c".into()),
            ],
            joins: vec![],
            criteria: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
            offset: None,
        };

        let plan = PlanBuilder::from_analyzed(&aq).expect("planner should accept multiple FROM items without explicit joins");

        // Optional: quickly spot-check that the second collection participates in a Join
        // (full structural check would mirror the previous test but one level deeper)
        let mut saw_join = false;
        if let LogicalPlan::Project { input, .. } = plan {
            if let LogicalPlan::Join { .. } = *input { saw_join = true; }
        }
        assert!(saw_join, "expected a Join chain for FROM a, b, c");
    }

    fn fn_agg(name: &str, args: Vec<ScalarExpr>, distinct: bool) -> ScalarExpr {
        ScalarExpr::Function(Function {
            name: name.to_string(),
            args,
            distinct,
        })
    }
    fn col(qual: &str, name: &str) -> ScalarExpr {
        ScalarExpr::Column(Column::WithCollection {
            collection: qual.to_string(),
            name: name.to_string(),
        })
    }
    fn lit_i(i: i64) -> ScalarExpr {
        ScalarExpr::Literal(Literal::Int(i))
    }

    // ---------- collect_aggregates_in_scalar ----------

    #[test]
    fn scalar_collects_single_aggregate_and_dedupes_duplicates() {
        // SUM(t.amt), SUM(t.amt) again
        let s1 = fn_agg("SUM", vec![col("t", "amt")], false);
        let s2 = fn_agg("sum", vec![col("t", "amt")], false); // different case

        let mut calls = Vec::<AggregateCall>::new();
        let mut table = HashMap::<AggregateCall, usize>::new();

        PlanBuilder::collect_aggregates_in_scalar(&s1, &mut table, &mut calls);
        PlanBuilder::collect_aggregates_in_scalar(&s2, &mut table, &mut calls);

        assert_eq!(calls.len(), 1, "same aggregate (case-insensitive) must be deduped");
        let c = &calls[0];
        assert_eq!(c.func, "sum");
        assert_eq!(c.args.len(), 1);
        assert!(!c.distinct);
    }

    #[test]
    fn scalar_distinguishes_distinct_flag_in_keys() {
        // COUNT(DISTINCT t.id) vs COUNT(t.id)
        let c_dist = fn_agg("COUNT", vec![col("t", "id")], true);
        let c_all  = fn_agg("COUNT", vec![col("t", "id")], false);

        let mut calls = Vec::<AggregateCall>::new();
        let mut table = HashMap::<AggregateCall, usize>::new();

        PlanBuilder::collect_aggregates_in_scalar(&c_dist, &mut table, &mut calls);
        PlanBuilder::collect_aggregates_in_scalar(&c_all,  &mut table, &mut calls);

        assert_eq!(calls.len(), 2, "DISTINCT must create a separate aggregate call");
        assert!(calls.iter().any(|c| c.func == "count" && c.distinct));
        assert!(calls.iter().any(|c| c.func == "count" && !c.distinct));
    }

    #[test]
    fn scalar_does_not_collect_inside_aggregate_arguments() {
        // SUM( avg(t.amt) )  → only collect the outer SUM, not the inner AVG
        let inner = fn_agg("AVG", vec![col("t", "amt")], false);
        let outer = fn_agg("SUM", vec![inner], false);

        let mut calls = Vec::<AggregateCall>::new();
        let mut table = HashMap::<AggregateCall, usize>::new();

        PlanBuilder::collect_aggregates_in_scalar(&outer, &mut table, &mut calls);

        assert_eq!(calls.len(), 1, "should only collect the outer aggregate");
        assert_eq!(calls[0].func, "sum");
    }

    #[test]
    fn scalar_traverses_scalar_functions_but_not_marked_as_aggs() {
        // UPPER(t.name) → no aggregates
        let expr = ScalarExpr::Function(Function {
            name: "UPPER".into(),
            args: vec![col("t", "name")],
            distinct: false
        });

        let mut calls = Vec::<AggregateCall>::new();
        let mut table = HashMap::<AggregateCall, usize>::new();
        PlanBuilder::collect_aggregates_in_scalar(&expr, &mut table, &mut calls);

        assert!(calls.is_empty(), "no aggregates should be collected for scalar-only expressions");
    }

    // ---------- collect_aggregates_in_predicate ----------

    #[test]
    fn predicate_collects_from_compare_and_dedupes_across_branches() {
        // SUM(t.amt) > 10 OR SUM(t.amt) < 100
        let left  = fn_agg("Sum", vec![col("t", "amt")], false);
        let right = lit_i(10);
        let cmp1 = Predicate::Compare { left: left.clone(), op: ComparatorOp::Gt, right };

        let left2  = fn_agg("sum", vec![col("t", "amt")], false);
        let right2 = lit_i(100);
        let cmp2 = Predicate::Compare { left: left2, op: ComparatorOp::Lt, right: right2 };

        let pred = Predicate::Or(vec![cmp1, cmp2]);

        let mut calls = Vec::<AggregateCall>::new();
        let mut table = HashMap::<AggregateCall, usize>::new();
        PlanBuilder::collect_aggregates_in_predicate(&pred, &mut table, &mut calls);

        assert_eq!(calls.len(), 1, "same SUM(t.amt) across branches should be deduped");
        assert_eq!(calls[0].func, "sum");
        assert_eq!(calls[0].args.len(), 1);
    }

    #[test]
    fn predicate_collects_from_isnull_inlist_like_variants() {
        let isnull = Predicate::IsNull {
            expr: fn_agg("max", vec![col("t", "x")], false),
            negated: false
        };
        let inlist = Predicate::InList {
            expr: col("t", "y"),
            list: vec![
                fn_agg("MIN", vec![col("t", "z")], false),
                ScalarExpr::Literal(Literal::Int(1)),
            ],
            negated: false
        };
        let like = Predicate::Like {
            expr: fn_agg("COUNT", vec![col("t", "k")], true),
            pattern: ScalarExpr::Literal(Literal::String("%A%".into())),
            negated: false
        };
        let pred = Predicate::And(vec![isnull, inlist, like]);

        let mut calls = Vec::<AggregateCall>::new();
        let mut table = HashMap::<AggregateCall, usize>::new();
        PlanBuilder::collect_aggregates_in_predicate(&pred, &mut table, &mut calls);

        // Expect MAX(t.x), MIN(t.z), COUNT(DISTINCT t.k)
        assert_eq!(calls.len(), 3);
        assert!(calls.iter().any(|c| c.func == "max"   && !c.distinct));
        assert!(calls.iter().any(|c| c.func == "min"   && !c.distinct));
        assert!(calls.iter().any(|c| c.func == "count" &&  c.distinct));
    }

    #[test]
    fn predicate_does_not_collect_inside_aggregate_arguments() {
        // Compare: SUM( avg(t.amt) ) > 0  → only SUM should be collected
        let inner = fn_agg("AVG", vec![col("t", "amt")], false);
        let outer = fn_agg("SUM", vec![inner], false);
        let pred = Predicate::Compare {
            left: outer,
            op: ComparatorOp::Gt,
            right: lit_i(0),
        };

        let mut calls = Vec::<AggregateCall>::new();
        let mut table = HashMap::<AggregateCall, usize>::new();
        PlanBuilder::collect_aggregates_in_predicate(&pred, &mut table, &mut calls);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].func, "sum");
    }

    #[test]
    fn predicate_ignores_const3() {
        let pred = Predicate::Const3(Truth::True);

        let mut calls = Vec::<AggregateCall>::new();
        let mut table = HashMap::<AggregateCall, usize>::new();
        PlanBuilder::collect_aggregates_in_predicate(&pred, &mut table, &mut calls);

        assert!(calls.is_empty());
    }
}

#[cfg(test)]
mod join_shape_tests {
    use super::*;
    use crate::parser::ast::{Collection as AstCollection, Column, ComparatorOp, JoinType, Literal, OrderBy, Predicate, ScalarExpr};
    use crate::parser::analyzer::{AnalyzedIdentifier};
    use crate::JsonPrimitive;

    fn col(a: &str, n: &str) -> Column {
        Column::WithCollection { collection: a.into(), name: n.into() }
    }

    fn id_col(a: &str, n: &str, ty: JsonPrimitive) -> AnalyzedIdentifier {
        AnalyzedIdentifier {
            expression: ScalarExpr::Column(col(a, n)),
            alias: None,
            ty,
            nullable: false,
        }
    }

    fn simple_on_eq(lc: &str, ln: &str, rc: &str, rn: &str) -> Predicate {
        Predicate::Compare {
            left:  ScalarExpr::Column(col(lc, ln)),
            op:    ComparatorOp::Eq,
            right: ScalarExpr::Column(col(rc, rn)),
        }
    }

    #[test]
    fn plan_for_inner_join_then_where() {
        let aq = AnalyzedQuery {
            projection: vec![
                id_col("a", "id", JsonPrimitive::Int),
                id_col("b", "name", JsonPrimitive::String),
            ],
            collections: vec![("a".into(), "a".into())],
            criteria: Some(Predicate::Compare {
                left: ScalarExpr::Column(col("b","age")),
                op: ComparatorOp::Gt,
                right: ScalarExpr::Literal(Literal::Int(18)),
            }),
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
            offset: None,
            joins: vec![crate::parser::ast::Join {
                join_type: JoinType::Inner,
                collection: AstCollection::Table { name: "b".into(), alias: None },
                predicate: simple_on_eq("a","id","b","a_id"),
            }],
        };

        let plan = PlanBuilder::from_analyzed(&aq).expect("plan");
        // Expect: Filter( WHERE ) over Join( Scan(a), Scan(b) )
        match plan {
            LogicalPlan::Project { input, .. } => match *input {
                LogicalPlan::Filter { input, .. } => match *input {
                    LogicalPlan::Join { left, right, join_type, on } => {
                        assert!(matches!(join_type, JoinType::Inner));
                        // left = Scan a
                        match *left {
                            LogicalPlan::Scan { backing, visible } => { assert_eq!(backing, "a"); assert_eq!(visible, "a"); }
                            other => panic!("expected left Scan(a), got {other:?}"),
                        }
                        // right = Scan b
                        match *right {
                            LogicalPlan::Scan { backing, visible } => { assert_eq!(backing, "b"); assert_eq!(visible, "b"); }
                            other => panic!("expected right Scan(b), got {other:?}"),
                        }
                        // ON predicate kept
                        match on {
                            Predicate::Compare { .. } => {}
                            other => panic!("expected compare ON, got {other:?}"),
                        }
                    }
                    other => panic!("expected Join under Filter, got {other:?}"),
                },
                other => panic!("expected Filter under Project, got {other:?}"),
            },
            other => panic!("expected Project root, got {other:?}"),
        }
    }

    #[test]
    fn plan_for_left_join_chain_and_order_limit() {
        let aq = AnalyzedQuery {
            projection: vec![id_col("a", "id", JsonPrimitive::Int)],
            collections: vec![("a".into(), "a".into())],
            criteria: None,
            group_by: vec![],
            having: None,
            order_by: vec![OrderBy { expr: ScalarExpr::Column(col("a","id")), ascending: true }],
            limit: Some(10),
            offset: None,
            joins: vec![
                crate::parser::ast::Join {
                    join_type: JoinType::Left,
                    collection: AstCollection::Table { name: "b".into(), alias: None },
                    predicate: simple_on_eq("a","id","b","a_id"),
                },
                crate::parser::ast::Join {
                    join_type: JoinType::Right,
                    collection: AstCollection::Table { name: "c".into(), alias: Some("c1".into()) },
                    predicate: simple_on_eq("b","id","c1","b_id"),
                },
            ],
        };

        let plan = PlanBuilder::from_analyzed(&aq).expect("plan");
        // Expect Limit(Sort(Project(Join(Join(Scan a, Scan b), Scan c1))))
        match plan {
            LogicalPlan::Limit { input, limit, .. } => { assert_eq!(limit, Some(10));
                match *input {
                    LogicalPlan::Sort { input, .. } => match *input {
                        LogicalPlan::Project { input, .. } => match *input {
                            LogicalPlan::Join { left, right, join_type, .. } => {
                                assert!(matches!(join_type, JoinType::Right));
                                match *left {
                                    LogicalPlan::Join { left: l2, right: r2, join_type: jt2, .. } => {
                                        assert!(matches!(jt2, JoinType::Left));
                                        match *l2 { LogicalPlan::Scan { backing, .. } => assert_eq!(backing, "a"), _ => panic!() }
                                        match *r2 { LogicalPlan::Scan { backing, .. } => assert_eq!(backing, "b"), _ => panic!() }
                                    }
                                    _ => panic!("expected inner join as left child"),
                                }
                                match *right { LogicalPlan::Scan { backing, visible } => { assert_eq!(backing, "c"); assert_eq!(visible, "c1"); } _ => panic!() }
                            }
                            other => panic!("expected Join at that level, got {other:?}"),
                        }
                        other => panic!("expected Project, got {other:?}"),
                    }
                    other => panic!("expected Sort, got {other:?}"),
                }
            }
            other => panic!("expected Limit root, got {other:?}"),
        }
    }
}
