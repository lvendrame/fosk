use crate::parser::QueryParser;

#[derive(Debug, Default)]
pub struct WordComparer {
    pub length: usize,
    pub word: Vec<char>,
    whitespace_postfix: bool,
    break_line_postfix: bool,
    full_block_delimiter_postfix: bool,
    eof: bool,
    delimiter: Option<char>,
    optional_postfix: Vec<char>,
}

impl WordComparer {
    pub fn new(word: &str) -> Self {
        Self {
            length: word.len(),
            word: word.to_uppercase().chars().collect(),
            whitespace_postfix: false,
            break_line_postfix: false,
            full_block_delimiter_postfix: false,
            eof: false,
            delimiter: None,
            optional_postfix: vec![],
        }
    }

    pub fn reach_eof(&self, parser: &QueryParser) -> bool {
        parser.position + self.length >= parser.length
    }

    pub fn is_block_delimiter(ch: char) -> bool {
        ch.is_ascii_whitespace()
    }

    pub fn is_any_delimiter(ch: char) -> bool {
        ch == ',' || ch == '(' || ch == ')' || ch == '.' || Self::is_block_delimiter(ch)
    }

    pub fn is_break_line(ch: char) -> bool {
        ch == '\r' || ch == '\n'
    }

    pub fn is_current_block_delimiter(parser: &QueryParser) -> bool {
        Self::is_block_delimiter(parser.current())
    }

    pub fn is_current_break_line(parser: &QueryParser) -> bool {
        Self::is_break_line(parser.current())
    }

    pub fn compare(&self, parser: &QueryParser) -> bool {
        let mut position = 0;
        while position < self.length {
            if (parser.position + position) >= parser.length ||
                self.word[position] != parser.text_v[parser.position + position].to_ascii_uppercase() {
                return false;
            }
            position += 1;
        }

        if self.reach_eof(parser) {
             return self.eof;
        }

        if self.delimiter.is_none() && !self.full_block_delimiter_postfix && !self.whitespace_postfix &&
            !self.break_line_postfix && self.optional_postfix.is_empty() {
            return true;
        }

        let next = parser.text_v[parser.position + position];

        if let Some(delimiter) = self.delimiter {
            if next == delimiter {
                return true;
            }
        }

        if self.full_block_delimiter_postfix && Self::is_any_delimiter(next) {
            return true;
        }

        if self.whitespace_postfix && Self::is_block_delimiter(next) {
            return true;
        }

        if self.break_line_postfix && Self::is_break_line(next) {
            return true;
        }

        for value in self.optional_postfix.iter() {
            if *value == next {
                return true;
            }
        }

        false
    }

    pub fn with_eof(mut self) -> Self { self.eof = true; self }
    pub fn with_whitespace_postfix(mut self) -> Self { self.whitespace_postfix = true; self }
    pub fn with_break_line_postfix(mut self) -> Self { self.break_line_postfix = true; self }
    pub fn with_any_delimiter_postfix(mut self) -> Self { self.full_block_delimiter_postfix = true; self }
    pub fn with_delimiter(mut self, delimiter: char) -> Self { self.delimiter = Some(delimiter); self }
    pub fn with_optional_postfix(mut self, value: char) -> Self { self.optional_postfix.push(value); self }

    pub fn compare_with_block_delimiter(&self, parser: &QueryParser) -> bool {
        self.compare(parser) &&
            (self.reach_eof(parser) || Self::is_any_delimiter(parser.peek(self.length)))
    }
}
