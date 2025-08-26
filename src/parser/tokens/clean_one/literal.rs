use crate::parser::tokens::clean_one::Identifier;

#[derive(Debug)]
pub enum Literal {
    String(String),
    Int(i32),
    Float(f32),
    Bool(bool),
    Null,
    Column { column: Identifier, alias: Option<String> }
}
