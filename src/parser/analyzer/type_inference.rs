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


