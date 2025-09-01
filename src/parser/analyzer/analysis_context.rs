use indexmap::IndexMap;

use crate::{
    database::SchemaProvider,
    parser::{
        analyzer::{AnalyzedIdentifier, AnalyzedQuery, AnalyzerError, ColumnResolver, IdentifierResolver, OrderByResolver, PredicateResolver, ScalarResolver, TypeInference},
        ast::{Collection, Query}
    }
};

pub struct AnalysisContext<'a> {
    /// map visible name -> underlying collection ref (alias or table)
    pub collections: IndexMap<String, String>,
    /// access to schemas
    pub schemas: &'a dyn SchemaProvider,
}

impl<'a> AnalysisContext<'a> {
    pub fn new(schemas: &'a dyn SchemaProvider) -> Self {
        Self { collections: IndexMap::new(), schemas }
    }

    pub fn add_collection(&mut self, visible: impl Into<String>, backing: impl Into<String>) {
        self.collections.insert(visible.into(), backing.into());
    }

    pub fn add_collection_alias(&mut self, visible: impl Into<String>, backing: impl Into<String>) {
        self.collections.insert(visible.into(), backing.into());
    }

    pub fn build_context_from_query(q: &Query, sp: &'a dyn SchemaProvider) -> Result<Self, AnalyzerError> {
        let mut ctx = Self::new(sp);
        for c in &q.collections {
            match c {
                Collection::Table { name, alias } => {
                    let visible = alias.clone().unwrap_or_else(|| name.clone());
                    ctx.add_collection(visible, name.clone());
                }
                Collection::Query => {
                    // you can extend to support subqueries later
                    return Err(AnalyzerError::Other("Collection::Query not yet supported in analyzer".into()));
                }
            }
        }
        // Joins often introduce new visible names too (if you allow `JOIN t AS x`)
        for join in &q.joins {
            match &join.collection {
                Collection::Table { name, alias } => {
                    let visible = alias.clone().unwrap_or_else(|| name.clone());
                    ctx.add_collection(visible, name.clone());
                }
                Collection::Query => {
                    return Err(AnalyzerError::Other("Join of subquery not yet supported in analyzer".into()));
                }
            }
        }

        Ok(ctx)
    }

    pub fn analyze_query(query: &Query, schema_provider: &'a dyn SchemaProvider) -> Result<AnalyzedQuery, AnalyzerError> {
        let ctx = Self::build_context_from_query(query, schema_provider)?;

        // 1) expand wildcards in projection
        let expanded_proj = IdentifierResolver::expand_projection_idents(&query.projection, &ctx)?;

        // 2) qualify + fold + type inference
        let mut analyzed_proj = Vec::with_capacity(expanded_proj.len());
        for id in expanded_proj {
            // qualify (no wildcards remain)
            let qexpr = ScalarResolver::qualify_scalar(&id.expression, &ctx)?;
            // fold constants
            let fexpr = ScalarResolver::fold_scalar(&qexpr);
            // infer type
            let (ty, nullable) = TypeInference::infer_scalar(&fexpr, &ctx)?;
            analyzed_proj.push(AnalyzedIdentifier {
                expression: fexpr,
                alias: id.alias.clone(),
                ty,
                nullable,
            });
        }

        // 3) qualify + fold predicates
        let criteria = match &query.criteria {
            Some(p) => Some(PredicateResolver::fold_predicate(&PredicateResolver::qualify_predicate(p, &ctx)?)),
            None => None
        };
        let having = match &query.having {
            Some(p) => Some(PredicateResolver::fold_predicate(&PredicateResolver::qualify_predicate(p, &ctx)?)),
            None => None
        };

        // 4) qualify group_by columns
        let mut group_by = Vec::with_capacity(query.group_by.len());
        for c in &query.group_by {
            let (qc, _) = ColumnResolver::qualify_column(c, &ctx)?;
            group_by.push(qc);
        }

        // 5) qualify order_by when you share OrderBy type
        let order_by = OrderByResolver::qualify_order_by(&query.order_by, &analyzed_proj, &ctx)?;

        Ok(AnalyzedQuery {
            projection: analyzed_proj,
            collections: ctx.collections.iter().map(|(v, b)| (v.clone(), b.clone())).collect(),
            criteria,
            group_by,
            having,
            order_by,
        })
    }
}
