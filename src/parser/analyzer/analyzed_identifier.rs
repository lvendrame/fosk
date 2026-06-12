use crate::{JsonPrimitive, parser::ast::ScalarExpr};

#[derive(Debug, Clone)]
pub struct AnalyzedIdentifier {
    pub expression: ScalarExpr, // qualified & folded
    pub alias: Option<String>,
    pub ty: JsonPrimitive,
    pub nullable: bool,
    /// Final, stable name of this projected column (unique within the SELECT list).
    /// - alias if present
    /// - else bare column name if unique, otherwise qualified "tbl.col"
    /// - else default expr name, with _1, _2… suffix to resolve collisions
    pub output_name: String,
}
