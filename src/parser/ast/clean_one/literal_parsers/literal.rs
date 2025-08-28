use crate::parser::ast::clean_one::Column;

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    String(String),
    Int(i32),
    Float(f32),
    Bool(bool),
    Null,
    Column { column: Column, alias: Option<String> }
}
