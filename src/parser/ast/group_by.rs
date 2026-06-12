use crate::parser::{
    ParseError, QueryParser,
    ast::{Column, ScalarExpr},
};

pub struct GroupBy;

impl GroupBy {
    pub fn parse(parser: &mut QueryParser) -> Result<Vec<Column>, ParseError> {
        if !parser.comparers.group_by.compare(parser) {
            return ParseError::new("Invalid group by", parser.position, parser).err();
        }
        parser.jump(parser.comparers.group_by.length);

        let mut groups: Vec<Column> = vec![];
        let mut can_consume = true;
        while !parser.check_next_phase() {
            if parser.current() == ',' {
                if can_consume {
                    return ParseError::new("Invalid group by", parser.position, parser).err();
                }
                can_consume = true;
                parser.next();
                parser.next_non_whitespace();
            }
            if can_consume {
                let expr = ScalarExpr::parse(parser, false)?;
                match expr {
                    ScalarExpr::Column(column) => groups.push(column),
                    _ => return ParseError::new("Invalid group by", parser.position, parser).err(),
                };
                parser.next_non_whitespace();
                can_consume = false;
            } else {
                return ParseError::new("Invalid group by", parser.position, parser).err();
            }
        }

        Ok(groups)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{
        QueryParser,
        ast::{Column, GroupBy},
    };

    fn parse_group_by(text: &str) -> Vec<Column> {
        let mut parser = QueryParser::new(text);
        parser.check_next_phase();
        match GroupBy::parse(&mut parser) {
            Ok(groups) => groups,
            Err(err) => panic!("expected GROUP BY to parse from {text:?}, got {err:?}"),
        }
    }

    fn parse_group_by_error(text: &str) -> (usize, usize, String) {
        let mut parser = QueryParser::new(text);
        parser.check_next_phase();
        match GroupBy::parse(&mut parser) {
            Ok(groups) => panic!("expected GROUP BY error from {text:?}, got {groups:?}"),
            Err(err) => (err.start, err.end, err.text),
        }
    }

    fn names(groups: &[Column]) -> Vec<&str> {
        groups
            .iter()
            .map(|column| match column {
                Column::Name { name } => name.as_str(),
                other => panic!("expected unqualified column, got {other:?}"),
            })
            .collect()
    }

    #[test]
    pub fn test_group_by() {
        let result = parse_group_by("GROUP BY columnA");
        assert_eq!(result.len(), 1);
        assert_eq!(names(&result), ["columnA"]);
    }

    #[test]
    pub fn test_group_by_four() {
        let result = parse_group_by("GROUP BY columnA, columnB, columnC, columnD");
        assert_eq!(result.len(), 4);
        assert_eq!(names(&result), ["columnA", "columnB", "columnC", "columnD"]);
    }

    #[test]
    pub fn test_group_by_four_with_spaces() {
        let result = parse_group_by("GROUP BY columnA , columnB , columnC , columnD");
        assert_eq!(result.len(), 4);
        assert_eq!(names(&result), ["columnA", "columnB", "columnC", "columnD"]);
    }

    #[test]
    pub fn test_group_by_with_wrong_phase() {
        let text = "GROUP BY columnA GROUP BY ";
        let (start, end, text) = parse_group_by_error(text);
        assert_eq!((start, end, text), (17, 17, "G".to_string()));
    }

    #[test]
    fn group_by_rejects_leading_comma() {
        let (start, end, text) = parse_group_by_error("GROUP BY , columnA");
        assert_eq!((start, end, text), (9, 9, ",".to_string()));
    }

    #[test]
    fn group_by_rejects_non_column_expression() {
        let (start, end, text) = parse_group_by_error("GROUP BY 1");
        assert_eq!((start, end, text), (10, 10, String::new()));
    }

    #[test]
    fn group_by_rejects_missing_keyword() {
        let mut parser = QueryParser::new("ORDER BY columnA");
        let result = GroupBy::parse(&mut parser);
        match result {
            Ok(groups) => panic!("expected invalid GROUP BY, got {groups:?}"),
            Err(err) => assert_eq!((err.start, err.end, err.text), (0, 0, "O".to_string())),
        }
    }
}
