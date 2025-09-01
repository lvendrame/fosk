use crate::parser::{analyzer::{AnalysisContext, AnalyzerError, WildcardResolver}, ast::Identifier};

pub struct IdentifierResolver;

impl IdentifierResolver {
    pub fn expand_projection_idents(proj: &[Identifier], ctx: &AnalysisContext) -> Result<Vec<Identifier>, AnalyzerError> {
        let mut result = Vec::new();
        for id in proj {
            let expr = WildcardResolver::expand_wildcard(&id.expression, ctx)?;
            if expr.len() == 1 {
                result.push(Identifier { expression: expr.into_iter().next().unwrap(), alias: id.alias.clone() });
            } else {
                // for expanded columns, keep alias = None; client can alias later
                for e in expr {
                    result.push(Identifier { expression: e, alias: None });
                }
            }
        }

        Ok(result)
    }
}
