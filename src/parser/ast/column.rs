use crate::parser::{ast::{ArgsExpr, Function, ScalarExpr, TextCollector}, ParseError, QueryParser, WordComparer};
use std::fmt;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Column {
    Name { name: String },
    WithCollection { collection: String, name: String },
}

impl Column {

    pub fn parse_column_or_function_or_wildcard(parser: &mut QueryParser) -> Result<ScalarExpr, ParseError> {
        Column::parse_general_scalar(parser, true)
    }

    pub fn parse_column_or_function(parser: &mut QueryParser) -> Result<ScalarExpr, ParseError> {
        Column::parse_general_scalar(parser, false)
    }

    pub fn parse_general_scalar(parser: &mut QueryParser, allow_wildcard: bool) -> Result<ScalarExpr, ParseError> {
        let mut pivot = parser.position;
        let mut collection: Option<String> = None;
        let mut args_expr: Option<ArgsExpr> = None;
        let mut name = "".to_string();
        let mut is_wildcard = false;

        let mut text = String::new();

        if parser.current().is_ascii_digit() {
            return Err(ParseError::new("Invalid column", pivot, parser));
        }

        while !parser.eof() && !WordComparer::is_any_delimiter(parser.current()) {
            if args_expr.is_some() {
                return Err(ParseError::new("Invalid function", pivot, parser));
            }

            if is_wildcard {
                return Err(ParseError::new("Invalid wildcard", pivot, parser));
            }

            text = TextCollector::collect_with_stopper(parser, |current| current == '*')?;

            let current = parser.current();
            if current == '.' {
                if collection.is_some() {
                    return Err(ParseError::new("Invalid column", pivot, parser));
                }
                collection = Some(text.clone());
                pivot = parser.position + 1;
                parser.next();
            } else if parser.current() == '(' {
                name = text.clone();
                args_expr = Some(ArgsExpr::parse(parser, allow_wildcard)?);
            } else if current == '*' {
                is_wildcard = true;
                parser.next();
            }
        }

        if is_wildcard && !allow_wildcard {
            return Err(ParseError::new("Invalid scalar", pivot, parser));
        }

        if name.is_empty() {
            name =  text;
        }

        let result = match is_wildcard {
            true => match collection {
                Some(collection) => ScalarExpr::WildCardWithCollection(collection),
                None => ScalarExpr::WildCard,
            },
            false => match args_expr {
                Some(args_expr) => ScalarExpr::Function(Function {
                    name: format!("{}{}", collection.map_or("".to_string(), |coll| format!("{}.", coll)), name),
                    args: args_expr.args,
                    distinct: args_expr.distinct,
                }),
                None => match collection {
                    Some(collection) => ScalarExpr::Column(Column::WithCollection { collection, name }),
                    None => ScalarExpr::Column(Column::Name { name }),
                },
            },
        };

        Ok(result)
    }
}

impl fmt::Display for Column {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Column::Name { name } => write!(f, "col: {}", name),
            Column::WithCollection { collection, name } => write!(f, "col: {}.{}", collection, name),
        }
    }
}

impl fmt::Debug for Column {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Column::Name { .. } => write!(f, "Column::Name({})", self),
            Column::WithCollection { .. } => write!(f, "Column::WithCollection({})", self),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{ast::{Column, ScalarExpr}, QueryParser};

