use crate::parser::{tokens::clean_one::Literal, ParseError, QueryComparers, QueryParser};

pub struct NumberParser;

impl NumberParser {
    pub fn is_number(parser: &QueryParser) -> bool {
        let current = parser.current();
        current.is_ascii_digit() || current == '+' || current == '-'
    }

    pub fn parse(parser: &mut QueryParser) -> Result<Literal, ParseError> {
        let pivot = parser.position;
        let mut is_float = false;

        if !NumberParser::is_number(parser) {
            return Err(ParseError::new("Invalid number value", pivot, parser));
        }

        while !parser.eof() && (NumberParser::is_number(parser) || parser.current() == '.') {
            if parser.current() == '.' {
                is_float = true;
            }
            parser.next();
        }

        if !parser.eof() && parser.current() != ',' && !QueryComparers::is_current_block_delimiter(parser) {
            return Err(ParseError::new("Invalid number value", pivot, parser));
        }

        let number = parser.text_from_pivot(pivot);
        let number = match is_float {
            true => Literal::Float(number.parse::<f32>().map_err(|_| ParseError::new("Invalid number", pivot, parser))?),
            false => Literal::Int(number.parse::<i32>().map_err(|_| ParseError::new("Invalid number", pivot, parser))?),
        };

        Ok(number)
    }
}

#[cfg(test)]
pub mod tests {
    use crate::parser::{tokens::clean_one::{Literal, NumberParser}, QueryParser};

    #[test]
    pub fn test_number_parser_int() {
        let text = "32";

        let mut parser = QueryParser::new(text);

        let result = NumberParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Int(value) => assert_eq!(value, 32),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_number_parser_int_positive() {
        let text = "+32";

        let mut parser = QueryParser::new(text);

        let result = NumberParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Int(value) => assert_eq!(value, 32),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_number_parser_int_negative() {
        let text = "-32";

        let mut parser = QueryParser::new(text);

        let result = NumberParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Int(value) => assert_eq!(value, -32),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_number_parser_float() {
        let text = "32.";

        let mut parser = QueryParser::new(text);

        let result = NumberParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Float(value) => assert_eq!(value, 32.0),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_number_parser_float_digit() {
        let text = "32.5";

        let mut parser = QueryParser::new(text);

        let result = NumberParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Float(value) => assert_eq!(value, 32.5),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_number_parser_float_positive() {
        let text = "+32.5";

        let mut parser = QueryParser::new(text);

        let result = NumberParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Float(value) => assert_eq!(value, 32.5),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_number_parser_float_negative() {
        let text = "-32.5";

        let mut parser = QueryParser::new(text);

        let result = NumberParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Float(value) => assert_eq!(value, -32.5),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_number_parser_comma_delimiter() {
        let text = "32,";

        let mut parser = QueryParser::new(text);

        let result = NumberParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Int(value) => assert_eq!(value, 32),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_number_parser_space_delimiter() {
        let text = "32 ";

        let mut parser = QueryParser::new(text);

        let result = NumberParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Int(value) => assert_eq!(value, 32),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_number_parser_break_line() {
        let text = "32\r";

        let mut parser = QueryParser::new(text);

        let result = NumberParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Int(value) => assert_eq!(value, 32),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_number_parser_wrong_value() {
        let text = "32a";

        let mut parser = QueryParser::new(text);

        let result = NumberParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "32a");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 2);
            },
        }
    }
}
