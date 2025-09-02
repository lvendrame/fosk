use crate::parser::ast::ScalarExpr;


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Function {
    pub name: String,
    pub args: Vec<ScalarExpr>,
    pub distinct: bool,
}
