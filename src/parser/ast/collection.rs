use crate::parser::{ast::{Query, TextCollector}, ParseError, QueryParser};

#[derive(Clone, PartialEq)]
pub enum Collection {
    Table { name: String, alias: Option<String> },
    Query { query: Box<Query>, alias: Option<String> },
}

impl Collection {

    pub fn parse(parser: &mut QueryParser) -> Result<Collection, ParseError> {
        parser.next_non_whitespace();
        if parser.current() == '(' {
            return Self::parse_query(parser);
        }

        let name = TextCollector::collect_with_stopper(parser, |current| current == '.')?;

        parser.next_non_whitespace();


        let mut alias: Option<String> = None;
        let next_phase = parser.check_next_phase();
        if parser.current() != ',' && !next_phase && !parser.comparers.on.compare(parser) {
            if parser.comparers.alias.compare(parser) {
                parser.jump(parser.comparers.alias.length);
                parser.next_non_whitespace();
            }
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

    fn parse_query(parser: &mut QueryParser) -> Result<Collection, ParseError> {
        let pivot = parser.position;
        let query_text = Self::collect_parenthesized_query(parser)?;
        let query = Query::try_from(query_text.trim())
            .map_err(|_| ParseError::new("Invalid subquery", pivot, parser))?;

        parser.next_non_whitespace();
        if parser.comparers.alias.compare(parser) {
            parser.jump(parser.comparers.alias.length);
            parser.next_non_whitespace();
        }

        if parser.eof()
            || parser.current() == ','
            || parser.comparers.on.compare(parser)
            || parser.check_next_phase()
        {
            return ParseError::new("Subquery requires an alias", parser.position, parser).err();
        }

        let alias = TextCollector::collect(parser)?;
        parser.next_non_whitespace();

        let next_phase = parser.check_next_phase();
        let pivot = parser.position;
        if parser.current() == ',' || next_phase || parser.comparers.on.compare(parser) {
            return Ok(Collection::Query {
                query: Box::new(query),
                alias: Some(alias),
            });
        }

        ParseError::new("Invalid collection", pivot, parser).err()
    }

    fn collect_parenthesized_query(parser: &mut QueryParser) -> Result<String, ParseError> {
        let pivot = parser.position;
        parser.next();
        let start = parser.position;
        let mut depth = 1usize;
        let mut in_single_quote = false;
        let mut in_double_quote = false;

        while !parser.eof() {
            let current = parser.current();
            match current {
                '\'' if !in_double_quote => in_single_quote = !in_single_quote,
                '"' if !in_single_quote => in_double_quote = !in_double_quote,
                '(' if !in_single_quote && !in_double_quote => depth += 1,
                ')' if !in_single_quote && !in_double_quote => {
                    depth -= 1;
                    if depth == 0 {
                        let end = parser.position;
                        let text = parser.text_from_range(start, end);
                        parser.next();
                        return Ok(text);
                    }
                }
                _ => {}
            }
            parser.next();
        }

        ParseError::new("Unclosed subquery", pivot, parser).err()
    }
}

use std::fmt;

impl fmt::Display for Collection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Collection::Table { name, alias } => {
                if let Some(a) = alias {
                    write!(f, "Table({} as {})", name, a)
                } else {
                    write!(f, "Table({})", name)
                }
            }
            Collection::Query { alias, .. } => {
                if let Some(a) = alias {
                    write!(f, "Query as {}", a)
                } else {
                    write!(f, "Query")
                }
            },
        }
    }
}

