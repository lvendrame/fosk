use crate::parser::{ParseError, Phase, QueryParser, ScalarExpr};

#[derive(Debug, Clone, PartialEq)]
pub struct OrderBy {
    pub expr: ScalarExpr,
    pub ascending: bool,
}

impl OrderBy {
    pub fn parse_single(parser: &mut QueryParser) -> Result<Self, ParseError> {
        let expr = ScalarExpr::parse(parser, false)?;
        parser.next_non_whitespace();
        if parser.current() == ',' || parser.check_next_phase() {
            return Ok(Self { expr, ascending: true });
        }

        if parser.comparers.asc.compare(parser) {
            parser.jump(parser.comparers.asc.length);
            parser.check_next_phase();
            return Ok(OrderBy { expr, ascending: true });
        }

        if parser.comparers.desc.compare(parser) {
            parser.jump(parser.comparers.desc.length);
            parser.check_next_phase();
            return Ok(OrderBy { expr, ascending: false });
        }

        ParseError::new("Invalid order by", parser.position, parser).err()
    }

    pub fn parse(parser: &mut QueryParser) -> Result<Vec<Self>, ParseError> {
        if !parser.comparers.order_by.compare(parser) {
            return ParseError::new("Invalid order by", parser.position, parser).err();
        }
        parser.jump(parser.comparers.order_by.length);

        let mut orders: Vec<Self> = vec![];
        let mut can_consume = true;
        while parser.phase == Phase::OrderBy {
            if parser.current() == ',' {
                if can_consume {
                    return ParseError::new("Invalid order by", parser.position, parser).err();
                }
                can_consume = true;
                parser.next();
                parser.next_non_whitespace();
            }
            if can_consume {
                orders.push(Self::parse_single(parser)?);
                parser.next_non_whitespace();
                can_consume = false;
            } else {
                return ParseError::new("Invalid order by", parser.position, parser).err();
            }
        }

        Ok(orders)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{Column, OrderBy, QueryParser, ScalarExpr};

    #[test]
    pub fn test_order_by_single() {
        let text = "tableA.columnA";

        let mut parser = QueryParser::new(text);

        let result = OrderBy::parse_single(&mut parser).expect("Failed to parse order by");

        assert!(result.ascending);

        match result.expr {
            ScalarExpr::Column(column) => {
                match column {
                    Column::WithCollection { collection, name } => {
                        assert_eq!(collection, "tableA");
                        assert_eq!(name, "columnA");
                    },
                    Column::Name { name: _ } => panic!(),
                }
            },
            _ => panic!(),
        }
    }

    #[test]
    pub fn test_order_by_single_asc() {
        let text = "tableA.columnA ASC";

        let mut parser = QueryParser::new(text);

        let result = OrderBy::parse_single(&mut parser).expect("Failed to parse order by");

        assert!(result.ascending);

        match result.expr {
            ScalarExpr::Column(column) => {
                match column {
                    Column::WithCollection { collection, name } => {
                        assert_eq!(collection, "tableA");
                        assert_eq!(name, "columnA");
                    },
                    Column::Name { name: _ } => panic!(),
                }
            },
            _ => panic!(),
        }
    }

    #[test]
    pub fn test_order_by_single_desc() {
        let text = "columnA DESC";

        let mut parser = QueryParser::new(text);

        let result = OrderBy::parse_single(&mut parser).expect("Failed to parse order by");

        assert!(!result.ascending);

        match result.expr {
            ScalarExpr::Column(column) => {
                match column {
                    Column::Name { name } => {
                        assert_eq!(name, "columnA");
                    },
                    Column::WithCollection { collection: _, name: _ } => panic!(),
                }
            },
            _ => panic!(),
        }
    }

    #[test]
    pub fn test_order_by_single_with_comma() {
        let text = "tableA.columnA ASC,";

        let mut parser = QueryParser::new(text);

        let result = OrderBy::parse_single(&mut parser).expect("Failed to parse order by");

        assert!(result.ascending);

        match result.expr {
            ScalarExpr::Column(column) => {
                match column {
                    Column::WithCollection { collection, name } => {
                        assert_eq!(collection, "tableA");
                        assert_eq!(name, "columnA");
                    },
                    Column::Name { name: _ } => panic!(),
                }
            },
            _ => panic!(),
        }
    }

    #[test]
    pub fn test_order_by_single_with_next_phase() {
        let text = "tableA.columnA ASC ORDER BY";

        let mut parser = QueryParser::new(text);

        let result = OrderBy::parse_single(&mut parser).expect("Failed to parse order by");

        assert!(result.ascending);

        match result.expr {
            ScalarExpr::Column(column) => {
                match column {
                    Column::WithCollection { collection, name } => {
                        assert_eq!(collection, "tableA");
                        assert_eq!(name, "columnA");
                    },
                    Column::Name { name: _ } => panic!(),
                }
            },
            _ => panic!(),
        }
    }

    #[test]
    pub fn test_order_by() {
        let text = "ORDER BY columnA DESC";

        let mut parser = QueryParser::new(text);
        parser.check_next_phase();

        let result = OrderBy::parse(&mut parser).expect("Failed to parse order by");

        assert_eq!(result.len(), 1);

        let expected_order = [false];
        let expected_column = ["columnA"];

        for (i, order_by) in result.iter().enumerate() {
            assert_eq!(order_by.ascending, expected_order[i]);

            match &order_by.expr {
                ScalarExpr::Column(column) => {
                    match column {
                        Column::Name { name } => {
                            assert_eq!(name, expected_column[i]);
                        },
                        Column::WithCollection { collection: _, name: _ } => panic!(),
                    }
                },
                _ => panic!(),
            }
        }

    }

    #[test]
    pub fn test_order_by_four() {
        let text = "ORDER BY columnA DESC, columnB ASC, columnC, columnD";

        let mut parser = QueryParser::new(text);
        parser.check_next_phase();

        let result = OrderBy::parse(&mut parser).expect("Failed to parse order by");

        assert_eq!(result.len(), 4);

        let expected_order = [false, true, true, true];
        let expected_column = ["columnA", "columnB", "columnC", "columnD"];

        for (i, order_by) in result.iter().enumerate() {
            assert_eq!(order_by.ascending, expected_order[i]);

            match &order_by.expr {
                ScalarExpr::Column(column) => {
                    match column {
                        Column::Name { name } => {
                            assert_eq!(name, expected_column[i]);
                        },
                        Column::WithCollection { collection: _, name: _ } => panic!(),
                    }
                },
                _ => panic!(),
            }
        }

    }

    #[test]
    pub fn test_order_by_four_with_spaces() {
        let text = "ORDER BY columnA DESC , columnB ASC , columnC , columnD";

        let mut parser = QueryParser::new(text);
        parser.check_next_phase();

        let result = OrderBy::parse(&mut parser).expect("Failed to parse order by");

        assert_eq!(result.len(), 4);

        let expected_order = [false, true, true, true];
        let expected_column = ["columnA", "columnB", "columnC", "columnD"];

        for (i, order_by) in result.iter().enumerate() {
            assert_eq!(order_by.ascending, expected_order[i]);

            match &order_by.expr {
                ScalarExpr::Column(column) => {
                    match column {
                        Column::Name { name } => {
                            assert_eq!(name, expected_column[i]);
                        },
                        Column::WithCollection { collection: _, name: _ } => panic!(),
                    }
                },
                _ => panic!(),
            }
        }

    }

    #[test]
    pub fn test_order_by_with_other_phase() {
        let text = "ORDER BY columnA DESC GROUP BY ";

        let mut parser = QueryParser::new(text);
        parser.check_next_phase();

        let result = OrderBy::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "G");
                assert_eq!(err.start, 22);
                assert_eq!(err.end, 22);
            },
        }
    }
}
