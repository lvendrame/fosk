use std::{collections::HashMap, sync::Arc};

use crate::{parser::{aggregators_helper::{AggregateImpl, AvgImpl, CountImpl, MaxImpl, MinImpl, SumImpl}, analyzer::{AnalysisContext, AnalyzerError}, ast::Function}, JsonPrimitive};

/// Case-insensitive registry of aggregates.
#[derive(Default)]
pub struct AggregateRegistry {
    by_name: HashMap<String, Arc<dyn AggregateImpl>>,
}

impl AggregateRegistry {
    pub fn new() -> Self { Self { by_name: HashMap::new() } }

    pub fn register<I: AggregateImpl + 'static>(&mut self, impl_: I) {
        self.by_name.insert(impl_.name().to_string(), Arc::new(impl_));
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn AggregateImpl>> {
        self.by_name.get(&name.to_ascii_lowercase()).cloned()
    }

    pub fn list(&self) -> Vec<String> {
        let mut v: Vec<_> = self.by_name.keys().cloned().collect();
        v.sort();
        v
    }

    /// Helper used by Scalar/Type inference to route aggregate typing.
    pub fn infer_type(&self, fun: &Function, ctx: &AnalysisContext) -> Result<(JsonPrimitive, bool), AnalyzerError> {
        let lname = fun.name.to_ascii_lowercase();
        let imp = self.get(&lname).ok_or_else(|| AnalyzerError::FunctionNotFound(fun.name.clone()))?;
        imp.infer_type(fun, ctx)
    }

    pub fn default_aggregate_registry() -> Self {
        let mut registry = Self::new();
        registry.register(CountImpl);
        registry.register(SumImpl);
        registry.register(AvgImpl);
        registry.register(MinImpl);
        registry.register(MaxImpl);
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Number, Value};
    use crate::parser::analyzer::{AnalysisContext, AnalyzerError};
    use crate::parser::ast::{Function, ScalarExpr};
    use crate::{JsonPrimitive, database::{SchemaProvider, SchemaDict, FieldInfo}};
    use indexmap::IndexMap;
    use std::collections::HashMap;

    fn num_i(n: i64) -> Value { Value::Number(Number::from(n)) }
    fn num_f(f: f64) -> Value { Value::Number(Number::from_f64(f).unwrap()) }

