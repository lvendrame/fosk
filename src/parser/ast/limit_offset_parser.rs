use crate::parser::{ast::{Literal, ScalarExpr}, ParseError, QueryParser};

pub struct LimitAndOffsetParser;

impl LimitAndOffsetParser {
    pub fn parse(parser: &mut QueryParser) -> Result<(Option<i64>, Option<i64>), ParseError> {
        let mut limit = None;
        let mut offset = None;

        while !parser.check_next_phase() {
            if parser.comparers.limit.compare(parser) {
                parser.jump(parser.comparers.limit.length);
                let value = ScalarExpr::parse(parser, false)?;
                match value {
                    ScalarExpr::Literal(Literal::Int(value)) => limit = Some(value),
                    _ => return ParseError::new("Invalid limit", parser.get_initial_sequence_pos(), parser).err(),
                }
            } else if parser.comparers.offset.compare(parser) {
                parser.jump(parser.comparers.offset.length);
                let value = ScalarExpr::parse(parser, false)?;
                match value {
                    ScalarExpr::Literal(Literal::Int(value)) => offset = Some(value),
                    _ => return ParseError::new("Invalid offset", parser.get_initial_sequence_pos() , parser).err(),
                }
            } else {
                return ParseError::new("Invalid offset", parser.get_initial_sequence_pos() , parser).err();
            }
        }

        Ok((limit, offset))
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{ast::{LimitAndOffsetParser}, QueryParser};

    #[test]
    pub fn test_limit() {
        let text = "LIMIT 10";

        let mut parser = QueryParser::new(text);
        parser.check_next_phase();

        let (limit, offset) = LimitAndOffsetParser::parse(&mut parser).expect("Failed to parse limit and offset");

        assert_eq!(limit.unwrap(), 10);
        assert!(offset.is_none());
    }

    #[test]
    pub fn test_offset() {
        let text = "OFFSET 10";

        let mut parser = QueryParser::new(text);
        parser.check_next_phase();

        let (limit, offset) = LimitAndOffsetParser::parse(&mut parser).expect("Failed to parse limit and offset");

        assert!(limit.is_none());
        assert_eq!(offset.unwrap(), 10);
    }

    #[test]
    pub fn test_offset_and_limit() {
        let text = "OFFSET 20 LIMIT 30";

        let mut parser = QueryParser::new(text);
        parser.check_next_phase();

        let (limit, offset) = LimitAndOffsetParser::parse(&mut parser).expect("Failed to parse limit and offset");

        assert_eq!(limit.unwrap(), 30);
        assert_eq!(offset.unwrap(), 20);
    }

    #[test]
    pub fn test_offset_and_limit_wrong_limit() {
        let text = "OFFSET 20 LIMIT AB";

        let mut parser = QueryParser::new(text);
        parser.check_next_phase();

        let result = LimitAndOffsetParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "AB");
                assert_eq!(err.start, 16);
                assert_eq!(err.end, 18);
            },
        }
    }

    #[test]
    pub fn test_offset_and_limit_wrong_offset() {
        let text = "OFFSET 20 30 LIMIT AB";

        let mut parser = QueryParser::new(text);
        parser.check_next_phase();

        let result = LimitAndOffsetParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "3");
                assert_eq!(err.start, 10);
                assert_eq!(err.end, 10);
            },
        }
    }

    #[test]
    pub fn test_offset_and_limit_wrong_phase() {
        let text = "OFFSET 20 LIMIT 60 GROUP BY ";

        let mut parser = QueryParser::new(text);
        parser.check_next_phase();

        let result = LimitAndOffsetParser::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "G");
                assert_eq!(err.start, 19);
                assert_eq!(err.end, 19);
            },
        }
    }
}
