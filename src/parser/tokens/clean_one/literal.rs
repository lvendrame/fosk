use crate::parser::{tokens::clean_one::{BoolParser, Identifier, NullParser, NumberParser, StringParser}, ParseError, QueryParser};

#[derive(Debug)]
pub enum Literal {
    String(String),
    Int(i32),
    Float(f32),
    Bool(bool),
    Null,
    Column { column: Identifier, alias: Option<String> }
}

impl Literal {
    pub fn parse(parser: &mut QueryParser) -> Result<Literal, ParseError> {
        if NumberParser::is_number(parser) {
            return NumberParser::parse(parser);
        }
        if StringParser::is_string_delimiter(parser) {
            return StringParser::parse(parser);
        }
        if BoolParser::is_bool(parser) {
            return BoolParser::parse(parser);
        }

        if NullParser::is_null(parser) {
            return  NullParser::parse(parser);
        }

        let identifier = Identifier::parse(parser)?;

        let alias = if parser.current().is_whitespace() && !parser.eof() {
            parser.next();
            if parser.comparers.alias.compare(parser) {
                parser.jump(parser.comparers.alias.length);
                let alias = Identifier::parse(parser)?;
                match alias {
                    Identifier::Name { name } => Some(name),
                    Identifier::WithCollection { collection: _, name: _ } =>
                        return Err(ParseError::new("Invalid identifier for alias", parser.position, parser)),
                }
            } else {
                return Err(ParseError::new("Invalid identifier for alias", parser.position, parser));
            }
        } else {
            None
        };

        Ok(Literal::Column { column: identifier, alias })
    }
}


#[cfg(test)]
mod tests {
    use crate::parser::{tokens::clean_one::{Identifier, Literal}, QueryParser};

    #[test]
    pub fn test_literal_identifier_name() {
        let text = "identifier";

        let mut parser = QueryParser::new(text);

        let result = Literal::parse(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    Literal::Column { column, alias } => match column {
                        Identifier::Name { name } => {
                            assert_eq!(name, "identifier");
                            assert_eq!(alias, None);
                        },
                        Identifier::WithCollection { collection: _, name: _ } => panic!(),
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_literal_identifier_name_with_alias() {
        let text = "identifier as nick";

        let mut parser = QueryParser::new(text);

        let result = Literal::parse(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    Literal::Column { column, alias } => match column {
                        Identifier::Name { name } => {
                            assert_eq!(name, "identifier");
                            assert_eq!(alias.unwrap(), "nick");
                        },
                        Identifier::WithCollection { collection: _, name: _ } => panic!(),
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_literal_identifier_name_and_collection_with_alias() {
        let text = "collection.identifier as nick";

        let mut parser = QueryParser::new(text);

        let result = Literal::parse(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    Literal::Column { column, alias } => match column {
                        Identifier::Name { name: _ } => panic!(),
                        Identifier::WithCollection { collection, name } => {
                            assert_eq!(collection, "collection");
                            assert_eq!(name, "identifier");
                            assert_eq!(alias.unwrap(), "nick");
                        },
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_literal_null_parser() {
        let text = "null";

        let mut parser = QueryParser::new(text);

        let result = Literal::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Null => {}, //should happen
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_literal_bool_parser_true() {
        let text = "true";

        let mut parser = QueryParser::new(text);

        let result = Literal::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Bool(value) => assert!(value),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_literal_number_parser_int() {
        let text = "32";

        let mut parser = QueryParser::new(text);

        let result = Literal::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::Int(value) => assert_eq!(value, 32),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_literal_string_parser() {
        let text = "\"identifier\"";

        let mut parser = QueryParser::new(text);

        let result = Literal::parse(&mut parser);

        match result {
            Ok(result) => match result {
                Literal::String(result) => assert_eq!(result, "identifier"),
                _ => todo!(),
            },
            Err(_) => panic!(),
        }
    }
}
