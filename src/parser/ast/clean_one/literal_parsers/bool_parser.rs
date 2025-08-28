use crate::parser::{ast::clean_one::Literal, ParseError, QueryParser};

pub struct BoolParser;

impl BoolParser {
    pub fn is_bool(parser: &QueryParser) -> bool {
        parser.comparers.b_true.compare(parser) ||
        parser.comparers.b_false.compare(parser)
    }

    pub fn parse(parser: &mut QueryParser) -> Result<Literal, ParseError> {
        if parser.comparers.b_true.compare(parser) {
            parser.jump(parser.comparers.b_true.length);
            return Ok(Literal::Bool(true))
        }
        if parser.comparers.b_false.compare(parser) {
            parser.jump(parser.comparers.b_false.length);
            return Ok(Literal::Bool(false))
        }

        Err(ParseError::new("Invalid boolean", parser.position, parser))
    }
}

#[cfg(test)]
pub mod tests {
    use crate::parser::{ast::clean_one::{BoolParser, Literal}, QueryParser};

    #[test]
    pub fn test_bool_parser_true() {
        let text = "true";

        let mut parser = QueryParser::new(text);

        let result = BoolParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Bool(value) => assert!(value),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_bool_parser_false() {
        let text = "false";

        let mut parser = QueryParser::new(text);

        let result = BoolParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Bool(value) => assert!(!value),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_bool_parser_true_upper() {
        let text = "TRUE";

        let mut parser = QueryParser::new(text);

        let result = BoolParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Bool(value) => assert!(value),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_bool_parser_false_upper() {
        let text = "FALSE";

        let mut parser = QueryParser::new(text);

        let result = BoolParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Bool(value) => assert!(!value),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_bool_parser_true_space_delimiter() {
        let text = "true ";

        let mut parser = QueryParser::new(text);

        let result = BoolParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Bool(value) => assert!(value),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_bool_parser_true_comma_delimiter() {
        let text = "true,";

        let mut parser = QueryParser::new(text);

        let result = BoolParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Bool(value) => assert!(value),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_bool_parser_true_parentheses_delimiter() {
        let text = "true)";

        let mut parser = QueryParser::new(text);

        let result = BoolParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Bool(value) => assert!(value),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_bool_parser_true_break_line() {
        let text = "true\r\n";

        let mut parser = QueryParser::new(text);

        let result = BoolParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Bool(value) => assert!(value),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_bool_parser_true_wrong() {
        let text = "truee";

        let mut parser = QueryParser::new(text);

        let result = BoolParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "t");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 0);
            },
        }
    }

    #[test]
    pub fn test_bool_parser_true_wrong_2() {
        let text = "true#";

        let mut parser = QueryParser::new(text);

        let result = BoolParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "t");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 0);
            },
        }
    }
}
