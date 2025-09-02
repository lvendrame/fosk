use crate::parser::{analyzer::{AnalysisContext, AnalyzedIdentifier, AnalyzerError, ScalarResolver}, ast::{Column, Literal, OrderBy, ScalarExpr}};

pub struct OrderByResolver;

impl OrderByResolver {
    pub fn qualify_order_by(
        order_by: &[OrderBy],
        projection: &[AnalyzedIdentifier], // qualified & folded
        ctx: &AnalysisContext
    ) -> Result<Vec<OrderBy>, AnalyzerError> {
        // alias map (case-insensitive)
        let mut alias_map = std::collections::HashMap::<String, &ScalarExpr>::new();
        for id in projection {
            if let Some(a) = &id.alias {
                alias_map.insert(a.to_ascii_lowercase(), &id.expression);
            }
        }

        let mut out = Vec::with_capacity(order_by.len());
        for ob in order_by {
            // positional (1-based)
            if let ScalarExpr::Literal(Literal::Int(n)) = &ob.expr {
                let pos = *n as usize;
                if pos == 0 || pos > projection.len() {
                    return Err(AnalyzerError::Other(format!("ORDER BY position {} out of range [1..{}]", pos, projection.len())));
                }
                out.push(OrderBy { expr: projection[pos - 1].expression.clone(), ascending: ob.ascending });
                continue;
            }

            // alias match: only when bare column name
            if let ScalarExpr::Column(Column::Name { name }) = &ob.expr {
                if let Some(src) = alias_map.get(&name.to_ascii_lowercase()) {
                    out.push(OrderBy { expr: (*src).clone(), ascending: ob.ascending });
                    continue;
                }
            }

            // normal path
            let qualified = ScalarResolver::qualify_scalar(&ob.expr, ctx)?;
            let folded = ScalarResolver::fold_scalar(&qualified);
            out.push(OrderBy { expr: folded, ascending: ob.ascending });
        }
        Ok(out)
    }
}
