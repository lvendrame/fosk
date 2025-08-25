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
}
