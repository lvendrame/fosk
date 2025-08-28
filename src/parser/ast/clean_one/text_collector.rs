use crate::parser::{ParseError, QueryComparers, QueryParser};

pub struct TextCollector;

pub type Stopper = dyn Fn(char) -> bool;

impl TextCollector {
    pub fn collect(parser: &mut QueryParser) -> Result<String, ParseError> {
        TextCollector::collect_with_stopper(parser, &|_|false)
    }

    pub fn collect_with_stopper(parser: &mut QueryParser, stopper: &Stopper) -> Result<String, ParseError> {
        let pivot = parser.position;
        while !parser.eof() && !QueryComparers::is_full_block_delimiter(parser.current()) && !stopper(parser.current()) {
            let current = parser.current();
            if !current.is_ascii_alphanumeric() && current != '_' {
                return Err(ParseError::new("Invalid text", pivot, parser));
            }
            parser.next();
        }
        Ok(parser.text_from_pivot(pivot))
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{ast::clean_one::TextCollector, QueryParser};

    #[test]
    pub fn test_text_collector_collect() {
        let text = "text ";

        let mut parser = QueryParser::new(text);

        let result = TextCollector::collect(&mut parser).expect("Failed to collect text");

        assert_eq!(result, "text");
    }

    #[test]
    pub fn test_text_collector_collect_eof() {
        let text = "text";

        let mut parser = QueryParser::new(text);

        let result = TextCollector::collect(&mut parser).expect("Failed to collect text");

        assert_eq!(result, "text");
    }

    #[test]
    pub fn test_text_collector_collect_break_line() {
        let text = "text\r";

        let mut parser = QueryParser::new(text);

        let result = TextCollector::collect(&mut parser).expect("Failed to collect text");

        assert_eq!(result, "text");
    }

    #[test]
    pub fn test_text_collector_collect_comma() {
        let text = "text,";

        let mut parser = QueryParser::new(text);

        let result = TextCollector::collect(&mut parser).expect("Failed to collect text");

        assert_eq!(result, "text");
    }

    #[test]
    pub fn test_text_collector_collect_open_parentheses() {
        let text = "text(";

        let mut parser = QueryParser::new(text);

        let result = TextCollector::collect(&mut parser).expect("Failed to collect text");

        assert_eq!(result, "text");
    }

    #[test]
    pub fn test_text_collector_collect_close_parentheses() {
        let text = "text)";

        let mut parser = QueryParser::new(text);

        let result = TextCollector::collect(&mut parser).expect("Failed to collect text");

        assert_eq!(result, "text");
    }

    #[test]
    pub fn test_text_collector_collect_with_stopper() {
        let text = "texta";

        let mut parser = QueryParser::new(text);

        let result = TextCollector::collect_with_stopper(&mut parser, &|current| current == 'a')
            .expect("Failed to collect text");

        assert_eq!(result, "text");
    }

    #[test]
    pub fn test_text_collector_collect_with_wrong_char() {
        let text = "text#123";

        let mut parser = QueryParser::new(text);

        let result = TextCollector::collect(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "text#");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 4);
            },
        }

    }
}