impl fmt::Debug for Collection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Collection({})", self)
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
            Collection::Query { .. } => panic!(),
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
            Collection::Query { .. } => panic!(),
        }
    }

    #[test]
    pub fn test_collection_with_as_alias() {
        let text = "table AS a";

        let mut parser = QueryParser::new(text);

        let result = Collection::parse(&mut parser).expect("Failed to parse collection");

        match result {
            Collection::Table { name, alias } => {
                assert_eq!(name, "table");
                assert_eq!(alias.as_deref(), Some("a"));
            },
            Collection::Query { .. } => panic!(),
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
            Collection::Query { .. } => panic!(),
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
            Collection::Query { .. } => panic!(),
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
            Collection::Query { .. } => panic!(),
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

    #[test]
    fn display_and_debug_format_table_variants() {
        let table = Collection::Table {
            name: "people".to_string(),
            alias: None,
        };
        let aliased = Collection::Table {
            name: "people".to_string(),
            alias: Some("p".to_string()),
        };

        assert_eq!(table.to_string(), "Table(people)");
        assert_eq!(format!("{:?}", table), "Collection(Table(people))");
        assert_eq!(aliased.to_string(), "Table(people as p)");
        assert_eq!(format!("{:?}", aliased), "Collection(Table(people as p))");
    }

    #[test]
    fn display_and_debug_format_query_variant() {
        let query = crate::parser::ast::Query::try_from("SELECT id FROM people").unwrap();
        let collection = Collection::Query {
            query: Box::new(query),
            alias: Some("p".to_string()),
        };

        assert_eq!(collection.to_string(), "Query as p");
        assert_eq!(format!("{:?}", collection), "Collection(Query as p)");
    }

    #[test]
    fn display_formats_unaliased_query_variant() {
        let query = crate::parser::ast::Query::try_from("SELECT id FROM people").unwrap();
        let collection = Collection::Query {
            query: Box::new(query),
            alias: None,
        };

        assert_eq!(collection.to_string(), "Query");
    }

    #[test]
    fn parses_subquery_with_required_alias() {
        let mut parser = QueryParser::new("(SELECT name FROM people) p");

        let result = Collection::parse(&mut parser).expect("subquery should parse");

        match result {
            Collection::Query { query, alias } => {
                assert_eq!(alias.as_deref(), Some("p"));
                assert_eq!(query.projection.len(), 1);
                assert_eq!(query.collections.len(), 1);
            },
            other => panic!("expected query collection, got {other:?}"),
        }
    }

    #[test]
    fn parses_subquery_with_as_alias() {
        let mut parser = QueryParser::new("(SELECT name FROM people) AS p");

        let result = Collection::parse(&mut parser).expect("subquery with AS should parse");

        match result {
            Collection::Query { query, alias } => {
                assert_eq!(alias.as_deref(), Some("p"));
                assert_eq!(query.collections.len(), 1);
            },
            other => panic!("expected query collection, got {other:?}"),
        }
    }

    #[test]
    fn subquery_without_alias_is_rejected() {
        let mut parser = QueryParser::new("(SELECT name FROM people) WHERE ");

        let result = Collection::parse(&mut parser);

        assert!(result.is_err(), "subquery aliases are required");
    }

    #[test]
    fn invalid_subquery_body_is_rejected() {
        let mut parser = QueryParser::new("(SELECT FROM) p");

        let result = Collection::parse(&mut parser);

        assert!(result.is_err(), "invalid inner query should be rejected");
    }

    #[test]
    fn unclosed_subquery_is_rejected() {
        let mut parser = QueryParser::new("(SELECT name FROM people");

        let result = Collection::parse(&mut parser);

        assert!(result.is_err(), "unclosed subquery should be rejected");
    }

    #[test]
    fn subquery_with_trailing_invalid_token_is_rejected() {
        let mut parser = QueryParser::new("(SELECT name FROM people) p were ");

        let result = Collection::parse(&mut parser);

        assert!(result.is_err(), "unexpected token after subquery alias should fail");
    }

    #[test]
    fn subquery_collector_ignores_parentheses_inside_string_literals() {
        let mut parser = QueryParser::new(r#"(SELECT ")" AS marker FROM people) p"#);

        let result = Collection::parse(&mut parser).expect("quoted paren should not close subquery");

        match result {
            Collection::Query { query, alias } => {
                assert_eq!(alias.as_deref(), Some("p"));
                assert_eq!(query.projection[0].alias.as_deref(), Some("marker"));
            },
            other => panic!("expected query collection, got {other:?}"),
        }
    }
}
