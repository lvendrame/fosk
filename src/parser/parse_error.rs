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
