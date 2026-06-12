use crate::parser::{ParseError, QueryParser, ast::Literal};

pub struct NullParser;

impl NullParser {
    pub fn is_null(parser: &QueryParser) -> bool {
        parser.comparers.null.compare(parser)
    }

    pub fn parse(parser: &mut QueryParser) -> Result<Literal, ParseError> {
        if parser.comparers.null.compare(parser) {
            parser.jump(parser.comparers.null.length);
            return Ok(Literal::Null);
        }

        Err(ParseError::new("Invalid boolean", parser.position, parser))
    }
}

#[cfg(test)]
pub mod tests {
    use crate::parser::{
        QueryParser,
        ast::{Literal, NullParser},
    };

    fn parse_null(text: &str) {
        let mut parser = QueryParser::new(text);
        match NullParser::parse(&mut parser) {
            Ok(Literal::Null) => {}
            other => panic!("expected null literal, got {other:?}"),
        }
    }

    #[test]
    pub fn test_null_parser() {
        parse_null("null");
    }

    #[test]
    pub fn test_null_parser_upper() {
        parse_null("NULL");
    }

    #[test]
    pub fn test_null_parser_null_space_delimiter() {
        parse_null("null ");
    }

    #[test]
    pub fn test_null_parser_null_comma_delimiter() {
        parse_null("null,");
    }

    #[test]
    pub fn test_null_parser_null_break_line() {
        parse_null("null\r\n");
    }

    #[test]
    pub fn test_null_parser_null_wrong() {
        let text = "nulle";

        let mut parser = QueryParser::new(text);

        let result = NullParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "n");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 0);
            }
        }
    }

    #[test]
    pub fn test_null_parser_null_wrong_2() {
        let text = "null#";

        let mut parser = QueryParser::new(text);

        let result = NullParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "n");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 0);
            }
        }
    }
}
