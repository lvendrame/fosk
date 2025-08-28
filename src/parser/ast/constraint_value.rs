use crate::parser::Field;

#[derive(Debug, Clone, PartialEq)]
pub enum ConstraintValue {
    String(String),
    Integer(i32),
    Float(f32),
    Boolean(bool),
    Null,
    Field(Field)
}
