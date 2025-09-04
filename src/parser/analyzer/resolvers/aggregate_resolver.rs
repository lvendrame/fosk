use std::collections::HashSet;

use crate::parser::{analyzer::ColumnKey, ast::{Predicate, ScalarExpr}};

pub struct AggregateResolver;

impl AggregateResolver {
    pub fn is_aggregate_name(name: &str) -> bool {
        matches!(name.to_ascii_lowercase().as_str(), "count" | "sum" | "avg" | "min" | "max")
    }

    pub fn contains_aggregate(expr: &ScalarExpr) -> bool {
        match expr {
            ScalarExpr::Function(f) if Self::is_aggregate_name(&f.name) => true,
            ScalarExpr::Function(f) => f.args.iter().any(Self::contains_aggregate),
            _ => false,
        }
    }

    pub fn uses_only_group_by(
        expr: &ScalarExpr,
        group: &HashSet<ColumnKey>,
        inside_agg_arg: bool,
    ) -> bool {
        match expr {
            ScalarExpr::Literal(_) => true,
            ScalarExpr::Column(c) => {
                if inside_agg_arg { true } else {
                    // must be qualified before calling this check
                    group.contains(&ColumnKey::of(c))
                }
            },
            ScalarExpr::Function(f) => {
                if Self::is_aggregate_name(&f.name) {
                    // args are inside aggregate
                    f.args.iter().all(|a| Self::uses_only_group_by(a, group, true))
                } else {
                    // scalar function: preserve flag
                    f.args.iter().all(|a| Self::uses_only_group_by(a, group, inside_agg_arg))
                }
            },
            ScalarExpr::WildCard | ScalarExpr::WildCardWithCollection(_) | ScalarExpr::Parameter | ScalarExpr::Args(_) => {
                inside_agg_arg
            },
        }
    }

    pub fn predicate_contains_aggregate(predicate: &Predicate) -> bool {
        match predicate {
            Predicate::And(predicates) | Predicate::Or(predicates) => predicates.iter().any(Self::predicate_contains_aggregate),
            Predicate::Compare { left, right, .. } => Self::contains_aggregate(left) || Self::contains_aggregate(right),
            Predicate::IsNull { expr, .. } => Self::contains_aggregate(expr),
            Predicate::InList { expr, list, .. } => Self::contains_aggregate(expr) || list.iter().any(Self::contains_aggregate),
            Predicate::Like { expr, pattern, .. } => Self::contains_aggregate(expr) || Self::contains_aggregate(pattern),
            Predicate::Const3(_) => false,
        }
    }

