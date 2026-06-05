//! Schema inference and collection reference metadata.

/// Coarse JSON primitive type classification.
pub mod json_primitive;
pub use json_primitive::*;

/// Field-level schema metadata.
pub mod field_info;
pub use field_info::*;

/// Collection schema dictionaries.
pub mod schema_dict;
pub use schema_dict::*;

pub(crate) mod compact_schema;
pub(crate) use compact_schema::*;

pub(crate) mod schema_file;
pub(crate) use schema_file::*;

pub(crate) mod schema_load;
pub(crate) use schema_load::*;

/// Foreign-key-like collection reference metadata.
pub mod reference_column;
pub use reference_column::*;

/// Provides schema metadata for named collections.
pub trait SchemaProvider {
    /// Given a collection *reference* (alias if present, otherwise the table name),
    /// return its schema if known.
    fn schema_of(&self, collection_ref: &str) -> Option<SchemaDict>;
}
