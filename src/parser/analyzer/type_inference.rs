use crate::{parser::{analyzer::{AnalysisContext, AnalyzerError, ColumnResolver}, ast::{Function, Literal, ScalarExpr}}, JsonPrimitive};

#[derive(Default)]
pub struct TypeInference;

impl TypeInference {
    pub fn infer_scalar(expr: &ScalarExpr, ctx: &AnalysisContext) -> Result<(JsonPrimitive, bool), AnalyzerError> {
        match expr {
            ScalarExpr::Literal(value) => {
                // Map Literal -> (type, nullability)
                match value {
                    Literal::Null => Ok((JsonPrimitive::Null, true)),
                    Literal::Bool(_) => Ok((JsonPrimitive::Bool, false)),
                    Literal::Int(_) => Ok((JsonPrimitive::Int, false)),
                    Literal::Float(_) => Ok((JsonPrimitive::Float, false)),
                    Literal::String(_) => Ok((JsonPrimitive::String, false)),
                }
            }
            ScalarExpr::Column(column) => {
                let (_qc, rf) = ColumnResolver::qualify_column(column, ctx)?;
                Ok((rf.ty, rf.nullable))
            }
            ScalarExpr::Function(function) => {
                // delegate to registry
                let ret = Self::infer_function_type(function, ctx)?;
                Ok(ret)
            }
            ScalarExpr::WildCard | ScalarExpr::WildCardWithCollection(_) => {
                Err(AnalyzerError::Other("wildcards should be expanded before type inference".into()))
            }
        }
    }

    // Very small built-in function typing (add more as you go)
    fn infer_function_type(function: &Function, ctx: &AnalysisContext) -> Result<(JsonPrimitive, bool), AnalyzerError> {
        let lname = function.name.to_ascii_lowercase();

        // Aggregates
        match lname.as_str() {
            "count" => {
                // COUNT(*) or COUNT(expr) or COUNT(DISTINCT expr) -> Int, non-nullable
                return Ok((JsonPrimitive::Int, false));
            }
            "sum" => {
                // SUM(arg) -> Int/Float, nullable
                let (t, _n) = match function.args.first() {
                    Some(arg) => TypeInference::infer_scalar(arg, ctx)?,
                    None => return Err(AnalyzerError::FunctionArgMismatch {
                        name: function.name.clone(), expected: "SUM(arg)".into(), got: vec![]
                    }),
                };
                return Ok((match t {
                    JsonPrimitive::Int   => JsonPrimitive::Int,
                    JsonPrimitive::Float => JsonPrimitive::Float,
                    _ => return Err(AnalyzerError::FunctionArgMismatch {
                        name: function.name.clone(), expected: "numeric".into(), got: vec![t]
                    }),
                }, true));
            }
            "avg" => {
                // AVG(arg) -> Float, nullable
                let (t, _n) = match function.args.first() {
                    Some(arg) => TypeInference::infer_scalar(arg, ctx)?,
                    None => return Err(AnalyzerError::FunctionArgMismatch {
                        name: function.name.clone(), expected: "AVG(arg)".into(), got: vec![]
                    }),
                };
                return Ok((match t {
                    JsonPrimitive::Int | JsonPrimitive::Float => JsonPrimitive::Float,
                    _ => return Err(AnalyzerError::FunctionArgMismatch {
                        name: function.name.clone(), expected: "numeric".into(), got: vec![t]
                    }),
                }, true));
            }
            "min" | "max" => {
                // MIN/MAX(arg) -> same type, nullable
                let (t, _n) = match function.args.first() {
                    Some(arg) => TypeInference::infer_scalar(arg, ctx)?,
                    None => return Err(AnalyzerError::FunctionArgMismatch {
                        name: function.name.clone(), expected: "MIN/MAX(arg)".into(), got: vec![]
                    }),
                };
                return Ok((t, true));
            }
            _ => { /* fall through to scalar functions */ }
        }

        let mut arg_types = Vec::with_capacity(function.args.len());
        for arg in &function.args {
            arg_types.push(TypeInference::infer_scalar(arg, ctx)?);
        }

        match (lname.as_str(), arg_types.as_slice()) {
            // UPPER(s), LOWER(s), TRIM(s)
            ("upper",  [(JsonPrimitive::String, nullable)]) |
            ("lower",  [(JsonPrimitive::String, nullable)]) |
            ("trim",   [(JsonPrimitive::String, nullable)]) => Ok((JsonPrimitive::String, *nullable)),

            // LENGTH(s) -> Int
            ("length", [(JsonPrimitive::String, nullable)]) => Ok((JsonPrimitive::Int, *nullable)),

            // COALESCE(a,b,...) -> promoted type, nullable if all inputs nullable
            ("coalesce", many) if !many.is_empty() => {
                let mut ty = many[0].0;
                let mut all_nullable = true;
                for (t, nullable) in many.iter().copied() {
                    ty = JsonPrimitive::promote(ty, t);
                    all_nullable = all_nullable && nullable;
                }
                Ok((ty, all_nullable)) // not all nullable -> result non-null; if all nullable, nullable
            }

            _ => Err(AnalyzerError::FunctionNotFound(function.name.clone()))
        }
    }
}


