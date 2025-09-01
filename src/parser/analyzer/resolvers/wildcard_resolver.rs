use crate::parser::{analyzer::{AnalysisContext, AnalyzerError}, ast::{Column, ScalarExpr}};

pub struct WildcardResolver;

impl WildcardResolver {
    pub fn expand_wildcard(expr: &ScalarExpr, ctx: &AnalysisContext) -> Result<Vec<ScalarExpr>, AnalyzerError> {
        match expr {
            ScalarExpr::WildCard => {
                // expand all visible collections in insertion order
                let mut result = Vec::new();
                for (visible_collection, backing) in &ctx.collections {
                    if let Some(schema) = ctx.schemas.schema_of(backing) {
                        for (col, _fi) in schema.fields {
                            result.push(ScalarExpr::Column(
                                Column::WithCollection { collection: visible_collection.clone(), name: col }
                            ));
                        }
                    } else {
                        return Err(AnalyzerError::UnknownCollection(backing.clone()));
                    }
                }
                Ok(result)
            }
            ScalarExpr::WildCardWithCollection(collection) => {
                let backing = ctx.collections.get(collection)
                    .ok_or_else(|| AnalyzerError::UnknownCollection(collection.clone()))?;
                let schema = ctx.schemas.schema_of(backing)
                    .ok_or_else(|| AnalyzerError::UnknownCollection(backing.clone()))?;
                let mut result = Vec::new();
                for (col, _fi) in schema.fields {
                    result.push(ScalarExpr::Column(
                        Column::WithCollection { collection: collection.clone(), name: col }
                    ));
                }
                Ok(result)
            }
            _ => Ok(vec![expr.clone()])
        }
    }

    /// Expand a whole projection vector that may contain wildcards.
    pub fn expand_projection(exprs: &[ScalarExpr], cx: &AnalysisContext) -> Result<Vec<ScalarExpr>, AnalyzerError> {
        let mut result = Vec::new();
        for e in exprs {
            let expanded = Self::expand_wildcard(e, cx)?;
            result.extend(expanded);
        }
        Ok(result)
    }
}
