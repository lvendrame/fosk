pub mod json_primitive;
pub use json_primitive::*;

pub mod field_info;
pub use field_info::*;

pub mod schema_dict;
pub use schema_dict::*;

pub mod reference_column;
pub use reference_column::*;

pub trait SchemaProvider {
    /// Given a collection *reference* (alias if present, otherwise the table name),
    /// return its schema if known.
    fn schema_of(&self, collection_ref: &str) -> Option<SchemaDict>;
}
