use crate::parser::{
    ParseError, QueryParser, WordComparer,
    ast::{ArgsExpr, Function, ScalarExpr, TextCollector},
};
use std::fmt;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Column {
    Name { name: String },
    WithCollection { collection: String, name: String },
}

impl Column {
    pub fn parse_column_or_function_or_wildcard(
        parser: &mut QueryParser,
    ) -> Result<ScalarExpr, ParseError> {
        Column::parse_general_scalar(parser, true)
    }

    pub fn parse_column_or_function(parser: &mut QueryParser) -> Result<ScalarExpr, ParseError> {
        Column::parse_general_scalar(parser, false)
    }

    pub fn parse_general_scalar(
        parser: &mut QueryParser,
        allow_wildcard: bool,
    ) -> Result<ScalarExpr, ParseError> {
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
            name = text;
        }

        let result = match is_wildcard {
            true => match collection {
                Some(collection) => ScalarExpr::WildCardWithCollection(collection),
                None => ScalarExpr::WildCard,
            },
            false => match args_expr {
                Some(args_expr) => ScalarExpr::Function(Function {
                    name: format!(
                        "{}{}",
                        collection.map_or("".to_string(), |coll| format!("{}.", coll)),
                        name
                    ),
                    args: args_expr.args,
                    distinct: args_expr.distinct,
                }),
                None => match collection {
                    Some(collection) => {
                        ScalarExpr::Column(Column::WithCollection { collection, name })
                    }
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
            Column::WithCollection { collection, name } => {
                write!(f, "col: {}.{}", collection, name)
            }
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
    use crate::parser::{
        QueryParser,
        ast::{Column, Function, ScalarExpr},
    };

    fn parse_scalar(text: &str, allow_wildcard: bool) -> ScalarExpr {
        let mut parser = QueryParser::new(text);
        let result = if allow_wildcard {
            Column::parse_column_or_function_or_wildcard(&mut parser)
        } else {
            Column::parse_column_or_function(&mut parser)
        };
        match result {
            Ok(expr) => expr,
            Err(err) => panic!("expected scalar from {text:?}, got {err:?}"),
        }
    }

    fn parse_error(text: &str, allow_wildcard: bool) -> (usize, usize, String) {
        let mut parser = QueryParser::new(text);
        let result = if allow_wildcard {
            Column::parse_column_or_function_or_wildcard(&mut parser)
        } else {
            Column::parse_column_or_function(&mut parser)
        };
        match result {
            Ok(expr) => panic!("expected parse error from {text:?}, got {expr:?}"),
            Err(err) => (err.start, err.end, err.text),
        }
    }

    fn parse_column_name(text: &str) -> String {
        match parse_scalar(text, false) {
            ScalarExpr::Column(Column::Name { name }) => name,
            expr => panic!("expected unqualified column from {text:?}, got {expr:?}"),
        }
    }

    fn parse_collection_column(text: &str) -> (String, String) {
        match parse_scalar(text, false) {
            ScalarExpr::Column(Column::WithCollection { collection, name }) => (collection, name),
            expr => panic!("expected qualified column from {text:?}, got {expr:?}"),
        }
    }

    #[test]
    pub fn test_column_name() {
        assert_eq!(parse_column_name("column"), "column");
    }

    #[test]
    pub fn test_column_name_snake_case() {
        assert_eq!(parse_column_name("column_01"), "column_01");
    }

    #[test]
    pub fn test_column_name_with_space() {
        assert_eq!(parse_column_name("column "), "column");
    }

    #[test]
    pub fn test_column_name_with_alias() {
        assert_eq!(parse_column_name("column as nick"), "column");
    }

    #[test]
    pub fn test_column_name_with_comma() {
        assert_eq!(parse_column_name("column,"), "column");
    }

    #[test]
    pub fn test_column_name_with_break_line() {
        assert_eq!(parse_column_name("column\r"), "column");
    }

    #[test]
    pub fn test_column_with_collection() {
        assert_eq!(
            parse_collection_column("collection.column"),
            ("collection".to_string(), "column".to_string())
        );
    }

    #[test]
    pub fn test_column_with_collection_with_comma() {
        assert_eq!(
            parse_collection_column("collection.column,"),
            ("collection".to_string(), "column".to_string())
        );
    }

    #[test]
    pub fn test_column_with_collection_with_space() {
        assert_eq!(
            parse_collection_column("collection.column "),
            ("collection".to_string(), "column".to_string())
        );
    }

    #[test]
    pub fn test_column_name_error_digit() {
        let text = "9column";

        let (start, end, text) = parse_error(text, false);
        assert_eq!((start, end, text), (0, 0, "9".to_string()));
    }

    #[test]
    pub fn test_column_name_error_dot() {
        let text = "col.column.err";

        let (start, end, text) = parse_error(text, false);
        assert_eq!((start, end, text), (4, 10, "column.".to_string()));
    }

    #[test]
    pub fn test_function_name() {
        assert_eq!(
            parse_scalar("fn_new()", false),
            ScalarExpr::Function(Function {
                name: "fn_new".to_string(),
                args: vec![],
                distinct: false
            })
        );
    }

    #[test]
    pub fn test_function_name_schema() {
        assert_eq!(
            parse_scalar("schema.fn_new()", false),
            ScalarExpr::Function(Function {
                name: "schema.fn_new".to_string(),
                args: vec![],
                distinct: false
            })
        );
    }

    #[test]
    pub fn test_function_name_with_args_1() {
        match parse_scalar("fn_new(true)", false) {
            ScalarExpr::Function(function) => {
                assert_eq!(function.name, "fn_new");
                assert_eq!(function.args.len(), 1);
            }
            expr => panic!("expected function, got {expr:?}"),
        }
    }

    #[test]
    pub fn test_function_name_with_args_3() {
        match parse_scalar(r#"fn_new(true, "hello", 123)"#, false) {
            ScalarExpr::Function(function) => {
                assert_eq!(function.name, "fn_new");
                assert_eq!(function.args.len(), 3);
            }
            expr => panic!("expected function, got {expr:?}"),
        }
    }

    #[test]
    pub fn test_function_name_error_end() {
        let text = "fn_new()f";

        let (start, end, text) = parse_error(text, false);
        assert_eq!((start, end, text), (0, 8, "fn_new()f".to_string()));
    }

    #[test]
    pub fn test_wildcard() {
        assert_eq!(parse_scalar("*", true), ScalarExpr::WildCard);
    }

    #[test]
    pub fn test_wildcard_with_collection() {
        assert_eq!(
            parse_scalar("coll.*", true),
            ScalarExpr::WildCardWithCollection("coll".to_string())
        );
    }

    #[test]
    pub fn test_wildcard_with_wrong_char() {
        let text = "*4";

        let (start, end, text) = parse_error(text, false);
        assert_eq!((start, end, text), (0, 1, "*4".to_string()));
    }

    #[test]
    pub fn test_wildcard_not_allowed() {
        let text = "*";

        let (start, end, text) = parse_error(text, false);
        assert_eq!((start, end, text), (0, 1, "*".to_string()));
    }

    #[test]
    pub fn test_wildcard_with_collection_not_allowed() {
        let text = "coll.*";

        let (start, end, text) = parse_error(text, false);
        assert_eq!((start, end, text), (5, 6, "*".to_string()));
    }

    #[test]
    fn debug_formats_column_variants() {
        assert_eq!(
            format!(
                "{:?}",
                Column::Name {
                    name: "name".to_string()
                }
            ),
            "Column::Name(col: name)"
        );
        assert_eq!(
            format!(
                "{:?}",
                Column::WithCollection {
                    collection: "people".to_string(),
                    name: "name".to_string()
                }
            ),
            "Column::WithCollection(col: people.name)"
        );
    }
}
