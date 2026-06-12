use crate::parser::{ParseError, QueryParser, WordComparer, ast::Literal};

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

        if !parser.eof() && !WordComparer::is_any_delimiter(parser.current()) {
            return Err(ParseError::new("Invalid number value", pivot, parser));
        }

        let number = parser.text_from_pivot(pivot);
        let number = match is_float {
            true => Literal::Float(
                ordered_float::NotNan::new(
                    number
                        .parse::<f64>()
                        .map_err(|_| ParseError::new("Invalid number", pivot, parser))?,
                )
                .map_err(|_| ParseError::new("Invalid number (NaN)", pivot, parser))?,
            ),
            false => Literal::Int(
                number
                    .parse::<i64>()
                    .map_err(|_| ParseError::new("Invalid number", pivot, parser))?,
            ),
        };

        Ok(number)
    }
}

#[cfg(test)]
pub mod tests {
    use crate::parser::{
        QueryParser,
        ast::{Literal, NumberParser},
    };

    fn parse_number(text: &str) -> Literal {
        let mut parser = QueryParser::new(text);
        NumberParser::parse(&mut parser)
            .unwrap_or_else(|error| panic!("expected number literal, got {error}"))
    }

    #[test]
    pub fn test_number_parser_int() {
        assert_eq!(parse_number("32"), Literal::Int(32));
    }

    #[test]
    pub fn test_number_parser_int_positive() {
        assert_eq!(parse_number("+32"), Literal::Int(32));
    }

    #[test]
    pub fn test_number_parser_int_negative() {
        assert_eq!(parse_number("-32"), Literal::Int(-32));
    }

    #[test]
    pub fn test_number_parser_float() {
        assert_eq!(
            parse_number("32."),
            Literal::Float(ordered_float::NotNan::new(32.0).unwrap())
        );
    }

    #[test]
    pub fn test_number_parser_float_digit() {
        assert_eq!(
            parse_number("32.5"),
            Literal::Float(ordered_float::NotNan::new(32.5).unwrap())
        );
    }

    #[test]
    pub fn test_number_parser_float_positive() {
        assert_eq!(
            parse_number("+32.5"),
            Literal::Float(ordered_float::NotNan::new(32.5).unwrap())
        );
    }

    #[test]
    pub fn test_number_parser_float_negative() {
        assert_eq!(
            parse_number("-32.5"),
            Literal::Float(ordered_float::NotNan::new(-32.5).unwrap())
        );
    }

    #[test]
    pub fn test_number_parser_comma_delimiter() {
        assert_eq!(parse_number("32,"), Literal::Int(32));
    }

    #[test]
    pub fn test_number_parser_space_delimiter() {
        assert_eq!(parse_number("32 "), Literal::Int(32));
    }

    #[test]
    pub fn test_number_parser_break_line() {
        assert_eq!(parse_number("32\r"), Literal::Int(32));
    }

    #[test]
    pub fn test_number_parser_wrong_value() {
        let text = "32a";

        let mut parser = QueryParser::new(text);

        let result = NumberParser::parse(&mut parser);

        let err = result.unwrap_err();
        assert_eq!(err.text, "32a");
        assert_eq!(err.start, 0);
        assert_eq!(err.end, 2);
    }

    #[test]
    pub fn test_number_parser_invalid_start() {
        let text = "abc";
        let mut parser = QueryParser::new(text);

        let err = NumberParser::parse(&mut parser).unwrap_err();

        assert_eq!(err.text, "a");
        assert_eq!(err.start, 0);
        assert_eq!(err.end, 0);
    }

    #[test]
    pub fn test_number_parser_rejects_sign_without_digits() {
        let text = "+";
        let mut parser = QueryParser::new(text);

        let err = NumberParser::parse(&mut parser).unwrap_err();

        assert_eq!(err.text, "+");
        assert_eq!(err.start, 0);
        assert_eq!(err.end, 1);
    }

    #[test]
    pub fn test_number_parser_rejects_invalid_float() {
        let text = "-.";
        let mut parser = QueryParser::new(text);

        let err = NumberParser::parse(&mut parser).unwrap_err();

        assert_eq!(err.text, "-.");
        assert_eq!(err.start, 0);
        assert_eq!(err.end, 2);
    }
}
