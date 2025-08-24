use crate::parser::{Collection, Field};

#[derive(Debug, Default)]
pub struct Query {
    pub projection_fields: Vec<Field>,
    pub collections: Vec<Collection>
}
