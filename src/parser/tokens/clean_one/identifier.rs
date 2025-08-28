use crate::parser::{tokens::clean_one::ScalarExpr, ParseError, QueryComparers, QueryParser};

#[derive(Debug, Clone, PartialEq)]
pub struct  Identifier {
    expression: ScalarExpr,
    alias: Option<String>,
}

impl Identifier {
    pub fn parse(parser: &mut QueryParser) -> Result<Identifier, ParseError> {
        let scalar = ScalarExpr::parse(parser, true)?;

        if !parser.current().is_whitespace() || parser.eof() {
            return Ok(Identifier {
                expression: scalar,
                alias: None
            });
        }
        parser.next();

        if !parser.comparers.alias.compare(parser) {
            return ParseError::new("Invalid alias for identifier", parser.position, parser).err();
        }

        parser.jump(parser.comparers.alias.length);

        let pivot = parser.position;
        while !parser.eof() && !QueryComparers::is_full_block_delimiter(parser.current()) {
            parser.next();
        }

        Ok(Identifier {
            expression: scalar,
            alias: Some(parser.text_from_pivot(pivot)),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{tokens::clean_one::{Column, Identifier, ScalarExpr}, QueryParser};

    #[test]
    pub fn test_identifier() {
        let text = "column";

        let mut parser = QueryParser::new(text);

        let result = Identifier::parse(&mut parser).expect("Failed to parse Identifier");

        match result.expression {
            ScalarExpr::Column(column) => match column {
                Column::Name { name } => assert_eq!(name, text),
                Column::WithCollection { collection: _, name: _ } => panic!(),
            },
            _ => panic!(),
        };

        assert_eq!(result.alias, None);
    }

    #[test]
    pub fn test_identifier_with_alias() {
        let text = "column as alias";

        let mut parser = QueryParser::new(text);

        let result = Identifier::parse(&mut parser).expect("Failed to parse ScalarExpr");

        match result.expression {
            ScalarExpr::Column(column) => match column {
                Column::Name { name } => assert_eq!(name, "column"),
                Column::WithCollection { collection: _, name: _ } => panic!(),
            },
            _ => panic!(),
        };
    }

    #[test]
    pub fn test_identifier_with_collection_and_alias() {
        let text = "collection.column as alias";

        let mut parser = QueryParser::new(text);

        let result = Identifier::parse(&mut parser).expect("Failed to parse ScalarExpr");

        match result.expression {
            ScalarExpr::Column(column) => match column {
                Column::Name { name: _ } => panic!(),
                Column::WithCollection { collection, name } => {
                    assert_eq!(name, "column");
                    assert_eq!(collection, "collection");
                    assert_eq!(result.alias.unwrap(), "alias");
                },
            },
            _ => panic!(),
        };
    }

    #[test]
    pub fn test_identifier_with_wrong_alias() {
        let text = "column asa alias";

        let mut parser = QueryParser::new(text);

        let result = Identifier::parse(&mut parser);

        match result {
            Ok(_) => panic!(),
            Err(err) => {
                assert_eq!(err.start, 7);
                assert_eq!(err.end, 7);
                assert_eq!(err.text, "a");
            },
        };
    }

}
