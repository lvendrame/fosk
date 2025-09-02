use crate::parser::{analyzer::{AnalysisContext, AnalyzerError, PredicateResolver}, ast::{Join, Query}};

pub struct JoinResolver;

impl JoinResolver {
    pub fn qualify_and_fold_joins(q: &Query, ctx: &AnalysisContext) -> Result<Vec<Join>, AnalyzerError> {
        let mut out = Vec::with_capacity(q.joins.len());
        for j in &q.joins {
            // predicate must be qualified against the full ctx (we already added left+all joined tables to ctx in build_context_from_query)
            let qp = PredicateResolver::qualify_predicate(&j.predicate, ctx)?;
            let fp = PredicateResolver::fold_predicate(&qp);
            out.push(Join {
                join_type: j.join_type.clone(),
                collection: j.collection.clone(), // table name/alias are already in ctx; we preserve as-is
                predicate: fp,
            });
        }
        Ok(out)
    }
}
