use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::{
    database::SchemaProvider, executor::{eval::Eval, helpers::Helpers}, parser::{
        aggregators_helper::{Accumulator as AggAcc, AggregateRegistry},
        analyzer::AnalyzerError, ast::{Column, JoinType, ScalarExpr, Truth}
    }, planner::{aggregate_call::AggregateCall, logical_plan::LogicalPlan}, Db
};

pub trait Executor {
    fn execute(&self, db: &Db) -> Result<Vec<Value>, AnalyzerError>;
}

pub struct PlanExecutor {
    plan: LogicalPlan,
}

impl Executor for PlanExecutor {
    fn execute(&self, db: &Db) -> Result<Vec<Value>, AnalyzerError> {
        Self::run_plan(&self.plan, db)
    }
}

type GroupEntry = (Vec<Value>, Vec<Box<dyn AggAcc>>);

impl PlanExecutor {
    pub fn new(plan: LogicalPlan) -> Self { Self { plan } }

    pub fn run_plan(plan: &LogicalPlan, db: &Db) -> Result<Vec<Value>, AnalyzerError> {
        match plan {
            LogicalPlan::Scan { backing, visible } => {
                let coll = db.read().unwrap().get(backing)
                    .ok_or_else(|| AnalyzerError::Other(format!("unknown collection {backing}")))?;
                let mut out = Vec::new();
                for v in coll.read().unwrap().get_all() {
                    // prefix keys with visible name to match qualified columns
                    if let Value::Object(map) = v {
                        let mut m = Map::new();
                        for (k, vv) in map {
                            m.insert(format!("{}.{}", visible, k), vv);
                        }
                        out.push(Value::Object(m));
                    }
                }
                Ok(out)
            }
            LogicalPlan::Filter { input, predicate } => {
                let rows = Self::run_plan(input, db)?;
                let mut out = Vec::new();
                for v in rows {
                    if let Value::Object(m) = &v {
                        if matches!(Eval::eval_predicate3(predicate, m), Truth::True) {
                            out.push(v);
                        }
                    }
                }
                Ok(out)
            }
            LogicalPlan::Aggregate { input, group_keys, aggs } => {
                let rows = Self::run_plan(input, db)?;
                Self::aggregate_rows(rows, group_keys, aggs)
            }
            LogicalPlan::Project { input, exprs } => {
                let rows = Self::run_plan(input, db)?;
                let mut out = Vec::new();
                for v in rows {
                    let mut proj = Map::new();
                    let obj = v.as_object().unwrap();
                    for id in exprs {
                        let val = Eval::eval_scalar(&id.expression, obj);
                        let key = id.alias.clone()
                            .unwrap_or_else(|| Self::default_name_for_expr(&id.expression));
                        proj.insert(key, val);
                    }
                    out.push(Value::Object(proj));
                }
                Ok(out)
            }
            LogicalPlan::Sort { input, keys } => {
                let mut rows = Self::run_plan(input, db)?;
                // stable sort
                rows.sort_by(|a, b| {
                    let ao = a.as_object().unwrap();
                    let bo = b.as_object().unwrap();
                    for k in keys {
                        let av = Eval::eval_scalar(&k.expr, ao);
                        let bv = Eval::eval_scalar(&k.expr, bo);
                        let ord = Helpers::cmp_json_for_sort(&av, &bv, k.ascending);
                        if !ord.is_eq() { return ord; }
                    }
                    std::cmp::Ordering::Equal
                });
                Ok(rows)
            }
            LogicalPlan::Limit { input, limit, offset } => {
                let rows = Self::run_plan(input, db)?;
                let start = offset.unwrap_or(0).max(0) as usize;
                let mut end = rows.len();
                if let Some(lim) = limit { end = (start + (*lim).max(0) as usize).min(rows.len()); }
                Ok(rows.get(start..end).unwrap_or(&[]).to_vec())
            }
            LogicalPlan::Join { left, right, join_type, on } => {
                // Execute children
                let left_rows  = Self::run_plan(left,  db)?;
                let right_rows = Self::run_plan(right, db)?;

                // Collect key sets for null-extension (derived from observed rows)
                let left_keys  = Self::keyset_for_side(left,  &left_rows,  db);
                let right_keys = Self::keyset_for_side(right, &right_rows, db);

                // helpers
                let merge_objs = |lo: &Map<String, Value>, ro: &Map<String, Value>| -> Value {
                    let mut out = Map::new();
                    for (k, v) in lo { out.insert(k.clone(), v.clone()); }
                    for (k, v) in ro { out.insert(k.clone(), v.clone()); }
                    Value::Object(out)
                };

                let null_extended = |obj: &Map<String, Value>, all_keys: &BTreeSet<String>| -> Map<String, Value> {
                    let mut out = Map::new();
                    for k in all_keys {
                        if let Some(v) = obj.get(k) {
                            out.insert(k.clone(), v.clone());
                        } else {
                            out.insert(k.clone(), Value::Null);
                        }
                    }
                    out
                };

                let mut out: Vec<Value> = Vec::new();

                match join_type {
                    JoinType::Inner => {
                        for l in &left_rows {
                            let lo = l.as_object().unwrap();
                            for r in &right_rows {
                                let ro = r.as_object().unwrap();
                                // evaluate ON over merged row
                                let merged = merge_objs(lo, ro);
                                let mref = merged.as_object().unwrap();
                                if matches!(crate::executor::eval::Eval::eval_predicate3(on, mref), Truth::True) {
                                    out.push(merged);
                                }
                            }
                        }
                    }
                    JoinType::Left => {
                        for l in &left_rows {
                            let lo = l.as_object().unwrap();
                            let mut matched = false;
                            for r in &right_rows {
                                let ro = r.as_object().unwrap();
                                let merged = merge_objs(lo, ro);
                                let mref = merged.as_object().unwrap();
                                if matches!(crate::executor::eval::Eval::eval_predicate3(on, mref), Truth::True) {
                                    out.push(merged);
                                    matched = true;
                                }
                            }
                            if !matched {
                                // left row with right side null-extended
                                let right_nulls = null_extended(&Map::new(), &right_keys);
                                out.push(Value::Object(merge_objs(lo, &right_nulls).as_object().unwrap().clone()));
                            }
                        }
                    }
                    JoinType::Right => {
                        for r in &right_rows {
                            let ro = r.as_object().unwrap();
                            let mut matched = false;
                            for l in &left_rows {
                                let lo = l.as_object().unwrap();
                                let merged = merge_objs(lo, ro);
                                let mref = merged.as_object().unwrap();
                                if matches!(crate::executor::eval::Eval::eval_predicate3(on, mref), Truth::True) {
                                    out.push(merged);
                                    matched = true;
                                }
                            }
                            if !matched {
                                let left_nulls = null_extended(&Map::new(), &left_keys);
                                out.push(Value::Object(merge_objs(&left_nulls, ro).as_object().unwrap().clone()));
                            }
                        }
                    }
                    JoinType::Full => {
                        let mut right_matched: Vec<bool> = vec![false; right_rows.len()];

                        for l in &left_rows {
                            let lo = l.as_object().unwrap();
                            let mut matched_any = false;
                            for (i, r) in right_rows.iter().enumerate() {
                                let ro = r.as_object().unwrap();
                                let merged = merge_objs(lo, ro);
                                let mref = merged.as_object().unwrap();
                                if matches!(crate::executor::eval::Eval::eval_predicate3(on, mref), Truth::True) {
                                    out.push(merged);
                                    right_matched[i] = true;
                                    matched_any = true;
                                }
                            }
                            if !matched_any {
                                let right_nulls = null_extended(&Map::new(), &right_keys);
                                out.push(Value::Object(merge_objs(lo, &right_nulls).as_object().unwrap().clone()));
                            }
                        }

                        // emit right-only rows not matched
                        for (i, r) in right_rows.iter().enumerate() {
                            if !right_matched[i] {
                                let ro = r.as_object().unwrap();
                                let left_nulls = null_extended(&Map::new(), &left_keys);
                                out.push(Value::Object(merge_objs(&left_nulls, ro).as_object().unwrap().clone()));
                            }
                        }
                    }
                }

                Ok(out)
            }
        }
    }


