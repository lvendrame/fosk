use crate::parser::{ParseError, QueryParser};

#[derive(Debug, Clone, PartialEq)]
pub enum Collection {
    Table { name: String, alias: Option<String> },
    Query,
}

impl Collection {

    pub fn parse(parser: &mut QueryParser) -> Result<Collection, ParseError> {
        // let mut name
        // while !parser.current().is_whitespace() {

        // }
        todo!()
    }
}
