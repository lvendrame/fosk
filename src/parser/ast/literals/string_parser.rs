use crate::parser::{ParseError, QueryParser, WordComparer, ast::Literal};

pub struct StringParser;

impl StringParser {
    pub fn is_string_delimiter(parser: &QueryParser) -> bool {
        parser.current() == '"' || parser.current() == '\''
    }

    pub fn parse(parser: &mut QueryParser) -> Result<Literal, ParseError> {
        let mut pivot = parser.position;

        if !StringParser::is_string_delimiter(parser) {
            return Err(ParseError::new("Invalid string value", pivot, parser));
        }

        let delimiter = parser.current();
        parser.next();
        pivot = parser.position;

        while !parser.eof() && parser.current() != delimiter {
            if WordComparer::is_current_break_line(parser) {
                return Err(ParseError::new("Invalid string", pivot, parser));
            }

            parser.next();
        }
        if parser.eof() {
            return Err(ParseError::new("Invalid string", pivot, parser));
        }

        let text = parser.text_from_pivot(pivot);
        parser.next();

        Ok(Literal::String(text))
    }
}

#[cfg(test)]
pub mod tests {
    use crate::parser::{
        QueryParser,
        ast::{Literal, StringParser},
    };

    fn parse_string(text: &str) -> String {
        let mut parser = QueryParser::new(text);
        match StringParser::parse(&mut parser) {
            Ok(Literal::String(result)) => result,
            other => panic!("expected string literal, got {other:?}"),
        }
    }

    #[test]
    pub fn test_string_parser() {
        assert_eq!(parse_string("\"identifier\""), "identifier");
    }

    #[test]
    pub fn test_string_parser_single_quote() {
        assert_eq!(parse_string("'identifier'"), "identifier");
    }

    #[test]
    pub fn test_string_parser_tab() {
        assert_eq!(parse_string("\"start\tend\""), "start\tend");
    }

    #[test]
    pub fn test_string_parser_invalid_start() {
        let mut parser = QueryParser::new("identifier");

        let err = StringParser::parse(&mut parser).unwrap_err();

        assert_eq!(err.text, "i");
        assert_eq!(err.start, 0);
        assert_eq!(err.end, 0);
    }

    #[test]
    pub fn test_string_parser_unclosed() {
        let mut parser = QueryParser::new("\"identifier");

        let err = StringParser::parse(&mut parser).unwrap_err();

        assert_eq!(err.text, "identifier");
        assert_eq!(err.start, 1);
        assert_eq!(err.end, 11);
    }

    #[test]
    pub fn test_string_parser_break_line() {
        let text = "\"lets\r\nbreak line\"";

        let mut parser = QueryParser::new(text);

        let result = StringParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "lets\r");
                assert_eq!(err.start, 1);
                assert_eq!(err.end, 5);
            }
        }
    }
}