    pub fn predicate_uses_only_group_by_or_agg(predicate: &Predicate, group: &HashSet<ColumnKey>) -> bool {
        match predicate {
            Predicate::And(v) | Predicate::Or(v) => v.iter()
                .all(|x| Self::predicate_uses_only_group_by_or_agg(x, group)),
            Predicate::Compare { left, right, .. } =>
                Self::uses_only_group_by(left, group, false) && Self::uses_only_group_by(right, group, false),
            Predicate::IsNull { expr, .. } => Self::uses_only_group_by(expr, group, false),
            Predicate::InList { expr, list, .. } =>
                Self::uses_only_group_by(expr, group, false) && list.iter().all(|e| Self::uses_only_group_by(e, group, false)),
            Predicate::Like { expr, pattern, .. } =>
                Self::uses_only_group_by(expr, group, false) && Self::uses_only_group_by(pattern, group, false),
            Predicate::Const3(_) => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::ast::{Column, ComparatorOp, Function, Literal, Truth};

    use super::*;
    use std::collections::HashSet;

    // --- quick constructors ---------------------------------------------------
    fn qc(coll: &str, name: &str) -> Column {
        Column::WithCollection { collection: coll.to_string(), name: name.to_string() }
    }
    fn lit_i(i: i64) -> ScalarExpr { ScalarExpr::Literal(Literal::Int(i)) }
    fn lit_s(s: &str) -> ScalarExpr { ScalarExpr::Literal(Literal::String(s.to_string())) }
    fn fun(name: &str, args: Vec<ScalarExpr>) -> ScalarExpr {
        ScalarExpr::Function(Function { name: name.to_string(), args, distinct: false })
    }
    fn agg(name: &str, args: Vec<ScalarExpr>) -> ScalarExpr {
        ScalarExpr::Function(Function { name: name.to_string(), args, distinct: false })
    }

    // --- is_aggregate_name ----------------------------------------------------
    #[test]
    fn is_aggregate_name_basic_and_case_insensitive() {
        assert!(AggregateResolver::is_aggregate_name("count"));
        assert!(AggregateResolver::is_aggregate_name("SUM"));
        assert!(AggregateResolver::is_aggregate_name("Avg"));
        assert!(AggregateResolver::is_aggregate_name("min"));
        assert!(AggregateResolver::is_aggregate_name("MAX"));
        assert!(!AggregateResolver::is_aggregate_name("upper"));
        assert!(!AggregateResolver::is_aggregate_name("coalesce"));
    }

    // --- contains_aggregate ---------------------------------------------------
    #[test]
    fn contains_aggregate_detects_nested() {
        // sum(t.a)
        let e1 = agg("sum", vec![ScalarExpr::Column(qc("t", "a"))]);
        assert!(AggregateResolver::contains_aggregate(&e1));

        // lower(sum(t.a))
        let e2 = fun("lower", vec![e1.clone()]);
        assert!(AggregateResolver::contains_aggregate(&e2));

        // sum(lower(t.a))
        let e3 = agg("sum", vec![fun("lower", vec![ScalarExpr::Column(qc("t", "a"))])]);
        assert!(AggregateResolver::contains_aggregate(&e3));

        // lower(t.a) (no aggregate)
        let e4 = fun("lower", vec![ScalarExpr::Column(qc("t", "a"))]);
        assert!(!AggregateResolver::contains_aggregate(&e4));
    }

    // --- uses_only_group_by ---------------------------------------------------
    #[test]
    fn uses_only_group_by_enforces_group_cols_but_allows_agg_args() {
        // GROUP BY t.a
        let mut group = HashSet::new();
        group.insert(ColumnKey { column: "t".into(), name: "a".into() });

        // plain column in group -> ok
        let e_ok = ScalarExpr::Column(qc("t", "a"));
        assert!(AggregateResolver::uses_only_group_by(&e_ok, &group, false));

        // plain column not in group -> not ok
        let e_bad = ScalarExpr::Column(qc("t", "b"));
        assert!(!AggregateResolver::uses_only_group_by(&e_bad, &group, false));

        // aggregate over non-group column -> ok (args are inside aggregate)
        let e_agg = agg("sum", vec![ScalarExpr::Column(qc("t", "b"))]);
        assert!(AggregateResolver::uses_only_group_by(&e_agg, &group, false));

        // scalar over group column -> ok
        let e_scalar_ok = fun("upper", vec![ScalarExpr::Column(qc("t", "a"))]);
        assert!(AggregateResolver::uses_only_group_by(&e_scalar_ok, &group, false));

        // scalar over non-group column (outside aggregate) -> not ok
        let e_scalar_bad = fun("upper", vec![ScalarExpr::Column(qc("t", "b"))]);
        assert!(!AggregateResolver::uses_only_group_by(&e_scalar_bad, &group, false));

        // scalar wrapping an aggregate -> ok
        let e_scalar_agg = fun("upper", vec![agg("sum", vec![ScalarExpr::Column(qc("t", "b"))])]);
        assert!(AggregateResolver::uses_only_group_by(&e_scalar_agg, &group, false));

        // wildcard must be rejected here
        assert!(!AggregateResolver::uses_only_group_by(&ScalarExpr::WildCard, &group, false));
    }

    // --- predicate_contains_aggregate ----------------------------------------
    #[test]
    fn predicate_contains_aggregate_various_nodes() {
        // SUM(t.a) > 1
        let p1 = Predicate::Compare {
            left: agg("sum", vec![ScalarExpr::Column(qc("t", "a"))]),
            op: ComparatorOp::Gt,
            right: lit_i(1),
        };
        assert!(AggregateResolver::predicate_contains_aggregate(&p1));

        // IS NULL(COUNT(*))
        let p2 = Predicate::IsNull {
            expr: ScalarExpr::Function(Function { name: "count".into(), args: vec![ScalarExpr::WildCard], distinct: false }),
            negated: false,
        };
        assert!(AggregateResolver::predicate_contains_aggregate(&p2));

        // IN (AVG(t.b)) in the list
        let p3 = Predicate::InList {
            expr: ScalarExpr::Column(qc("t", "a")),
            list: vec![agg("avg", vec![ScalarExpr::Column(qc("t", "b"))])],
            negated: false,
        };
        assert!(AggregateResolver::predicate_contains_aggregate(&p3));

        // LIKE(lower(t.a), 'x%') -> no aggregate
        let p4 = Predicate::Like {
            expr: fun("lower", vec![ScalarExpr::Column(qc("t", "a"))]),
            pattern: lit_s("x%"),
            negated: false,
        };
        assert!(!AggregateResolver::predicate_contains_aggregate(&p4));

        // Const3 only -> no aggregate
        let p5 = Predicate::Const3(Truth::True);
        assert!(!AggregateResolver::predicate_contains_aggregate(&p5));
    }

    // --- predicate_uses_only_group_by_or_agg ---------------------------------
    #[test]
    fn predicate_group_by_validation() {
        // GROUP BY t.a
        let mut group = HashSet::new();
        group.insert(ColumnKey { column: "t".into(), name: "a".into() });

        // t.a = 1 -> ok
        let ok1 = Predicate::Compare {
            left: ScalarExpr::Column(qc("t", "a")),
            op: ComparatorOp::Eq,
            right: lit_i(1),
        };
        assert!(AggregateResolver::predicate_uses_only_group_by_or_agg(&ok1, &group));

        // t.b = 1 -> not ok
        let bad1 = Predicate::Compare {
            left: ScalarExpr::Column(qc("t", "b")),
            op: ComparatorOp::Eq,
            right: lit_i(1),
        };
        assert!(!AggregateResolver::predicate_uses_only_group_by_or_agg(&bad1, &group));

        // SUM(t.b) > 1 -> ok (aggregate)
        let ok2 = Predicate::Compare {
            left: agg("sum", vec![ScalarExpr::Column(qc("t", "b"))]),
            op: ComparatorOp::Gt,
            right: lit_i(1),
        };
        assert!(AggregateResolver::predicate_uses_only_group_by_or_agg(&ok2, &group));

        // UPPER(SUM(t.b)) > 1 -> ok (scalar over aggregate)
        let ok3 = Predicate::Compare {
            left: fun("upper", vec![agg("sum", vec![ScalarExpr::Column(qc("t", "b"))])]),
            op: ComparatorOp::Gt,
            right: lit_i(1),
        };
        assert!(AggregateResolver::predicate_uses_only_group_by_or_agg(&ok3, &group));

        // LIKE(t.b, 'x%') -> not ok (t.b not in group, outside aggregate)
        let bad2 = Predicate::Like {
            expr: ScalarExpr::Column(qc("t", "b")),
            pattern: lit_s("x%"),
            negated: false,
        };
        assert!(!AggregateResolver::predicate_uses_only_group_by_or_agg(&bad2, &group));

        // IN: expr uses group col; list mixes literal and aggregate -> ok
        let ok4 = Predicate::InList {
            expr: ScalarExpr::Column(qc("t", "a")),
            list: vec![lit_i(1), agg("avg", vec![ScalarExpr::Column(qc("t", "b"))])],
            negated: false,
        };
        assert!(AggregateResolver::predicate_uses_only_group_by_or_agg(&ok4, &group));

        // AND/OR combine correctly
        let combo = Predicate::And(vec![
            ok1.clone(),
            ok2.clone(),
            Predicate::Or(vec![bad1.clone(), ok3.clone()]), // one bad in OR → overall true only if you require "all" → our function requires all subpredicates valid, so this should be false
        ]);
        assert!(!AggregateResolver::predicate_uses_only_group_by_or_agg(&combo, &group));
    }

    #[test]
    fn uses_only_group_by_allows_wildcard_inside_aggregate_args() {
        use std::collections::HashSet;
        let group = HashSet::new();

        // COUNT(*) is an aggregate whose arg is a wildcard
        let expr = ScalarExpr::Function(Function {
            name: "count".into(),
            args: vec![ScalarExpr::WildCard],
            distinct: false
        });

        // Top-level check: ok
        assert!(AggregateResolver::uses_only_group_by(&expr, &group, false));
    }

    #[test]
    fn predicate_uses_only_group_by_or_agg_rejects_or_with_bad_branch() {
        use std::collections::HashSet;
        // GROUP BY t.a
        let mut group = HashSet::new();
        group.insert(ColumnKey { column:"t".into(), name:"a".into() });

        // (t.b = 1) OR (SUM(t.b) > 0)  → our validator requires *all* subpredicates valid,
        // so the overall OR should be rejected because left branch is invalid.
        let bad_left = Predicate::Compare {
            left: ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"b".into() }),
            op: ComparatorOp::Eq,
            right: ScalarExpr::Literal(Literal::Int(1)),
        };
        let good_right = Predicate::Compare {
            left: ScalarExpr::Function(Function {
                name: "sum".into(),
                args: vec![ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"b".into() })],
                distinct: false
            }),
            op: ComparatorOp::Gt,
            right: ScalarExpr::Literal(Literal::Int(0)),
        };
        let p = Predicate::Or(vec![bad_left, good_right]);
        assert!(!AggregateResolver::predicate_uses_only_group_by_or_agg(&p, &group));
    }

    #[test]
    fn uses_only_group_by_rejects_scalar_over_non_grouped_columns_even_when_nested() {
        use std::collections::HashSet;
        let mut group = HashSet::new();
        group.insert(ColumnKey { column:"t".into(), name:"a".into() });

        // lower(upper(t.b))  → still outside aggregate, b not in group → false
        let expr = ScalarExpr::Function(Function {
            name: "lower".into(),
            distinct: false,
            args: vec![ScalarExpr::Function(Function {
                name: "upper".into(),
                distinct: false,
                args: vec![ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"b".into() })],
            })],
        });
        assert!(!AggregateResolver::uses_only_group_by(&expr, &group, false));
    }

    #[test]
    fn predicate_contains_aggregate_returns_true_when_aggregate_is_deep_inside() {
        // AND( a = 1, LIKE( lower( max(t.b) ), 'x%') )
        let p = Predicate::And(vec![
            Predicate::Compare {
                left: ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"a".into() }),
                op: ComparatorOp::Eq,
                right: ScalarExpr::Literal(Literal::Int(1)),
            },
            Predicate::Like {
                expr: ScalarExpr::Function(Function {
                    name: "lower".into(),
                    distinct: false,
                    args: vec![ScalarExpr::Function(Function {
                        name: "max".into(),
                        distinct: false,
                        args: vec![ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"b".into() })],
                    })],
                }),
                pattern: ScalarExpr::Literal(Literal::String("x%".into())),
                negated: false,
            }
        ]);
        assert!(AggregateResolver::predicate_contains_aggregate(&p));
    }
}
