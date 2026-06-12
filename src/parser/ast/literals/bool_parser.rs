use crate::parser::{ParseError, QueryParser, ast::Literal};

pub struct BoolParser;

impl BoolParser {
    pub fn is_bool(parser: &QueryParser) -> bool {
        parser.comparers.b_true.compare(parser) || parser.comparers.b_false.compare(parser)
    }

    pub fn parse(parser: &mut QueryParser) -> Result<Literal, ParseError> {
        if parser.comparers.b_true.compare(parser) {
            parser.jump(parser.comparers.b_true.length);
            return Ok(Literal::Bool(true));
        }
        if parser.comparers.b_false.compare(parser) {
            parser.jump(parser.comparers.b_false.length);
            return Ok(Literal::Bool(false));
        }

        Err(ParseError::new("Invalid boolean", parser.position, parser))
    }
}

#[cfg(test)]
pub mod tests {
    use crate::parser::{
        QueryParser,
        ast::{BoolParser, Literal},
    };

    fn parse_bool(text: &str) -> bool {
        let mut parser = QueryParser::new(text);
        match BoolParser::parse(&mut parser) {
            Ok(Literal::Bool(value)) => value,
            other => panic!("expected bool literal, got {other:?}"),
        }
    }

    #[test]
    pub fn test_bool_parser_true() {
        assert!(parse_bool("true"));
    }

    #[test]
    pub fn test_bool_parser_false() {
        assert!(!parse_bool("false"));
    }

    #[test]
    pub fn test_bool_parser_true_upper() {
        assert!(parse_bool("TRUE"));
    }

    #[test]
    pub fn test_bool_parser_false_upper() {
        assert!(!parse_bool("FALSE"));
    }

    #[test]
    pub fn test_bool_parser_true_space_delimiter() {
        assert!(parse_bool("true "));
    }

    #[test]
    pub fn test_bool_parser_true_comma_delimiter() {
        assert!(parse_bool("true,"));
    }

    #[test]
    pub fn test_bool_parser_true_parentheses_delimiter() {
        assert!(parse_bool("true)"));
    }

    #[test]
    pub fn test_bool_parser_true_break_line() {
        assert!(parse_bool("true\r\n"));
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
            }
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
            }
        }
    }
}
