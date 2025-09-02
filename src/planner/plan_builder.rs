use crate::{parser::{analyzer::{AggregateResolver, AnalyzedQuery, AnalyzerError}, ast::{Function, ScalarExpr}}, planner::{aggregate_call::AggregateCall, logical_plan::LogicalPlan}};

pub struct PlanBuilder;

impl PlanBuilder {
    pub fn from_analyzed(aq: &AnalyzedQuery) -> Result<LogicalPlan, AnalyzerError> {
        // ---- 0) Source: single collection only (joins later) ----
        if aq.collections.len() != 1 {
            return Err(AnalyzerError::Other(
                "Planner (baby step): multiple collections require joins; thread joins into AnalyzedQuery first"
                    .into(),
            ));
        }
        let (visible, backing) = aq.collections[0].clone();
        let mut plan = LogicalPlan::Scan { backing, visible };

        // ---- 1) WHERE (criteria) ----
        if let Some(pred) = &aq.criteria {
            plan = LogicalPlan::Filter { input: Box::new(plan), predicate: pred.clone() };
        }

        // ---- 2) Aggregate if needed ----
        let needs_agg = !aq.group_by.is_empty()
            || aq.projection.iter().any(|id| AggregateResolver::contains_aggregate(&id.expression))
            || aq.having.as_ref().map(AggregateResolver::predicate_contains_aggregate).unwrap_or(false);

        if needs_agg {
            // collect aggregate calls from projection and having (dedupe by textual key)
            let mut calls: Vec<AggregateCall> = Vec::new();
            let mut seen = std::collections::HashSet::<String>::new();

            let mut collect_from_expr = |e: &ScalarExpr| {
                Self::collect_aggregates(e, &mut calls, &mut seen);
            };
            for id in &aq.projection { collect_from_expr(&id.expression); }
            if let Some(h) = &aq.having { Self::collect_aggregates_from_pred(h, &mut calls, &mut seen); }

            plan = LogicalPlan::Aggregate {
                input: Box::new(plan),
                group_keys: aq.group_by.clone(),
                aggs: calls,
            };

            // ---- 3) HAVING (after aggregate) ----
            if let Some(pred) = &aq.having {
                plan = LogicalPlan::Filter { input: Box::new(plan), predicate: pred.clone() };
            }
        }

        // ---- 4) Project (always after WHERE/Aggregate/HAVING) ----
        plan = LogicalPlan::Project { input: Box::new(plan), exprs: aq.projection.clone() };

        // ---- 5) ORDER BY (stable, NULLS LAST in executor) ----
        if !aq.order_by.is_empty() {
            plan = LogicalPlan::Sort { input: Box::new(plan), keys: aq.order_by.clone() };
        }

        // ---- 6) LIMIT/OFFSET ----
        if aq.limit.is_some() || aq.offset.is_some() {
            plan = LogicalPlan::Limit {
                input: Box::new(plan),
                limit: aq.limit,
                offset: aq.offset,
            };
        }

        Ok(plan)
    }

    fn collect_aggregates(expr: &ScalarExpr, out: &mut Vec<AggregateCall>, seen: &mut std::collections::HashSet<String>) {
        if let ScalarExpr::Function(Function { name, args, distinct }) = expr {
            if AggregateResolver::is_aggregate_name(name) {
                // args were qualified+folded already upstream
                let key = format!("{}({}{})",
                    name.to_ascii_lowercase(),
                    if *distinct { "distinct " } else { "" },
                    args.len());
                if seen.insert(key) {
                    out.push(AggregateCall {
                        func: name.to_ascii_lowercase(),
                        distinct: *distinct,
                        args: args.clone(),
                    });
                }
            } else {
                for a in args {
                    Self::collect_aggregates(a, out, seen);
                }
            }
        }
    }

    fn collect_aggregates_from_pred(pred: &crate::parser::ast::Predicate,
                              out: &mut Vec<AggregateCall>,
                              seen: &mut std::collections::HashSet<String>) {
        use crate::parser::ast::Predicate::*;
        match pred {
            And(v) | Or(v) => for p in v { Self::collect_aggregates_from_pred(p, out, seen); }
            Compare { left, right, .. } => { Self::collect_aggregates(left, out, seen); Self::collect_aggregates(right, out, seen); }
            IsNull { expr, .. } => Self::collect_aggregates(expr, out, seen),
            InList { expr, list, .. } => {
                Self::collect_aggregates(expr, out, seen);
                for e in list { Self::collect_aggregates(e, out, seen); }
            }
            Like { expr, pattern, .. } => { Self::collect_aggregates(expr, out, seen); Self::collect_aggregates(pattern, out, seen); }
            Const3(_) => {}
        }
    }
}

// src/planner/plan_builder_tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::analyzer::{AnalyzedQuery, AnalyzedIdentifier};
    use crate::parser::ast::{ScalarExpr, Literal, Column, OrderBy, Predicate, Function};
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
    fn planner_rejects_multiple_collections_until_joins_supported() {
        let aq = AnalyzedQuery {
            projection: vec![id_col_t("id")],
            collections: vec![
                ("a".into(), "a".into()),
                ("b".into(), "b".into()),
            ],
            criteria: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
            offset: None,
        };
        let err = PlanBuilder::from_analyzed(&aq).unwrap_err();
        let msg = format!("{err:?}").to_lowercase();
        assert!(msg.contains("joins"));
    }
}
