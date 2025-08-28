use crate::parser::{ast::clean_one::{Identifier}, ParseError, QueryComparers, QueryParser};

pub struct ProjectionParser;

impl ProjectionParser {
    pub fn is_projection_start(parser: &QueryParser) -> bool {
        parser.comparers.select.compare(parser)
    }

    pub fn is_projection_end(parser: &QueryParser) -> bool {
        parser.comparers.from.compare(parser)
    }

    pub fn parse(parser: &mut QueryParser) -> Result<Vec<Identifier>, ParseError> {
        let mut result: Vec<Identifier> = vec![];
        let mut can_consume = true;
        while !parser.eof() && !parser.comparers.from.compare(parser) {
            let current = parser.current();

            if current == ',' {
                if can_consume {
                    return ParseError::new("Invalid projection", parser.position, parser).err();
                }

                can_consume = true;
                parser.next();
                continue;
            }

            if !current.is_whitespace() && !QueryComparers::is_block_delimiter(current) {
                if !can_consume {
                    return ParseError::new("Invalid projection", parser.position, parser).err();
                }
                result.push(Identifier::parse(parser)?);
                can_consume = false;
                continue;
            }

            parser.next();
        }

        if parser.eof() {
            return ParseError::new("Invalid projection", parser.position, parser).err();
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{ast::clean_one::ProjectionParser, QueryParser};

    #[test]
    pub fn test_projection() {
        let text = "column FROM table";

        let mut parser = QueryParser::new(text);

        let result = ProjectionParser::parse(&mut parser).expect("Failed to parse Projection");

        assert_eq!(result.len(), 1);
    }

    #[test]
    pub fn test_projection_three_columns() {
        let text = "column, other_column, column as alias FROM table";

        let mut parser = QueryParser::new(text);

        let result = ProjectionParser::parse(&mut parser).expect("Failed to parse Projection");

        assert_eq!(result.len(), 3);
    }

    #[test]
    pub fn test_projection_three_columns_with_break_line() {
        let text = r#"column,
other_column,
column as alias FROM table"#;

        let mut parser = QueryParser::new(text);

        let result = ProjectionParser::parse(&mut parser).expect("Failed to parse Projection");

        assert_eq!(result.len(), 3);
    }

    #[test]
    pub fn test_projection_three_columns_with_break_line_and_tab() {
        let text = "column,\tother_column,\t123 as alias FROM table";

        let mut parser = QueryParser::new(text);

        let result = ProjectionParser::parse(&mut parser).expect("Failed to parse Projection");

        assert_eq!(result.len(), 3);
    }

    #[test]
    pub fn test_projection_with_wrong_from() {
        let text = "column FROMM table";

        let mut parser = QueryParser::new(text);

        let result = ProjectionParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "F");
                assert_eq!(err.start, 7);
                assert_eq!(err.end, 7);
            },
        }
    }

    #[test]
    pub fn test_projection_wrong_comma() {
        let text = "column, other_column, , column as alias FROM table";

        let mut parser = QueryParser::new(text);

        let result = ProjectionParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, ",");
                assert_eq!(err.start, 22);
                assert_eq!(err.end, 22);
            },
        }
    }

    #[test]
    pub fn test_projection_eof() {
        let text = "column, other_column, column as alias";

        let mut parser = QueryParser::new(text);

        let result = ProjectionParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "");
                assert_eq!(err.start, 37);
                assert_eq!(err.end, 37);
            },
        }
    }
}
