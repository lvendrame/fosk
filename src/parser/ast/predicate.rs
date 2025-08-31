use crate::parser::{ast::{ArgsParser, ComparatorOp, ScalarExpr}, ParseError, QueryParser};

#[derive(Debug, Clone, PartialEq)]
pub enum Predicate {
    And(Vec<Predicate>),
    Or(Vec<Predicate>),
    //Not(Box<Predicate>),

    // Predicates that *embed* scalars:
    Compare { left: ScalarExpr, op: ComparatorOp, right: ScalarExpr }, // =, <, <=, >, >=, <>, !=
    IsNull  { expr: ScalarExpr, negated: bool },
    InList  { expr: ScalarExpr, list: Vec<ScalarExpr>, negated: bool },
    Like    { expr: ScalarExpr, pattern: ScalarExpr, negated: bool },
}

impl Predicate {
    pub fn is_start(parser: &QueryParser) -> bool {
        parser.comparers.on.compare(parser)
    }

    pub fn parse_single(parser: &mut QueryParser, allow_wildcard: bool) -> Result<Self, ParseError> {

        let left = ScalarExpr::parse(parser, allow_wildcard)?;

        parser.next_non_whitespace();

        let pivot = parser.position;
        let comparator = ComparatorOp::check(parser);

        if let Some(op) = comparator {
            parser.next();
            let right = ScalarExpr::parse(parser, false)?;

            return Ok(Self::Compare { left, op, right });
        }

        if parser.comparers.is_null.compare(parser) {
            parser.jump(parser.comparers.is_null.length);
            return Ok(Self::IsNull { expr: left, negated: false });
        }

        if parser.comparers.is_not_null.compare(parser) {
            parser.jump(parser.comparers.is_not_null.length);
            return Ok(Self::IsNull { expr: left, negated: true });
        }

        if parser.comparers.r#in.compare(parser) {
            parser.jump(parser.comparers.r#in.length);
            let args = ArgsParser::parse(parser, allow_wildcard)?;
            return Ok(Self::InList { expr: left, list: args, negated: false });
        }

        if parser.comparers.not_in.compare(parser) {
            parser.jump(parser.comparers.not_in.length);
            let args = ArgsParser::parse(parser, allow_wildcard)?;
            return Ok(Self::InList { expr: left, list: args, negated: true });
        }

        if parser.comparers.like.compare(parser) {
            parser.jump(parser.comparers.like.length);
            parser.next_non_whitespace();

            let pattern = ScalarExpr::parse(parser, false)?;
            return Ok(Self::Like { expr: left, pattern, negated: false });
        }

        if parser.comparers.not_like.compare(parser) {
            parser.jump(parser.comparers.not_like.length);
            parser.next_non_whitespace();

            let pattern = ScalarExpr::parse(parser, true)?;
            return Ok(Self::Like { expr: left, pattern, negated: false });
        }

        ParseError::new("Invalid predicate", pivot, parser).err()
    }

    pub fn parse_all(parser: &mut QueryParser, allow_wildcard: bool, depth: i8) -> Result<Self, ParseError> {
        let mut pivot = parser.position;
        let mut predicates: Vec<Predicate> = vec![];
        let mut and = false;
        let mut or = false;
        while (!parser.check_next_phase()) && (depth == 0 || parser.current() != ')')   {
            predicates.push(Self::parse_single(parser, allow_wildcard)?);
            parser.next_non_whitespace();

            if parser.comparers.and.compare(parser) {
                parser.jump(parser.comparers.and.length);
                parser.next_non_whitespace();
                and = true;
            }

            if parser.comparers.or.compare(parser) {
                parser.jump(parser.comparers.or.length);
                parser.next_non_whitespace();
                or = true;
            }

            if parser.current() == '(' {
                parser.next();
                predicates.push(Self::parse_all(parser, allow_wildcard, depth + 1)?);
                parser.next();
            }

            pivot = parser.position;
        }

        if depth > 0 && parser.current() != ')' {
            return ParseError::new("Invalid predicate", pivot, parser).err();
        }

        if and {
            if predicates.len() == 1 {
                return ParseError::new("Invalid predicate", pivot, parser).err();
            }
            return Ok(Self::And(predicates));
        }

        if or {
            if predicates.len() == 1 {
                return ParseError::new("Invalid predicate", pivot, parser).err();
            }
            return Ok(Self::Or(predicates));
        }

        if predicates.len() == 1 {
            return Ok(predicates.pop().unwrap());
        }

        ParseError::new("Invalid predicate", pivot, parser).err()
    }

