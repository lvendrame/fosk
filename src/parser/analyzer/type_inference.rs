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
        let mut arg_types = Vec::with_capacity(function.args.len());
        for arg in &function.args {
            arg_types.push(TypeInference::infer_scalar(arg, ctx)?);
        }

        // Aggregates
        match lname.as_str() {
            "count" => {
                // COUNT(*) or COUNT(expr) or COUNT(DISTINCT expr)
                // -> Int, non-nullable
                return Ok((JsonPrimitive::Int, false));
            },
            "sum" => {
                // nullable=true (NULL if all inputs NULL)
                // Int->Int, Float->Float
                let (t, _n) = arg_types.get(0).ok_or_else(|| AnalyzerError::FunctionArgMismatch {
                    name: function.name.clone(), expected: "SUM(arg)".into(), got: vec![]
                })?;
                return Ok((match t {
                    JsonPrimitive::Int => JsonPrimitive::Int,
                    JsonPrimitive::Float => JsonPrimitive::Float,
                    _ => return Err(AnalyzerError::FunctionArgMismatch {
                        name: function.name.clone(), expected: "numeric".into(), got: vec![*t]
                    }),
                }, true));
            },
            "avg" => {
                // numeric -> Float, nullable
                let (t, _n) = arg_types.get(0).ok_or_else(|| AnalyzerError::FunctionArgMismatch {
                    name: function.name.clone(), expected: "AVG(arg)".into(), got: vec![]
                })?;
                return Ok((match t {
                    JsonPrimitive::Int | JsonPrimitive::Float => JsonPrimitive::Float,
                    _ => return Err(AnalyzerError::FunctionArgMismatch {
                        name: function.name.clone(), expected: "numeric".into(), got: vec![*t]
                    }),
                }, true));
            },
            "min" | "max" => {
                // same type as input, nullable
                let (ty, _n) = arg_types.get(0).ok_or_else(|| AnalyzerError::FunctionArgMismatch {
                    name: function.name.clone(), expected: "MIN/MAX(arg)".into(), got: vec![]
                })?;
                return Ok((*ty, true));
            },
            _ => { /* fall through to scalar functions below */ },
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


