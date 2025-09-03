use ordered_float::NotNan;
use std::fmt::{self, Display};

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Literal {
    String(String),
    Int(i64),
    Float(NotNan<f64>),
    Bool(bool),
    Null,
}

impl Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::String(s) => write!(f, "s: \"{}\"", s),
            Literal::Int(i) => write!(f, "i: {}", i),
            Literal::Float(n) => write!(f, "f: {}", n.into_inner()),
            Literal::Bool(b) => write!(f, "b: {}", b),
            Literal::Null => write!(f, "n: NULL"),
        }
    }
}

impl fmt::Debug for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::String(_) => write!(f, "String({})", self),
            Literal::Int(_) => write!(f, "Int({})", self),
            Literal::Float(_) => write!(f, "Float({})", self),
            Literal::Bool(_) => write!(f, "Bool({})", self),
            Literal::Null => write!(f, "Null(n: NULL)"),
        }
    }
}
