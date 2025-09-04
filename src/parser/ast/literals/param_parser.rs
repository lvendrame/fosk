use crate::parser::{ast::ScalarExpr, ParseError, QueryParser};

pub struct ParamParser;

impl ParamParser {
    pub fn is_param(parser: &QueryParser) -> bool {
        parser.comparers.param.compare(parser)
    }

    pub fn parse(parser: &mut QueryParser) -> Result<ScalarExpr, ParseError> {
        if parser.comparers.param.compare(parser) {
            parser.jump(parser.comparers.param.length);
            return Ok(ScalarExpr::Parameter)
        }

        Err(ParseError::new("Invalid Parameter", parser.position, parser))
    }
}

#[cfg(test)]
pub mod tests {
    use crate::parser::{ast::{ParamParser, ScalarExpr}, QueryParser};

    #[test]
    pub fn test_param_parser() {
        let text = "?";

        let mut parser = QueryParser::new(text);

        let result = ParamParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                ScalarExpr::Parameter => {}, //should happen
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_param_parser_upper() {
        let text = "?";

        let mut parser = QueryParser::new(text);

        let result = ParamParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                ScalarExpr::Parameter => {}, //should happen
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_param_parser_param_space_delimiter() {
        let text = "? ";

        let mut parser = QueryParser::new(text);

        let result = ParamParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                ScalarExpr::Parameter => {}, //should happen
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_param_parser_param_comma_delimiter() {
        let text = "?,";

        let mut parser = QueryParser::new(text);

        let result = ParamParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                ScalarExpr::Parameter => {}, //should happen
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_param_parser_param_break_line() {
        let text = "?\r\n";

        let mut parser = QueryParser::new(text);

        let result = ParamParser::parse(&mut parser);

        match result {
            Ok(result) => match result {
                ScalarExpr::Parameter => {}, //should happen
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_param_parser_param_wrong() {
        let text = "?e";

        let mut parser = QueryParser::new(text);

        let result = ParamParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "?");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 0);
            },
        }
    }

    #[test]
    pub fn test_param_parser_param_wrong_2() {
        let text = "?#";

        let mut parser = QueryParser::new(text);

        let result = ParamParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "?");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 0);
            },
        }
    }
}
