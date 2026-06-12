use crate::parser::{
    ParseError, Phase, QueryParser,
    ast::{Collection, Predicate},
};

#[derive(Debug, Clone, PartialEq)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
}

impl JoinType {
    pub fn parse(parser: &mut QueryParser) -> Result<JoinType, ParseError> {
        if parser.comparers.join.compare(parser) {
            parser.jump(parser.comparers.join.length);
            return Ok(JoinType::Inner);
        }

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

            let predicate = Predicate::parse(parser, false)?;

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
    use crate::parser::{
        QueryParser,
        ast::{Collection, Join, JoinType, Predicate},
    };

    fn parse_joins(text: &str) -> Vec<Join> {
        let mut parser = QueryParser::new(text);
        assert!(parser.check_next_phase());
        match Join::parse(&mut parser) {
            Ok(joins) => joins,
            Err(err) => panic!("expected join parse from {text:?}, got {err:?}"),
        }
    }

    fn table_name_and_alias(collection: &Collection) -> (&str, Option<&str>) {
        match collection {
            Collection::Table { name, alias } => (name.as_str(), alias.as_deref()),
            Collection::Query { .. } => panic!("expected table collection, got {collection:?}"),
        }
    }

    fn parse_join_error(text: &str) -> (usize, usize, String) {
        let mut parser = QueryParser::new(text);
        parser.check_next_phase();
        match Join::parse(&mut parser) {
            Ok(joins) => panic!("expected join error from {text:?}, got {joins:?}"),
            Err(err) => (err.start, err.end, err.text),
        }
    }

    #[test]
    pub fn test_inner_join() {
        let text = "INNER JOIN tableA ON tableA.columnA = tableB.columnA";
        let result = parse_joins(text);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].join_type, JoinType::Inner);
        assert_eq!(
            table_name_and_alias(&result[0].collection),
            ("tableA", None)
        );
        assert!(matches!(result[0].predicate, Predicate::Compare { .. }));
    }

    #[test]
    pub fn test_inner_join_two_predicates() {
        let text = "INNER JOIN tableA ON tableA.columnA = tableB.columnA AND tableA.columnB = tableB.columnB";
        let result = parse_joins(text);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].join_type, JoinType::Inner);
        assert_eq!(
            table_name_and_alias(&result[0].collection),
            ("tableA", None)
        );
        assert!(matches!(result[0].predicate, Predicate::And(_)));
    }

    #[test]
    pub fn test_inner_join_with_alias_and_two_predicates() {
        let text =
            "INNER JOIN tableA a ON a.columnA = tableB.columnA AND a.columnB = tableB.columnB";
        let result = parse_joins(text);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].join_type, JoinType::Inner);
        assert_eq!(
            table_name_and_alias(&result[0].collection),
            ("tableA", Some("a"))
        );
    }

    #[test]
    pub fn test_inner_join_and_left_join() {
        let text = r#"INNER JOIN tableA ON tableA.columnA = tableB.columnA
        LEFT JOIN tableC ON tableC.columnB = tableA.columnB"#;
        let result = parse_joins(text);
        assert_eq!(result.len(), 2);

        let expect_names = ["tableA", "tableC"];
        let expect_types = [JoinType::Inner, JoinType::Left];

        for (i, item) in result.iter().enumerate() {
            assert_eq!(item.join_type, expect_types[i]);
            assert_eq!(
                table_name_and_alias(&item.collection),
                (expect_names[i], None)
            );
        }
    }

    #[test]
    pub fn test_all_joins() {
        let text = r#"
        INNER JOIN tableA ON tableA.columnA = tableB.columnA
        LEFT JOIN tableC ON tableC.columnB = tableA.columnB
        RIGHT JOIN tableD ON tableD.columnB = tableC.columnB
        FULL JOIN tableE ON tableE.columnB = tableA.columnB
        "#;

        let result = parse_joins(text);
        assert_eq!(result.len(), 4);

        let expect_names = ["tableA", "tableC", "tableD", "tableE"];
        let expect_types = [
            JoinType::Inner,
            JoinType::Left,
            JoinType::Right,
            JoinType::Full,
        ];

        for (i, item) in result.iter().enumerate() {
            assert_eq!(item.join_type, expect_types[i]);
            assert_eq!(
                table_name_and_alias(&item.collection),
                (expect_names[i], None)
            );
        }
    }

    #[test]
    fn short_join_keyword_is_inner_join() {
        let result = parse_joins("JOIN tableA ON tableA.id = tableB.id");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].join_type, JoinType::Inner);
    }

    #[test]
    fn join_rejects_missing_on_clause() {
        let (start, end, text) = parse_join_error("INNER JOIN tableA WHERE tableA.id = tableB.id");
        assert_eq!((start, end, text), (18, 18, "W".to_string()));
    }

    #[test]
    fn join_type_rejects_unknown_keyword() {
        let mut parser = QueryParser::new("OUTER JOIN tableA ON tableA.id = tableB.id");
        parser.phase = crate::parser::Phase::Joins;
        let result = JoinType::parse(&mut parser);
        match result {
            Ok(join_type) => panic!("expected invalid join type, got {join_type:?}"),
            Err(err) => assert_eq!((err.start, err.end, err.text), (0, 0, "O".to_string())),
        }
    }
}
