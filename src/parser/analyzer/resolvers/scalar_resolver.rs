use crate::parser::{
    analyzer::{AnalysisContext, AnalyzerError, ColumnResolver},
    ast::{Function, Literal, ScalarExpr}
};

pub struct ScalarResolver;

impl ScalarResolver {
    pub fn scalar_literal(expr: &ScalarExpr) -> Option<Literal> {
        match expr {
            ScalarExpr::Literal(l) => Some(l.clone()),
            // you can add folding for nested functions here later
            _ => None
        }
    }

    pub fn fold_scalar(expr: &ScalarExpr) -> ScalarExpr {
        match expr {
            ScalarExpr::Function(Function { name, args , distinct}) => {
                let lname = name.to_ascii_lowercase();
                if matches!(lname.as_str(), "count" | "sum" | "avg" | "min" | "max") {
                    return ScalarExpr::Function(Function { name: name.clone(), args: args.clone(), distinct: *distinct });
                }

                // Fold args first
                let folded_args: Vec<ScalarExpr> = args.iter().map(Self::fold_scalar).collect();

                // If all literals, try to fold
                let mut lit_args = Vec::with_capacity(folded_args.len());
                for arg in &folded_args {
                    if let ScalarExpr::Literal(l) = arg { lit_args.push(l.clone()); } else {
                        return ScalarExpr::Function(Function { name: name.clone(), args: folded_args, distinct: *distinct });
                    }
                }

                let folded = match (lname.as_str(), lit_args.as_slice()) {
                    ("upper",  [Literal::String(value)]) => Some(Literal::String(value.to_uppercase())),
                    ("lower",  [Literal::String(value)]) => Some(Literal::String(value.to_lowercase())),
                    ("trim",   [Literal::String(value)]) => Some(Literal::String(value.trim().to_string())),
                    ("length", [Literal::String(value)]) => Some(Literal::Int(value.chars().count() as i64)),
                    _ => None,
                };

                folded.map(ScalarExpr::Literal)
                      .unwrap_or_else(|| ScalarExpr::Function(Function { name: name.clone(), args: folded_args, distinct: *distinct }))
            }
            _ => expr.clone()
        }
    }

    pub fn qualify_scalar(expr: &ScalarExpr, ctx: &AnalysisContext) -> Result<ScalarExpr, AnalyzerError> {
        match expr {
            ScalarExpr::Column(c) => Ok(ScalarExpr::Column(ColumnResolver::qualify_column(c, ctx)?.0)),
            ScalarExpr::Function(Function{name, args, distinct}) => {
                let mut new_args = Vec::with_capacity(args.len());
                for arg in args {
                    new_args.push(Self::qualify_scalar(arg, ctx)?);
                }
                Ok(ScalarExpr::Function(Function { name: name.clone(), args: new_args, distinct: *distinct }))
            }
            ScalarExpr::WildCard | ScalarExpr::WildCardWithCollection(_) => {
                Err(AnalyzerError::Other("wildcards must be expanded before qualification".into()))
            }
            _ => Ok(expr.clone())
        }
    }
}
