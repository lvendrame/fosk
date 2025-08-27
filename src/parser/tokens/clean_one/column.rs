use crate::parser::{tokens::clean_one::{ArgsParser, Function, ScalarExpr}, ParseError, QueryComparers, QueryParser};

#[derive(Debug, Clone)]
pub enum Column {
    Name { name: String },
    WithCollection { collection: String, name: String },
}

impl Column {
    pub fn parse_column_or_function(parser: &mut QueryParser) -> Result<ScalarExpr, ParseError> {
        let mut pivot = parser.position;
        let mut collection: Option<String> = None;
        let mut args: Option<Vec<ScalarExpr>> = None;
        let mut name = "".to_string();

        if parser.current().is_ascii_digit() {
            return Err(ParseError::new("Invalid column", pivot, parser));
        }

        while !parser.eof() && !QueryComparers::is_full_block_delimiter(parser.current()) {
            if args.is_some() {
                return Err(ParseError::new("Invalid function", pivot, parser));
            }

            let current = parser.current();
            if current == '.' {
                if collection.is_some() {
                    return Err(ParseError::new("Invalid column", pivot, parser));
                }
                collection = Some(parser.text_from_pivot(pivot));
                pivot = parser.position + 1;
            } else if parser.current() == '(' {
                name = parser.text_from_pivot(pivot);
                args = Some(ArgsParser::parse(parser)?);
            } else if !current.is_ascii_alphanumeric() && current != '_' {
                return Err(ParseError::new("Invalid column", pivot, parser));
            }
            parser.next();
        }

        if name.is_empty() {
            name =  parser.text_from_pivot(pivot);
        }

        let result = match args {
            Some(args) => ScalarExpr::Function(Function {
                name: format!("{}{}", collection.map_or("".to_string(), |coll| format!("{}.", coll)), name),
                args,
            }),
            None => match collection {
                Some(collection) => ScalarExpr::Column(Column::WithCollection { collection, name }),
                None => ScalarExpr::Column(Column::Name { name }),
            },
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{tokens::clean_one::{Column, ScalarExpr}, QueryParser};

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
}
