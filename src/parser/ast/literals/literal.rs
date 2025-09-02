use ordered_float::NotNan;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Literal {
    String(String),
    Int(i64),
    Float(NotNan<f64>),
    Bool(bool),
    Null,
}