    #[test]
    pub fn test_column_name() {
        let text = "column";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    ScalarExpr::Column(column) => match column {
                        Column::Name { name } => assert_eq!(name, text),
                        Column::WithCollection { collection: _, name: _ } => panic!(),
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_column_name_snake_case() {
        let text = "column_01";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    ScalarExpr::Column(column) => match column {
                        Column::Name { name } => assert_eq!(name, text),
                        Column::WithCollection { collection: _, name: _ } => panic!(),
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_column_name_with_space() {
        let text = "column ";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    ScalarExpr::Column(column) => match column {
                        Column::Name { name } => assert_eq!(name, "column"),
                        Column::WithCollection { collection: _, name: _ } => panic!(),
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_column_name_with_alias() {
        let text = "column as nick";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    ScalarExpr::Column(column) => match column {
                        Column::Name { name } => assert_eq!(name, "column"),
                        Column::WithCollection { collection: _, name: _ } => panic!(),
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_column_name_with_comma() {
        let text = "column,";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    ScalarExpr::Column(column) => match column {
                        Column::Name { name } => assert_eq!(name, "column"),
                        Column::WithCollection { collection: _, name: _ } => panic!(),
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_column_name_with_break_line() {
        let text = "column\r";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    ScalarExpr::Column(column) => match column {
                        Column::Name { name } => assert_eq!(name, "column"),
                        Column::WithCollection { collection: _, name: _ } => panic!(),
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_column_with_collection() {
        let text = "collection.column";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    ScalarExpr::Column(column) => match column {
                        Column::Name { name: _ } => panic!(),
                        Column::WithCollection { collection, name } => {
                            assert_eq!(name, "column");
                            assert_eq!(collection, "collection")
                        },
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_column_with_collection_with_comma() {
        let text = "collection.column,";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    ScalarExpr::Column(column) => match column {
                        Column::Name { name: _ } => panic!(),
                        Column::WithCollection { collection, name } => {
                            assert_eq!(name, "column");
                            assert_eq!(collection, "collection")
                        },
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_column_with_collection_with_space() {
        let text = "collection.column ";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    ScalarExpr::Column(column) => match column {
                        Column::Name { name: _ } => panic!(),
                        Column::WithCollection { collection, name } => {
                            assert_eq!(name, "column");
                            assert_eq!(collection, "collection")
                        },
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_column_name_error_digit() {
        let text = "9column";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.end, 0);
                assert_eq!(err.text, "9");
            },
        }
    }

    #[test]
    pub fn test_column_name_error_dot() {
        let text = "col.column.err";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.end, 10);
                assert_eq!(err.text, "column.");
            },
        }
    }

    #[test]
    pub fn test_function_name() {
        let text = "fn_new()";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    ScalarExpr::Function(function) => {
                        assert_eq!(function.name, "fn_new");
                        assert_eq!(function.args.len(), 0);
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_function_name_schema() {
        let text = "schema.fn_new()";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    ScalarExpr::Function(function) => {
                        assert_eq!(function.name, "schema.fn_new");
                        assert_eq!(function.args.len(), 0);
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_function_name_with_args_1() {
        let text = "fn_new(true)";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    ScalarExpr::Function(function) => {
                        assert_eq!(function.name, "fn_new");
                        assert_eq!(function.args.len(), 1);
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_function_name_with_args_3() {
        let text = r#"fn_new(true, "hello", 123)"#;

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(result) => {
                match result {
                    ScalarExpr::Function(function) => {
                        assert_eq!(function.name, "fn_new");
                        assert_eq!(function.args.len(), 3);
                    },
                    _ => panic!(),
                }
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_function_name_error_end() {
        let text = "fn_new()f";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.end, 8);
                assert_eq!(err.text, "fn_new()f");
            },
        }
    }

     #[test]
    pub fn test_wildcard() {
        let text = "*";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function_or_wildcard(&mut parser);

        match result {
            Ok(result) => match result {
                ScalarExpr::WildCard => {}, //allowed
                _ => panic!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_wildcard_with_collection() {
        let text = "coll.*";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function_or_wildcard(&mut parser);

        match result {
            Ok(result) => match result {
                ScalarExpr::WildCardWithCollection(collection) => assert_eq!(collection, "coll"),
                _ => todo!(),
            },
            Err(_) => panic!(),
        }
    }

    #[test]
    pub fn test_wildcard_with_wrong_char() {
        let text = "*4";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 1);
                assert_eq!(err.text, "*4");
            },
        }
    }

    #[test]
    pub fn test_wildcard_not_allowed() {
        let text = "*";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.start, 0);
                assert_eq!(err.end, 1);
                assert_eq!(err.text, "*");
            },
        }
    }

    #[test]
    pub fn test_wildcard_with_collection_not_allowed() {
        let text = "coll.*";

        let mut parser = QueryParser::new(text);

        let result = Column::parse_column_or_function(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.start, 5);
                assert_eq!(err.end, 6);
                assert_eq!(err.text, "*");
            },
        }
    }
}
