use crate::{
    parser::ast::{Column, JoinType, OrderBy, Predicate}, planner::aggregate_call::AggregateCall
};

#[derive(Debug, Clone)]
pub enum LogicalPlan {
    /// Scan a single collection (backing table) with a visible name (alias or table name).
    Scan {
        backing: String,   // backing collection (table) name
        visible: String,   // visible name (alias or table)
    },

    Join {
        left: Box<LogicalPlan>,
        right: Box<LogicalPlan>,
        join_type: JoinType,
        on: Predicate,                       // already qualified + folded
    },

    /// Row-level filter (WHERE or HAVING depending on position in the tree).
    Filter {
        input: Box<LogicalPlan>,
        predicate: crate::parser::ast::Predicate,
    },

    /// Group-by aggregation.
    Aggregate {
        input: Box<LogicalPlan>,
        group_keys: Vec<Column>,            // qualified
        aggs: Vec<AggregateCall>,           // aggregate calls we’ll compute
    },

    /// Projection in SELECT order (qualified & folded).
    Project {
        input: Box<LogicalPlan>,
        exprs: Vec<crate::parser::analyzer::AnalyzedIdentifier>,
    },

    /// Stable sort with NULLS LAST policy (enforced in executor).
    Sort {
        input: Box<LogicalPlan>,
        keys: Vec<OrderBy>,                 // qualified & folded
    },

    /// LIMIT / OFFSET
    Limit {
        input: Box<LogicalPlan>,
        limit: Option<i64>,
        offset: Option<i64>,
    },
}
