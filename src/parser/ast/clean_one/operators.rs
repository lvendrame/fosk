use crate::parser::QueryParser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparatorOp {
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq
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
