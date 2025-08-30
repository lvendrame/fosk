use crate::parser::{ast::clean_one::Collection, ParseError, QueryParser};

#[derive(Debug, Clone, PartialEq)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
}

impl JoinType {
    pub fn parse(parser: &mut QueryParser) -> Result<JoinType, ParseError> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Join {
    pub join_type: JoinType,
    pub collection: Collection,
}

impl Join {
    pub fn parse(parser: &mut QueryParser) -> Result<Join, ParseError> {
        todo!()
    }
}
