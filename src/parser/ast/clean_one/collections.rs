use crate::parser::{ast::clean_one::Collection, ParseError, Phase, QueryParser};

pub struct Collections;

impl Collections {

    fn parse(parser: &mut QueryParser) -> Result<Vec<Collection>, ParseError> {

        if !parser.comparers.from.compare(parser) {
            return ParseError::new("Invalid select statement", parser.position, parser).err();
        }
        parser.jump(parser.comparers.from.length);

        parser.next_non_whitespace();

        let mut collections: Vec<Collection> = vec![];
        let mut can_consume = true;

        while parser.phase == Phase::Collections && !parser.eof() {
            if parser.current() == ',' {
                if can_consume {
                    return ParseError::new("Invalid select statement", parser.position, parser).err();
                }
                can_consume = true;
                parser.next();
            }


            if parser.current().is_whitespace() {
                parser.next_non_whitespace();
            } else if can_consume {
                collections.push(Collection::parse(parser)?);
                can_consume = false;
            }
        }

        if collections.is_empty() {
            return ParseError::new("Invalid select statement", parser.position, parser).err();
        }

        Ok(collections)
    }

}

#[cfg(test)]
mod tests {
    use crate::parser::{ast::clean_one::{Collections}, Phase, QueryParser};

    #[test]
    pub fn test_collections() {
        let text = "FROM table";

        let mut parser = QueryParser::new(text);
        parser.phase = Phase::Collections;

        let result = Collections::parse(&mut parser).expect("Failed to parse collections");

        assert_eq!(result.len(), 1);
    }

    #[test]
    pub fn test_collections_with_alias() {
        let text = "FROM tableA a";

        let mut parser = QueryParser::new(text);
        parser.phase = Phase::Collections;

        let result = Collections::parse(&mut parser).expect("Failed to parse collections");

        assert_eq!(result.len(), 1);
    }

    #[test]
    pub fn test_collections_two() {
        let text = "FROM tableA, tableB";

        let mut parser = QueryParser::new(text);
        parser.phase = Phase::Collections;

        let result = Collections::parse(&mut parser).expect("Failed to parse collections");

        assert_eq!(result.len(), 2);
    }

    #[test]
    pub fn test_collections_three() {
        let text = "FROM tableA, tableB b, tableC c";

        let mut parser = QueryParser::new(text);
        parser.phase = Phase::Collections;

        let result = Collections::parse(&mut parser).expect("Failed to parse collections");

        assert_eq!(result.len(), 3);
    }

    #[test]
    pub fn test_collections_three_with_where() {
        let text = "FROM tableA, tableB b, tableC c WHERE ";

        let mut parser = QueryParser::new(text);
        parser.phase = Phase::Collections;

        let result = Collections::parse(&mut parser).expect("Failed to parse collections");

        assert_eq!(result.len(), 3);
    }

    #[test]
    pub fn test_collections_with_wrong_comma() {
        let text = "FROM tableA, tableB b, , tableC c WHERE ";

        let mut parser = QueryParser::new(text);
        parser.phase = Phase::Collections;

        let result = Collections::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, ",");
                assert_eq!(err.start, 23);
                assert_eq!(err.end, 23);
            },
        };
    }

    #[test]
    pub fn test_collections_with_wrong_alias() {
        let text = "FROM tableA, tableB b c, tableC c WHERE ";

        let mut parser = QueryParser::new(text);
        parser.phase = Phase::Collections;

        let result = Collections::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "c");
                assert_eq!(err.start, 22);
                assert_eq!(err.end, 22);
            },
        };
    }

    #[test]
    pub fn test_collections_with_wrong_delimiter() {
        let text = "FROM tableA, tableB b, tableC c WHEE ";

        let mut parser = QueryParser::new(text);
        parser.phase = Phase::Collections;

        let result = Collections::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "W");
                assert_eq!(err.start, 32);
                assert_eq!(err.end, 32);
            },
        };
    }
}
