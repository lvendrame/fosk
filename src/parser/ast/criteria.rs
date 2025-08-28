use crate::parser::Field;

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Null,
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    // Decimal(String), Date(...), Timestamp(...), etc.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithOp { Add, Sub, Mul, Div }

#[derive(Debug, Clone, PartialEq)]
pub enum ScalarExpr {
    Field(Field),
    Literal(Literal),
    BinaryArith { left: Box<ScalarExpr>, op: ArithOp, right: Box<ScalarExpr> },
    Func { name: String, args: Vec<ScalarExpr> },

    // Predicates that *embed* scalars:
    Compare { left: Box<ScalarExpr>, op: CmpOp, right: Box<ScalarExpr> }, // =, <, <=, >, >=, <>, !=
    IsNull  { expr: Box<ScalarExpr>, negated: bool },
    InList  { expr: Box<ScalarExpr>, list: Vec<ScalarExpr>, negated: bool },
    Like    { expr: Box<ScalarExpr>, pattern: Box<ScalarExpr>, negated: bool, escape: Option<char> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp { Eq, NotEq, Lt, LtEq, Gt, GtEq }

pub enum Criteria {
    And(Vec<Criteria>),
    Or(Vec<Criteria>),
    Not(Box<Criteria>),

    Compare {left: ScalarExpr, }
}
