use crate::parser::{analyzer::AnalyzedIdentifier, ast::{Column, JoinType, OrderBy, Predicate}};

#[derive(Debug, Clone)]
pub enum AnalyzedSource {
    Table {
        visible: String,
        backing: String,
    },
    Subquery {
        visible: String,
        query: Box<AnalyzedQuery>,
    },
}

#[derive(Debug, Clone)]
pub struct AnalyzedJoin {
    pub join_type: JoinType,
    pub source: AnalyzedSource,
    pub predicate: Predicate,
}

#[derive(Debug, Clone)]
pub struct AnalyzedQuery {
    pub projection: Vec<AnalyzedIdentifier>, // qualified + typed
    pub collections: Vec<AnalyzedSource>,
    pub joins: Vec<AnalyzedJoin>,
    pub criteria: Option<Predicate>,         // qualified + folded
    pub group_by: Vec<Column>,               // qualified
    pub having: Option<Predicate>,           // qualified + folded
    pub order_by: Vec<OrderBy>,  // OrderBy
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
