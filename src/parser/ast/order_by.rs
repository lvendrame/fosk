use crate::parser::{ParseError, Phase, QueryParser, ast::ScalarExpr};

#[derive(Clone, PartialEq)]
pub struct OrderBy {
    pub expr: ScalarExpr,
    pub ascending: bool,
}

impl OrderBy {
    pub fn parse_single(parser: &mut QueryParser) -> Result<Self, ParseError> {
        let expr = ScalarExpr::parse(parser, false)?;
        parser.next_non_whitespace();
        if parser.current() == ',' || parser.check_next_phase() {
            return Ok(Self {
                expr,
                ascending: true,
            });
        }

        if parser.comparers.asc.compare(parser) {
            parser.jump(parser.comparers.asc.length);
            parser.check_next_phase();
            return Ok(OrderBy {
                expr,
                ascending: true,
            });
        }

        if parser.comparers.desc.compare(parser) {
            parser.jump(parser.comparers.desc.length);
            parser.check_next_phase();
            return Ok(OrderBy {
                expr,
                ascending: false,
            });
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

use std::fmt;

impl fmt::Display for OrderBy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ascending {
            write!(f, "{} ASC", self.expr)
        } else {
            write!(f, "{} DESC", self.expr)
        }
    }
}

impl fmt::Debug for OrderBy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OrderBy({})", self)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{
        QueryParser,
        ast::{Column, OrderBy, ScalarExpr},
    };

    fn parse_single(text: &str) -> OrderBy {
        let mut parser = QueryParser::new(text);
        match OrderBy::parse_single(&mut parser) {
            Ok(order) => order,
            Err(err) => panic!("expected ORDER BY item from {text:?}, got {err:?}"),
        }
    }

    fn parse_order_by(text: &str) -> Vec<OrderBy> {
        let mut parser = QueryParser::new(text);
        parser.check_next_phase();
        match OrderBy::parse(&mut parser) {
            Ok(orders) => orders,
            Err(err) => panic!("expected ORDER BY from {text:?}, got {err:?}"),
        }
    }

    fn parse_order_by_error(text: &str) -> (usize, usize, String) {
        let mut parser = QueryParser::new(text);
        parser.check_next_phase();
        match OrderBy::parse(&mut parser) {
            Ok(orders) => panic!("expected ORDER BY error from {text:?}, got {orders:?}"),
            Err(err) => (err.start, err.end, err.text),
        }
    }

    fn column_path(expr: &ScalarExpr) -> (&str, Option<&str>) {
        match expr {
            ScalarExpr::Column(Column::Name { name }) => (name.as_str(), None),
            ScalarExpr::Column(Column::WithCollection { collection, name }) => {
                (name.as_str(), Some(collection.as_str()))
            }
            other => panic!("expected ORDER BY column, got {other:?}"),
        }
    }

    #[test]
    pub fn test_order_by_single() {
        let result = parse_single("tableA.columnA");
        assert!(result.ascending);
        assert_eq!(column_path(&result.expr), ("columnA", Some("tableA")));
    }

    #[test]
    pub fn test_order_by_single_asc() {
        let result = parse_single("tableA.columnA ASC");
        assert!(result.ascending);
        assert_eq!(column_path(&result.expr), ("columnA", Some("tableA")));
    }

    #[test]
    pub fn test_order_by_single_desc() {
        let result = parse_single("columnA DESC");
        assert!(!result.ascending);
        assert_eq!(column_path(&result.expr), ("columnA", None));
    }

    #[test]
    pub fn test_order_by_single_with_comma() {
        let result = parse_single("tableA.columnA ASC,");
        assert!(result.ascending);
        assert_eq!(column_path(&result.expr), ("columnA", Some("tableA")));
    }

    #[test]
    pub fn test_order_by_single_with_next_phase() {
        let result = parse_single("tableA.columnA ASC ORDER BY");
        assert!(result.ascending);
        assert_eq!(column_path(&result.expr), ("columnA", Some("tableA")));
    }

    #[test]
    pub fn test_order_by() {
        let result = parse_order_by("ORDER BY columnA DESC");
        assert_eq!(result.len(), 1);
        assert!(!result[0].ascending);
        assert_eq!(column_path(&result[0].expr), ("columnA", None));
    }

    #[test]
    pub fn test_order_by_four() {
        let result = parse_order_by("ORDER BY columnA DESC, columnB ASC, columnC, columnD");
        assert_eq!(result.len(), 4);

        let expected_order = [false, true, true, true];
        let expected_column = ["columnA", "columnB", "columnC", "columnD"];

        for (i, order_by) in result.iter().enumerate() {
            assert_eq!(order_by.ascending, expected_order[i]);
            assert_eq!(column_path(&order_by.expr), (expected_column[i], None));
        }
    }

    #[test]
    pub fn test_order_by_four_with_spaces() {
        let result = parse_order_by("ORDER BY columnA DESC , columnB ASC , columnC , columnD");
        assert_eq!(result.len(), 4);

        let expected_order = [false, true, true, true];
        let expected_column = ["columnA", "columnB", "columnC", "columnD"];

        for (i, order_by) in result.iter().enumerate() {
            assert_eq!(order_by.ascending, expected_order[i]);
            assert_eq!(column_path(&order_by.expr), (expected_column[i], None));
        }
    }

    #[test]
    pub fn test_order_by_with_other_phase() {
        let text = "ORDER BY columnA DESC GROUP BY ";
        let (start, end, text) = parse_order_by_error(text);
        assert_eq!((start, end, text), (22, 22, "G".to_string()));
    }

    #[test]
    fn order_by_keeps_existing_empty_column_behavior() {
        let leading_comma = parse_order_by("ORDER BY , columnA");
        assert_eq!(leading_comma.len(), 2);
        assert_eq!(column_path(&leading_comma[0].expr), ("", None));
        assert_eq!(column_path(&leading_comma[1].expr), ("columnA", None));

        let double_comma = parse_order_by("ORDER BY columnA, , columnB");
        assert_eq!(double_comma.len(), 3);
        assert_eq!(column_path(&double_comma[0].expr), ("columnA", None));
        assert_eq!(column_path(&double_comma[1].expr), ("", None));
        assert_eq!(column_path(&double_comma[2].expr), ("columnB", None));
    }

    #[test]
    fn order_by_rejects_trailing_comma_before_other_phase() {
        let (start, end, text) = parse_order_by_error("ORDER BY columnA, GROUP BY");
        assert_eq!((start, end, text), (24, 24, "B".to_string()));
    }

    #[test]
    fn order_by_rejects_missing_keyword() {
        let mut parser = QueryParser::new("GROUP BY columnA");
        let result = OrderBy::parse(&mut parser);
        match result {
            Ok(orders) => panic!("expected invalid ORDER BY, got {orders:?}"),
            Err(err) => assert_eq!((err.start, err.end, err.text), (0, 0, "G".to_string())),
        }
    }

    #[test]
    fn order_by_rejects_invalid_direction() {
        let mut parser = QueryParser::new("columnA SIDEWAYS");
        let result = OrderBy::parse_single(&mut parser);
        match result {
            Ok(order) => panic!("expected invalid order direction, got {order:?}"),
            Err(err) => assert_eq!((err.start, err.end, err.text), (8, 8, "S".to_string())),
        }
    }

    #[test]
    fn display_and_debug_format_direction() {
        let asc = OrderBy {
            expr: ScalarExpr::Column(Column::Name {
                name: "age".to_string(),
            }),
            ascending: true,
        };
        let desc = OrderBy {
            expr: ScalarExpr::Column(Column::Name {
                name: "age".to_string(),
            }),
            ascending: false,
        };

        assert_eq!(asc.to_string(), "col: age ASC");
        assert_eq!(format!("{:?}", asc), "OrderBy(col: age ASC)");
        assert_eq!(desc.to_string(), "col: age DESC");
        assert_eq!(format!("{:?}", desc), "OrderBy(col: age DESC)");
    }
}
