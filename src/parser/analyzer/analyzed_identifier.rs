use crate::{parser::ast::ScalarExpr, JsonPrimitive};

#[derive(Debug, Clone)]
pub struct AnalyzedIdentifier {
    pub expression: ScalarExpr,     // qualified & folded
    pub alias: Option<String>,
    pub ty: JsonPrimitive,
    pub nullable: bool,
}
