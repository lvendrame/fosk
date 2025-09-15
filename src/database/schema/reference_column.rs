use std::{collections::HashMap};

use crate::{database::SchemaProvider, Db};

/// Metadata for a relationship between two collections via a specific field.
///
/// A `ReferenceColumn` represents a foreign-key-like mapping from one collection
/// to another. It contains:
/// - `collection`: the name of the collection holding the reference.
/// - `column`: the field in the referencing collection.
/// - `ref_collection`: the target (referenced) collection name.
/// - `ref_column`: the field in the referenced collection.
/// - `is_referrer`: whether this entry is the inverse side (true) or the direct reference (false).
///
/// Used by `Db` to register and traverse object expansions based on defined references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceColumn {
    pub collection: String,
    pub column: String,
    pub ref_collection: String,
    pub ref_column: String,
    pub is_referrer: bool,
}

impl ReferenceColumn {
    pub fn new(collection: String, column: String, ref_collection: String, ref_column: String, is_referrer: bool) -> Self {
        Self { collection, column, ref_collection, ref_column, is_referrer }
    }
}

pub type ReferenceFieldMap = HashMap<String, ReferenceColumn>;

#[derive(Debug, Default)]
pub struct DbReferences {
    /// Map of collections name -> field name and reference
    references: HashMap<String, ReferenceFieldMap>,
}

impl DbReferences {
    pub fn create_reference(&mut self, db: &Db, collection_name: &str, column: &str, ref_collection_name: &str, ref_column: &str) -> bool {
        // referenced
        let collection_references = self.references
            .entry(collection_name.to_string()).or_default();

        if !collection_references.create_reference(db, collection_name, column, ref_collection_name, ref_column, false) {
            return false;
        }

        // referrer
        let collection_references = self.references
            .entry(ref_collection_name.to_string()).or_default();

        collection_references.create_reference(db, collection_name, column, ref_collection_name, ref_column, true)
    }

    pub fn infer_reference(&mut self, db: &Db, collection_name: &str, ref_collection_name: &str) -> bool {
        // referenced
        let collection_references = self.references
            .entry(collection_name.to_string()).or_default();

        if !collection_references.infer_reference(db, collection_name, ref_collection_name, false) {
            return false;
        }

        // referrer
        let collection_references = self.references
            .entry(ref_collection_name.to_string()).or_default();

        collection_references.infer_reference(db, collection_name, ref_collection_name, true)
    }

    pub fn get_collection_refs(&self, collection_name: &str) -> Option<&ReferenceFieldMap> {
        self.references.get(collection_name)
    }

    pub fn get_collection_column_ref(&self, collection_name: &str, column: &str) -> Option<&ReferenceColumn> {
        match self.references.get(collection_name) {
            Some(collection_refs) => collection_refs.get(column),
            None => None,
        }
    }
}


trait CollectionReferences {
    fn create_reference(&mut self, db: &Db, collection_name: &str, column: &str, ref_collection_name: &str, ref_column: &str, is_referrer: bool) -> bool;
    fn infer_reference(&mut self, db: &Db, collection_name: &str, ref_collection_name: &str, is_referrer: bool) -> bool;
}

impl CollectionReferences for HashMap<String, ReferenceColumn> {
    fn create_reference(&mut self, db: &Db, collection_name: &str, column: &str, ref_collection_name: &str, ref_column: &str, is_referrer: bool) -> bool {
        let collection_schema = db.schema_of(collection_name);
        let ref_collection_schema = db.schema_of(ref_collection_name);
        match (collection_schema, ref_collection_schema) {
            (Some(collection_schema), Some(ref_collection_schema)) => {
                if collection_schema.fields.contains_key(column) && ref_collection_schema.fields.contains_key(ref_column){
                    let key = if is_referrer { ref_column } else { column };
                    self.insert(
                        key.to_string(),
                        ReferenceColumn::new(
                            collection_name.to_string(),
                            column.to_string(),
                            ref_collection_name.to_string(),
                            ref_column.to_string(),
                            is_referrer,
                        )
                    );
                    return true;
                }
                false
            },
            _ => false,
        }
    }

    fn infer_reference(&mut self, db: &Db, collection_name: &str, ref_collection_name: &str, is_referrer: bool) -> bool {
        let ref_collection = db.get(ref_collection_name);
        let ref_collection = match ref_collection {
            Some(collection) => collection,
            None => return false,
        };

        let column = ref_collection.get_reference_column_name();
        let ref_column = ref_collection.get_config().id_key;

        self.create_reference(db, collection_name, &column, ref_collection_name, &ref_column, is_referrer)
    }
}
