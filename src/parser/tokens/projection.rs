use crate::parser::Field;

#[derive(Debug, Default)]
pub struct Projection {
    pub fields: Vec<Field>
}
