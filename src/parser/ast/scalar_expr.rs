use crate::parser::{
    ParseError, QueryParser,
    ast::{
        BoolParser, Column, Function, Literal, NullParser, NumberParser, ParamParser, StringParser,
    },
};
use std::fmt;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum ScalarExpr {
    Literal(Literal),
    Column(Column),
    Function(Function),
    WildCard,
    WildCardWithCollection(String),
    Parameter,
    Args(Vec<ScalarExpr>),
}

impl ScalarExpr {
    pub fn parse(parser: &mut QueryParser, allow_wildcard: bool) -> Result<ScalarExpr, ParseError> {
        parser.next_non_whitespace();

        if parser.eof() {
            return ParseError::new("Invalid scalar value", parser.position, parser).err();
        }

        if NumberParser::is_number(parser) {
            return NumberParser::parse(parser).map(ScalarExpr::Literal);
        }

        if StringParser::is_string_delimiter(parser) {
            return StringParser::parse(parser).map(ScalarExpr::Literal);
        }

        if BoolParser::is_bool(parser) {
            return BoolParser::parse(parser).map(ScalarExpr::Literal);
        }

        if NullParser::is_null(parser) {
            return NullParser::parse(parser).map(ScalarExpr::Literal);
        }

        if ParamParser::is_param(parser) {
            return ParamParser::parse(parser);
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
            ScalarExpr::WildCardWithCollection(coll) => write!(f, "{}.*", coll),
            ScalarExpr::Parameter => write!(f, "?"),
            ScalarExpr::Args(args) => write!(
                f,
                "({})",
                args.iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
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
            ScalarExpr::WildCardWithCollection(coll) => {
                write!(f, "WildCardWithCollection({}.*)", coll)
            }
            ScalarExpr::Parameter => write!(f, "Parameter(?)"),
            ScalarExpr::Args(_) => write!(f, "Parameter({})", self),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{
        QueryParser,
        ast::{Column, Function, Literal, ScalarExpr},
    };

    fn parse_scalar(text: &str, allow_wildcard: bool) -> ScalarExpr {
        let mut parser = QueryParser::new(text);
        match ScalarExpr::parse(&mut parser, allow_wildcard) {
            Ok(expr) => expr,
            Err(err) => panic!("expected scalar from {text:?}, got {err:?}"),
        }
    }

    fn parse_error(text: &str, allow_wildcard: bool) -> (usize, usize, String) {
        let mut parser = QueryParser::new(text);
        match ScalarExpr::parse(&mut parser, allow_wildcard) {
            Ok(expr) => panic!("expected scalar parse error from {text:?}, got {expr:?}"),
            Err(err) => (err.start, err.end, err.text),
        }
    }

    #[test]
    pub fn test_scalar_column_name() {
        assert_eq!(
            parse_scalar("column", true),
            ScalarExpr::Column(Column::Name {
                name: "column".to_string()
            })
        );
    }

    #[test]
    pub fn test_scalar_column_name_and_collection() {
        assert_eq!(
            parse_scalar("collection.column", true),
            ScalarExpr::Column(Column::WithCollection {
                collection: "collection".to_string(),
                name: "column".to_string()
            })
        );
    }

    #[test]
    pub fn test_scalar_column_name_prefixed_with_whitespace() {
        assert_eq!(
            parse_scalar("  column", true),
            ScalarExpr::Column(Column::Name {
                name: "column".to_string()
            })
        );
    }

    #[test]
    pub fn test_scalar_null_parser() {
        assert_eq!(
            parse_scalar("null", true),
            ScalarExpr::Literal(Literal::Null)
        );
    }

    #[test]
    pub fn test_scalar_bool_parser_true() {
        assert_eq!(
            parse_scalar("true", true),
            ScalarExpr::Literal(Literal::Bool(true))
        );
    }

    #[test]
    pub fn test_scalar_number_parser_int() {
        assert_eq!(
            parse_scalar("32", true),
            ScalarExpr::Literal(Literal::Int(32))
        );
    }

    #[test]
    pub fn test_scalar_string_parser() {
        assert_eq!(
            parse_scalar("\"identifier\"", true),
            ScalarExpr::Literal(Literal::String("identifier".to_string()))
        );
    }

    #[test]
    pub fn test_scalar_empty() {
        let (start, end, text) = parse_error(" ", true);
        assert_eq!((start, end, text), (1, 1, String::new()));
    }

    #[test]
    pub fn test_scalar_wildcard() {
        assert_eq!(parse_scalar("*", true), ScalarExpr::WildCard);
    }

    #[test]
    pub fn test_scalar_wildcard_with_collection() {
        assert_eq!(
            parse_scalar("collection.*", true),
            ScalarExpr::WildCardWithCollection("collection".to_string())
        );
    }

    #[test]
    pub fn test_scalar_wildcard_not_allowed() {
        let (start, end, text) = parse_error("*", false);
        assert_eq!((start, end, text), (0, 1, "*".to_string()));
    }

    #[test]
    pub fn test_scalar_wildcard_with_collection_not_allowed() {
        let (start, end, text) = parse_error("collection.*", false);
        assert_eq!((start, end, text), (11, 12, "*".to_string()));
    }

    #[test]
    fn display_and_debug_cover_all_scalar_expr_variants() {
        let literal = ScalarExpr::Literal(Literal::Int(1));
        let column = ScalarExpr::Column(Column::Name {
            name: "age".to_string(),
        });
        let function = ScalarExpr::Function(Function {
            name: "sum".to_string(),
            args: vec![column.clone()],
            distinct: false,
        });
        let wildcard = ScalarExpr::WildCard;
        let collection_wildcard = ScalarExpr::WildCardWithCollection("people".to_string());
        let parameter = ScalarExpr::Parameter;
        let args = ScalarExpr::Args(vec![literal.clone(), parameter.clone()]);

        assert_eq!(literal.to_string(), "lit: i: 1");
        assert_eq!(format!("{:?}", literal), "Literal(lit: i: 1)");
        assert_eq!(column.to_string(), "col: age");
        assert_eq!(format!("{:?}", column), "Column(col: age)");
        assert_eq!(function.to_string(), "sum(col: age)");
        assert_eq!(format!("{:?}", function), "Function(sum(col: age))");
        assert_eq!(wildcard.to_string(), "*");
        assert_eq!(format!("{:?}", wildcard), "WildCard(*)");
        assert_eq!(collection_wildcard.to_string(), "people.*");
        assert_eq!(
            format!("{:?}", collection_wildcard),
            "WildCardWithCollection(people.*)"
        );
        assert_eq!(parameter.to_string(), "?");
        assert_eq!(format!("{:?}", parameter), "Parameter(?)");
        assert_eq!(args.to_string(), "(lit: i: 1, ?)");
        assert_eq!(format!("{:?}", args), "Parameter((lit: i: 1, ?))");
    }
}