#[cfg(test)]
mod tests {
    use crate::{database::{FieldInfo, SchemaProvider}, parser::ast::Column, SchemaDict};

    use super::*;
    use indexmap::IndexMap;

    // ---------- Dummy SchemaProvider ----------
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

    fn ctx_with_table<'a>(sp: &'a DummySchemas, table: &'a str, alias: Option<&'a str>) -> AnalysisContext<'a> {
        let mut ctx = AnalysisContext::new(sp);
        let visible = alias.unwrap_or(table).to_string();
        ctx.add_collection(visible.clone(), table.to_string());
        ctx
    }

    // ---------- helpers ----------
    fn lit_null() -> ScalarExpr { ScalarExpr::Literal(Literal::Null) }
    fn lit_i(v: i64) -> ScalarExpr { ScalarExpr::Literal(Literal::Int(v)) }
    fn lit_f(v: f64) -> ScalarExpr { ScalarExpr::Literal(Literal::Float(v)) }
    fn lit_s(s: &str) -> ScalarExpr { ScalarExpr::Literal(Literal::String(s.to_string())) }
    fn lit_b(b: bool) -> ScalarExpr { ScalarExpr::Literal(Literal::Bool(b)) }
    fn col(coll: &str, name: &str) -> ScalarExpr {
        ScalarExpr::Column(Column::WithCollection { collection: coll.to_string(), name: name.to_string() })
    }
    fn col_unq(name: &str) -> ScalarExpr {
        ScalarExpr::Column(Column::Name { name: name.to_string() })
    }
    fn fun(name: &str, args: Vec<ScalarExpr>) -> ScalarExpr {
        ScalarExpr::Function(Function { name: name.to_string(), args, distinct: false })
    }
    fn agg(name: &str, args: Vec<ScalarExpr>) -> ScalarExpr {
        ScalarExpr::Function(Function { name: name.to_string(), args, distinct: false })
    }

    // -------------------- Literals --------------------

    #[test]
    fn infer_literals_types_and_nullability() {
        let sp = DummySchemas::new();
        let ctx = ctx_with_table(&sp, "t", None); // unused

        assert_eq!(TypeInference::infer_scalar(&lit_null(), &ctx).unwrap(), (JsonPrimitive::Null, true));
        assert_eq!(TypeInference::infer_scalar(&lit_b(true), &ctx).unwrap(), (JsonPrimitive::Bool, false));
        assert_eq!(TypeInference::infer_scalar(&lit_i(1), &ctx).unwrap(),    (JsonPrimitive::Int,  false));
        assert_eq!(TypeInference::infer_scalar(&lit_f(1.0), &ctx).unwrap(),  (JsonPrimitive::Float,false));
        assert_eq!(TypeInference::infer_scalar(&lit_s("x"), &ctx).unwrap(),  (JsonPrimitive::String,false));
    }

    // -------------------- Columns --------------------

    #[test]
    fn infer_column_uses_schema_and_qualification() {
        // t(a:int not null, s:string nullable)
        let sp = DummySchemas::new().with("t", vec![
            ("a", JsonPrimitive::Int,    false),
            ("s", JsonPrimitive::String, true),
        ]);
        let ctx = ctx_with_table(&sp, "t", None);

        // unqualified should qualify -> t.a / t.s
        assert_eq!(TypeInference::infer_scalar(&col_unq("a"), &ctx).unwrap(), (JsonPrimitive::Int, false));
        assert_eq!(TypeInference::infer_scalar(&col_unq("s"), &ctx).unwrap(), (JsonPrimitive::String, true));

        // qualified works too
        assert_eq!(TypeInference::infer_scalar(&col("t","s"), &ctx).unwrap(), (JsonPrimitive::String, true));
    }

    // -------------------- Wildcards (error) --------------------

    #[test]
    fn infer_on_wildcards_errors() {
        let sp = DummySchemas::new();
        let ctx = ctx_with_table(&sp, "t", None);
        let err1 = TypeInference::infer_scalar(&ScalarExpr::WildCard, &ctx);
        let err2 = TypeInference::infer_scalar(&ScalarExpr::WildCardWithCollection("t".into()), &ctx);
        assert!(err1.is_err());
        assert!(err2.is_err());
        let m1 = format!("{err1:?}").to_lowercase();
        assert!(m1.contains("wildcards"));
    }

    // -------------------- Scalar functions --------------------

    #[test]
    fn infer_scalar_functions_simple() {
        let sp = DummySchemas::new().with("t", vec![("s", JsonPrimitive::String, true)]);
        let ctx = ctx_with_table(&sp, "t", None);

        // upper(s) -> String, nullability follows arg
        let (ty, nullable) = TypeInference::infer_scalar(&fun("upper", vec![col_unq("s")]), &ctx).unwrap();
        assert_eq!(ty, JsonPrimitive::String);
        assert!(nullable);

        // length(s) -> Int, nullability follows arg
        let (ty2, nullable2) = TypeInference::infer_scalar(&fun("length", vec![col_unq("s")]), &ctx).unwrap();
        assert_eq!(ty2, JsonPrimitive::Int);
        assert!(nullable2);
    }

    #[test]
    fn infer_coalesce_promotes_types_and_nullability_all_inputs_nullable() {
        let sp = DummySchemas::new();
        let ctx = ctx_with_table(&sp, "t", None);

        // COALESCE(NULL, 1, 2.0) -> promotes Int + Float => Float; all inputs: Null(true), Int(false), Float(false)
        // Our rule: nullable if ALL inputs nullable; here not all => false
        let (ty, nullable) = TypeInference::infer_scalar(
            &fun("coalesce", vec![lit_null(), lit_i(1), lit_f(2.0)]), &ctx
        ).unwrap();
        assert_eq!(ty, JsonPrimitive::Float);
        assert_eq!(nullable, false);
    }

    #[test]
    fn infer_coalesce_all_inputs_nullable_yields_nullable() {
        let sp = DummySchemas::new();
        let ctx = ctx_with_table(&sp, "t", None);

        // All inputs are NULL literals (nullable)
        let (ty, nullable) = TypeInference::infer_scalar(
            &fun("coalesce", vec![lit_null(), lit_null()]), &ctx
        ).unwrap();
        // promote(Null, Null) returns Null in our logic; but for COALESCE it's acceptable (it'll still be nullable)
        assert_eq!(ty, JsonPrimitive::Null);
        assert_eq!(nullable, true);
    }

    #[test]
    fn infer_unknown_scalar_function_errors() {
        let sp = DummySchemas::new();
        let ctx = ctx_with_table(&sp, "t", None);
        let err = TypeInference::infer_scalar(&fun("unknown_fun", vec![lit_i(1)]), &ctx);
        assert!(matches!(err, Err(AnalyzerError::FunctionNotFound(_))));
    }

    // -------------------- Aggregates --------------------

    #[test]
    fn infer_count_variants_are_int_and_not_nullable() {
        let sp = DummySchemas::new().with("t", vec![("a", JsonPrimitive::Int, true)]);
        let ctx = ctx_with_table(&sp, "t", None);

        // COUNT(*)
        let (ty1, n1) = TypeInference::infer_scalar(&agg("count", vec![ScalarExpr::WildCard]), &ctx).unwrap();
        assert_eq!(ty1, JsonPrimitive::Int);
        assert!(!n1);

        // COUNT(a)
        let (ty2, n2) = TypeInference::infer_scalar(&agg("count", vec![col_unq("a")]), &ctx).unwrap();
        assert_eq!(ty2, JsonPrimitive::Int);
        assert!(!n2);

        // COUNT(DISTINCT a) — distinct is ignored for type/nullable
        let (ty3, n3) = TypeInference::infer_scalar(
            &ScalarExpr::Function(Function { name: "count".into(), args: vec![col_unq("a")], distinct: true }),
            &ctx
        ).unwrap();
        assert_eq!(ty3, JsonPrimitive::Int);
        assert!(!n3);
    }

    #[test]
    fn infer_sum_int_is_int_sum_float_is_float_nullable_true() {
        let sp = DummySchemas::new().with("t", vec![
            ("i", JsonPrimitive::Int,   false),
            ("f", JsonPrimitive::Float, false),
        ]);
        let ctx = ctx_with_table(&sp, "t", None);

        let (ty_i, n_i) = TypeInference::infer_scalar(&agg("sum", vec![col_unq("i")]), &ctx).unwrap();
        let (ty_f, n_f) = TypeInference::infer_scalar(&agg("sum", vec![col_unq("f")]), &ctx).unwrap();

        assert_eq!(ty_i, JsonPrimitive::Int);
        assert_eq!(ty_f, JsonPrimitive::Float);
        assert!(n_i);
        assert!(n_f);
    }

    #[test]
    fn infer_avg_returns_float_nullable_true_for_numeric_inputs() {
        let sp = DummySchemas::new().with("t", vec![
            ("i", JsonPrimitive::Int,   false),
            ("f", JsonPrimitive::Float, true),
        ]);
        let ctx = ctx_with_table(&sp, "t", None);

        let (ty_i, n_i) = TypeInference::infer_scalar(&agg("avg", vec![col_unq("i")]), &ctx).unwrap();
        let (ty_f, n_f) = TypeInference::infer_scalar(&agg("avg", vec![col_unq("f")]), &ctx).unwrap();

        assert_eq!(ty_i, JsonPrimitive::Float);
        assert_eq!(ty_f, JsonPrimitive::Float);
        assert!(n_i);
        assert!(n_f);
    }

    #[test]
    fn infer_min_max_return_same_type_nullable_true() {
        let sp = DummySchemas::new().with("t", vec![
            ("s", JsonPrimitive::String, false),
            ("i", JsonPrimitive::Int, true),
        ]);
        let ctx = ctx_with_table(&sp, "t", None);

        let (ty_min, n_min) = TypeInference::infer_scalar(&agg("min", vec![col_unq("s")]), &ctx).unwrap();
        let (ty_max, n_max) = TypeInference::infer_scalar(&agg("max", vec![col_unq("i")]), &ctx).unwrap();

        assert_eq!(ty_min, JsonPrimitive::String);
        assert_eq!(ty_max, JsonPrimitive::Int);
        assert!(n_min);
        assert!(n_max);
    }

    // -------------------- Aggregate errors --------------------

    #[test]
    fn infer_sum_with_non_numeric_errors() {
        let sp = DummySchemas::new().with("t", vec![("s", JsonPrimitive::String, false)]);
        let ctx = ctx_with_table(&sp, "t", None);

        let err = TypeInference::infer_scalar(&agg("sum", vec![col_unq("s")]), &ctx);
        match err {
            Err(AnalyzerError::FunctionArgMismatch { name, .. }) => assert_eq!(name.to_ascii_lowercase(), "sum"),
            other => panic!("expected FunctionArgMismatch for SUM(string), got {other:?}"),
        }
    }

    #[test]
    fn infer_avg_with_non_numeric_errors() {
        let sp = DummySchemas::new().with("t", vec![("s", JsonPrimitive::String, true)]);
        let ctx = ctx_with_table(&sp, "t", None);

        let err = TypeInference::infer_scalar(&agg("avg", vec![col_unq("s")]), &ctx);
        assert!(matches!(err, Err(AnalyzerError::FunctionArgMismatch{ name, .. }) if name.eq_ignore_ascii_case("avg")));
    }

    #[test]
    fn infer_min_max_without_args_errors() {
        let sp = DummySchemas::new();
        let ctx = ctx_with_table(&sp, "t", None);

        let err_min = TypeInference::infer_scalar(&agg("min", vec![]), &ctx);
        let err_max = TypeInference::infer_scalar(&agg("max", vec![]), &ctx);
        assert!(matches!(err_min, Err(AnalyzerError::FunctionArgMismatch{ name, .. }) if name.eq_ignore_ascii_case("min")));
        assert!(matches!(err_max, Err(AnalyzerError::FunctionArgMismatch{ name, .. }) if name.eq_ignore_ascii_case("max")));
    }
}
