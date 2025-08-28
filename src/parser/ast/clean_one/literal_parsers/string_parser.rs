use crate::parser::{ast::clean_one::Literal, ParseError, QueryParser, WordComparer};

pub struct StringParser;

impl StringParser {
    pub fn is_string_delimiter(parser: &QueryParser) -> bool {
        parser.current() == '"'
    }

    pub fn parse(parser: &mut QueryParser) -> Result<Literal, ParseError> {
        let mut pivot = parser.position;

        if !StringParser::is_string_delimiter(parser) {
            return Err(ParseError::new("Invalid string value", pivot, parser));
        }
        parser.next();
        pivot = parser.position;

        while !parser.eof() && !StringParser::is_string_delimiter(parser) {
            if WordComparer::is_current_breal_line(parser) {
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
    use crate::parser::{ast::clean_one::{Literal, StringParser}, QueryParser};

    #[test]
    pub fn test_string_parser() {
        let text = "\"identifier\"";

        let mut parser = QueryParser::new(text);

        let result = StringParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::String(result) => assert_eq!(result, "identifier"),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_string_parser_tab() {
        let text = "\"start\tend\"";

        let mut parser = QueryParser::new(text);

        let result = StringParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::String(result) => assert_eq!(result, "start\tend"),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
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
            },
        }
    }
}