    // --- tiny schema provider ---
    struct SP { by: HashMap<String, SchemaDict> }
    impl SP {
        fn new() -> Self { Self { by: HashMap::new() } }
        fn with(mut self, name: &str, fields: Vec<(&str, JsonPrimitive, bool)>) -> Self {
            let mut m = IndexMap::new();
            for (k,t,n) in fields { m.insert(k.to_string(), FieldInfo{ty:t, nullable:n}); }
            self.by.insert(name.to_string(), SchemaDict { fields: m });
            self
        }
    }
    impl SchemaProvider for SP {
        fn schema_of(&self, name: &str) -> Option<SchemaDict> { self.by.get(name).cloned() }
    }
    fn ctx<'a>(sp: &'a SP) -> AnalysisContext<'a> {
        let mut c = AnalysisContext::new(sp);
        c.add_collection("t", "t");
        c
    }

    #[test]
    fn registry_contains_all_and_lookup_is_case_insensitive() {
        let r = AggregateRegistry::default_aggregate_registry();
        let mut names = r.list();
        names.sort();
        assert_eq!(names, vec!["avg","count","max","min","sum"]);

        // case-insensitive
        assert!(r.get("COUNT").is_some());
        assert!(r.get("sUm").is_some());
        assert!(r.get("Avg").is_some());
    }

    #[test]
    fn registry_infer_type_matches_rules() {
        let sp = SP::new().with("t", vec![
            ("i", JsonPrimitive::Int, false),
            ("f", JsonPrimitive::Float, true),
            ("s", JsonPrimitive::String, true),
        ]);
        let ctx = ctx(&sp);
        let r = AggregateRegistry::default_aggregate_registry();

        // COUNT(*) -> Int, non-nullable
        let c_star = Function { name: "count".into(), distinct: false, args: vec![ScalarExpr::WildCard] };
        assert_eq!(r.infer_type(&c_star, &ctx).unwrap(), (JsonPrimitive::Int, false));

        // SUM(i) -> Int, nullable
        let sum_i = Function { name: "sum".into(), distinct: false, args: vec![ScalarExpr::Column(
            crate::parser::ast::Column::WithCollection{ collection:"t".into(), name:"i".into() }
        )]};
        assert_eq!(r.infer_type(&sum_i, &ctx).unwrap(), (JsonPrimitive::Int, true));

        // AVG(f) -> Float, nullable
        let avg_f = Function { name:"avg".into(), distinct:false, args: vec![ScalarExpr::Column(
            crate::parser::ast::Column::WithCollection{ collection:"t".into(), name:"f".into() }
        )]};
        assert_eq!(r.infer_type(&avg_f, &ctx).unwrap(), (JsonPrimitive::Float, true));

        // MIN(s) -> String, nullable
        let min_s = Function { name:"min".into(), distinct:false, args: vec![ScalarExpr::Column(
            crate::parser::ast::Column::WithCollection{ collection:"t".into(), name:"s".into() }
        )]};
        assert_eq!(r.infer_type(&min_s, &ctx).unwrap(), (JsonPrimitive::String, true));

        // SUM(string) -> error
        let sum_s = Function { name:"sum".into(), distinct:false, args: vec![ScalarExpr::Column(
            crate::parser::ast::Column::WithCollection{ collection:"t".into(), name:"s".into() }
        )]};
        assert!(matches!(r.infer_type(&sum_s, &ctx), Err(AnalyzerError::FunctionArgMismatch{..})));
    }

    #[test]
    fn accumulators_basic_semantics() {
        let r = AggregateRegistry::default_aggregate_registry();

        // COUNT: *, NULL, non-null
        let mut acc_c = r.get("count").unwrap().create_accumulator();
        acc_c.update(&[]).unwrap();                 // *
        acc_c.update(&[Value::Null]).unwrap();      // count(expr) with NULL
        acc_c.update(&[num_i(1)]).unwrap();         // count(expr) with non-null
        assert_eq!(acc_c.finalize(), num_i(2));

        // SUM int: nulls ignored
        let mut acc_s = r.get("sum").unwrap().create_accumulator();
        acc_s.update(&[Value::Null]).unwrap();
        acc_s.update(&[num_i(2)]).unwrap();
        acc_s.update(&[num_i(3)]).unwrap();
        assert_eq!(acc_s.finalize(), num_i(5));

        // AVG float
        let mut acc_a = r.get("avg").unwrap().create_accumulator();
        acc_a.update(&[num_f(1.5)]).unwrap();
        acc_a.update(&[Value::Null]).unwrap();
        acc_a.update(&[num_f(2.5)]).unwrap();
        assert_eq!(acc_a.finalize(), num_f(2.0));

        // MIN / MAX string
        let mut acc_min = r.get("min").unwrap().create_accumulator();
        for s in ["pear","apple","plum"] {
            acc_min.update(&[Value::String(s.into())]).unwrap();
        }
        assert_eq!(acc_min.finalize(), Value::String("apple".into()));

        let mut acc_max = r.get("max").unwrap().create_accumulator();
        for s in ["pear","apple","plum"] {
            acc_max.update(&[Value::String(s.into())]).unwrap();
        }
        assert_eq!(acc_max.finalize(), Value::String("plum".into()));
    }

    // ---- COUNT ----
    #[test]
    fn count_star_and_count_expr() {
        let mut acc = CountImpl.create_accumulator();
        acc.update(&[]).unwrap();              // *
        acc.update(&[Value::Null]).unwrap();   // expr NULL
        acc.update(&[num_i(1)]).unwrap();      // expr non-null
        assert_eq!(acc.finalize(), num_i(2));
    }

    // ---- SUM ----
    #[test]
    fn sum_int_and_float_and_nulls() {
        let mut a = SumImpl.create_accumulator();
        a.update(&[Value::Null]).unwrap();
        a.update(&[num_i(2)]).unwrap();
        a.update(&[num_i(3)]).unwrap();
        assert_eq!(a.finalize(), num_i(5));

        let mut b = SumImpl.create_accumulator();
        b.update(&[num_f(1.5)]).unwrap();
        b.update(&[num_f(2.25)]).unwrap();
        assert_eq!(b.finalize(), num_f(3.75));
    }

    // ---- AVG ----
    #[test]
    fn avg_ignores_null_and_returns_float() {
        let mut a = AvgImpl.create_accumulator();
        a.update(&[Value::Null]).unwrap();
        a.update(&[num_i(2)]).unwrap();
        a.update(&[num_i(3)]).unwrap();
        // (2 + 3) / 2 = 2.5
        assert_eq!(a.finalize(), num_f(2.5));

        let mut b = AvgImpl.create_accumulator();
        b.update(&[num_f(1.0)]).unwrap();
        b.update(&[num_f(2.0)]).unwrap();
        b.update(&[num_f(3.0)]).unwrap();
        assert_eq!(b.finalize(), num_f(2.0));
    }

    // ---- MIN / MAX ----
    #[test]
    fn min_max_numeric_and_string() {
        let mut min_i = MinImpl.create_accumulator();
        for v in [num_i(5), num_i(2), num_i(9)] { min_i.update(&[v]).unwrap(); }
        assert_eq!(min_i.finalize(), num_i(2));

        let mut max_f = MaxImpl.create_accumulator();
        for v in [num_f(1.25), num_f(3.5), num_f(2.75)] { max_f.update(&[v]).unwrap(); }
        assert_eq!(max_f.finalize(), num_f(3.5));

        let mut min_s = MinImpl.create_accumulator();
        for v in ["pear","apple","plum"].map(|s| Value::String(s.into())) { min_s.update(&[v]).unwrap(); }
        assert_eq!(min_s.finalize(), Value::String("apple".into()));

        let mut max_s = MaxImpl.create_accumulator();
        for v in ["pear","apple","plum"].map(|s| Value::String(s.into())) { max_s.update(&[v]).unwrap(); }
        assert_eq!(max_s.finalize(), Value::String("plum".into()));
    }

    #[test]
    fn sum_mix_float_into_int_errors_strict() {
        let mut s = SumImpl.create_accumulator();
        s.update(&[num_i(1)]).unwrap();
        let err = s.update(&[num_f(1.0)]).unwrap_err();
        let msg = format!("{err:?}").to_lowercase();
        assert!(msg.contains("sum received float"));
    }
}