    // Build key sets for null-extension; prefer schema if the side is a Scan.
    fn keyset_for_side(side_plan: &LogicalPlan, rows: &Vec<Value>, db: &Db) -> BTreeSet<String> {
        let mut keys: BTreeSet<String> = BTreeSet::new();
        if let LogicalPlan::Scan { backing, visible } = side_plan {
            if let Some(schema) = db.schema_of(backing) {
                for (col, _fi) in schema.fields {
                    keys.insert(format!("{}.{}", visible, col));
                }
                return keys;
            }
        }
        // fallback to observed row keys
        for v in rows {
            if let Some(m) = v.as_object() {
                for k in m.keys() { keys.insert(k.clone()); }
            }
        }
        keys
    }

    // ---- Aggregation runner ----

    fn aggregate_rows(
        rows: Vec<Value>,
        group_keys: &[Column],
        calls: &[AggregateCall],
    ) -> Result<Vec<Value>, AnalyzerError> {
        use std::collections::{HashMap, HashSet};
        let mut groups: HashMap<String, GroupEntry> = HashMap::new();
        let registry = AggregateRegistry::default_aggregate_registry();
        let mut distinct: HashMap<(String, usize), HashSet<String>> = HashMap::new();

        for v in rows {
            let obj = v.as_object().unwrap();

            // eval group key values
            let gb_vals: Vec<Value> = group_keys.iter().map(|c| {
                let expr = ScalarExpr::Column(c.clone());
                Eval::eval_scalar(&expr, obj)
            }).collect();
            let gk = Helpers::canonical_tuple(&gb_vals);

            // create group tuple on first sight
            let entry = groups.entry(gk.clone()).or_insert_with(|| {
                // create accumulators per call
                let accs: Vec<Box<dyn AggAcc>> = calls.iter().map(|call| {
                    registry.get(&call.func).unwrap().create_accumulator()
                }).collect();
                (gb_vals.clone(), accs)
            });

            // feed each aggregate
            for (i, call) in calls.iter().enumerate() {
                // COUNT(*) special-case: increment per-row regardless of Nulls
                let args: Vec<Value> = if call.func.eq_ignore_ascii_case("count")
                    && call.args.len() == 1
                    && matches!(call.args[0], ScalarExpr::WildCard)
                {
                    // pass a definite non-null sentinel so CountImpl "counts" it
                    vec![Value::Bool(true)]
                } else {
                    call.args.iter().map(|a| Eval::eval_scalar(a, obj)).collect()
                };

                if call.distinct {
                    let key = Helpers::canonical_tuple(&args);
                    let set = distinct.entry((gk.clone(), i)).or_default();
                    if set.insert(key) {
                        entry.1[i].update(&args)?;
                    }
                } else {
                    entry.1[i].update(&args)?;
                }
            }
        }

        // build output rows: group keys first, then aggregates
        let mut out = Vec::new();
        for (_gk, (gb_vals, accs)) in groups.into_iter() {
            let mut m = Map::new();

            // materialize group keys
            for (idx, c) in group_keys.iter().enumerate() {
                let key = match c {
                    Column::WithCollection { collection, name } => format!("{}.{}", collection, name),
                    Column::Name { name } => name.clone(),
                };
                m.insert(key, gb_vals[idx].clone());
            }

            // keep track of used names so we can assign base or base_1, base_2, ...
            let mut used: HashSet<String> = m.keys().cloned().collect();

            for (call, acc) in calls.iter().zip(accs.iter()) {
                let base = call.func.to_ascii_lowercase();
                let mut name = base.clone();
                let mut k = 1usize;
                while used.contains(&name) {
                    name = format!("{}_{}", base, k);
                    k += 1;
                }
                used.insert(name.clone());
                m.insert(name, acc.finalize());
            }

            out.push(Value::Object(m));
        }
        Ok(out)
    }

