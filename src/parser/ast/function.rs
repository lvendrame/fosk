use crate::parser::ast::ScalarExpr;


#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub args: Vec<ScalarExpr>,
}
