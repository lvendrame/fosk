use crate::parser::{ast::clean_one::Literal, ParseError, QueryComparers, QueryParser};

pub struct NullParser;

impl NullParser {
    pub fn is_null(parser: &QueryParser) -> bool {
        parser.comparers.null.compare(parser)
    }

    pub fn parse(parser: &mut QueryParser) -> Result<Literal, ParseError> {
        if parser.comparers.null.compare(parser) {
            parser.jump(parser.comparers.null.length);
            return Ok(Literal::Null)
        }

        Err(ParseError::new("Invalid boolean", parser.position, parser))
    }
}

#[cfg(test)]
pub mod tests {
    use crate::parser::{ast::clean_one::{NullParser, Literal}, QueryParser};

    #[test]
    pub fn test_null_parser() {
        let text = "null";

        let mut parser = QueryParser::new(text);

        let result = NullParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Null => {}, //should happen
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_null_parser_upper() {
        let text = "NULL";

        let mut parser = QueryParser::new(text);

        let result = NullParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Null => {}, //should happen
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_null_parser_null_space_delimiter() {
        let text = "null ";

        let mut parser = QueryParser::new(text);

        let result = NullParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Null => {}, //should happen
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_null_parser_null_comma_delimiter() {
        let text = "null,";

        let mut parser = QueryParser::new(text);

        let result = NullParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Null => {}, //should happen
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_null_parser_null_break_line() {
        let text = "null\r\n";

        let mut parser = QueryParser::new(text);

        let result = NullParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Null => {}, //should happen
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
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
            },
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
            },
        }
    }
}
