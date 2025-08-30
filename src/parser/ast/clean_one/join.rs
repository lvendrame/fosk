use crate::parser::{ast::clean_one::{Collection, Predicate}, ParseError, Phase, QueryParser};

#[derive(Debug, Clone, PartialEq)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
}

impl JoinType {
    pub fn parse(parser: &mut QueryParser) -> Result<JoinType, ParseError> {
        if parser.comparers.inner_join.compare(parser) {
            parser.jump(parser.comparers.inner_join.length);
            return Ok(JoinType::Inner);
        }

        if parser.comparers.left_join.compare(parser) {
            parser.jump(parser.comparers.left_join.length);
            return Ok(JoinType::Left);
        }

        if parser.comparers.right_join.compare(parser) {
            parser.jump(parser.comparers.right_join.length);
            return Ok(JoinType::Right);
        }

        if parser.comparers.full_join.compare(parser) {
            parser.jump(parser.comparers.full_join.length);
            return Ok(JoinType::Full);
        }

        ParseError::new("Invalid Join type", parser.position, parser).err()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Join {
    pub join_type: JoinType,
    pub collection: Collection,
    pub predicate: Predicate,
}

impl Join {
    pub fn parse(parser: &mut QueryParser) -> Result<Vec<Join>, ParseError> {
        let mut joins: Vec<Join> = vec![];
        while parser.phase == Phase::Joins {
            let join_type = JoinType::parse(parser)?;
            let collection = Collection::parse(parser)?;

            if parser.comparers.on.compare(parser) {
                parser.jump(parser.comparers.on.length);
            } else {
                return ParseError::new("Invalid join statement", parser.position, parser).err();
            }

            let predicate = Predicate::parse(parser)?;

            joins.push(Join {
                join_type,
                collection,
                predicate,
            });
        }

        Ok(joins)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{ast::clean_one::{Collection, Join, JoinType}, QueryParser};

    #[test]
    pub fn test_inner_join() {
        let text = "INNER JOIN tableA ON tableA.columnA = tableB.columnA";

        let mut parser = QueryParser::new(text);
        assert!(parser.check_next_phase());

        let result = Join::parse(&mut parser).expect("Failed to parse join");

        assert_eq!(result.len(), 1);

        match result.first() {
            Some(first) => match first.join_type {
                JoinType::Inner => {
                    match &first.collection {
                        Collection::Table { name, alias } => {
                            assert_eq!(name, "tableA");
                            assert!(alias.is_none());
                        },
                        Collection::Query => todo!(),
                    }
                },
                _ => panic!(),
            },
            None => panic!(),
        }
    }

    #[test]
    pub fn test_inner_join_two_predicates() {
        let text = "INNER JOIN tableA ON tableA.columnA = tableB.columnA AND tableA.columnB = tableB.columnB";

        let mut parser = QueryParser::new(text);
        assert!(parser.check_next_phase());

        let result = Join::parse(&mut parser).expect("Failed to parse join");

        assert_eq!(result.len(), 1);

        match result.first() {
            Some(first) => match first.join_type {
                JoinType::Inner => {
                    match &first.collection {
                        Collection::Table { name, alias } => {
                            assert_eq!(name, "tableA");
                            assert!(alias.is_none());
                        },
                        Collection::Query => todo!(),
                    }
                },
                _ => panic!(),
            },
            None => panic!(),
        }
    }

    #[test]
    pub fn test_inner_join_with_alias_and_two_predicates() {
        let text = "INNER JOIN tableA a ON a.columnA = tableB.columnA AND a.columnB = tableB.columnB";

        let mut parser = QueryParser::new(text);
        assert!(parser.check_next_phase());

        let result = Join::parse(&mut parser).expect("Failed to parse join");

        assert_eq!(result.len(), 1);

        match result.first() {
            Some(first) => match first.join_type {
                JoinType::Inner => {
                    match &first.collection {
                        Collection::Table { name, alias } => {
                            assert_eq!(name, "tableA");
                            assert_eq!(alias.clone().unwrap(), "a");
                        },
                        Collection::Query => todo!(),
                    }
                },
                _ => panic!(),
            },
            None => panic!(),
        }
    }



    #[test]
    pub fn test_inner_join_and_left_join() {
        let text = r#"INNER JOIN tableA ON tableA.columnA = tableB.columnA
        LEFT JOIN tableC ON tableC.columnB = tableA.columnB"#;

        let mut parser = QueryParser::new(text);
        assert!(parser.check_next_phase());

        let result = Join::parse(&mut parser).expect("Failed to parse join");

        assert_eq!(result.len(), 2);

        match result.first() {
            Some(first) => match first.join_type {
                JoinType::Inner => {
                    match &first.collection {
                        Collection::Table { name, alias } => {
                            assert_eq!(name, "tableA");
                            assert!(alias.is_none());
                        },
                        Collection::Query => todo!(),
                    }
                },
                _ => panic!(),
            },
            None => panic!(),
        }
    }
}


