use crate::parser::{ast::TextCollector, ParseError, QueryParser};

#[derive(Debug, Clone, PartialEq)]
pub enum Collection {
    Table { name: String, alias: Option<String> },
    Query,
}

impl Collection {

    pub fn parse(parser: &mut QueryParser) -> Result<Collection, ParseError> {
        parser.next_non_whitespace();
        let name = TextCollector::collect_with_stopper(parser, |current| current == '.')?;

        parser.next_non_whitespace();


        let mut alias: Option<String> = None;
        let next_phase = parser.check_next_phase();
        if parser.current() != ',' && !next_phase && !parser.comparers.on.compare(parser) {
            alias = Some(TextCollector::collect(parser)?)
        }

        parser.next_non_whitespace();


        let next_phase = next_phase || parser.check_next_phase();

        let pivot = parser.position;
        if parser.current() == ',' || next_phase || parser.comparers.on.compare(parser) {
            return Ok(Collection::Table { name, alias });
        }

        ParseError::new("Invalid collection", pivot, parser).err()
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{ast::Collection, QueryParser};

    #[test]
    pub fn test_collection() {
        let text = "table";

        let mut parser = QueryParser::new(text);

        let result = Collection::parse(&mut parser).expect("Failed to parse collection");

        match result {
            Collection::Table { name, alias } => {
                assert_eq!(name, "table");
                assert_eq!(alias, None);
            },
            Collection::Query => panic!(),
        }
    }

    #[test]
    pub fn test_collection_with_alias() {
        let text = "table a";

        let mut parser = QueryParser::new(text);

        let result = Collection::parse(&mut parser).expect("Failed to parse collection");

        match result {
            Collection::Table { name, alias } => {
                assert_eq!(name, "table");
                assert_eq!(alias.unwrap(), "a");
            },
            Collection::Query => panic!(),
        }
    }

    #[test]
    pub fn test_collection_with_alias_and_comma() {
        let text = "table a,";

        let mut parser = QueryParser::new(text);

        let result = Collection::parse(&mut parser).expect("Failed to parse collection");

        match result {
            Collection::Table { name, alias } => {
                assert_eq!(name, "table");
                assert_eq!(alias.unwrap(), "a");
            },
            Collection::Query => panic!(),
        }
    }

    #[test]
    pub fn test_collection_with_alias_and_on() {
        let text = "table a ON ";

        let mut parser = QueryParser::new(text);

        let result = Collection::parse(&mut parser).expect("Failed to parse collection");

        match result {
            Collection::Table { name, alias } => {
                assert_eq!(name, "table");
                assert_eq!(alias.unwrap(), "a");
            },
            Collection::Query => panic!(),
        }
    }

    #[test]
    pub fn test_collection_with_alias_and_where() {
        let text = "table a WHERE ";

        let mut parser = QueryParser::new(text);

        let result = Collection::parse(&mut parser).expect("Failed to parse collection");

        match result {
            Collection::Table { name, alias } => {
                assert_eq!(name, "table");
                assert_eq!(alias.unwrap(), "a");
            },
            Collection::Query => panic!(),
        }
    }

    #[test]
    pub fn test_collection_with_alias_and_wrong_char() {
        let text = "table a were ";

        let mut parser = QueryParser::new(text);

        let result = Collection::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "w");
                assert_eq!(err.start, 8);
                assert_eq!(err.end, 8);
            },
        }
    }
}
