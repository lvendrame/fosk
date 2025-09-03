use crate::parser::QueryParser;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ComparatorOp {
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq
}

use std::fmt;

impl fmt::Display for ComparatorOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComparatorOp::Eq => write!(f, "="),
            ComparatorOp::NotEq => write!(f, "<>"),
            ComparatorOp::Lt => write!(f, "<"),
            ComparatorOp::LtEq => write!(f, "<="),
            ComparatorOp::Gt => write!(f, ">"),
            ComparatorOp::GtEq => write!(f, ">="),
        }
    }
}

impl fmt::Debug for ComparatorOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ComparatorOp({})", self)
    }
}

impl ComparatorOp {
    pub fn check(parser: &mut QueryParser) -> Option<ComparatorOp> {
        if parser.comparers.equal.compare(parser) {
            parser.jump(parser.comparers.equal.length);
            return Some(ComparatorOp::Eq);
        }

        if parser.comparers.not_equal_b.compare(parser) || parser.comparers.not_equal_c.compare(parser) {
            parser.jump(parser.comparers.not_equal_b.length);
            return Some(ComparatorOp::NotEq);
        }

        if parser.comparers.less_than.compare(parser) {
            parser.jump(parser.comparers.less_than.length);
            return Some(ComparatorOp::Lt);
        }

        if parser.comparers.less_than_or_equal.compare(parser) {
            parser.jump(parser.comparers.less_than_or_equal.length);
            return Some(ComparatorOp::LtEq);
        }

        if parser.comparers.greater_than.compare(parser) {
            parser.jump(parser.comparers.greater_than.length);
            return Some(ComparatorOp::Gt);
        }

        if parser.comparers.greater_than_or_equal.compare(parser) {
            parser.jump(parser.comparers.greater_than_or_equal.length);
            return Some(ComparatorOp::GtEq);
        }

        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithmeticOp {
    Add,
    Sub,
    Mul,
    Div
}
