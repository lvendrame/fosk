use crate::JsonPrimitive;

#[derive(Debug, Clone)]
pub struct ResolvedField {
    /// resolved collection/alias
    pub collection: String,
    /// column name
    pub name: String,
    /// from SchemaDict
    pub ty: JsonPrimitive,
    /// from SchemaDict
    pub nullable: bool,
}
