use crate::parser::{tokens::clean_one::Literal, ParseError, QueryParser};

pub struct ArgsParser;

impl ArgsParser {
    pub fn is_args_start(parser: &QueryParser) -> bool {
        parser.current() == '('
    }

    pub fn is_args_end(parser: &QueryParser) -> bool {
        parser.current() == ')'
    }

    pub fn parse(parser: &mut QueryParser) -> Result<Vec<Literal>, ParseError> {
        let pivot = parser.position;
        let mut args: Vec<Literal> = vec![];

        if !ArgsParser::is_args_start(parser) {
            return Err(ParseError::new("Invalid args value", pivot, parser));
        }
        parser.next();
        // pivot = parser.position;

        while !parser.eof() && !ArgsParser::is_args_end(parser) {
            if parser.current().is_whitespace() || parser.current() == ',' {
                parser.next();
            } else {
                args.push(Literal::parse(parser)?);
            }
        }

        if parser.eof() {
            return Err(ParseError::new("Invalid args value", pivot, parser));
        }

        Ok(args)
    }
}

#[cfg(test)]
pub mod tests {
    use crate::parser::{tokens::clean_one::ArgsParser, QueryParser};

    #[test]
    pub fn test_args_empty() {
        let text = "()";

        let mut parser = QueryParser::new(text);

        let result = ArgsParser::parse(&mut parser);

        match result {
            Ok(result) => assert_eq!(result.len(), 0),
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_args_one() {
        let text = "(true)";

        let mut parser = QueryParser::new(text);

        let result = ArgsParser::parse(&mut parser);

        match result {
            Ok(result) => {
                assert_eq!(result.len(), 1);
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_args_two() {
        let text = "(true, 1)";

        let mut parser = QueryParser::new(text);

        let result = ArgsParser::parse(&mut parser);

        match result {
            Ok(result) => {
                assert_eq!(result.len(), 2);
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_args_three() {
        let text = "(\"hello\", true, 1)";

        let mut parser = QueryParser::new(text);

        let result = ArgsParser::parse(&mut parser);

        match result {
            Ok(result) => {
                assert_eq!(result.len(), 3);
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_args_wrong() {
        let text = "\"hello\", true, 1";

        let mut parser = QueryParser::new(text);

        let result = ArgsParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "\"");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 0);
            },
        }
    }

    #[test]
    pub fn test_args_without_end() {
        let text = "(\"hello\", true, 1";

        let mut parser = QueryParser::new(text);

        let result = ArgsParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "(\"hello\", true, 1");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 17);
            },
        }
    }
}
