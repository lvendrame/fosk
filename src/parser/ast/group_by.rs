use crate::parser::{ast::{Column, ScalarExpr}, ParseError, QueryParser};

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
    use crate::parser::{ast::{Column, GroupBy}, QueryParser};

    #[test]
    pub fn test_group_by() {
        let text = "GROUP BY columnA";

        let mut parser = QueryParser::new(text);
        parser.check_next_phase();

        let result = GroupBy::parse(&mut parser).expect("Failed to parse group by");

        assert_eq!(result.len(), 1);

        let expected_column = ["columnA"];

        for (i, column) in result.iter().enumerate() {
            match &column {
                Column::Name { name } => assert_eq!(name, expected_column[i]),
                _ => panic!(),
            }
        }
    }

    #[test]
    pub fn test_group_by_four() {
        let text = "GROUP BY columnA, columnB, columnC, columnD";

        let mut parser = QueryParser::new(text);
        parser.check_next_phase();

        let result = GroupBy::parse(&mut parser).expect("Failed to parse group by");

        assert_eq!(result.len(), 4);

        let expected_column = ["columnA", "columnB", "columnC", "columnD"];

        for (i, column) in result.iter().enumerate() {
            match &column {
                Column::Name { name } => assert_eq!(name, expected_column[i]),
                _ => panic!(),
            }
        }
    }

    #[test]
    pub fn test_group_by_four_with_spaces() {
        let text = "GROUP BY columnA , columnB , columnC , columnD";

        let mut parser = QueryParser::new(text);
        parser.check_next_phase();

        let result = GroupBy::parse(&mut parser).expect("Failed to parse group by");

        assert_eq!(result.len(), 4);

        let expected_column = ["columnA", "columnB", "columnC", "columnD"];

        for (i, column) in result.iter().enumerate() {
            match &column {
                Column::Name { name } => assert_eq!(name, expected_column[i]),
                _ => panic!(),
            }
        }
    }

    #[test]
    pub fn test_group_by_with_wrong_phase() {
        let text = "GROUP BY columnA GROUP BY ";

        let mut parser = QueryParser::new(text);
        parser.check_next_phase();

        let result = GroupBy::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "G");
                assert_eq!(err.start, 17);
                assert_eq!(err.end, 17);
            },
        }
    }
}
