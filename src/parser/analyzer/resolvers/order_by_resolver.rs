use crate::parser::{analyzer::{AggregateResolver, AnalysisContext, AnalyzedIdentifier, AnalyzerError, ColumnKey, ScalarResolver}, ast::{Column, Literal, OrderBy, ScalarExpr}};

pub struct OrderByResolver;

impl OrderByResolver {
    pub fn qualify_order_by(
        order_bys: &[OrderBy],
        projection: &[AnalyzedIdentifier], // qualified & folded
        ctx: &AnalysisContext,
        group_set: &std::collections::HashSet<ColumnKey>,
    ) -> Result<Vec<OrderBy>, AnalyzerError> {
        // alias map (case-insensitive)
        let mut alias_map = std::collections::HashMap::<String, &ScalarExpr>::new();
        for analyzed_id in projection {
            if let Some(alias) = &analyzed_id.alias {
                alias_map.insert(alias.to_ascii_lowercase(), &analyzed_id.expression);
            }
        }

        let mut out = Vec::with_capacity(order_bys.len());
        for order_by in order_bys {
            // positional (1-based)
            if let ScalarExpr::Literal(Literal::Int(value)) = &order_by.expr {
                let pos = *value as usize;
                if pos == 0 || pos > projection.len() {
                    return Err(AnalyzerError::Other(format!("ORDER BY position {} out of range [1..{}]", pos, projection.len())));
                }
                let expr = projection[pos - 1].expression.clone();
                out.push(OrderBy { expr, ascending: order_by.ascending });
                continue;
            }

            // alias match: only when bare column name
            if let ScalarExpr::Column(Column::Name { name }) = &order_by.expr {
                if let Some(src) = alias_map.get(&name.to_ascii_lowercase()) {
                    out.push(OrderBy { expr: (*src).clone(), ascending: order_by.ascending });
                    continue;
                }
            }

            // normal path
            let qualified = ScalarResolver::qualify_scalar(&order_by.expr, ctx)?;
            let folded = ScalarResolver::fold_scalar(&qualified);

            if !AggregateResolver::uses_only_group_by(&folded, group_set, false) {
                return Err(AnalyzerError::Other("ORDER BY references columns not in GROUP BY and outside aggregates".into()));
            }

            out.push(OrderBy { expr: folded, ascending: order_by.ascending });
        }
        Ok(out)
    }
}
