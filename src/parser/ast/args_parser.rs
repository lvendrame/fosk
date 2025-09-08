use crate::parser::{ast::{ScalarExpr}, ParseError, QueryParser};

#[derive(Debug, Default)]
pub struct ArgsExpr {
    pub args: Vec<ScalarExpr>,
    pub distinct: bool,
}


impl ArgsExpr {
    pub fn is_args_start(parser: &QueryParser) -> bool {
        parser.current() == '('
    }

    pub fn is_args_end(parser: &QueryParser) -> bool {
        parser.current() == ')'
    }

    pub fn parse(parser: &mut QueryParser, allow_wildcard: bool) -> Result<ArgsExpr, ParseError> {
        let pivot = parser.position;
        let mut expr = ArgsExpr::default();
        let mut can_consume = true;

        if !ArgsExpr::is_args_start(parser) {
            return Err(ParseError::new("Invalid args value", pivot, parser));
        }
        parser.next();
        // pivot = parser.position;

        if parser.comparers.distinct.compare(parser) {
            if !allow_wildcard {
                return Err(ParseError::new("Invalid distinct on args value", pivot, parser));
            }
            expr.distinct = true;
            parser.jump(parser.comparers.distinct.length);
            parser.next_non_whitespace();
        }

        while !parser.eof() && !ArgsExpr::is_args_end(parser) {
            if parser.current().is_whitespace() {
                parser.next();
            } else if parser.current() == ',' {
                if can_consume {
                    return Err(ParseError::new("Invalid args value", pivot, parser));
                }
                can_consume = true;
                parser.next();
            } else {
                if !can_consume {
                    return Err(ParseError::new("Invalid args value", pivot, parser));
                }
                expr.args.push(ScalarExpr::parse(parser, allow_wildcard)?);
                can_consume = false;
            }
        }

        if parser.eof() {
            return Err(ParseError::new("Invalid args value", pivot, parser));
        }
        parser.next();

        Ok(expr)
    }
}

#[cfg(test)]
pub mod tests {
    use crate::parser::{ast::ArgsExpr, QueryParser};

    #[test]
    pub fn test_args_empty() {
        let text = "()";

        let mut parser = QueryParser::new(text);

        let result = ArgsExpr::parse(&mut parser, false);

        match result {
            Ok(result) => {
                assert_eq!(result.args.len(), 0);
                assert!(!result.distinct);
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_args_one() {
        let text = "(true)";

        let mut parser = QueryParser::new(text);

        let result = ArgsExpr::parse(&mut parser, false);

        match result {
            Ok(result) => {
                assert_eq!(result.args.len(), 1);
                assert!(!result.distinct);
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_args_two() {
        let text = "(true, 1)";

        let mut parser = QueryParser::new(text);

        let result = ArgsExpr::parse(&mut parser, false);

        match result {
            Ok(result) => {
                assert_eq!(result.args.len(), 2);
                assert!(!result.distinct);
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_args_three() {
        let text = "(\"hello\", true, 1)";

        let mut parser = QueryParser::new(text);

        let result = ArgsExpr::parse(&mut parser, false);

        match result {
            Ok(result) => {
                assert_eq!(result.args.len(), 3);
                assert!(!result.distinct);
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_args_wrong() {
        let text = "\"hello\", true, 1";

        let mut parser = QueryParser::new(text);

        let result = ArgsExpr::parse(&mut parser, false);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "\"");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 0);
            },
        }
    }

    #[test]
    pub fn test_args_wrong_comma() {
        let text = "(\"hello\", true, , 1";

        let mut parser = QueryParser::new(text);

        let result = ArgsExpr::parse(&mut parser, false);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "(\"hello\", true, ,");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 16);
            },
        }
    }

    #[test]
    pub fn test_args_without_end() {
        let text = "(\"hello\", true, 1";

        let mut parser = QueryParser::new(text);

        let result = ArgsExpr::parse(&mut parser, false);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "(\"hello\", true, 1");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 17);
            },
        }
    }

    #[test]
    pub fn test_args_without_right_separation() {
        let text = "(\"hello\" true 1)";

        let mut parser = QueryParser::new(text);

        let result = ArgsExpr::parse(&mut parser, false);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "(\"hello\" t");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 9);
            },
        }
    }

    #[test]
    pub fn test_args_wildcard_allowed() {
        let text = "(*)";

        let mut parser = QueryParser::new(text);

        let result = ArgsExpr::parse(&mut parser, true);

        match result {
            Ok(result) => {
                assert_eq!(result.args.len(), 1);
                assert!(!result.distinct);
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_args_wildcard_not_allowed() {
        let text = "(*)";

        let mut parser = QueryParser::new(text);

        let result = ArgsExpr::parse(&mut parser, false);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "*)");
                assert_eq!(err.start, 1);
                assert_eq!(err.end, 2);
            },
        }
    }

    #[test]
    pub fn test_args_two_distinct() {
        let text = "(DISTINCT true, 1)";

        let mut parser = QueryParser::new(text);

        let result = ArgsExpr::parse(&mut parser, true);

        match result {
            Ok(result) => {
                assert_eq!(result.args.len(), 2);
                assert!(result.distinct);
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_args_distinct_not_allowed() {
        let text = "(DISTINCT true, 1)";

        let mut parser = QueryParser::new(text);

        let result = ArgsExpr::parse(&mut parser, false);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "(D");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 1);
            },
        }
    }
}
