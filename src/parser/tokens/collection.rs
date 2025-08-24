use crate::parser::Query;

#[derive(Debug)]
pub enum Collection {
    Name(String),
    NameAlias(String, String),
    Query(Query, String),
}
