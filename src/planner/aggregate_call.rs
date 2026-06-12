use std::collections::HashMap;

use crate::parser::{
    analyzer::AggregateResolver,
    ast::{Column, Function, Predicate, ScalarExpr},
};

/// A normalized aggregate call extracted from expressions.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct AggregateCall {
    pub func: String,          // normalized lowercase
    pub args: Vec<ScalarExpr>, // analyzed, qualified & folded
    pub distinct: bool,
}

impl From<&Function> for AggregateCall {
    fn from(f: &Function) -> Self {
        Self {
            func: f.name.to_ascii_lowercase(),
            args: f.args.clone(),
            distinct: f.distinct,
        }
    }
}

impl AggregateCall {
    pub fn rewrite_scalar_using_call_names(
        expr: &ScalarExpr,
        map: &HashMap<AggregateCall, String>,
    ) -> ScalarExpr {
        match expr {
            ScalarExpr::Function(f) if AggregateResolver::is_aggregate_name(&f.name) => {
                let key: AggregateCall = f.into();
                let name = map.get(&key).expect("aggregate call must be named");
                ScalarExpr::Column(Column::Name { name: name.clone() })
            }
            ScalarExpr::Function(f) => {
                let new_args = f
                    .args
                    .iter()
                    .map(|a| Self::rewrite_scalar_using_call_names(a, map))
                    .collect();
                ScalarExpr::Function(Function {
                    name: f.name.clone(),
                    args: new_args,
                    distinct: f.distinct,
                })
            }
            _ => expr.clone(),
        }
    }

