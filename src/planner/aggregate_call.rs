use crate::parser::ast::ScalarExpr;

/// A normalized aggregate call extracted from expressions.
/// (We keep it minimal; executor will evaluate args per row).
#[derive(Debug, Clone)]
pub struct AggregateCall {
    pub func: String,               // "count" | "sum" | ...
    pub distinct: bool,
    pub args: Vec<ScalarExpr>,      // qualified & folded
    // Optional: stable id to map back into projection later if needed
}
