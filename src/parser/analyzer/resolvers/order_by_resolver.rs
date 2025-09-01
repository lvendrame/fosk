use std::collections::HashMap;

use crate::parser::{analyzer::{AnalysisContext, AnalyzedIdentifier, AnalyzerError, ScalarResolver}, ast::{Column, Literal, OrderBy, ScalarExpr}};

pub struct OrderByResolver;

impl OrderByResolver {
    pub fn qualify_order_by(
        order_by: &[OrderBy],
        projection: &[AnalyzedIdentifier], // already expanded & qualified
        ctx: &AnalysisContext
    ) -> Result<Vec<OrderBy>, AnalyzerError> {
        // alias map (case-insensitive)
        let mut alias_map: HashMap<String, &ScalarExpr> = HashMap::new();
        for (_, id) in projection.iter().enumerate() {
            if let Some(a) = &id.alias {
                alias_map.insert(a.to_ascii_lowercase(), &id.expression);
            }
        }

        let mut out = Vec::with_capacity(order_by.len());

        for ob in order_by {
            // try positional index
            if let ScalarExpr::Literal(Literal::Int(n)) = &ob.expr {
                let pos = *n as usize;
                if pos == 0 || pos > projection.len() {
                    return Err(AnalyzerError::Other(format!("ORDER BY position {} out of range [1..{}]", pos, projection.len())));
                }
                let expr = projection[pos - 1].expression.clone();
                out.push(OrderBy { expr, ascending: ob.ascending });
                continue;
            }

            // try alias name (ORDER BY alias) – only when the expr is a bare column name
            if let ScalarExpr::Column(Column::Name { name }) = &ob.expr {
                if let Some(src) = alias_map.get(&name.to_ascii_lowercase()) {
                    out.push(OrderBy { expr: (*src).clone(), ascending: ob.ascending });
                    continue;
                }
            }

            // regular path: qualify + fold
            let qualified = ScalarResolver::qualify_scalar(&ob.expr, ctx)?;
            let folded = ScalarResolver::fold_scalar(&qualified);
            out.push(OrderBy { expr: folded, ascending: ob.ascending });
        }

        Ok(out)
    }
}
