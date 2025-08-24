#[derive(Debug)]
pub enum Field {
    /// *
    All,
    /// collection.*
    CollectionAll(String),
    /// field
    Name(String),
    /// field as alias
    NameAlias(String, String),
    /// collection.field
    CollectionName(String, String),
    /// collection.field as alias
    CollectionNameAlias(String, String, String),
    /// function(arg)
    Function(String, String),
    /// function(arg) as alias
    FunctionAlias(String, String, String),
}
