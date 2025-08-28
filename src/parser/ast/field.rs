use crate::parser::FieldType;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub collection: Option<String>,
    pub field_type: Option<FieldType>
}

impl Field {
    pub fn new(name: String, collection: Option<String>) -> Self{
        Self{
            name,
            collection,
            field_type: None,
        }
    }
}
