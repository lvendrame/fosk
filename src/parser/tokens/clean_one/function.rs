use crate::parser::tokens::clean_one::ScalarExpr;


#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub args: Vec<ScalarExpr>,
}