    pub fn rewrite_predicate_using_call_names(
        predicate: &Predicate,
        map: &HashMap<AggregateCall, String>,
    ) -> Predicate {
        match predicate {
            Predicate::And(v) => Predicate::And(
                v.iter()
                    .map(|x| Self::rewrite_predicate_using_call_names(x, map))
                    .collect(),
            ),
            Predicate::Or(v) => Predicate::Or(
                v.iter()
                    .map(|x| Self::rewrite_predicate_using_call_names(x, map))
                    .collect(),
            ),
            Predicate::Compare { left, op, right } => Predicate::Compare {
                left: Self::rewrite_scalar_using_call_names(left, map),
                op: *op,
                right: Self::rewrite_scalar_using_call_names(right, map),
            },
            Predicate::IsNull { expr, negated } => Predicate::IsNull {
                expr: Self::rewrite_scalar_using_call_names(expr, map),
                negated: *negated,
            },
            Predicate::InList {
                expr,
                list,
                negated,
            } => Predicate::InList {
                expr: Self::rewrite_scalar_using_call_names(expr, map),
                list: list
                    .iter()
                    .map(|e| Self::rewrite_scalar_using_call_names(e, map))
                    .collect(),
                negated: *negated,
            },
            Predicate::Like {
                expr,
                pattern,
                negated,
            } => Predicate::Like {
                expr: Self::rewrite_scalar_using_call_names(expr, map),
                pattern: Self::rewrite_scalar_using_call_names(pattern, map),
                negated: *negated,
            },
            Predicate::Const3(t) => Predicate::Const3(*t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{
        Column, ComparatorOp, Function, Literal, Predicate, ScalarExpr, Truth,
    };
    use std::collections::HashMap;

    // ---- helpers ----
    fn col(qual: &str, name: &str) -> ScalarExpr {
        ScalarExpr::Column(Column::WithCollection {
            collection: qual.to_string(),
            name: name.to_string(),
        })
    }
    fn lit_i(i: i64) -> ScalarExpr {
        ScalarExpr::Literal(Literal::Int(i))
    }
    fn lit_s(s: &str) -> ScalarExpr {
        ScalarExpr::Literal(Literal::String(s.into()))
    }
    fn fn_agg(name: &str, args: Vec<ScalarExpr>, distinct: bool) -> ScalarExpr {
        ScalarExpr::Function(Function {
            name: name.to_string(),
            args,
            distinct,
        })
    }
    fn fn_scalar(name: &str, args: Vec<ScalarExpr>) -> ScalarExpr {
        ScalarExpr::Function(Function {
            name: name.to_string(),
            args,
            distinct: false,
        })
    }

    // Build a mapping entry for a given aggregate function call to a chosen output name.
    fn map_entry(
        name: &str,
        args: Vec<ScalarExpr>,
        distinct: bool,
        as_name: &str,
    ) -> (AggregateCall, String) {
        let f = Function {
            name: name.to_string(),
            args,
            distinct,
        };
        let key: AggregateCall = (&f).into();
        (key, as_name.to_string())
    }

    // ---------- scalar rewrite ----------

    #[test]
    fn rewrite_scalar_replaces_aggregate_with_column_name() {
        // SUM(t.amt) -> Column(Name { "total" })
        let expr = fn_agg("SUM", vec![col("t", "amt")], false);

        let (key, out_name) = map_entry("SUM", vec![col("t", "amt")], false, "total");
        let mut map = HashMap::<AggregateCall, String>::new();
        map.insert(key, out_name.clone());

        let rewritten = AggregateCall::rewrite_scalar_using_call_names(&expr, &map);
        assert_eq!(
            rewritten,
            ScalarExpr::Column(Column::Name {
                name: out_name.clone()
            })
        );
    }

    #[test]
    fn rewrite_scalar_nested_scalar_function_wraps_rewritten_agg() {
        // UPPER(SUM(t.amt)) -> UPPER(Column(Name { "total" }))
        let expr = fn_scalar("UPPER", vec![fn_agg("sum", vec![col("t", "amt")], false)]);

        let (key, out_name) = map_entry("sum", vec![col("t", "amt")], false, "total");
        let mut map = HashMap::new();
        map.insert(key, out_name.clone());

        let rewritten = AggregateCall::rewrite_scalar_using_call_names(&expr, &map);
        assert_eq!(
            rewritten,
            fn_scalar(
                "UPPER",
                vec![ScalarExpr::Column(Column::Name {
                    name: "total".to_string()
                })]
            )
        );
    }

    #[test]
    fn rewrite_scalar_leaves_non_aggregate_expressions_untouched() {
        // LENGTH(t.name) has no aggregates → unchanged tree shape
        let expr = fn_scalar("LENGTH", vec![col("t", "name")]);
        let map = HashMap::<AggregateCall, String>::new();

        let rewritten = AggregateCall::rewrite_scalar_using_call_names(&expr, &map);
        assert_eq!(rewritten, expr);
    }

    // ---------- predicate rewrite ----------

    #[test]
    fn rewrite_predicate_handles_all_variants() {
        // AND(
        //   Compare( SUM(t.amt) > 10 ),
        //   InList( t.k, [ MIN(t.z), 1 ] ),
        //   IsNull( MAX(t.x) ),
        //   Like( COUNT(DISTINCT t.y), "%A%" )
        // )
        let p = Predicate::And(vec![
            Predicate::Compare {
                left: fn_agg("SUM", vec![col("t", "amt")], false),
                op: ComparatorOp::Gt,
                right: lit_i(10),
            },
            Predicate::InList {
                expr: col("t", "k"),
                list: vec![fn_agg("MIN", vec![col("t", "z")], false), lit_i(1)],
                negated: false,
            },
            Predicate::IsNull {
                expr: fn_agg("MAX", vec![col("t", "x")], false),
                negated: false,
            },
            Predicate::Like {
                expr: fn_agg("COUNT", vec![col("t", "y")], true),
                pattern: lit_s("%A%"),
                negated: false,
            },
        ]);

        // mapping for all 4 aggs
        let mut map = HashMap::<AggregateCall, String>::new();
        map.insert(
            map_entry("SUM", vec![col("t", "amt")], false, "sum_amt").0,
            "sum_amt".into(),
        );
        map.insert(
            map_entry("MIN", vec![col("t", "z")], false, "min_z").0,
            "min_z".into(),
        );
        map.insert(
            map_entry("MAX", vec![col("t", "x")], false, "max_x").0,
            "max_x".into(),
        );
        map.insert(
            map_entry("COUNT", vec![col("t", "y")], true, "cnt_y_dist").0,
            "cnt_y_dist".into(),
        );

        let out = AggregateCall::rewrite_predicate_using_call_names(&p, &map);

        assert_eq!(
            out,
            Predicate::And(vec![
                Predicate::Compare {
                    left: ScalarExpr::Column(Column::Name {
                        name: "sum_amt".to_string()
                    }),
                    op: ComparatorOp::Gt,
                    right: lit_i(10),
                },
                Predicate::InList {
                    expr: col("t", "k"),
                    list: vec![
                        ScalarExpr::Column(Column::Name {
                            name: "min_z".to_string()
                        }),
                        lit_i(1)
                    ],
                    negated: false,
                },
                Predicate::IsNull {
                    expr: ScalarExpr::Column(Column::Name {
                        name: "max_x".to_string()
                    }),
                    negated: false,
                },
                Predicate::Like {
                    expr: ScalarExpr::Column(Column::Name {
                        name: "cnt_y_dist".to_string()
                    }),
                    pattern: lit_s("%A%"),
                    negated: false,
                },
            ])
        );
    }

    #[test]
    fn rewrite_predicate_handles_or_variants() {
        let predicate = Predicate::Or(vec![
            Predicate::Compare {
                left: fn_agg("SUM", vec![col("t", "amt")], false),
                op: ComparatorOp::Gt,
                right: lit_i(10),
            },
            Predicate::Const3(Truth::False),
        ]);
        let mut map = HashMap::<AggregateCall, String>::new();
        map.insert(
            map_entry("SUM", vec![col("t", "amt")], false, "sum_amt").0,
            "sum_amt".into(),
        );

        let out = AggregateCall::rewrite_predicate_using_call_names(&predicate, &map);

        assert_eq!(
            out,
            Predicate::Or(vec![
                Predicate::Compare {
                    left: ScalarExpr::Column(Column::Name {
                        name: "sum_amt".to_string()
                    }),
                    op: ComparatorOp::Gt,
                    right: lit_i(10),
                },
                Predicate::Const3(Truth::False),
            ])
        );
    }

    #[test]
    fn rewrite_distinct_and_non_distinct_use_different_keys() {
        // COUNT(DISTINCT t.id) and COUNT(t.id) must map independently
        let e = fn_scalar("LOWER", vec![fn_agg("COUNT", vec![col("t", "id")], true)]);
        let e2 = fn_scalar("LOWER", vec![fn_agg("COUNT", vec![col("t", "id")], false)]);

        let mut map = HashMap::<AggregateCall, String>::new();
        map.insert(
            map_entry("COUNT", vec![col("t", "id")], true, "cnt_dist").0,
            "cnt_dist".into(),
        );
        map.insert(
            map_entry("COUNT", vec![col("t", "id")], false, "cnt_all").0,
            "cnt_all".into(),
        );

        let r1 = AggregateCall::rewrite_scalar_using_call_names(&e, &map);
        let r2 = AggregateCall::rewrite_scalar_using_call_names(&e2, &map);

        assert_eq!(
            r1,
            fn_scalar(
                "LOWER",
                vec![ScalarExpr::Column(Column::Name {
                    name: "cnt_dist".to_string()
                })]
            )
        );
        assert_eq!(
            r2,
            fn_scalar(
                "LOWER",
                vec![ScalarExpr::Column(Column::Name {
                    name: "cnt_all".to_string()
                })]
            )
        );
    }

    #[test]
    fn rewrite_keeps_const3_predicates_untouched() {
        let p = Predicate::Const3(Truth::Unknown);
        let out = AggregateCall::rewrite_predicate_using_call_names(&p, &HashMap::new());
        assert_eq!(out, Predicate::Const3(Truth::Unknown));
    }

    // Optional: When a mapping is missing, we expect a panic (because of .expect(..)).
    #[test]
    #[should_panic(expected = "aggregate call must be named")]
    fn rewrite_panics_when_mapping_is_missing() {
        let expr = fn_agg("SUM", vec![col("t", "amt")], false);
        let map = HashMap::<AggregateCall, String>::new(); // no entry
        let _ = AggregateCall::rewrite_scalar_using_call_names(&expr, &map);
    }
}
