use crate::parser::{tokens::clean_one::{BoolParser, Column, Function, Literal, NullParser, NumberParser, StringParser}, ParseError, QueryParser};

#[derive(Debug, Clone, PartialEq)]
pub enum ScalarExpr {
    Literal(Literal),
    Column(Column),
    Function(Function),
    WildCard,
    WildCardWithCollection(String),
}

impl ScalarExpr {
    pub fn parse(parser: &mut QueryParser, allow_wildcard: bool) -> Result<ScalarExpr, ParseError> {
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

#[cfg(test)]
mod tests {
    use crate::parser::{tokens::clean_one::{Column, Literal, ScalarExpr}, QueryParser};

    #[test]
    pub fn test_literal_column_name() {
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
    pub fn test_literal_column_name_and_collection() {
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
    pub fn test_literal_null_parser() {
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
    pub fn test_literal_bool_parser_true() {
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
    pub fn test_literal_number_parser_int() {
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
    pub fn test_literal_string_parser() {
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
}