    pub fn parse(parser: &mut QueryParser, allow_wildcard: bool) -> Result<Self, ParseError> {
        Self::parse_all(parser, allow_wildcard, 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{ast::{ComparatorOp, Predicate}, QueryParser};

    #[test]
    pub fn test_predicate_single_equal() {
        let text = "columnA = columnB";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse_single(&mut parser, false).expect("Failed to parse predicate");

        match result {
            Predicate::Compare { left: _, op, right: _ } => assert_eq!(op, ComparatorOp::Eq),
            _ => panic!(),
        };
    }

    #[test]
    pub fn test_predicate_single_not_equal_b() {
        let text = r#"columnA <> "columnB""#;

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse_single(&mut parser, false).expect("Failed to parse predicate");

        match result {
            Predicate::Compare { left: _, op, right: _ } => assert_eq!(op, ComparatorOp::NotEq),
            _ => panic!(),
        };
    }

    #[test]
    pub fn test_predicate_single_not_equal_c() {
        let text = "columnA != 'columnB'";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse_single(&mut parser, false).expect("Failed to parse predicate");

        match result {
            Predicate::Compare { left: _, op, right: _ } => assert_eq!(op, ComparatorOp::NotEq),
            _ => panic!(),
        };
    }

    #[test]
    pub fn test_predicate_single_not_less_than() {
        let text = "columnA < 10";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse_single(&mut parser, false).expect("Failed to parse predicate");

        match result {
            Predicate::Compare { left: _, op, right: _ } => assert_eq!(op, ComparatorOp::Lt),
            _ => panic!(),
        };
    }

    #[test]
    pub fn test_predicate_single_not_less_than_or_equal() {
        let text = "columnA <= 10";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse_single(&mut parser, false).expect("Failed to parse predicate");

        match result {
            Predicate::Compare { left: _, op, right: _ } => assert_eq!(op, ComparatorOp::LtEq),
            _ => panic!(),
        };
    }

    #[test]
    pub fn test_predicate_single_not_greater_than() {
        let text = "columnA > 10";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse_single(&mut parser, false).expect("Failed to parse predicate");

        match result {
            Predicate::Compare { left: _, op, right: _ } => assert_eq!(op, ComparatorOp::Gt),
            _ => panic!(),
        };
    }

    #[test]
    pub fn test_predicate_single_not_greater_than_or_equal() {
        let text = "columnA >= 10";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse_single(&mut parser, false).expect("Failed to parse predicate");

        match result {
            Predicate::Compare { left: _, op, right: _ } => assert_eq!(op, ComparatorOp::GtEq),
            _ => panic!(),
        };
    }

    #[test]
    pub fn test_predicate_single_is_null() {
        let text = "columnA IS NULL";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse_single(&mut parser, false).expect("Failed to parse predicate");

        match result {
            Predicate::IsNull { expr: _, negated } => assert!(!negated),
            _ => panic!(),
        };
    }

    #[test]
    pub fn test_predicate_single_is_not_null() {
        let text = "columnA IS NOT NULL";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse_single(&mut parser, false).expect("Failed to parse predicate");

        match result {
            Predicate::IsNull { expr: _, negated } => assert!(negated),
            _ => panic!(),
        };
    }

    #[test]
    pub fn test_predicate_single_in() {
        let text = "columnA IN(1, 2, 3)";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse_single(&mut parser, false).expect("Failed to parse predicate");

        match result {
            Predicate::InList { expr: _, list, negated } => {
                assert_eq!(list.len(), 3);
                assert!(!negated);
            },
            _ => panic!(),
        };
    }

    #[test]
    pub fn test_predicate_single_not_in() {
        let text = "columnA NOT IN(1, 2, 3)";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse_single(&mut parser, false).expect("Failed to parse predicate");

        match result {
            Predicate::InList { expr: _, list, negated } => {
                assert_eq!(list.len(), 3);
                assert!(negated);
            },
            _ => panic!(),
        };
    }

    #[test]
    pub fn test_predicate_single_like() {
        let text = "columnA LIKE '%pattern%'";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse_single(&mut parser, false).expect("Failed to parse predicate");

        match result {
            Predicate::Like { expr: _, pattern: _, negated } => assert!(!negated),
            _ => panic!(),
        };
    }

    #[test]
    pub fn test_predicate_single_not_like() {
        let text = r#"columnA Not Like "%pattern%""#;

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse_single(&mut parser, false).expect("Failed to parse predicate");

        match result {
            Predicate::Like { expr: _, pattern: _, negated } => assert!(!negated),
            _ => panic!(),
        };
    }

    #[test]
    pub fn test_predicate_single_wrong_comparer() {
        let text = "columnA =! 'columnB'";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse_single(&mut parser, false);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "=");
                assert_eq!(err.start, 8);
                assert_eq!(err.end, 8);
            },
        };
    }

    #[test]
    pub fn test_predicate_single_without_right_side() {
        let text = "columnA != ";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse_single(&mut parser, false);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "");
                assert_eq!(err.start, 11);
                assert_eq!(err.end, 11);
            },
        };
    }

    #[test]
    pub fn test_predicate_and() {
        let text = "columnA = columnB AND columnC = columnD";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse(&mut parser, false).expect("Failed to parse predicate");

        match result {
            Predicate::And(predicates) => assert_eq!(predicates.len(), 2),
            _ => panic!(),
        };
    }

    #[test]
    pub fn test_predicate_and_p_or() {
        let text = "columnA = columnB AND (columnC = columnD OR columnE = columnF)";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse(&mut parser, false).expect("Failed to parse predicate");

        match result {
            Predicate::And(predicates) => assert_eq!(predicates.len(), 2),
            _ => panic!(),
        };
    }

    #[test]
    pub fn test_predicate_and_p_or_without_end() {
        let text = "columnA = columnB AND (columnC = columnD OR columnE = columnF FROM";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse(&mut parser, false);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "");
                assert_eq!(err.start, 66);
                assert_eq!(err.end, 66);
            },
        };
    }

    #[test]
    pub fn test_predicate_and_p_or_without_end_case2() {
        let text = "columnA = columnB AND (columnC = columnD OR columnE = columnF FROM ";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse(&mut parser, false);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "F");
                assert_eq!(err.start, 62);
                assert_eq!(err.end, 62);
            },
        };
    }

    #[test]
    pub fn test_predicate_and_p_or_without_start() {
        let text = "columnA = columnB AND columnC = columnD OR columnE = columnF) FROM ";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse(&mut parser, false);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, ")");
                assert_eq!(err.start, 60);
                assert_eq!(err.end, 60);
            },
        };
    }

    #[test]
    pub fn test_predicate_double_and_p_or_without_start() {
        let text = "columnA = columnB AND AND columnC = columnD OR columnE = columnF FROM ";

        let mut parser = QueryParser::new(text);

        let result = Predicate::parse(&mut parser, false);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "c");
                assert_eq!(err.start, 26);
                assert_eq!(err.end, 26);
            },
        };
    }
}
