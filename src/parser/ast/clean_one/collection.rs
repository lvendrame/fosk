#[derive(Debug, Clone, PartialEq)]
pub enum Collection {
    Table { name: String, alias: Option<String> },
    Query,
}

impl Collection {
    //pub fn parse
}
