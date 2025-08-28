#[derive(Debug, Clone, PartialEq)]
pub enum collection {
    Table { name: String, alias: Option<String> },
    Query,
}
