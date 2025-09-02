use crate::parser::{analyzer::AnalyzedIdentifier, ast::{Column, OrderBy, Predicate}};

#[derive(Debug, Clone)]
pub struct AnalyzedQuery {
    pub projection: Vec<AnalyzedIdentifier>, // qualified + typed
    pub collections: Vec<(String /*visible*/, String /*backing*/ )>,
    pub criteria: Option<Predicate>,         // qualified + folded
    pub group_by: Vec<Column>,               // qualified
    pub having: Option<Predicate>,           // qualified + folded
    pub order_by: Vec<OrderBy>,  // OrderBy
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
