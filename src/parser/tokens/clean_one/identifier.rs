use crate::parser::{ParseError, QueryComparers, QueryParser};

#[derive(Debug)]
pub enum Identifier {
    Name { name: String },
    WithCollection { collection: String, name: String },
}

impl Identifier {
    pub fn parse(parser: &mut QueryParser) -> Result<Identifier, ParseError> {
        let mut pivot = parser.position;
        let mut collection: Option<String> = None;

        if parser.current().is_ascii_digit() {
            return Err(ParseError::new("Invalid identifier", pivot, parser));
        }

        while !parser.eof() && parser.current() != '(' && !QueryComparers::is_full_block_delimiter(parser.current()) {
            let current = parser.current();
            if current == '.' {
                if collection.is_some() {
                    return Err(ParseError::new("Invalid identifier", pivot, parser));
                }
                collection = Some(parser.text_from_pivot(pivot));
                pivot = parser.position + 1;
            } else if !current.is_ascii_alphanumeric() && current != '_' {
                return Err(ParseError::new("Invalid identifier", pivot, parser));
            }
            parser.next();
        }
        let name = parser.text_from_pivot(pivot);

        let column = match collection {
            Some(collection) => Identifier::WithCollection { collection, name },
            None => Identifier::Name { name },
        };

        Ok(column)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{tokens::clean_one::Identifier, QueryParser};

    #[test]
    pub fn test_identifier_name() {
        let text = "identifier";

        let mut parser = QueryParser::new(text);

        let result = Identifier::parse(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    Identifier::Name { name } => assert_eq!(name, text),
                    Identifier::WithCollection { collection: _, name: _ } => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_identifier_name_snake_case() {
        let text = "identifier_01";

        let mut parser = QueryParser::new(text);

        let result = Identifier::parse(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    Identifier::Name { name } => assert_eq!(name, text),
                    Identifier::WithCollection { collection: _, name: _ } => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_identifier_name_with_space() {
        let text = "identifier ";

        let mut parser = QueryParser::new(text);

        let result = Identifier::parse(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    Identifier::Name { name } => assert_eq!(name, "identifier"),
                    Identifier::WithCollection { collection: _, name: _ } => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_identifier_name_with_comma() {
        let text = "identifier,";

        let mut parser = QueryParser::new(text);

        let result = Identifier::parse(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    Identifier::Name { name } => assert_eq!(name, "identifier"),
                    Identifier::WithCollection { collection: _, name: _ } => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_identifier_name_with_break_line() {
        let text = "identifier\r";

        let mut parser = QueryParser::new(text);

        let result = Identifier::parse(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    Identifier::Name { name } => assert_eq!(name, "identifier"),
                    Identifier::WithCollection { collection: _, name: _ } => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_identifier_with_collection() {
        let text = "collection.identifier";

        let mut parser = QueryParser::new(text);

        let result = Identifier::parse(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    Identifier::Name { name: _ } => panic!(),
                    Identifier::WithCollection { collection, name } => {
                        assert_eq!(name, "identifier");
                        assert_eq!(collection, "collection")
                    },
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_identifier_with_collection_with_comma() {
        let text = "collection.identifier,";

        let mut parser = QueryParser::new(text);

        let result = Identifier::parse(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    Identifier::Name { name: _ } => panic!(),
                    Identifier::WithCollection { collection, name } => {
                        assert_eq!(name, "identifier");
                        assert_eq!(collection, "collection")
                    },
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_identifier_with_collection_with_space() {
        let text = "collection.identifier ";

        let mut parser = QueryParser::new(text);

        let result = Identifier::parse(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    Identifier::Name { name: _ } => panic!(),
                    Identifier::WithCollection { collection, name } => {
                        assert_eq!(name, "identifier");
                        assert_eq!(collection, "collection")
                    },
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_identifier_name_error_digit() {
        let text = "9identifier";

        let mut parser = QueryParser::new(text);

        let result = Identifier::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.end, 0);
                assert_eq!(err.text, "9");
            },
        }
    }

    #[test]
    pub fn test_identifier_name_error_dot() {
        let text = "col.identifier.err";

        let mut parser = QueryParser::new(text);

        let result = Identifier::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.end, 14);
                assert_eq!(err.text, "identifier.");
            },
        }
    }
}