    // Simple default naming when no alias is set (used by Project)
    pub fn default_name_for_expr(e: &ScalarExpr) -> String {
        match e {
            ScalarExpr::Column(Column::WithCollection{ collection, name }) => format!("{}.{}", collection, name),
            ScalarExpr::Column(Column::Name{ name }) => name.clone(),
            ScalarExpr::Function(f) => f.name.to_ascii_lowercase(),
            ScalarExpr::Literal(_) => "_lit".into(),
            ScalarExpr::WildCard | ScalarExpr::WildCardWithCollection(_) => "*".into(),
        }
    }
}


// src/executor/tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use crate::database::{Config, DbCollection, DbCommon, IdType};
    use crate::planner::plan_builder::PlanBuilder;
    use crate::parser::analyzer::AnalyzedQuery;
    use crate::parser::ast::{Column, ComparatorOp, Function, JoinType, Literal, OrderBy, Predicate, ScalarExpr};
    use crate::parser::analyzer::AnalyzedIdentifier;
    use crate::JsonPrimitive;
    use crate::planner::logical_plan::LogicalPlan;

    fn mk_db() -> Db {
        let mut db = Db::new_db_with_config(Config { id_type: IdType::None, id_key: "id".into() });
        let mut t = db.create("t");
        t.add_batch(json!([
            { "id": 1, "cat": "a", "amt": 10.0 },
            { "id": 2, "cat": "a", "amt": 15.0 },
            { "id": 3, "cat": "b", "amt":  7.5 },
            { "id": 4, "cat": "b", "amt": null },
            { "id": 5, "cat": "a", "amt": 22.5 }
        ]));
        db
    }

    fn analyzed_sum_by_cat() -> AnalyzedQuery {
        AnalyzedQuery {
            projection: vec![
                AnalyzedIdentifier {
                    expression: ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"cat".into() }),
                    alias: Some("cat".into()),
                    ty: JsonPrimitive::String,
                    nullable: false,
                },
                AnalyzedIdentifier {
                    expression: ScalarExpr::Function(Function {
                        name:"sum".into(),
                        distinct: false,
                        args: vec![ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"amt".into() })],
                    }),
                    alias: Some("total".into()),
                    ty: JsonPrimitive::Float, // analyzer would set Float for SUM(float)
                    nullable: true,
                },
            ],
            collections: vec![("t".into(), "t".into())],
            criteria: Some(Predicate::Compare {
                left:  ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"id".into() }),
                op: ComparatorOp::Gt,
                right: ScalarExpr::Literal(Literal::Int(1)),
            }),
            group_by: vec![Column::WithCollection{ collection:"t".into(), name:"cat".into() }],
            having: Some(Predicate::Compare {
                left: ScalarExpr::Function(Function{
                    name:"sum".into(),
                    distinct:false,
                    args: vec![ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"amt".into() })],
                }),
                op: ComparatorOp::Gt,
                right: ScalarExpr::Literal(Literal::Float(ordered_float::NotNan::new(20.0).unwrap())),
            }),
            order_by: vec![OrderBy {
                expr: ScalarExpr::Column(Column::Name{ name: "cat".into() }), // already qualified in analyzer; kept simple
                ascending: true
            }],
            limit: Some(10),
            offset: None,
            joins: vec![],
        }
    }

    #[test]
    fn execute_group_by_sum_having_sort_limit() {
        let db = mk_db();
        let aq = analyzed_sum_by_cat();
        let plan = PlanBuilder::from_analyzed(&aq).unwrap();
        let exec = PlanExecutor::new(plan);
        let rows = exec.execute(&db).unwrap();

        // Expect only cat="a": ids > 1 gives rows 2,3,4,5 ⇒ by cat
        //  cat a: amt=15 + 22.5 = 37.5 (row 4 null ignored) -> pass HAVING > 20
        //  cat b: amt=7.5 + null  = 7.5  -> filtered out by HAVING
        assert_eq!(rows.len(), 1);
        let obj = rows[0].as_object().unwrap();
        assert_eq!(obj.get("cat").unwrap(), "a");
        let total = obj.get("total").unwrap().as_f64().unwrap();
        assert!((total - 37.5).abs() < 1e-9);
    }

    fn mk_db_simple() -> Db {
        // id_type None so we keep provided ids
        Db::new_db_with_config(Config { id_type: IdType::None, id_key: "id".into() })
    }

    fn mk_db_for_scan() -> Db {
        let mut db = mk_db_simple();
        let mut t = db.create("t");
        t.add_batch(json!([
            { "id": 1, "name": "Ana", "k": 2, "val": 10.0 },
            { "id": 2, "name": "Bob", "k": 1, "val": null }
        ]));
        db
    }

    // ---------- Scan ---------------------------------------------------------

    #[test]
    fn scan_prefixes_columns_with_visible_name() {
        let db = mk_db_for_scan();
        let plan = LogicalPlan::Scan { backing: "t".into(), visible: "t".into() };
        let out = PlanExecutor::run_plan(&plan, &db).unwrap();
        // Should have qualified keys like "t.id" and "t.name"
        for row in &out {
            let obj = row.as_object().unwrap();
            assert!(obj.contains_key("t.id"));
            assert!(obj.contains_key("t.name"));
        }
    }

    // ---------- Filter (3VL) ------------------------------------------------

    #[test]
    fn filter_only_truth_rows_pass() {
        let db = mk_db_for_scan();
        let scan = LogicalPlan::Scan { backing: "t".into(), visible: "t".into() };

        // WHERE t.val > 5  (row with null -> Unknown -> filtered out)
        let pred = Predicate::Compare {
            left: ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"val".into() }),
            op: ComparatorOp::Gt,
            right: ScalarExpr::Literal(Literal::Float(ordered_float::NotNan::new(5.0).unwrap())),
        };
        let plan = LogicalPlan::Filter { input: Box::new(scan), predicate: pred };
        let out = PlanExecutor::run_plan(&plan, &db).unwrap();
        // Only id=1 has 10.0
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["t.id"], json!(1));
    }

    // ---------- Project ------------------------------------------------------

    #[test]
    fn project_uses_alias_or_default_names() {
        let db = mk_db_for_scan();
        let scan = LogicalPlan::Scan { backing: "t".into(), visible: "t".into() };
        // SELECT UPPER(t.name) AS uname, t.k
        let exprs = vec![
            crate::parser::analyzer::AnalyzedIdentifier {
                expression: ScalarExpr::Function(Function {
                    name: "upper".into(),
                    distinct: false,
                    args: vec![ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"name".into() })],
                }),
                alias: Some("uname".into()),
                ty: crate::JsonPrimitive::String,
                nullable: false,
            },
            crate::parser::analyzer::AnalyzedIdentifier {
                expression: ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"k".into() }),
                alias: None,
                ty: crate::JsonPrimitive::Int,
                nullable: false,
            },
        ];
        let plan = LogicalPlan::Project { input: Box::new(scan), exprs };
        let out = PlanExecutor::run_plan(&plan, &db).unwrap();
        let row = out[0].as_object().unwrap();
        assert!(row.contains_key("uname")); // aliased
        assert!(row.contains_key("t.k"));   // default name for qualified column
    }

    // ---------- Sort (asc/desc, NULLS LAST) ---------------------------------

    #[test]
    fn sort_ascending_and_descending_nulls_last() {
        let db = mk_db_for_scan();
        let scan = LogicalPlan::Scan { backing: "t".into(), visible: "t".into() };

        // ORDER BY t.val ASC (null last)
        let asc = LogicalPlan::Sort {
            input: Box::new(scan.clone()),
            keys: vec![OrderBy {
                expr: ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"val".into() }),
                ascending: true,
            }],
        };
        let rows_asc = PlanExecutor::run_plan(&asc, &db).unwrap();
        // First row must be val=10.0 (id=1), then null (id=2)
        assert_eq!(rows_asc[0]["t.id"], json!(1));
        assert_eq!(rows_asc[1]["t.id"], json!(2));

        // ORDER BY t.val DESC (null still last)
        let desc = LogicalPlan::Sort {
            input: Box::new(scan),
            keys: vec![OrderBy {
                expr: ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"val".into() }),
                ascending: false,
            }],
        };
        let rows_desc = PlanExecutor::run_plan(&desc, &db).unwrap();
        // Same because only one non-null precedes null
        assert_eq!(rows_desc[0]["t.id"], json!(1));
        assert_eq!(rows_desc[1]["t.id"], json!(2));
    }

    // ---------- Limit / Offset ----------------------------------------------

    #[test]
    fn limit_and_offset_bounds() {
        let db = mk_db_for_scan();
        let scan = LogicalPlan::Scan { backing: "t".into(), visible: "t".into() };
        let sorted = LogicalPlan::Sort {
            input: Box::new(scan),
            keys: vec![OrderBy {
                expr: ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"id".into() }),
                ascending: true,
            }],
        };

        // OFFSET 1 LIMIT 5 (overrun okay)
        let p = LogicalPlan::Limit { input: Box::new(sorted), limit: Some(5), offset: Some(1) };
        let out = PlanExecutor::run_plan(&p, &db).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["t.id"], json!(2));
    }

    // ---------- Aggregate: SUM/COUNT/MIN/MAX/AVG, DISTINCT, NULLs -----------

    fn mk_db_for_agg() -> Db {
        let mut db = mk_db_simple();
        let mut t = db.create("t");
        t.add_batch(json!([
            { "id": 1, "cat": "a", "amt": 10.0 },
            { "id": 2, "cat": "a", "amt": 15.0 },
            { "id": 3, "cat": "b", "amt":  7.5 },
            { "id": 4, "cat": "b", "amt": null },
            { "id": 5, "cat": "a", "amt": 22.5 },
            { "id": 6, "cat": "c", "amt": null }
        ]));
        db
    }

    #[test]
    fn aggregate_group_by_with_multiple_aggs_and_having_and_order() {
        let db = mk_db_for_agg();
        // Scan -> Filter id > 1 -> Aggregate GROUP BY t.cat with SUM(t.amt), COUNT(*)
        let scan = LogicalPlan::Scan { backing: "t".into(), visible: "t".into() };
        let filter = LogicalPlan::Filter {
            input: Box::new(scan),
            predicate: Predicate::Compare {
                left: ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"id".into() }),
                op: ComparatorOp::Gt,
                right: ScalarExpr::Literal(Literal::Int(1)),
            }
        };
        let aggs = vec![
            AggregateCall { func: "sum".into(),  args: vec![ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"amt".into() })], distinct: false },
            AggregateCall { func: "count".into(), args: vec![ScalarExpr::WildCard], distinct: false },
        ];
        let group_keys = vec![Column::WithCollection{ collection:"t".into(), name:"cat".into() }];
        let agg = LogicalPlan::Aggregate { input: Box::new(filter), group_keys: group_keys.clone(), aggs };

        // HAVING sum(t.amt) > 20  (simulate by Filter over Aggregate result)
        // sum is materialized under key "sum" (first agg name)
        let having = LogicalPlan::Filter {
            input: Box::new(agg),
            predicate: Predicate::Compare {
                left: ScalarExpr::Column(Column::Name{ name: "sum".into() }),
                op: ComparatorOp::Gt,
                right: ScalarExpr::Literal(Literal::Float(ordered_float::NotNan::new(20.0).unwrap())),
            }
        };

        // ORDER BY cat asc
        let sort = LogicalPlan::Sort {
            input: Box::new(having),
            keys: vec![OrderBy {
                expr: ScalarExpr::Column(Column::Name{ name:"t.cat".into() }),
                ascending: true,
            }],
        };

        // PROJECT cat alias + sum alias + count alias for readability
        let proj = LogicalPlan::Project {
            input: Box::new(sort),
            exprs: vec![
                crate::parser::analyzer::AnalyzedIdentifier {
                    expression: ScalarExpr::Column(Column::Name{ name:"t.cat".into() }),
                    alias: Some("cat".into()),
                    ty: crate::JsonPrimitive::String,
                    nullable: false,
                },
                crate::parser::analyzer::AnalyzedIdentifier {
                    expression: ScalarExpr::Column(Column::Name{ name:"sum".into() }),
                    alias: Some("total".into()),
                    ty: crate::JsonPrimitive::Float,
                    nullable: true,
                },
                crate::parser::analyzer::AnalyzedIdentifier {
                    expression: ScalarExpr::Column(Column::Name{ name:"count".into() }),
                    alias: Some("n".into()),
                    ty: crate::JsonPrimitive::Int,
                    nullable: false,
                },
            ],
        };

        let out = PlanExecutor::run_plan(&proj, &db).unwrap();
        // Expect only cat="a": rows id>1 -> (2,3,4,5,6)
        //   a: 15 + 22.5 = 37.5, count=3 (ids 2,5 and 6? wait: id=6 cat c) → actually rows with cat=a are id=2,5 => count(*) over filtered rows per group is 2
        //   b: 7.5 + null = 7.5
        //   c: null only -> sum null -> filtered by HAVING
        // so only "a" remains
        assert_eq!(out.len(), 1);
        let r = out[0].as_object().unwrap();
        assert_eq!(r["cat"], json!("a"));
        assert!((r["total"].as_f64().unwrap() - 37.5).abs() < 1e-9);
        assert_eq!(r["n"], json!(2)); // COUNT(*) over rows in group a (id=2,5)
    }

    #[test]
    fn aggregate_distinct_count_and_sum_distinct() {
        let db = mk_db_simple();
        let mut t = db.write().unwrap().create("t");
        t.add_batch(json!([
            { "id": 1, "x": 1, "y": 10.0 },
            { "id": 2, "x": 1, "y": 10.0 },
            { "id": 3, "x": 2, "y": 10.0 },
            { "id": 4, "x": 2, "y": null }
        ]));
        drop(t);

        // GROUP BY t.x, COUNT(DISTINCT t.y), SUM(DISTINCT t.y)
        let scan = LogicalPlan::Scan { backing: "t".into(), visible: "t".into() };
        let aggs = vec![
            AggregateCall { func: "count".into(), args: vec![ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"y".into() })], distinct: true },
            AggregateCall { func: "sum".into(),   args: vec![ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"y".into() })], distinct: true },
        ];
        let group_keys = vec![Column::WithCollection{ collection:"t".into(), name:"x".into() }];
        let plan = LogicalPlan::Aggregate { input: Box::new(scan), group_keys: group_keys.clone(), aggs };
        let out = PlanExecutor::run_plan(&plan, &db).unwrap();

        // Expect two groups:
        // x=1: y distinct {10.0} => count=1, sum=10.0
        // x=2: y distinct {10.0, null} => null ignored in SUM, count DISTINCT should ignore null -> count=1, sum=10.0
        // materialized columns: "t.x", "count", "sum"
        let mut byx = std::collections::HashMap::new();
        for r in out {
            let o = r.as_object().unwrap();
            byx.insert(o["t.x"].clone(), (o["count"].clone(), o["sum"].clone()));
        }
        let (c1, s1) = byx.get(&json!(1)).unwrap();
        assert_eq!(*c1, json!(1));
        assert!((s1.as_f64().unwrap() - 10.0).abs() < 1e-9);

        let (c2, s2) = byx.get(&json!(2)).unwrap();
        assert_eq!(*c2, json!(1));
        assert!((s2.as_f64().unwrap() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn aggregate_avg_min_max_and_null_only_group() {
        let db = mk_db_simple();
        let mut t = db.write().unwrap().create("t");
        t.add_batch(json!([
            { "id": 1, "g": "a", "v": 2.0 },
            { "id": 2, "g": "a", "v": 4.0 },
            { "id": 3, "g": "b", "v": null }
        ]));
        drop(t);

        // GROUP BY g: AVG(v), MIN(v), MAX(v)
        let scan = LogicalPlan::Scan { backing: "t".into(), visible: "t".into() };
        let aggs = vec![
            AggregateCall { func: "avg".into(), args: vec![ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"v".into() })], distinct: false },
            AggregateCall { func: "min".into(), args: vec![ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"v".into() })], distinct: false },
            AggregateCall { func: "max".into(), args: vec![ScalarExpr::Column(Column::WithCollection{ collection:"t".into(), name:"v".into() })], distinct: false },
        ];
        let group_keys = vec![Column::WithCollection{ collection:"t".into(), name:"g".into() }];
        let plan = LogicalPlan::Aggregate { input: Box::new(scan), group_keys, aggs };
        let mut out = PlanExecutor::run_plan(&plan, &db).unwrap();

        // Create a map by group
        let mut map = std::collections::HashMap::new();
        for r in out.drain(..) {
            let o = r.as_object().unwrap();
            map.insert(o["t.g"].clone(), (o["avg"].clone(), o["min"].clone(), o["max"].clone()));
        }

        // g=a: avg=3.0, min=2.0, max=4.0
        let (a_avg, a_min, a_max) = map.get(&json!("a")).unwrap();
        assert!((a_avg.as_f64().unwrap() - 3.0).abs() < 1e-9);
        assert!((a_min.as_f64().unwrap() - 2.0).abs() < 1e-9);
        assert!((a_max.as_f64().unwrap() - 4.0).abs() < 1e-9);

        // g=b: only nulls => avg=null, min/max=null
        let (b_avg, b_min, b_max) = map.get(&json!("b")).unwrap();
        assert!(b_avg.is_null());
        assert!(b_min.is_null());
        assert!(b_max.is_null());
    }

    // ---------- Error path: JOIN not implemented ----------------------------

    #[test]
    fn join_node_executes_inner_cross_join() {
        use serde_json::json;

        // Build a tiny DB with two tables
        let mut db = Db::new_db_with_config(Config { id_type: IdType::None, id_key: "id".into() });
        let mut t = db.create("t");
        let mut u = db.create("u");

        t.add_batch(json!([
            { "id": 1, "x": "A" },
            { "id": 2, "x": "B" }
        ]));
        u.add_batch(json!([
            { "id": 10, "y": true  },
            { "id": 20, "y": false }
        ]));

        // Plan: INNER JOIN with ON TRUE (i.e., cross join)
        let plan = LogicalPlan::Join {
            left: Box::new(LogicalPlan::Scan { backing: "t".into(), visible: "t".into() }),
            right: Box::new(LogicalPlan::Scan { backing: "u".into(), visible: "u".into() }),
            join_type: JoinType::Inner,
            on: Predicate::Const3(Truth::True),
        };

        let rows = PlanExecutor::run_plan(&plan, &db).expect("join should execute");
        // 2 x 2 = 4 rows
        assert_eq!(rows.len(), 4);

        // Verify merged/prefixed columns exist and are correct for at least one row
        // (Scan prefixes with visible name, so keys look like "t.id", "t.x", "u.id", "u.y")
        let r0 = rows[0].as_object().expect("object row");
        assert!(r0.contains_key("t.id"));
        assert!(r0.contains_key("t.x"));
        assert!(r0.contains_key("u.id"));
        assert!(r0.contains_key("u.y"));
    }

    #[test]
    fn left_join_emits_unmatched_left_rows_with_null_right_side() {
        let mut db = Db::new_db_with_config(Config { id_type: IdType::None, id_key: "id".into() });
        let mut t = db.create("t");
        let _u = db.create("u"); // keep it empty

        t.add_batch(json!([
            { "id": 1, "x": "A" },
            { "id": 2, "x": "B" }
        ]));

        let plan = LogicalPlan::Join {
            left: Box::new(LogicalPlan::Scan { backing: "t".into(), visible: "t".into() }),
            right: Box::new(LogicalPlan::Scan { backing: "u".into(), visible: "u".into() }),
            join_type: JoinType::Left,
            on: Predicate::Const3(Truth::True),
        };

        let rows = PlanExecutor::run_plan(&plan, &db).expect("left join should execute");
        // Expect as many rows as left input
        assert_eq!(rows.len(), 2);

        // Each output row has t.* keys, and (because right is truly empty and we
        // can't infer its schema here) there are no u.* keys present.
        for row in rows {
            let obj = row.as_object().unwrap();
            assert!(obj.contains_key("t.id"));
            assert!(obj.contains_key("t.x"));
            assert!(obj.keys().all(|k| !k.starts_with("u.")));
        }
    }

    #[test]
    fn left_join_null_ext_uses_schema_even_when_right_is_empty() {
        // Build DB
        let mut db = Db::new_db_with_config(Config { id_type: IdType::None, id_key: "id".into() });
        let mut t = db.create("t");
        let mut u = db.create("u");

        // Seed left with data
        t.add_batch(json!([
            { "id": 1, "x": "A" },
            { "id": 2, "x": "B" }
        ]));

        // Seed right ONCE to register schema, then clear rows to make it empty at execution.
        u.add_batch(json!([
            { "id": 999, "y": true }
        ]));
        u.clear(); // assumes schema persists; if not, remove this and adjust expectations below.

        // LEFT JOIN with ON TRUE (right yields 0 rows)
        let plan = LogicalPlan::Join {
            left: Box::new(LogicalPlan::Scan { backing: "t".into(), visible: "t".into() }),
            right: Box::new(LogicalPlan::Scan { backing: "u".into(), visible: "u".into() }),
            join_type: JoinType::Left,
            on: Predicate::Const3(Truth::True),
        };

        let rows = PlanExecutor::run_plan(&plan, &db).expect("left join should execute");
        assert_eq!(rows.len(), 2);

        // Expect t.* keys present and u.* keys present but null
        for row in rows {
            let obj = row.as_object().unwrap();
            assert!(obj.contains_key("t.id"));
            assert!(obj.contains_key("t.x"));

            // u.id and u.y should exist and be null (schema-based null extension)
            assert!(obj.contains_key("u.id") && obj.get("u.id").unwrap().is_null());
            assert!(obj.contains_key("u.y")  && obj.get("u.y").unwrap().is_null());
        }
    }

    #[test]
    fn keyset_for_side_scan_uses_schema_with_visible_prefix() {
        let db = mk_db();
        let mut t = db.clone().create("t");
        // Seed to register schema (id, x)
        t.add_batch(json!([
            { "id": 1, "x": "A" },
            { "id": 2, "x": "B" }
        ]));

        let plan = LogicalPlan::Scan { backing: "t".into(), visible: "tt".into() };
        let rows: Vec<serde_json::Value> = vec![]; // no observed rows needed

        let keys = PlanExecutor::keyset_for_side(&plan, &rows, &db);
        let expected = BTreeSet::from_iter(["tt.id".to_string(), "tt.x".to_string()]);
        assert_eq!(keys, expected);
    }

    #[test]
    fn keyset_for_side_scan_empty_rows_but_schema_known_still_returns_prefixed_keys() {
        let db = mk_db();
        let mut u = db.clone().create("u");
        // register schema
        u.add_batch(json!([{ "id": 99, "y": true }]));
        // make it empty for execution but schema remains
        // NOTE: if your clear() also clears schema, remove this line and adapt expectations.
        let _ = u.clear();

        let plan = LogicalPlan::Scan { backing: "u".into(), visible: "uuu".into() };
        let rows: Vec<serde_json::Value> = vec![];

        let keys = PlanExecutor::keyset_for_side(&plan, &rows, &db);
        let expected = BTreeSet::from_iter(["uuu.id".to_string(), "uuu.y".to_string()]);
        assert_eq!(keys, expected);
    }

    #[test]
    fn keyset_for_side_non_scan_falls_back_to_observed_row_keys() {
        let db = mk_db();
        let mut t = db.clone().create("t");
        t.add_batch(json!([{ "id": 1, "x": "A" }]));

        // Produce observed rows by executing a scan, then ask keyset for a NON-Scan plan
        let scan = LogicalPlan::Scan { backing: "t".into(), visible: "t".into() };
        let observed = PlanExecutor::run_plan(&scan, &db).unwrap();

        // Non-scan node (e.g., Project), so helper must use observed keys
        let non_scan = LogicalPlan::Project {
            input: Box::new(scan),
            exprs: vec![], // irrelevant here
        };

        let keys = PlanExecutor::keyset_for_side(&non_scan, &observed, &db);
        let expected = BTreeSet::from_iter(["t.id".to_string(), "t.x".to_string()]);
        assert_eq!(keys, expected);
    }

    #[test]
    fn keyset_for_side_no_schema_and_no_rows_returns_empty_set() {
        let db = mk_db();
        // Intentionally do NOT create the table → no schema known
        let plan = LogicalPlan::Scan { backing: "missing".into(), visible: "m".into() };
        let rows: Vec<Value> = vec![];

        let keys = PlanExecutor::keyset_for_side(&plan, &rows, &db);
        assert!(keys.is_empty());
    }
}
