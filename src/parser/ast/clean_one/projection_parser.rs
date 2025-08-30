use crate::parser::{ast::clean_one::Identifier, ParseError, Phase, QueryParser, WordComparer};

pub struct ProjectionParser;

impl ProjectionParser {
    pub fn is_projection_start(parser: &QueryParser) -> bool {
        parser.comparers.select.compare(parser)
    }

    pub fn parse(parser: &mut QueryParser) -> Result<Vec<Identifier>, ParseError> {
        if !ProjectionParser::is_projection_start(parser) {
            return ParseError::new("Invalid projection", parser.position, parser).err();
        }
        parser.jump(parser.comparers.select.length);

        let mut pivot = parser.position;
        let mut result: Vec<Identifier> = vec![];
        let mut can_consume = true;
        while parser.phase == Phase::Projection {
            parser.next_non_whitespace();

            let current = parser.current();

            if current == ',' {
                if can_consume {
                    return ParseError::new("Invalid projection", pivot, parser).err();
                }

                can_consume = true;
                parser.next();
                continue;
            }

            if parser.check_next_phase() {
                if can_consume {
                    return ParseError::new("Invalid projection", pivot, parser).err();
                }
                continue;
            }

            if !can_consume {
                return ParseError::new("Invalid projection", pivot, parser).err();
            }
            result.push(Identifier::parse(parser)?);
            can_consume = false;

            pivot = parser.position;
            //parser.next();
        }

        if parser.eof() {
            return ParseError::new("Invalid projection", pivot, parser).err();
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{ast::clean_one::ProjectionParser, QueryParser};

    #[test]
    pub fn test_projection() {
        let text = "SELECT column FROM table";

        let mut parser = QueryParser::new(text);

        let result = ProjectionParser::parse(&mut parser).expect("Failed to parse Projection");

        assert_eq!(result.len(), 1);
    }

    #[test]
    pub fn test_projection_three_columns() {
        let text = "SELECT column, other_column, column as alias FROM table";

        let mut parser = QueryParser::new(text);

        let result = ProjectionParser::parse(&mut parser).expect("Failed to parse Projection");

        assert_eq!(result.len(), 3);
    }

    #[test]
    pub fn test_projection_three_columns_with_break_line() {
        let text = r#"SELECT column,
other_column,
column as alias FROM table"#;

        let mut parser = QueryParser::new(text);

        let result = ProjectionParser::parse(&mut parser).expect("Failed to parse Projection");

        assert_eq!(result.len(), 3);
    }

    #[test]
    pub fn test_projection_three_columns_with_break_line_and_tab() {
        let text = "SELECT column,\tother_column,\t123 as alias FROM table";

        let mut parser = QueryParser::new(text);

        let result = ProjectionParser::parse(&mut parser).expect("Failed to parse Projection");

        assert_eq!(result.len(), 3);
    }

    #[test]
    pub fn test_projection_with_wrong_from() {
        let text = "SELECT column FROMM table";

        let mut parser = QueryParser::new(text);

        let result = ProjectionParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "F");
                assert_eq!(err.start, 14);
                assert_eq!(err.end, 14);
            },
        }
    }

    #[test]
    pub fn test_projection_wrong_comma() {
        let text = "SELECT column, other_column, , column as alias FROM table";

        let mut parser = QueryParser::new(text);

        let result = ProjectionParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, ", ,");
                assert_eq!(err.start, 27);
                assert_eq!(err.end, 29);
            },
        }
    }

    #[test]
    pub fn test_projection_eof() {
        let text = "SELECT column, other_column, column as alias";

        let mut parser = QueryParser::new(text);

        let result = ProjectionParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "");
                assert_eq!(err.start, 44);
                assert_eq!(err.end, 44);
            },
        }
    }

    #[test]
    pub fn test_projection_comma_before_from() {
        let text = "SELECT column, other_column, column as alias, FROM table";

        let mut parser = QueryParser::new(text);

        let result = ProjectionParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, ", F");
                assert_eq!(err.start, 44);
                assert_eq!(err.end, 46);
            },
        }
    }
}
