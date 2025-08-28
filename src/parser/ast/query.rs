use crate::parser::{Collection, ProjectionField};

#[derive(Debug, Default)]
pub struct Query {
    pub projection_fields: Vec<ProjectionField>,
    pub collections: Vec<Collection>
}
