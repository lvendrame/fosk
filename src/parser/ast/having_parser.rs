use crate::parser::{ast::Predicate, ParseError, QueryParser};

pub struct HavingParser;

impl HavingParser {
    pub fn parse(parser: &mut QueryParser) -> Result<Predicate, ParseError> {
        if !parser.comparers.having.compare(parser) {
            return ParseError::new("Invalid having", parser.position, parser).err();
        }
        parser.jump(parser.comparers.having.length);

        Predicate::parse(parser, true)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{ast::{ComparatorOp, HavingParser, Predicate}, Phase, QueryParser};

    #[test]
    pub fn test_having() {
        let text = "HAVING COUNT(*) > 35";

        let mut parser = QueryParser::new(text);
        assert!(parser.check_next_phase());

        let result = HavingParser::parse(&mut parser).expect("Failed to parse having");

        match result {
            Predicate::Compare { left: _, op, right: _ } => assert_eq!(ComparatorOp::Gt, op),
            _ => panic!(),
        }
    }

    #[test]
    pub fn test_having_and() {
        let text = "HAVING tableA.columnA > 35 AND tableB.columnB is null AND tableB.columnC like '%thing%'";

        let mut parser = QueryParser::new(text);
        assert!(parser.check_next_phase());

        let result = HavingParser::parse(&mut parser).expect("Failed to parse having");

        match result {
            Predicate::And(predicates) => assert_eq!(predicates.len(), 3),
            _ => panic!(),
        }
    }

    #[test]
    pub fn test_having_wrong() {
        let text = "HAVEN tableA.columnA > 35 AND tableB.columnB is null AND tableB.columnC like '%thing%'";

        let mut parser = QueryParser::new(text);
        parser.phase = Phase::Having;

        let result = HavingParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "H");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 0);
            },
        }
    }
}
