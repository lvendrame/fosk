use std::fmt::Display;

use crate::parser::QueryParser;

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub text: String,
    pub start: usize,
    pub end: usize,
}

impl ParseError {
    pub fn new(message: &str, pivot: usize, parser: &QueryParser) -> Self {
        Self {
            message: message.to_string(),
            text: parser.text_from_range(pivot, parser.position + 1),
            start: pivot,
            end: parser.position,
        }
    }

    pub fn err<T>(self) -> Result<T, ParseError> {
        Err(self)
    }
}

impl Display for ParseError  {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "ParseError: {}\n  at [{}:{}] -> '{}'",
                self.message,
                self.start,
                self.end,
                self.text
            )
    }
}

#[cfg(test)]
mod tests {
    use super::ParseError;
    use crate::parser::QueryParser;

    #[test]
    fn new_captures_parser_range_at_pivot() {
        let mut parser = QueryParser::new("select *");
        parser.jump(3);

        let error = ParseError::new("bad token", 1, &parser);

        assert_eq!(error.message, "bad token");
        assert_eq!(error.text, "ele");
        assert_eq!(error.start, 1);
        assert_eq!(error.end, 3);
    }

    #[test]
    fn err_wraps_error_in_result() {
        let parser = QueryParser::new("x");
        let error = ParseError::new("bad token", 0, &parser);
        let result: Result<(), ParseError> = error.err();

        assert!(result.is_err());
    }

    #[test]
    fn display_includes_message_location_and_text() {
        let parser = QueryParser::new("x");
        let error = ParseError::new("bad token", 0, &parser);

        assert_eq!(error.to_string(), "ParseError: bad token\n  at [0:0] -> 'x'");
    }
}
