use crate::parser::{ast::{BoolParser, Column, Function, Literal, NullParser, NumberParser, StringParser}, ParseError, QueryParser};
use std::fmt;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum ScalarExpr {
    Literal(Literal),
    Column(Column),
    Function(Function),
    WildCard,
    WildCardWithCollection(String),
}

impl ScalarExpr {
    pub fn parse(parser: &mut QueryParser, allow_wildcard: bool) -> Result<ScalarExpr, ParseError> {
        parser.next_non_whitespace();

        if parser.eof() {
            return ParseError::new("Invalid scalar value", parser.position, parser).err();
        }

        if NumberParser::is_number(parser) {
            return NumberParser::parse(parser)
                .map(ScalarExpr::Literal);
        }
        if StringParser::is_string_delimiter(parser) {
            return StringParser::parse(parser)
                .map(ScalarExpr::Literal);
        }
        if BoolParser::is_bool(parser) {
            return BoolParser::parse(parser)
                .map(ScalarExpr::Literal);
        }

        if NullParser::is_null(parser) {
            return  NullParser::parse(parser)
                .map(ScalarExpr::Literal);
        }

        Column::parse_general_scalar(parser, allow_wildcard)
    }
}

impl fmt::Display for ScalarExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScalarExpr::Literal(l) => write!(f, "lit: {}", l),
            ScalarExpr::Column(c) => write!(f, "{}", c),
            ScalarExpr::Function(fun) => write!(f, "{}", fun),
            ScalarExpr::WildCard => write!(f, "*"),
            ScalarExpr::WildCardWithCollection(coll) => write!(f, "{}.{}", coll, "*"),
        }
    }
}

impl fmt::Debug for ScalarExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScalarExpr::Literal(_) => write!(f, "Literal({})", self),
            ScalarExpr::Column(_) => write!(f, "Column({})", self),
            ScalarExpr::Function(_) => write!(f, "Function({})", self),
            ScalarExpr::WildCard => write!(f, "WildCard(*)"),
            ScalarExpr::WildCardWithCollection(coll) => write!(f, "WildCardWithCollection({}.*)", coll),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{ast::{Column, Literal, ScalarExpr}, QueryParser};

    #[test]
    pub fn test_scalar_column_name() {
        let text = "column";

        let mut parser = QueryParser::new(text);

        let result = ScalarExpr::parse(&mut parser, true);

        match result {
            Ok(result) => {
                match result {
                    ScalarExpr::Column(column) => match column {
                        Column::Name { name } => {
                            assert_eq!(name, "column");
                        },
                        Column::WithCollection { collection: _, name: _ } => panic!(),
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_scalar_column_name_and_collection() {
        let text = "collection.column";

        let mut parser = QueryParser::new(text);

        let result = ScalarExpr::parse(&mut parser, true);

        match result {
            Ok(result) => {
                match result {
                    ScalarExpr::Column(column) => match column {
                        Column::Name { name: _ } => panic!(),
                        Column::WithCollection { collection, name } => {
                            assert_eq!(collection, "collection");
                            assert_eq!(name, "column");
                        },
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_scalar_column_name_prefixed_with_whitespace() {
        let text = "  column";

        let mut parser = QueryParser::new(text);

        let result = ScalarExpr::parse(&mut parser, true);

        match result {
            Ok(result) => {
                match result {
                    ScalarExpr::Column(column) => match column {
                        Column::Name { name } => {
                            assert_eq!(name, "column");
                        },
                        Column::WithCollection { collection: _, name: _ } => panic!(),
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_scalar_null_parser() {
        let text = "null";

        let mut parser = QueryParser::new(text);

        let result = ScalarExpr::parse(&mut parser, true);

        match result {
            Ok(result) => match result {
                ScalarExpr::Literal(literal) => match literal {
                    Literal::Null => {}, //should happen
                    _ => panic!(),
                },
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_scalar_bool_parser_true() {
        let text = "true";

        let mut parser = QueryParser::new(text);

        let result = ScalarExpr::parse(&mut parser, true);

        match result {
            Ok(result) => match result {
                ScalarExpr::Literal(literal) => match literal {
                    Literal::Bool(value) => assert!(value),
                    _ => panic!(),
                },
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_scalar_number_parser_int() {
        let text = "32";

        let mut parser = QueryParser::new(text);

        let result = ScalarExpr::parse(&mut parser, true);

        match result {
            Ok(result) => match result {
                ScalarExpr::Literal(literal) => match literal {
                    Literal::Int(value) => assert_eq!(value, 32),
                    _ => panic!(),
                },
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_scalar_string_parser() {
        let text = "\"identifier\"";

        let mut parser = QueryParser::new(text);

        let result = ScalarExpr::parse(&mut parser, true);

        match result {
            Ok(result) => match result {
                ScalarExpr::Literal(literal) => match literal {
                    Literal::String(result) => assert_eq!(result, "identifier"),
                    _ => panic!(),
                }
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_scalar_empty() {
        let text = " ";

        let mut parser = QueryParser::new(text);

        let result = ScalarExpr::parse(&mut parser, true);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "");
                assert_eq!(err.start, 1);
                assert_eq!(err.end, 1);
            },
        }
    }

    #[test]
    pub fn test_scalar_wildcard() {
        let text = "*";

        let mut parser = QueryParser::new(text);

        let result = ScalarExpr::parse(&mut parser, true);

        match result {
            Ok(result) => match result {
                ScalarExpr::WildCard => {}, //should pass
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_scalar_wildcard_with_collection() {
        let text = "collection.*";

        let mut parser = QueryParser::new(text);

        let result = ScalarExpr::parse(&mut parser, true);

        match result {
            Ok(result) => match result {
                ScalarExpr::WildCardWithCollection(collection) => assert_eq!(collection, "collection"),
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_scalar_wildcard_not_allowed() {
        let text = "*";

        let mut parser = QueryParser::new(text);

        let result = ScalarExpr::parse(&mut parser, false);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "*");
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 1);
            },
        }
    }

    #[test]
    pub fn test_scalar_wildcard_with_collection_not_allowed() {
        let text = "collection.*";

        let mut parser = QueryParser::new(text);

        let result = ScalarExpr::parse(&mut parser, false);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.text, "*");
                assert_eq!(err.start, 11);
                assert_eq!(err.end, 12);
            },
        }
    }
}
