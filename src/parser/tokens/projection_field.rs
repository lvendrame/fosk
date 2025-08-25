#[derive(Debug)]
pub enum ProjectionField {
    /// *
    Wildcard,
    /// collection.*
    CollectionWildcard{ collection: String, },
    /// field
    Name{ name: String, },
    /// field as alias
    NameAlias{ name: String, alias: String, },
    /// collection.field
    CollectionName{ name: String, collection: String, },
    /// collection.field as alias
    CollectionNameAlias{ name: String, collection: String, alias: String, },
    /// function{ name: arg, }
    Function{ name: String, args: Vec<String>, },
    /// function{ name: arg, } as alias
    FunctionAlias{ name: String, args: Vec<String>, alias: String, },
}
