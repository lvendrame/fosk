use crate::parser::{ParseError, QueryComparers, QueryParser};

pub struct StringParser;

impl StringParser {
    pub fn parse(parser: &mut QueryParser) -> Result<String, ParseError> {
        let mut pivot = parser.position;

        if parser.current() != '"' {
            return Err(ParseError::new("Invalid string value", pivot, parser));
        }
        parser.next();
        pivot = parser.position;

        while !parser.eof() && parser.current() != '"' {
            if QueryComparers::is_block_delimiter(parser) {
                return Err(ParseError::new("Invalid string", pivot, parser));
            }

            parser.next();
        }
        if parser.eof() {
            return Err(ParseError::new("Invalid string", pivot, parser));
        }

        Ok(parser.text_from_pivot(pivot))
    }
}

#[cfg(test)]
pub mod tests {
    use crate::parser::{tokens::clean_one::StringParser, QueryParser};


    #[test]
    pub fn test_string_parser() {
        let text = "\"identifier\"";

        let mut parser = QueryParser::new(text);

        let result = StringParser::parse(&mut parser);

        match result {
            Ok(result) => assert_eq!(result, "identifier"),
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_string_parser_tab() {
        let text = "\"start\tend\"";

        let mut parser = QueryParser::new(text);

        let result = StringParser::parse(&mut parser);

        match result {
            Ok(result) => assert_eq!(result, "start\tend"),
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
