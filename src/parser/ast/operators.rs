use crate::parser::QueryParser;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ComparatorOp {
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
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

        if parser.comparers.not_equal_b.compare(parser)
            || parser.comparers.not_equal_c.compare(parser)
        {
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
    Div,
}

#[cfg(test)]
mod tests {
    use super::{ArithmeticOp, ComparatorOp};
    use crate::parser::QueryParser;

    #[test]
    fn comparator_display_and_debug_cover_every_operator() {
        let cases = [
            (ComparatorOp::Eq, "="),
            (ComparatorOp::NotEq, "<>"),
            (ComparatorOp::Lt, "<"),
            (ComparatorOp::LtEq, "<="),
            (ComparatorOp::Gt, ">"),
            (ComparatorOp::GtEq, ">="),
        ];

        for (op, expected) in cases {
            assert_eq!(op.to_string(), expected);
            assert_eq!(format!("{:?}", op), format!("ComparatorOp({expected})"));
        }
    }

    #[test]
    fn comparator_check_recognizes_supported_tokens() {
        let cases = [
            ("= ", ComparatorOp::Eq),
            ("<> ", ComparatorOp::NotEq),
            ("!= ", ComparatorOp::NotEq),
            ("< ", ComparatorOp::Lt),
            ("<= ", ComparatorOp::LtEq),
            ("> ", ComparatorOp::Gt),
            (">= ", ComparatorOp::GtEq),
        ];

        for (input, expected) in cases {
            let mut parser = QueryParser::new(input);
            assert_eq!(ComparatorOp::check(&mut parser), Some(expected));
        }
    }

    #[test]
    fn comparator_check_returns_none_when_no_operator_matches() {
        let mut parser = QueryParser::new("like");

        assert_eq!(ComparatorOp::check(&mut parser), None);
    }

    #[test]
    fn arithmetic_operator_debug_names_variants() {
        assert_eq!(format!("{:?}", ArithmeticOp::Add), "Add");
        assert_eq!(format!("{:?}", ArithmeticOp::Sub), "Sub");
        assert_eq!(format!("{:?}", ArithmeticOp::Mul), "Mul");
        assert_eq!(format!("{:?}", ArithmeticOp::Div), "Div");
    }
}
