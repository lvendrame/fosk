use crate::parser::{ParseError, Predicate, QueryParser};

pub struct WhereParser;

impl WhereParser {
    pub fn parse(parser: &mut QueryParser) -> Result<Predicate, ParseError> {
        if !parser.comparers.r#where.compare(parser) {
            return ParseError::new("Invalid where", parser.position, parser).err();
        }
        parser.jump(parser.comparers.r#where.length);

        Predicate::parse(parser, false)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{ComparatorOp, Phase, Predicate, QueryParser, WhereParser};

    #[test]
    pub fn test_where() {
        let text = "WHERE tableA.columnA > 35";

        let mut parser = QueryParser::new(text);
        assert!(parser.check_next_phase());

        let result = WhereParser::parse(&mut parser).expect("Failed to parse where");

        match result {
            Predicate::Compare { left: _, op, right: _ } => assert_eq!(ComparatorOp::Gt, op),
            _ => panic!(),
        }
    }

    #[test]
    pub fn test_where_and() {
        let text = "WHERE tableA.columnA > 35 AND tableB.columnB is null AND tableB.columnC like '%thing%'";

        let mut parser = QueryParser::new(text);
        assert!(parser.check_next_phase());

        let result = WhereParser::parse(&mut parser).expect("Failed to parse where");

        match result {
            Predicate::And(predicates) => assert_eq!(predicates.len(), 3),
            _ => panic!(),
        }
    }

    #[test]
    pub fn test_where_wrong() {
        let text = "WHETE tableA.columnA > 35 AND tableB.columnB is null AND tableB.columnC like '%thing%'";

        let mut parser = QueryParser::new(text);
        parser.phase = Phase::Criteria;

        let result = WhereParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "W");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 0);
            },
        }
    }
}
